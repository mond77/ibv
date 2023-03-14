//! cargo run --exampmle client

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
    thread::sleep(std::time::Duration::from_millis(500));
    // send operation
    // send(qp);

    // client write/read

    read_write(qp);
}

#[allow(dead_code)]
fn send(qp: QP) {
    thread::sleep(std::time::Duration::from_millis(500));

    let mut send_data: Vec<u8> = vec![1, 2, 3, 4];
    let access = (ibv_access_flags::IBV_ACCESS_LOCAL_WRITE
        | ibv_access_flags::IBV_ACCESS_REMOTE_WRITE
        | ibv_access_flags::IBV_ACCESS_REMOTE_READ
        | ibv_access_flags::IBV_ACCESS_REMOTE_ATOMIC)
        .0
        .cast();
    let mr = &mut unsafe {
        *ibv_reg_mr(
            qp.pd(),
            send_data.as_mut_ptr().cast(),
            send_data.len(),
            access,
        )
    };
    let mut wr = unsafe { std::mem::zeroed::<ibv_send_wr>() };
    wr.wr_id = 1;

    wr.opcode = IBV_WR_SEND;
    wr.send_flags = ibv_send_flags::IBV_SEND_SIGNALED.0;

    let mut segs: Vec<ibv_sge> = vec![];
    segs.push(ibv_sge {
        addr: mr.addr as u64,
        length: mr.length as u32,
        lkey: mr.lkey,
    });
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
    let mut remote_mr_info = vec![0u8; 20];
    qp.recv_stream(&mut remote_mr_info);
    let remote_mr = MR::deserialize(remote_mr_info);

    let access = (ibv_access_flags::IBV_ACCESS_LOCAL_WRITE
        | ibv_access_flags::IBV_ACCESS_REMOTE_WRITE
        | ibv_access_flags::IBV_ACCESS_REMOTE_READ
        | ibv_access_flags::IBV_ACCESS_REMOTE_ATOMIC)
        .0
        .cast();
    // Allocate a memory buffer for the data to be written
    let data: [u8; 4] = [1, 2, 3, 4];
    let data_mr = &unsafe {
        *ibv_reg_mr(
            qp.pd(),
            data.as_ptr() as *mut _,
            std::mem::size_of_val(&data),
            access,
        )
    };

    // Allocate a memory buffer for the received data
    let mut recv_data = vec![0u8; 4];
    let recv_data_mr = &unsafe {
        *ibv_reg_mr(
            qp.pd(),
            recv_data.as_mut_ptr() as *mut _,
            recv_data.len(),
            access,
        )
    };

    let mut wr = unsafe { std::mem::zeroed::<ibv_send_wr>() };
    wr.wr_id = 1;
    wr.opcode = IBV_WR_RDMA_WRITE;
    wr.send_flags = ibv_send_flags::IBV_SEND_SIGNALED.0;
    wr.num_sge = 1;
    wr.sg_list = &mut ibv_sge {
        addr: data_mr.addr as u64,
        length: data_mr.length as u32,
        lkey: data_mr.lkey,
    };
    wr.wr.rdma.remote_addr = remote_mr.addr;
    wr.wr.rdma.rkey = remote_mr.rkey;
    let mut bad_send_wr = std::ptr::null_mut();
    let _ = unsafe { ibv_post_send(qp.inner(), &mut wr, &mut bad_send_wr) };
    // Wait for the write to complete
    println!("write data: {:?}", data);
    thread::sleep(std::time::Duration::from_millis(500));

    // Read the data from the remote buffer
    let mut wr = unsafe { std::mem::zeroed::<ibv_send_wr>() };
    wr.wr_id = 1;
    wr.opcode = IBV_WR_RDMA_READ;
    wr.num_sge = 1;
    wr.sg_list = &mut ibv_sge {
        addr: recv_data_mr.addr as u64,
        length: recv_data_mr.length as u32,
        lkey: recv_data_mr.lkey,
    };
    wr.wr.rdma.remote_addr = remote_mr.addr;
    wr.wr.rdma.rkey = remote_mr.rkey;
    let mut bad_send_wr = std::ptr::null_mut();
    let _ = unsafe { ibv_post_send(qp.inner(), &mut wr, &mut bad_send_wr) };

    thread::sleep(std::time::Duration::from_millis(500));
    // Print the read data
    println!("read data{:?}", recv_data);
}
