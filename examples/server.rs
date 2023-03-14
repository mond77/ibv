//! cargo run --exampmle server

use clippy_utilities::Cast;
use ibv::types::mr::MR;
use ibv::{connection::server::Server, types::qp::QP};
use rdma_sys::*;
use std::{ptr, thread};

fn main() {
    let server = Server::new("127.0.0.1:7777".to_owned());
    let mut qp = server.accept();

    println!("server ready to use");
    // recv operation
    // recv(qp);

    // client write/read
    wait_for_client(&mut qp);
}

#[allow(dead_code)]
fn recv(qp: &QP) {
    let mut recv_data: Vec<u8> = vec![0u8; 4];
    let access = (ibv_access_flags::IBV_ACCESS_LOCAL_WRITE
        | ibv_access_flags::IBV_ACCESS_REMOTE_WRITE
        | ibv_access_flags::IBV_ACCESS_REMOTE_READ
        | ibv_access_flags::IBV_ACCESS_REMOTE_ATOMIC)
        .0
        .cast();
    let mr = &mut unsafe {
        *ibv_reg_mr(
            qp.pd(),
            recv_data.as_mut_ptr().cast(),
            recv_data.len(),
            access,
        )
    };
    let mut wr = ibv_recv_wr {
        wr_id: 1,
        next: ptr::null_mut(),
        sg_list: ptr::null_mut(),
        num_sge: 1,
    };

    let segs = &mut ibv_sge {
        addr: mr.addr as u64,
        length: mr.length as u32,
        lkey: mr.lkey,
    };
    wr.sg_list = segs;
    let mut bad_wr = std::ptr::null_mut::<ibv_recv_wr>();
    println!("server post recv");
    unsafe {
        let no = ibv_post_recv(qp.inner(), &mut wr, &mut bad_wr);
        if no != 0 {
            println!("post recv fail errno: {}", no);
        }
        thread::sleep(std::time::Duration::from_secs(1));
        let mut wc = std::mem::zeroed::<ibv_wc>();
        let no = ibv_poll_cq(qp.cq(), 1, &mut wc);
        if no != 0 {
            println!("poll cq : {}", no);
        }
        println!("server poll_cq: wr_id {}", wc.wr_id);
    };
    println!("server recv_data: {:?}", recv_data);

    //
    thread::sleep(std::time::Duration::from_secs(1));
    println!("done");
}

pub fn wait_for_client(qp: &mut QP) {
    let mut recv_data: Vec<u8> = vec![0u8; 4];
    let access = (ibv_access_flags::IBV_ACCESS_LOCAL_WRITE
        | ibv_access_flags::IBV_ACCESS_REMOTE_WRITE
        | ibv_access_flags::IBV_ACCESS_REMOTE_READ
        | ibv_access_flags::IBV_ACCESS_REMOTE_ATOMIC)
        .0
        .cast();
    let mr = &mut unsafe {
        *ibv_reg_mr(
            qp.pd(),
            recv_data.as_mut_ptr().cast(),
            recv_data.len(),
            access,
        )
    };
    let mr_info = MR::from_ibv_mr(mr).serialize();
    qp.send_stream(&mr_info);
    thread::sleep(std::time::Duration::from_secs(1));
    println!("server send mr info: {:?}", mr_info);
    // data writen from client
    println!("server recv_data: {:?}", recv_data);
    // wait for client read
    thread::sleep(std::time::Duration::from_secs(1));
}
