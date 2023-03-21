//! cargo run --example server
//!

use ibv::types::mr::{RemoteMR, MR};
use ibv::types::wr::{WRType, WR};
use ibv::{connection::server::Server, types::qp::QP};
use std::thread;

fn main() {
    let server = Server::new("127.0.0.1:7777".to_owned());
    let mut conn = server.accept();

    println!("server ready to use");

    conn.recv_msg();

    // client write/read
    // read_write(&mut qp);

    // write_with_imm
    // write_imm(&mut qp);
}

pub fn read_write(qp: &mut QP) {
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

pub fn write_imm(qp: &mut QP) {
    let mut recv_data: Vec<u8> = vec![0u8; 4];
    let mr = MR::new(&qp.pd, &mut recv_data);
    // post to RQ
    let mut wr = WR::new(1, WRType::RECV, vec![], None);
    if let Err(e) = wr.post_to_qp(qp) {
        println!("post recv error: {:?}", e);
    }
    let remote_mr = RemoteMR::from(&mr);
    qp.send_mr(remote_mr.clone());
    thread::sleep(std::time::Duration::from_millis(700));
    println!("server send mr info: {:?}", remote_mr);
    // data writen from client
    println!("server recv_data: {:?}", recv_data);

    let wcs = qp.cq.poll_wc(1).unwrap();
    if wcs.len() == 0 {
        println!("no wc");
        return;
    }
    let wc = wcs.get(0).unwrap();
    println!("wc: {:?}", wc);
}
