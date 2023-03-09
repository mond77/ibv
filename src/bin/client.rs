#[allow(unused)]
extern crate rdma_sys;
use std::{mem, ptr, sync::mpsc::Sender};

use clippy_utilities::Cast;
use ibv::types::cq::CQ;
use ibv::types::{device::{default_device, Device}, pd::PD};
use rdma_sys::{ibv_qp_attr_mask, *};
use std::sync::{mpsc::*, Arc};
use std::thread;

fn get_qp_num(qp: *mut ibv_qp) -> u32 {
    return unsafe { (*qp).qp_num };
}

fn main() {
    let device = Device::new(default_device());

    let pd_client = Arc::new(PD::new(&device));
    let pd_server = Arc::new(PD::new(&device));

    let cq_client = Arc::new(CQ::new(&device));
    let cq_server = Arc::new(CQ::new(&device));

    let (tx1, rx1) = channel();
    let (tx2, rx2) = channel();

    println!("client start");
    let pd = pd_client.clone();
    let cq = cq_client.clone();
    let h1 = thread::spawn(move || client(pd, cq, tx1, rx2));

    println!("server start");
    let pd = pd_client.clone();
    let cq = cq_client.clone();
    let h2 = thread::spawn(move || server(pd, cq, tx1, rx1));
    h1.join().unwrap();
    h2.join().unwrap();
    unsafe {
        ibv_dealloc_pd(pd.inner());
        ibv_close_device(pd.device().inner());
    }
}

fn client(pd: Arc<PD>, cq: Arc<CQ>, tx: Sender<u32>, rx: Receiver<u32>) {
    unsafe {
        // Create a Completion Queue (CQ)
        // Initialize the QP (Queue Pair) attributes
        let mut qp_init_attr: ibv_qp_init_attr = mem::zeroed();
        qp_init_attr.qp_type = ibv_qp_type::IBV_QPT_RC;
        qp_init_attr.send_cq = cq.inner();
        qp_init_attr.recv_cq = cq.inner();
        qp_init_attr.cap.max_send_wr = 10;
        qp_init_attr.cap.max_recv_wr = 10;
        qp_init_attr.cap.max_send_sge = 1;
        qp_init_attr.cap.max_recv_sge = 1;

        // Create a QP
        let qp = ibv_create_qp(pd.inner(), &mut qp_init_attr);
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
            | ibv_access_flags::IBV_ACCESS_REMOTE_ATOMIC)
            .0
            .cast();
        attr.ah_attr.is_global = 0;
        // The LID qp_attr.ah_attr.dlid(X) must be assigned to port qp_attr.port_num(Y) in side Y
        attr.ah_attr.dlid = 1;
        attr.ah_attr.sl = 0;
        attr.ah_attr.src_path_bits = 0;
        attr.ah_attr.port_num = 1;
        // qp_attr.path_mtu(X) must be equal to qp_attr.path_mtu(Y)
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
        // qp_attr.rq_psn(X) must be equal to qp_attr.sq_psn(Y)
        attr.rq_psn = 100;
        attr.max_dest_rd_atomic = 1;
        attr.min_rnr_timer = 0x12;
        ibv_modify_qp(
            qp,
            &mut attr,
            (ibv_qp_attr_mask::IBV_QP_STATE
                | ibv_qp_attr_mask::IBV_QP_PATH_MTU
                | ibv_qp_attr_mask::IBV_QP_DEST_QPN
                | ibv_qp_attr_mask::IBV_QP_RQ_PSN)
                .0
                .cast(),
        );

        // Move the QP to the RTS (Ready state
        attr.qp_state = ibv_qp_state::IBV_QPS_RTS;
        attr.timeout = 14;
        attr.retry_cnt = 7;
        attr.rnr_retry = 7;
        // qp_attr.rq_psn(X) must be equal to qp_attr.sq_psn(Y)
        attr.sq_psn = 200;
        attr.max_rd_atomic = 1;
        ibv_modify_qp(
            qp,
            &mut attr,
            (ibv_qp_attr_mask::IBV_QP_STATE
                | ibv_qp_attr_mask::IBV_QP_TIMEOUT
                | ibv_qp_attr_mask::IBV_QP_RETRY_CNT
                | ibv_qp_attr_mask::IBV_QP_SQ_PSN
                | ibv_qp_attr_mask::IBV_QP_MAX_DEST_RD_ATOMIC)
                .0
                .cast(),
        );

        // The QP is now ready to use
        println!("client ready to use");
        thread::sleep(std::time::Duration::from_millis(500));

        let mut send_data: Vec<u8> = vec![1, 2, 3, 4];
        let access = attr.qp_access_flags;
        let mr = &mut *ibv_reg_mr(
            pd.inner(),
            send_data.as_mut_ptr().cast(),
            send_data.len(),
            access.cast(),
        );
        let mut wr = std::mem::zeroed::<ibv_send_wr>();
        wr.opcode = ibv_wr_opcode::IBV_WR_SEND;
        wr.send_flags = 0;
        wr.wr_id = 1;
        let segs = &mut ibv_sge {
            addr: mr.addr as u64,
            length: mr.length as u32,
            lkey: mr.lkey,
        };
        wr.sg_list = segs;
        let mut bad_wr = std::ptr::null_mut::<ibv_send_wr>();
        println!("client post send");
        ibv_post_send(qp, &mut wr, &mut bad_wr);
        println!("client send data: {:?}", send_data);

        //

        // Clean up resources
        ibv_destroy_qp(qp);
        ibv_destroy_cq(cq.inner());

        println!("done");
    }
}

fn server(pd: Arc<PD>, cq: Arc<CQ>, tx: Sender<u32>, rx: Receiver<u32>) {
    unsafe {
        // Create a Completion Queue (CQ)
        let cq = CQ::new(&device).as_ptr();

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
        let qp = ibv_create_qp(device.pd(), &mut qp_init_attr);
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
        // The LID qp_attr.ah_attr.dlid(X) must be assigned to port qp_attr.port_num(Y) in side Y
        attr.port_num = 1;
        attr.qp_access_flags = (ibv_access_flags::IBV_ACCESS_LOCAL_WRITE
            | ibv_access_flags::IBV_ACCESS_REMOTE_WRITE
            | ibv_access_flags::IBV_ACCESS_REMOTE_READ
            | ibv_access_flags::IBV_ACCESS_REMOTE_ATOMIC)
            .0
            .cast();
        attr.ah_attr.is_global = 0;
        // qp_attr.port_num(X) + qp_attr.ah_attr.src_path_bits(X) must be equal to qp_attr.ah_attr.dlid(Y)
        attr.ah_attr.dlid = 1 + 0;
        attr.ah_attr.sl = 0;
        attr.ah_attr.src_path_bits = 0;
        // qp_attr.port_num(X) must be equal to qp_attr.ah_attr.port_num(X)
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
        // qp_attr.rq_psn(X) must be equal to qp_attr.sq_psn(Y)
        attr.rq_psn = 200;
        attr.max_dest_rd_atomic = 1;
        attr.min_rnr_timer = 0x12;
        ibv_modify_qp(
            qp,
            &mut attr,
            (ibv_qp_attr_mask::IBV_QP_STATE
                | ibv_qp_attr_mask::IBV_QP_AV
                | ibv_qp_attr_mask::IBV_QP_PATH_MTU
                | ibv_qp_attr_mask::IBV_QP_DEST_QPN
                | ibv_qp_attr_mask::IBV_QP_RQ_PSN)
                .0
                .cast(),
        );

        // Move the QP to the RTS (Ready state
        attr.qp_state = ibv_qp_state::IBV_QPS_RTS;
        attr.timeout = 14;
        attr.retry_cnt = 7;
        attr.rnr_retry = 7;
        // qp_attr.rq_psn(X) must be equal to qp_attr.sq_psn(Y)
        attr.sq_psn = 100;
        attr.max_rd_atomic = 1;
        ibv_modify_qp(qp, &mut attr, (ibv_qp_attr_mask::IBV_QP_STATE).0.cast());

        // The QP is now ready to use
        println!("server ready to use");
        let mut recv_data: Vec<u8> = vec![0u8; 4];
        let access = attr.qp_access_flags;
        let mr = &mut *ibv_reg_mr(
            device.pd(),
            recv_data.as_mut_ptr().cast(),
            recv_data.len(),
            access.cast(),
        );
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
        ibv_post_recv(qp, &mut wr, &mut bad_wr);
        thread::sleep(std::time::Duration::from_secs(1));
        let mut wc = mem::zeroed::<ibv_wc>();
        ibv_req_notify_cq(cq, 1);
        ibv_poll_cq(cq, 1, &mut wc);
        println!("server poll_cq: wr_id {}", wc.wr_id);
        println!("server recv_data: {:?}", recv_data);

        //
        thread::sleep(std::time::Duration::from_secs(1));
        // Clean up resources
        ibv_destroy_qp(qp);
        ibv_destroy_cq(cq);
        println!("done");
    }
}
