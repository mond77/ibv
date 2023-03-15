//! cargo run --example server
//!

use ibv::types::mr::{RemoteMR, MR};
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
    let mr = MR::new(&qp.pd, &mut recv_data);
    let mut wr = ibv_recv_wr {
        wr_id: 1,
        next: ptr::null_mut(),
        sg_list: ptr::null_mut(),
        num_sge: 1,
    };
    wr.sg_list = vec![mr.sge()].as_mut_ptr();
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
    let mr = MR::new(&qp.pd, &mut recv_data);
    let remote_mr = RemoteMR::from(&mr);
    qp.send_mr(remote_mr.clone());
    thread::sleep(std::time::Duration::from_millis(700));
    println!("server send mr info: {:?}", remote_mr);
    // data writen from client
    println!("server recv_data: {:?}", recv_data);
    // wait for client read
    let mut append_data = vec![1u8; 4];
    recv_data.append(&mut append_data);
    thread::sleep(std::time::Duration::from_millis(700));
}
