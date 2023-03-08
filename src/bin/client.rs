extern crate rdma_sys;
use std::{ptr, mem, sync::mpsc::Sender};

use rdma_sys::{*,ibv_qp_attr_mask};
use clippy_utilities::Cast;
use std::sync::mpsc::*;
use std::thread;

fn get_qp_num(qp: *mut ibv_qp) -> u32 {
    return unsafe {(*qp).qp_num}
}

fn main() {
    let (tx1, rx1) = channel();
    let (tx2, rx2) = channel();
    let h1 = thread::spawn(|| {
        client(tx1,rx2)
    });
    let h2 = thread::spawn(|| {
        server(tx2,rx1)
    });
    h1.join().unwrap();
    h2.join().unwrap();
}

fn client(tx: Sender<u32>, rx: Receiver<u32>) {
    unsafe {
        let mut x = 1;

        // Open the InfiniBand device
        let context = ibv_open_device(*ibv_get_device_list(&mut x));
        // Allocate a Protection Domain (PD)
        let pd = ibv_alloc_pd(context);
        // Create a Completion Queue (CQ)
        let cq = ibv_create_cq(context, 10, ptr::null_mut(), ptr::null_mut(), 0);
        // Initialize the QP (Queue Pair) attributes
        let mut qp_init_attr: ibv_qp_init_attr = mem::zeroed();
        qp_init_attr.qp_type = ibv_qp_type::IBV_QPT_RC;
        qp_init_attr.send_cq = cq;
        qp_init_attr.recv_cq = cq;
        qp_init_attr.cap.max_send_wr = 10;
        qp_init_attr.cap.max_recv_wr = 10;
        qp_init_attr.cap.max_send_sge = 1;
        qp_init_attr.cap.max_recv_sge = 1;
        // Create a QP
        let qp = ibv_create_qp(pd, &mut qp_init_attr);
        let qp_num = get_qp_num(qp);
        println!("qp_num: {:?}", get_qp_num(qp));
        // Initialize the QP attributes for the local side
        let mut attr: ibv_qp_attr = mem::zeroed();
        let flags = ibv_qp_attr_mask::IBV_QP_STATE
            | ibv_qp_attr_mask::IBV_QP_PKEY_INDEX
            | ibv_qp_attr_mask::IBV_QP_PORT
            | ibv_qp_attr_mask::IBV_QP_ACCESS_FLAGS
            | ibv_qp_attr_mask::IBV_QP_AV
            | ibv_qp_attr_mask::IBV_QP_PATH_MTU;
        attr.qp_state = ibv_qp_state::IBV_QPS_INIT;
        attr.pkey_index = 0;
        attr.port_num = 1;
        attr.qp_access_flags = (ibv_access_flags::IBV_ACCESS_LOCAL_WRITE
            | ibv_access_flags::IBV_ACCESS_REMOTE_WRITE
            | ibv_access_flags::IBV_ACCESS_REMOTE_READ
            | ibv_access_flags::IBV_ACCESS_REMOTE_ATOMIC).0.cast();
        attr.ah_attr.is_global = 0;
        attr.ah_attr.dlid = 0;
        attr.ah_attr.sl = 0;
        attr.ah_attr.src_path_bits = 0;
        attr.ah_attr.port_num = 1;
        attr.path_mtu = ibv_mtu::IBV_MTU_1024;
        ibv_modify_qp(qp, &mut attr, flags.0.cast());
        // Exchange QP information with the remote side (e.g. using sockets)
        tx.send(qp_num);
        let remote_qp_num = rx.recv().unwrap();
        println!("client receive qp_num: {:?}", remote_qp_num);
        // Initialize the QP attributes for the remote side
        attr.qp_state = ibv_qp_state::IBV_QPS_RTR;
        attr.path_mtu = ibv_mtu::IBV_MTU_1024;
        attr.dest_qp_num = remote_qp_num;
        attr.rq_psn = 0;
        attr.max_dest_rd_atomic = 1;
        attr.min_rnr_timer = 0x12;
        ibv_modify_qp(qp, &mut attr, (ibv_qp_attr_mask::IBV_QP_STATE | ibv_qp_attr_mask::IBV_QP_AV).0.cast());
        // Move the QP to the RTS (Ready state
        attr.qp_state = ibv_qp_state::IBV_QPS_RTS;
        attr.timeout = 14;
        attr.retry_cnt = 7;
        attr.rnr_retry = 7;
        attr.sq_psn = 0;
        attr.max_rd_atomic = 1;
        ibv_modify_qp(qp, &mut attr, (ibv_qp_attr_mask::IBV_QP_STATE).0.cast());
        // The QP is now ready to use
            



        //

        // Clean up resources
        ibv_destroy_qp(qp);
        ibv_destroy_cq(cq);
        ibv_dealloc_pd(pd);
        ibv_close_device(context);
        println!("done");
    }
}

fn server(tx: Sender<u32>, rx: Receiver<u32>) {
    unsafe {
        let mut x = 1;

        // Open the InfiniBand device
        let context = ibv_open_device(*ibv_get_device_list(&mut x));

        // Allocate a Protection Domain (PD)
        let pd = ibv_alloc_pd(context);

        // Create a Completion Queue (CQ)
        let cq = ibv_create_cq(context, 10, ptr::null_mut(), ptr::null_mut(), 0);

        // Initialize the QP (Queue Pair) attributes
        let mut qp_init_attr: ibv_qp_init_attr = mem::zeroed();
        qp_init_attr.qp_type = ibv_qp_type::IBV_QPT_RC;
        qp_init_attr.send_cq = cq;
        qp_init_attr.recv_cq = cq;
        qp_init_attr.cap.max_send_wr = 10;
        qp_init_attr.cap.max_recv_wr = 10;
        qp_init_attr.cap.max_send_sge = 1;
        qp_init_attr.cap.max_recv_sge = 1;
        // Create a QP
        let qp = ibv_create_qp(pd, &mut qp_init_attr);

        let qp_num = get_qp_num(qp);
        println!("qp_num: {:?}", get_qp_num(qp));
        // Initialize the QP attributes for the local side
        let mut attr: ibv_qp_attr = mem::zeroed();
        let flags = ibv_qp_attr_mask::IBV_QP_STATE
            | ibv_qp_attr_mask::IBV_QP_PKEY_INDEX
            | ibv_qp_attr_mask::IBV_QP_PORT
            | ibv_qp_attr_mask::IBV_QP_ACCESS_FLAGS
            | ibv_qp_attr_mask::IBV_QP_AV
            | ibv_qp_attr_mask::IBV_QP_PATH_MTU;
        attr.qp_state = ibv_qp_state::IBV_QPS_INIT;
        attr.pkey_index = 0;
        attr.port_num = 1;
        attr.qp_access_flags = (ibv_access_flags::IBV_ACCESS_LOCAL_WRITE
            | ibv_access_flags::IBV_ACCESS_REMOTE_WRITE
            | ibv_access_flags::IBV_ACCESS_REMOTE_READ
            | ibv_access_flags::IBV_ACCESS_REMOTE_ATOMIC).0.cast();
        attr.ah_attr.is_global = 0;
        attr.ah_attr.dlid = 0;
        attr.ah_attr.sl = 0;
        attr.ah_attr.src_path_bits = 0;
        attr.ah_attr.port_num = 1;
        attr.path_mtu = ibv_mtu::IBV_MTU_1024;
        ibv_modify_qp(qp, &mut attr, flags.0.cast());

        // Exchange QP information with the remote side (e.g. using sockets)
        tx.send(qp_num);
        let remote_qp_num = rx.recv().unwrap();
        println!("server receive qp_num: {:?}", remote_qp_num);
        // Initialize the QP attributes for the remote side
        attr.qp_state = ibv_qp_state::IBV_QPS_RTR;
        attr.path_mtu = ibv_mtu::IBV_MTU_1024;
        attr.dest_qp_num = remote_qp_num;
        attr.rq_psn = 0;
        attr.max_dest_rd_atomic = 1;
        attr.min_rnr_timer = 0x12;
        ibv_modify_qp(qp, &mut attr, (ibv_qp_attr_mask::IBV_QP_STATE | ibv_qp_attr_mask::IBV_QP_AV).0.cast());

        // Move the QP to the RTS (Ready state
        attr.qp_state = ibv_qp_state::IBV_QPS_RTS;
        attr.timeout = 14;
        attr.retry_cnt = 7;
        attr.rnr_retry = 7;
        attr.sq_psn = 0;
        attr.max_rd_atomic = 1;
        ibv_modify_qp(qp, &mut attr, (ibv_qp_attr_mask::IBV_QP_STATE).0.cast());

        // The QP is now ready to use
            



        //

        // Clean up resources
        ibv_destroy_qp(qp);
        ibv_destroy_cq(cq);
        ibv_dealloc_pd(pd);
        ibv_close_device(context);
        println!("done");
    }
}