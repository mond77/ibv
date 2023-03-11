#[allow(unused)]
extern crate rdma_sys;
use std::sync::mpsc::Sender;

#[macro_use]
extern crate lazy_static;

use clippy_utilities::Cast;
use ibv::types::cq::CQ;
use ibv::types::qp::{EndPoint, QPCap};
use ibv::types::{
    device::{default_device, Device},
    pd::PD,
    qp::QP,
};
use rdma_sys::ibv_wr_opcode::IBV_WR_SEND;
use rdma_sys::*;
use std::sync::{mpsc::*, Arc};
use std::{ptr, thread};

lazy_static! {
    static ref DEVICE: Device = Device::new(default_device());
}

fn main() {
    let device = &DEVICE;

    let pd_client = Arc::new(PD::new(device));
    let pd_server = Arc::new(PD::new(device));

    let cq_client = Arc::new(CQ::new(device));
    let cq_server = Arc::new(CQ::new(device));

    let (tx1, rx1) = channel();
    let (tx2, rx2) = channel();

    println!("client start");
    let pd = pd_client.clone();
    let cq = cq_client.clone();
    let h1 = thread::spawn(move || client(pd, cq, tx1, rx2));

    println!("server start");
    let pd = pd_server.clone();
    let cq = cq_server.clone();
    let h2 = thread::spawn(move || server(pd, cq, tx2, rx1));
    h1.join().unwrap();
    h2.join().unwrap();
}

fn client(pd: Arc<PD>, cq: Arc<CQ>, tx: Sender<EndPoint>, rx: Receiver<EndPoint>) {
    let cap = QPCap::new(10, 10, 1, 1);
    // Create a QP
    let qp = QP::new(&pd, &cq, cap);
    if let Err(err) = qp.init() {
        println!("err: {}", err);
    }

    let enp = qp.endpoint();

    // Exchange QP information with the remote side (e.g. using sockets)
    tx.send(enp);
    let remote_enp = rx.recv().unwrap();

    if let Err(err) = qp.ready_to_receive(remote_enp) {
        println!("client err : {}", err);
    }
    if let Err(err) = qp.ready_to_send() {
        println!("client err: {}", err);
    }
    // The QP is now ready to use
    println!("client ready to use");
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
            pd.inner(),
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

fn server(pd: Arc<PD>, cq: Arc<CQ>, tx: Sender<EndPoint>, rx: Receiver<EndPoint>) {
    let cap = QPCap::new(10, 10, 1, 1);
    // Create a QP
    let qp = QP::new(&pd, &cq, cap);
    qp.init();

    let enp = qp.endpoint();

    // Exchange QP information withw the remote side (e.g. using sockets)
    tx.send(enp);
    let remote_enp = rx.recv().unwrap();

    if let Err(err) = qp.ready_to_receive(remote_enp) {
        println!("server err : {}", err);
    }
    if let Err(err) = qp.ready_to_send() {
        println!("server err: {}", err);
    }
    // The QP is now ready to use
    println!("server ready to use");
    let mut recv_data: Vec<u8> = vec![0u8; 4];
    let access = (ibv_access_flags::IBV_ACCESS_LOCAL_WRITE
        | ibv_access_flags::IBV_ACCESS_REMOTE_WRITE
        | ibv_access_flags::IBV_ACCESS_REMOTE_READ
        | ibv_access_flags::IBV_ACCESS_REMOTE_ATOMIC)
        .0
        .cast();
    let mr = &mut unsafe {
        *ibv_reg_mr(
            pd.inner(),
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
        let no = ibv_poll_cq(cq.inner(), 1, &mut wc);
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
