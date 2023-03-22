use std::sync::Arc;

use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use tokio::sync::mpsc::Sender;

use clippy_utilities::Cast;

use crate::types::{
    mr::{RecvBuffer, RemoteBufManager, RemoteMR, SendBuffer},
    qp::QP,
};

use super::{daemon::polling, DEFAULT_BUFFER_SIZE};

pub struct Conn {
    qp: Arc<QP>,
    send_buf: SendBuffer,
    //
    // recv_buf: RecvBuffer,
    pub recving: JoinHandle<()>,
    // remote recv_buf
    allocator: RemoteBufManager,
    _polling: JoinHandle<()>,
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
        let send_buf = SendBuffer::new(&qp.pd, DEFAULT_BUFFER_SIZE);
        let cq = qp.cq.clone();
        // add sufficient RQE, maybe use SRQ to notify adding RQE
        qp.post_null_recv(1000);
        let polling = tokio::spawn(polling(cq, tx));
        let recving = tokio::spawn(recv_msg(recv_buf));
        Conn {
            qp,
            allocator,
            lock: Mutex::new(()),
            send_buf,
            recving,
            _polling: polling,
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
        let buf = { self.allocator.alloc(msg.len() as u32).unwrap() };

        // post a send operation
        self.qp.write_with_imm(local_buf, buf, 32);
    }
}

pub async fn recv_msg(mut recv_buf: RecvBuffer) {
    println!("start recv_msg");
    loop {
        let length = recv_buf.rx.recv().await.unwrap();
        let data = recv_buf.read(length);
        // handel data
        println!("recv data: {:?}", data);
    }
}
