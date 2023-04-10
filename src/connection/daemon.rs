use tokio::sync::mpsc::Sender;

use crate::types::{
    cq::Opcode::{Write, WriteWithImm},
    mr::RecvBuffer,
    qp::QP,
};
use std::sync::{atomic::AtomicBool, Arc};

// if use tokio run a task of polling, the task will be blocked by the tokio runtime.
pub async fn polling(qp: Arc<QP>, tx: Sender<(u32, u32)>) {
    loop {
        let wcs = match qp.cq.poll_wc(100) {
            Ok(wcs) => wcs,
            Err(_) => {
                println!("poll wc error");
                break;
            }
        };
        for wc in wcs.iter() {
            // dipatch the wc

            // todo: check the wc status

            // match opcode
            match wc.opcode() {
                WriteWithImm => {
                    // post recv request immediately to avoid RQE shortage
                    qp.post_null_recv();
                    let length = wc.byte_len();
                    let imm = wc.imm_data();
                    // there is no need to spawn a task.
                    tx.send((length, imm)).await.unwrap();
                }
                Write => {
                    let using_p = wc.wr_id() as *const AtomicBool;
                    unsafe {
                        let using = Arc::from_raw(using_p);
                        using.store(false, std::sync::atomic::Ordering::Relaxed);
                        // println!("send release done")
                    }
                }
                _ => {
                    // todo: handle other opcode
                }
            }
        }
        if wcs.len() == 0 {
            // the interval of polling mattes a little with the throughput.
            // too long interval will affect latency.
            // too short interval will cause high cpu usage and other tasks can't be executed.
            // influence the situation of instantaneous mass requests that may cause RQE shortage.
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }
}

// cann't work
pub fn notify(qp: Arc<QP>, recv_buf: RecvBuffer) {
    loop {
        println!("req_notify");
        if let Err(e) = qp.cq.req_notify(true) {
            println!("req_notify error: {:?}", e);
            break;
        }
        println!("wait for wc");
        qp.cq.get_event();
        println!("ack_event");
        qp.cq.ack_event(1);
        // polling
        let wcs = qp.cq.poll_wc(10).unwrap();
        for wc in wcs.iter() {
            let length = wc.byte_len();
            let data = recv_buf.read(length).unwrap();
            // handel data
            println!("recv data: {:?}", data);
        }
    }
}
