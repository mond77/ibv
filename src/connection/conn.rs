use std::sync::{Arc, Mutex};

use clippy_utilities::Cast;

use crate::types::{
    mr::{RecvBuffer, RemoteBufManager, RemoteMR, SendBuffer},
    qp::QP,
};

use super::DEFAULT_BUFFER_SIZE;

pub struct Conn {
    qp: Arc<QP>,
    send_buf: SendBuffer,
    //
    recv_buf: RecvBuffer,
    allocator: RemoteBufManager,
    lock: Mutex<()>,
}

unsafe impl Send for Conn {}
unsafe impl Sync for Conn {}

impl Conn {
    pub fn new(qp: Arc<QP>, recv_buf: RecvBuffer, remote_mr: RemoteMR) -> Self {
        let allocator = RemoteBufManager::new(remote_mr);
        let send_buf = SendBuffer::new(&qp.pd, DEFAULT_BUFFER_SIZE);
        Conn {
            qp,
            allocator,
            lock: Mutex::new(()),
            send_buf,
            recv_buf,
        }
    }

    pub fn qp(&self) -> Arc<QP> {
        self.qp.clone()
    }

    pub fn send_msg(&mut self, msg: &[u8]) {
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
        let _lock = self.lock.lock().unwrap();
        let buf = self.allocator.alloc(msg.len() as u32).unwrap();

        // post a send operation
        self.qp.write_with_imm(local_buf, buf, 32);
    }

    pub fn recv_msg(&mut self) {
        // maybe use SRQ to notify adding RQE
        self.qp.post_null_recv(1000);
        // if let Err(_) = self.qp.cq.req_notify(true) {
        //     println!("req notify error");
        //     return;
        // }
        loop {
            let wcs = match self.qp.cq.poll_wc(5) {
                Ok(wcs) => wcs,
                Err(_) => {
                    println!("poll wc error");
                    break;
                }
            };
            for wc in wcs.iter() {
                let data = self.recv_buf.read(wc.byte_len());

                // handle the data
                println!("recv data: {:?}", data);
            }
            if wcs.len() == 0 {
                // sleep for 10ms
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            self.qp.post_null_recv(wcs.len());
            
        }
    }
}
