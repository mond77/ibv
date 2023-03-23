//! interfaec Conn:
//!
//!     1.send_msg(data: &[IoSlice]) -> Result<()>
//!     2.recv_msg() -> Result<&[u8]>

use crate::types::{
    device::{default_device, Device},
    qp::QPCap,
};
use std::io::Result;
use std::{io::IoSlice, sync::Arc};
use tokio::net::{TcpListener, TcpStream};

use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio::{io, sync::mpsc::Sender};

use crate::types::{
    mr::{RecvBuffer, RemoteBufManager, RemoteMR, SendBuffer},
    qp::QP,
};

use super::daemon::polling;

pub struct Conn {
    qp: Arc<QP>,
    send_buf: SendBuffer,
    recv_buf: RecvBuffer,
    // remote recv_buf
    allocator: RemoteBufManager,
    pub daemon: JoinHandle<()>,
    lock: Mutex<()>,
}

unsafe impl Send for Conn {}
unsafe impl Sync for Conn {}

impl Conn {
    pub async fn new(
        qp: Arc<QP>,
        recv_buf: RecvBuffer,
        remote_mr: RemoteMR,
        tx: Sender<u32>,
    ) -> Self {
        let allocator = RemoteBufManager::new(remote_mr);
        let send_buf = SendBuffer::new(&qp.pd);
        let qp_c = qp.clone();
        // add sufficient RQE, maybe use SRQ to notify adding RQE
        qp.post_null_recv(1000);
        let daemon = tokio::spawn(polling(qp_c, tx));
        Conn {
            qp,
            allocator,
            lock: Mutex::new(()),
            send_buf,
            daemon,
            recv_buf,
        }
    }

    pub fn qp(&self) -> Arc<QP> {
        self.qp.clone()
    }

    pub async fn send_msg(&self, msg: &[IoSlice<'_>]) -> io::Result<()> {
        // get the total length of the IoSlice of msg
        let total_len = msg.iter().map(|slice| slice.len()).sum::<usize>();
        // allocate the local buffer once.
        let local_buf = self.send_buf.alloc(total_len as u32).await.unwrap();
        // iterate over the slices and copy the data to the local buffer, and send the buffer to the remote
        let mut addr_idx = local_buf.addr;
        for slice in msg {
            unsafe {
                std::ptr::copy_nonoverlapping(slice.as_ptr(), addr_idx as *mut _, slice.len())
            };
            addr_idx += slice.len() as u64;
        }
        if addr_idx != local_buf.addr + local_buf.length as u64 {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "the length of the IoSlice is not equal to the total length",
            ));
        }

        let _lock = self.lock.lock().await;
        // allocate a remote buffer
        let buf = self.allocator.alloc(total_len as u32).unwrap();
        // post a send operation
        self.qp.write_with_imm(local_buf, buf, 32);
        Ok(())
    }

    pub async fn recv_msg(&self) -> io::Result<&[u8]> {
        self.recv_buf.recv().await
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnType {
    Client,
    Server,
}

// client side use this function to connect to server
pub async fn connect(addr: &str) -> Result<Conn> {
    // connect to server
    let stream = TcpStream::connect(addr).await?;

    let device = Arc::new(Device::new(default_device()));
    // Create a new QP
    let mut qp = QP::new(device, QPCap::new(1000, 1000, 5, 5));
    if let Err(err) = qp.init() {
        println!("err: {}", err);
    }
    qp.set_stream(stream);
    qp.handshake().await;
    println!("handshake done");
    // exchange recv_buf with client
    let (recv_buf, remote_mr, rx) = qp.exchange_recv_buf().await;
    let conn = Conn::new(Arc::new(qp), recv_buf, remote_mr, rx).await;

    Ok(conn)
}

// server side use this function to listen to client
pub async fn run(addr: String, sender: Sender<Conn>) {
    let listener = TcpListener::bind(addr.clone()).await.unwrap();
    let device = Arc::new(Device::new(default_device()));
    loop {
        match listener.accept().await {
            Ok((stream, addr)) => {
                println!("New connection: {}", addr);
                // Create a QP for the new connection
                let mut qp = QP::new(device.clone(), QPCap::new(1000, 1000, 5, 5));
                if let Err(err) = qp.init() {
                    println!("err: {}", err);
                }
                qp.set_stream(stream);
                qp.handshake().await;
                println!("handshake done");
                // exchange recv_buf with client
                let (recv_buf, remote_mr, tx) = qp.exchange_recv_buf().await;
                let conn = Conn::new(Arc::new(qp), recv_buf, remote_mr, tx).await;

                if let Err(e) = sender.send(conn).await {
                    println!("Error: {}", e);
                    break;
                }
            }
            Err(e) => {
                println!("Error: {}", e);
                break;
            }
        }
    }
}
