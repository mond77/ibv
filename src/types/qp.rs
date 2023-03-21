use std::{
    fmt::{self, Debug, Formatter},
    io::{Error, Result},
    mem::{self, size_of},
    ptr::{self, NonNull},
    sync::Arc,
};

use std::{
    io::{Read, Write},
    net::TcpStream,
};
extern crate bincode;
use serde::{Deserialize, Serialize};

use clippy_utilities::Cast;
use rdma_sys::*;

use crate::connection::DEFAULT_BUFFER_SIZE;

use super::{
    cq::CQ,
    default::DEFAULT_GID_INDEX,
    device::Device,
    mr::{LocalBuf, RecvBuffer, RemoteBuf, RemoteMR, MR},
    pd::PD,
    wr::{RDMAType, WRType, RDMA, WR},
};

pub struct QP {
    inner: NonNull<ibv_qp>,
    pub pd: Arc<PD>,
    pub cq: Arc<CQ>,
    stream: Option<TcpStream>,
}

impl QP {
    pub fn new(device: Arc<Device>, qp_cap: QPCap) -> Self {
        let pd = Arc::new(PD::new(device.clone()));
        let cq = Arc::new(CQ::new(device.clone()));
        Self {
            inner: create_qp(&pd, &cq, qp_cap),
            pd,
            cq,
            stream: None,
        }
    }

    pub fn inner(&self) -> *mut ibv_qp {
        self.inner.as_ptr()
    }

    pub fn pd(&self) -> *mut ibv_pd {
        self.pd.inner()
    }

    pub fn cq(&self) -> *mut ibv_cq {
        self.cq.inner()
    }

    pub fn set_stream(&mut self, stream: TcpStream) {
        self.stream = Some(stream);
    }

    pub fn qpn(&self) -> u32 {
        unsafe { self.inner.as_ref().qp_num }
    }

    pub fn endpoint(&self) -> EndPoint {
        EndPoint {
            lid: self.pd.device.lid(),
            qpn: self.qpn(),
            gid: self.pd.device.gid(DEFAULT_GID_INDEX as i8),
        }
    }

    pub fn init(&self) -> Result<()> {
        let mut attr = unsafe { std::mem::zeroed::<ibv_qp_attr>() };
        attr.qp_state = ibv_qp_state::IBV_QPS_INIT;
        attr.pkey_index = 0;
        attr.port_num = 1;
        attr.qp_access_flags = (ibv_access_flags::IBV_ACCESS_LOCAL_WRITE
            | ibv_access_flags::IBV_ACCESS_REMOTE_WRITE
            | ibv_access_flags::IBV_ACCESS_REMOTE_READ)
            .0;
        let attr_mask = ibv_qp_attr_mask::IBV_QP_STATE
            | ibv_qp_attr_mask::IBV_QP_PKEY_INDEX
            | ibv_qp_attr_mask::IBV_QP_PORT
            | ibv_qp_attr_mask::IBV_QP_ACCESS_FLAGS;
        if unsafe { ibv_modify_qp(self.inner(), &mut attr, attr_mask.0.cast()) } != 0 {
            return Err(Error::last_os_error());
        }
        Ok(())
    }

    pub fn handshake(&mut self) {
        // Exchange QP information withw the remote side (e.g. using sockets)
        let enp = self.endpoint();
        println!("server enp: {:?}", enp);
        let bytes = enp.to_bytes();
        if let Err(_) = self.stream.as_mut().unwrap().write_all(&bytes) {
            println!("write stream error");
        }
        let mut buf = vec![0; bytes.len()];
        if let Err(_) = self.stream.as_mut().unwrap().read_exact(&mut buf) {
            println!("read stream error");
        }
        let remote_enp = EndPoint::from_bytes(&buf);
        if let Err(err) = self.ready_to_receive(remote_enp) {
            println!("err: {}", err);
        }
        if let Err(err) = self.ready_to_send() {
            println!("server err: {}", err);
        }
    }

    pub fn ready_to_receive(&self, remote_emp: EndPoint) -> Result<()> {
        let mut attr = unsafe { std::mem::zeroed::<ibv_qp_attr>() };
        attr.qp_state = ibv_qp_state::IBV_QPS_RTR;
        attr.path_mtu = ibv_mtu::IBV_MTU_1024;
        attr.dest_qp_num = remote_emp.qpn;
        // qp_attr.rq_psn(X) must be equal to qp_attr.sq_psn(Y)
        attr.rq_psn = 0;
        attr.max_dest_rd_atomic = 1;
        attr.min_rnr_timer = 18;
        attr.ah_attr = ibv_ah_attr {
            dlid: remote_emp.lid.cast(),
            sl: 0,
            src_path_bits: 0,
            static_rate: 0,
            is_global: 1,
            port_num: 1,
            grh: ibv_global_route {
                sgid_index: DEFAULT_GID_INDEX as u8,
                dgid: ibv_gid {
                    raw: remote_emp.gid,
                },
                hop_limit: 255,
                traffic_class: 0,
                flow_label: 0,
            },
        };
        let attr_mask = ibv_qp_attr_mask::IBV_QP_STATE
            | ibv_qp_attr_mask::IBV_QP_AV
            | ibv_qp_attr_mask::IBV_QP_PATH_MTU
            | ibv_qp_attr_mask::IBV_QP_DEST_QPN
            | ibv_qp_attr_mask::IBV_QP_RQ_PSN
            | ibv_qp_attr_mask::IBV_QP_MAX_DEST_RD_ATOMIC
            | ibv_qp_attr_mask::IBV_QP_MIN_RNR_TIMER;
        if unsafe { ibv_modify_qp(self.inner(), &mut attr, attr_mask.0.cast()) } != 0 {
            return Err(Error::last_os_error());
        }
        Ok(())
    }

    pub fn ready_to_send(&self) -> Result<()> {
        let mut attr = unsafe { std::mem::zeroed::<ibv_qp_attr>() };
        attr.qp_state = ibv_qp_state::IBV_QPS_RTS;
        attr.timeout = 14;
        attr.retry_cnt = 6;
        attr.rnr_retry = 6;
        attr.sq_psn = 0;
        attr.max_rd_atomic = 1;
        let attr_mask = ibv_qp_attr_mask::IBV_QP_STATE
            | ibv_qp_attr_mask::IBV_QP_TIMEOUT
            | ibv_qp_attr_mask::IBV_QP_RETRY_CNT
            | ibv_qp_attr_mask::IBV_QP_RNR_RETRY
            | ibv_qp_attr_mask::IBV_QP_SQ_PSN
            | ibv_qp_attr_mask::IBV_QP_MAX_QP_RD_ATOMIC;
        // SAFETY: ffi, and qp will not modify by other threads
        if unsafe { ibv_modify_qp(self.inner(), &mut attr, attr_mask.0.cast()) } != 0 {
            return Err(Error::last_os_error());
        }
        Ok(())
    }

    pub fn exchange_recv_buf(&mut self) -> (RecvBuffer, RemoteMR) {
        let mut recv_buffer: Vec<u8> = vec![0u8; DEFAULT_BUFFER_SIZE];
        let mr = MR::new(&self.pd, &mut recv_buffer);
        // send local_buf to remote
        self.send_mr(RemoteMR::from(&mr));
        // receive remote_buf from remote
        let remote_mr = self.recv_mr();
        (RecvBuffer::new(Arc::new(mr)), remote_mr)
    }

    pub fn send_mr(&mut self, remote_mr: RemoteMR) {
        let bytes = remote_mr.serialize();
        self.stream.as_mut().unwrap().write_all(&bytes).unwrap();
    }

    // receive RemoteMR from stream
    pub fn recv_mr(&mut self) -> RemoteMR {
        let mut remote_mr_info = vec![0u8; size_of::<RemoteMR>()];
        self.stream
            .as_mut()
            .unwrap()
            .read_exact(&mut remote_mr_info)
            .unwrap();
        RemoteMR::deserialize(remote_mr_info)
    }

    pub fn write_with_imm(&self, local_buf: LocalBuf, remote_buf: RemoteBuf, imm: u32) {
        let mut wr_write = WR::new(
            1,
            WRType::SEND,
            vec![local_buf.into()],
            Some(RDMA::new(
                RDMAType::WRITEIMM(imm),
                remote_buf.addr,
                remote_buf.rkey,
            )),
        );
        if let Err(e) = wr_write.post_to_qp(self) {
            println!("post send error: {:?}", e);
        }
    }

    pub fn post_null_recv(&self, count: usize) {
        for _ in 0..count {
            let mut wr_recv = WR::new(0, WRType::RECV, vec![], None);
            if let Err(e) = wr_recv.post_to_qp(self) {
                println!("post recv error: {:?}", e);
            }
        }
    }
}

// impl Drop for QP {
//     fn drop(&mut self) {
//         unsafe {
//             ibv_destroy_qp(self.inner());
//         }
//     }
// }

unsafe impl Send for QP {}
unsafe impl Sync for QP {}

pub fn create_qp(pd: &PD, cq: &CQ, qp_cap: QPCap) -> NonNull<ibv_qp> {
    let mut qp_init_attr = unsafe { mem::zeroed::<ibv_qp_init_attr>() };
    qp_init_attr.send_cq = cq.inner();
    qp_init_attr.recv_cq = cq.inner();
    qp_init_attr.qp_type = ibv_qp_type::IBV_QPT_RC;
    qp_init_attr.cap = qp_cap.into();
    // while send_flag in WR has IBV_SEND_SIGNALED. with sq_sig_all=0, a Work Completion will be generated when the processing of this WR will be ended.
    qp_init_attr.sq_sig_all = 0;
    qp_init_attr.qp_context = ptr::null_mut();
    qp_init_attr.srq = ptr::null_mut();

    let qp = unsafe { ibv_create_qp(pd.inner(), &mut qp_init_attr) };
    NonNull::new(qp).unwrap()
}

pub struct QPCap {
    max_send_wr: u32,
    max_recv_wr: u32,
    max_send_sge: u32,
    max_recv_sge: u32,
    max_inline_data: u32,
}

impl QPCap {
    pub fn new(max_send_wr: u32, max_recv_wr: u32, max_send_sge: u32, max_recv_sge: u32) -> Self {
        Self {
            max_send_wr,
            max_recv_wr,
            max_send_sge,
            max_recv_sge,
            max_inline_data: 0,
        }
    }
}

impl Into<ibv_qp_cap> for QPCap {
    fn into(self) -> ibv_qp_cap {
        ibv_qp_cap {
            max_send_wr: self.max_send_wr,
            max_recv_wr: self.max_recv_wr,
            max_send_sge: self.max_send_sge,
            max_recv_sge: self.max_recv_sge,
            max_inline_data: self.max_inline_data,
        }
    }
}

// pub struct QPInitAttr<'a> {
//     qp_type: Type,
//     send_cq: &'a CQ<'a>,
//     recv_cq: &'a CQ<'a>,
//     sq_sig_all: i32,
//     qp_cap: QPCap,
// }

// pub enum Type {
//     RC,
//     UD,
// }

// impl<'a> QPInitAttr<'a> {
//     pub fn new(qp_type: Type, send_cq: &'a CQ, recv_cq: &'a CQ, sq_sig_all: i32, qp_cap: QPCap) -> Self {
//         Self {
//             qp_type,
//             send_cq,
//             recv_cq,
//             sq_sig_all,
//             qp_cap,
//         }
//     }
// }

// impl Into<ibv_qp_init_attr> for QPInitAttr<'_> {
//     fn into(self) -> ibv_qp_init_attr {
//         let mut init_attr = unsafe { mem::zeroed::<ibv_qp_init_attr>() };
//         init_attr.qp_type = match self.qp_type {
//             Type::RC => ibv_qp_type::IBV_QPT_RC,
//             Type::UD => ibv_qp_type::IBV_QPT_UD,
//         };
//         init_attr.send_cq = self.send_cq.inner();
//         init_attr.recv_cq = self.recv_cq.inner();
//         init_attr.sq_sig_all = self.sq_sig_all;
//         init_attr.cap = self.qp_cap.into();
//         init_attr
//     }
// }

pub fn new_ah(enp: EndPoint) -> ibv_ah_attr {
    let mut ah_attr = unsafe { mem::zeroed::<ibv_ah_attr>() };
    ah_attr.is_global = 1;
    // If the destination is in same subnet, the LID of the port to which the subnet delivers the packets to.
    // If the destination is in another subnet, the LID of the Router
    ah_attr.dlid = enp.lid;
    // The local physical port that the packets will be sent from
    ah_attr.port_num = 1;

    //Gloabel route information about remote end. This is useful when sending packets to another subnet.
    ah_attr.grh.dgid = ibv_gid { raw: enp.gid };
    ah_attr.grh.flow_label = 0;
    ah_attr.grh.hop_limit = 255;
    ah_attr.grh.traffic_class = 0;
    ah_attr.grh.sgid_index = 1;

    // service level
    ah_attr.sl = 0;
    //The used Source Path Bits. This is useful when LMC is used in the port, i.e. each port covers a range of LIDs.
    ah_attr.src_path_bits = 0;
    // A value which limits the rate of packets that being sent to the subnet.
    ah_attr.static_rate = 0;
    ah_attr
}

#[derive(Serialize, Deserialize, Clone, Copy)]
pub struct EndPoint {
    pub gid: [u8; 16],
    qpn: u32,
    lid: u16,
}

impl Debug for EndPoint {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "qpn: {}, lid: {}, gid: {:?}",
            self.qpn, self.lid, self.gid
        )
    }
}

impl EndPoint {
    pub fn new(qpn: u32, lid: u16, gid: [u8; 16]) -> Self {
        Self { qpn, lid, gid }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        bincode::serialize(self).unwrap()
    }

    pub fn from_bytes(bytes: &[u8]) -> Self {
        bincode::deserialize(bytes).unwrap()
    }
}
