use crate::types::{
    cq::Opcode::{Write, WriteWithImm},
    mr::RecvBuffer,
    qp::QP,
};
use std::sync::Arc;

// if use tokio run a task of polling, the task will be blocked by the tokio runtime. If use tokio(mpsc), the disorder of wc will be a problem.(it doesn't matter)
pub fn polling(qp: Arc<QP>, mut recv_buf: RecvBuffer) {
    let mut count = 0;
    loop {
        let wcs = match qp.cq.poll_wc(10) {
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
                    let length = wc.byte_len();
                    let data = recv_buf.read(length);
                    // handel data

                    if data.get(data.len() - 1).unwrap() == &0 {
                        println!("error");
                    }
                    count = count + 1;
                    println!("{} recv data: {:?}", count, data);
                    // todo: add RQE in task
                    // qp.post_null_recv(1);
                }
                Write => {
                    // send data
                }
                _ => {
                    // todo: handle other opcode
                }
            }
        }
        if wcs.len() == 0 {
            // sleep for 10ms
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
}

pub fn notify(qp: Arc<QP>, mut recv_buf: RecvBuffer) {
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
            let data = recv_buf.read(length);
            // handel data
            println!("recv data: {:?}", data);
        }
    }
}
