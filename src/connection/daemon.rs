use crate::types::cq::{
    Opcode::{Write, WriteWithImm},
    CQ,
};
use std::sync::Arc;
use tokio::sync::mpsc::Sender;

pub async fn polling(cq: Arc<CQ>, tx: Sender<u32>) {
    loop {
        let wcs = match cq.poll_wc(5) {
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
                    // receive data
                    if let Err(e) = tx.send(wc.byte_len()).await {
                        println!("polling send error: {}", e);
                    }
                    // todo: add RQE in task
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
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }
}
