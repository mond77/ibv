//! interfaec Conn:
//!
//!     1.send_msg(data: &[IoSlice]) -> Result<()>
//!     2.recv_msg() -> Result<&[u8]>

use crate::types::{
    default::DEFAULT_RQE_COUNT,
    device::{default_device, Device},
    qp::QPCap,
};
use std::{io::IoSlice, sync::Arc};
use std::{
    io::Result,
    sync::atomic::{AtomicI32, Ordering},
};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::mpsc::error::TryRecvError,
};

use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio::{
    io,
    sync::mpsc::{Receiver, Sender},
};

use crate::types::{
    mr::{RecvBuffer, RemoteBufManager, RemoteMR, SendBuffer},
    qp::QP,
};

use super::daemon::polling;

// RQE of the remote side might be shortage, so we need to limit the number of sending
// test shows that the max sending is 1023 in RoCE that equal to the max RQE of the remote side
pub static MAX_SENDING: i32 = DEFAULT_RQE_COUNT as i32;

pub struct Conn {
    recv_buf: RecvBuffer,
    sending: AtomicI32,
    // protect three below: remote_buf alloc, sending and release.
    lock: Mutex<()>,
    allocator: RemoteBufManager,
    send_buf: SendBuffer,
    qp: Arc<QP>,
    release: (Sender<u32>, MyReceiver<u32>),
    pub daemon: JoinHandle<()>,
}

unsafe impl Send for Conn {}
unsafe impl Sync for Conn {}

impl Conn {
    pub async fn new(
        qp: Arc<QP>,
        recv_buf: RecvBuffer,
        remote_mr: RemoteMR,
        tx: Sender<(u32, u32)>,
    ) -> Self {
        let allocator = RemoteBufManager::new(remote_mr);
        let send_buf = SendBuffer::new(qp.pd.clone()).await;
        let qp_c = qp.clone();
        // add sufficient RQE, maybe use SRQ to notify adding RQE
        for _ in 0..DEFAULT_RQE_COUNT {
            qp.post_null_recv();
        }
        let daemon = tokio::spawn(polling(qp_c, tx));
        let (tx, rx) = tokio::sync::mpsc::channel(DEFAULT_RQE_COUNT as usize);
        let release = (tx, MyReceiver::new(rx));
        Conn {
            qp,
            allocator,
            sending: AtomicI32::new(0),
            lock: Mutex::new(()),
            send_buf,
            daemon,
            release,
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
        let (local_buf, wr_id) = self.send_buf.alloc(total_len as u32).await;
        // iterate over the slices and copy the data to the local buffer, and send the buffer to the remote
        let mut addr_idx = local_buf.addr;
        msg.iter().for_each(|slice| {
            unsafe {
                std::ptr::copy_nonoverlapping(slice.as_ptr(), addr_idx as *mut _, slice.len())
            };
            addr_idx += slice.len() as u64;
        });
        {
            let _lock = self.lock.lock().await;
            // too much sending will cause device error(memory exhausted or something)
            while self.sending.load(Ordering::Acquire) >= MAX_SENDING {
                tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
            }
            self.sending.fetch_add(1, Ordering::AcqRel);
            let release_length = self.get_release_length();

            // allocate a remote buffer
            let buf = self.allocator.alloc(total_len as u32).await;
            // post a send operation
            self.qp
                .write_with_imm(local_buf, buf, release_length, wr_id);
        }
        Ok(())
    }

    // after calling recv_msg(), need to call release() before calling recv_msg again
    pub async fn recv_msg(&self) -> io::Result<&[u8]> {
        let (length, imm) = self.recv_buf.recv().await;
        self.sending.fetch_add(-1, Ordering::AcqRel);
        if imm != 0 {
            self.allocator.update(imm);
        }
        self.recv_buf.read(length)
    }

    pub async fn release(&self, buf: &[u8]) {
        let length = buf.len() as u32;
        if let Some(release_len) = self.recv_buf.notify_release(length) {
            self.release.0.send(release_len).await.unwrap();
        }
    }

    pub fn get_release_length(&self) -> u32 {
        match self.release.1.try_recv() {
            Ok(imm) => imm,
            Err(TryRecvError::Empty) => 0,
            Err(_) => {
                panic!("get imm error");
            }
        }
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
    // println!("handshake done");
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
                // println!("handshake done");
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

pub struct MyReceiver<T>(*mut Receiver<T>);

unsafe impl<T> Send for MyReceiver<T> {}
unsafe impl<T> Sync for MyReceiver<T> {}

impl<T> MyReceiver<T> {
    pub fn new(rx: Receiver<T>) -> Self {
        MyReceiver(Box::into_raw(Box::new(rx)))
    }

    pub fn try_recv(&self) -> core::result::Result<T, TryRecvError> {
        let rx = unsafe { &mut *self.0 };
        rx.try_recv()
    }

    pub fn get_receiver(&self) -> &mut Receiver<T> {
        let rx = unsafe { &mut *self.0 };
        rx
    }

    pub async fn recv(&self) -> T {
        let rx = self.get_receiver();
        rx.recv().await.unwrap()
    }
}

impl<T> Drop for MyReceiver<T> {
    fn drop(&mut self) {
        unsafe {
            let _ = Box::from_raw(self.0);
        }
    }
}
