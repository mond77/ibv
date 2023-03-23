use std::sync::Arc;

use tokio::sync::mpsc::Sender;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use clippy_utilities::Cast;

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

    pub async fn send_msg(&self, msg: &[u8]) {
        let local_buf = self.send_buf.alloc(msg.len() as u32).unwrap();
        // copy bytes of msg to the memory in buf
        unsafe {
            std::ptr::copy_nonoverlapping(
                msg.as_ptr(),
                local_buf.addr as *mut _,
                local_buf.length.cast(),
            )
        };

        // allocate a remote buffer
        let _lock = self.lock.lock().await;
        let buf = self.allocator.alloc(msg.len() as u32).unwrap();

        // post a send operation
        self.qp.write_with_imm(local_buf, buf, 32);
    }

    pub async fn recv_msg(&self) -> Vec<u8> {
        self.recv_buf.recv().await
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnType {
    Client,
    Server,
}
