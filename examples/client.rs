//! cargo run --exampmle client

use clippy_utilities::Cast;

use ibv::connection::client::Client;
use rdma_sys::{ibv_wr_opcode::IBV_WR_SEND, *};
use std::thread;
fn main() {
    let cli = Client::new();
    let qp = cli.connect("127.0.0.1:7777").unwrap();

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
