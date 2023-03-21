//! cargo run --example client

use ibv::{
    connection::client::Client,
    types::{
        mr::MR,
        qp::QP,
        wr::{RDMAType, WRType, RDMA, WR},
    },
};
use std::thread;

fn main() {
    let cli = Client::new();
    let mut conn = cli.connect("127.0.0.1:7777").unwrap();

    println!("client ready to use");

    thread::sleep(std::time::Duration::from_millis(500));

    for i in 0..100 {
        let data = vec![i as u8; 16];
        conn.send_msg(&data);
    }

    //sleep for 1s
    thread::sleep(std::time::Duration::from_millis(1000));
    // client write/read
    // read_write(qp);

    // write_imm
    // write_imm(&mut qp);
}

pub fn read_write(mut qp: QP) {
    let remote_mr = qp.recv_mr();

    thread::sleep(std::time::Duration::from_millis(500));

    // Allocate a memory buffer for the data to be written
    let mut data: Vec<u8> = vec![1, 2, 3, 4];
    let data_mr = MR::new(&qp.pd, &mut data);

    // Allocate a memory buffer for the received data
    let mut recv_data = vec![0u8; 4];
    let recv_data_mr = MR::new(&qp.pd, &mut recv_data);

    // Write the data to the remote buffer
    let mut wr_write = WR::new(
        1,
        WRType::SEND,
        vec![data_mr.sge()],
        Some(RDMA::new(RDMAType::WRITE, remote_mr.addr, remote_mr.rkey)),
    );
    if let Err(e) = wr_write.post_to_qp(&qp) {
        println!("post send error: {:?}", e);
    }
    // Wait for the write to complete
    println!("write data: {:?}", data);
    thread::sleep(std::time::Duration::from_millis(500));

    // Read the data from the remote buffer
    let mut wr_read = WR::new(
        2,
        WRType::SEND,
        vec![recv_data_mr.sge()],
        Some(RDMA::new(RDMAType::READ, remote_mr.addr, remote_mr.rkey)),
    );

    if let Err(e) = wr_read.post_to_qp(&qp) {
        println!("post send error: {:?}", e);
    }

    thread::sleep(std::time::Duration::from_millis(500));
    // Print the read data
    println!("read data{:?}", recv_data);
}

pub fn write_imm(qp: &mut QP) {
    let remote_mr = qp.recv_mr();

    thread::sleep(std::time::Duration::from_millis(500));

    // Allocate a memory buffer for the data to be written
    let mut data: Vec<u8> = vec![1, 2, 3, 4];
    let data_mr = MR::new(&qp.pd, &mut data);

    // Write the data to the remote buffer
    let mut wr_write = WR::new(
        1,
        WRType::SEND,
        vec![data_mr.sge()],
        Some(RDMA::new(
            RDMAType::WRITEIMM(32),
            remote_mr.addr,
            remote_mr.rkey,
        )),
    );
    if let Err(e) = wr_write.post_to_qp(&qp) {
        println!("post send error: {:?}", e);
    }
    // Wait for the write to complete
    println!("write data: {:?}", data);
    thread::sleep(std::time::Duration::from_millis(500));
    let wcs = qp.cq.poll_wc(1).unwrap();
    if wcs.len() == 0 {
        println!("no wc");
        return;
    }
    let wc = wcs.get(0).unwrap();
    println!("wc: {:?}", wc);
}
