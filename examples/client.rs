//! cargo run --example client

use clippy_utilities::Cast;

use ibv::{
    connection::client::Client,
    types::{mr::MR, qp::QP},
};
use rdma_sys::ibv_wr_opcode::*;
use rdma_sys::{ibv_wr_opcode::IBV_WR_SEND, *};
use std::thread;

fn main() {
    let cli = Client::new();
    let qp = cli.connect("127.0.0.1:7777").unwrap();

    println!("client ready to use");

    // send operation
    // send(qp);

    // client write/read

    read_write(qp);
}

#[allow(dead_code)]
fn send(qp: QP) {
    thread::sleep(std::time::Duration::from_millis(500));

    let mut send_data: Vec<u8> = vec![1, 2, 3, 4];
    let mr = MR::new(&qp.pd, &mut send_data);
    let mut wr = unsafe { std::mem::zeroed::<ibv_send_wr>() };
    wr.wr_id = 1;

    wr.opcode = IBV_WR_SEND;
    wr.send_flags = ibv_send_flags::IBV_SEND_SIGNALED.0;

    let mut segs: Vec<ibv_sge> = vec![];
    segs.push(mr.sge());
    wr.sg_list = segs.as_mut_ptr();
    wr.num_sge = segs.len().cast();
    let mut bad_wr = std::ptr::null_mut::<ibv_send_wr>();
    println!("client post send");
    unsafe {
        let no = ibv_post_send(qp.inner(), &mut wr, &mut bad_wr);
        if no != 0 {
            println!("post send : {}", no);
        }
    }

    println!("client send data: {:?}", send_data);

    println!("done");
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
    let mut wr = unsafe { std::mem::zeroed::<ibv_send_wr>() };
    wr.wr_id = 1;
    wr.opcode = IBV_WR_RDMA_WRITE;
    wr.num_sge = 1;
    let mut sgs = vec![data_mr.sge()];
    wr.sg_list = sgs.as_mut_ptr();
    wr.wr.rdma.remote_addr = remote_mr.addr;
    wr.wr.rdma.rkey = remote_mr.rkey;
    let mut bad_send_wr = std::ptr::null_mut();
    let _ = unsafe { ibv_post_send(qp.inner(), &mut wr, &mut bad_send_wr) };
    // Wait for the write to complete
    println!("write data: {:?}", data);
    thread::sleep(std::time::Duration::from_millis(500));

    // Read the data from the remote buffer
    let mut wr = unsafe { std::mem::zeroed::<ibv_send_wr>() };
    wr.wr_id = 2;
    wr.opcode = IBV_WR_RDMA_READ;
    wr.num_sge = 1;
    let mut sgs = vec![recv_data_mr.sge()];
    wr.sg_list = sgs.as_mut_ptr();
    wr.wr.rdma.remote_addr = remote_mr.addr;
    wr.wr.rdma.rkey = remote_mr.rkey;
    let mut bad_send_wr = std::ptr::null_mut();
    let _ = unsafe { ibv_post_send(qp.inner(), &mut wr, &mut bad_send_wr) };

    thread::sleep(std::time::Duration::from_millis(500));
    // Print the read data
    println!("read data{:?}", recv_data);
}
