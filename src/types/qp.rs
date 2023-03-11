use std::{
    io::{Error, Result},
    mem,
    ptr::NonNull,
    str::FromStr,
};

use clippy_utilities::Cast;
use rdma_sys::*;

use super::{cq::CQ, pd::PD};

pub struct QP<'a> {
    inner: NonNull<ibv_qp>,
    pub pd: &'a PD<'a>,
    pub cq: &'a CQ<'a>,
}

impl<'a> QP<'a> {
    pub fn new(pd: &'a PD, cq: &'a CQ, qp_cap: QPCap) -> Self {
        Self {
            inner: create_qp(pd, cq, qp_cap),
            pd,
            cq,
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

    pub fn qpn(&self) -> u32 {
        unsafe { self.inner.as_ref().qp_num }
    }

    pub fn endpoint(&self) -> EndPoint {
        EndPoint {
            lid: self.pd.device.lid(),
            qpn: self.qpn(),
            gid: self.pd.device.gid(),
        }
    }

    pub fn init(&self) -> Result<()> {
        let mut attr = unsafe { std::mem::zeroed::<ibv_qp_attr>() };
        attr.qp_state = ibv_qp_state::IBV_QPS_INIT;
        attr.pkey_index = 0;
        attr.port_num = 1;
        attr.qp_access_flags = (ibv_access_flags::IBV_ACCESS_LOCAL_WRITE
            | ibv_access_flags::IBV_ACCESS_REMOTE_WRITE
            | ibv_access_flags::IBV_ACCESS_REMOTE_READ
            | ibv_access_flags::IBV_ACCESS_REMOTE_ATOMIC)
            .0.cast();
        let attr_mask = ibv_qp_attr_mask::IBV_QP_STATE
            | ibv_qp_attr_mask::IBV_QP_PKEY_INDEX
            | ibv_qp_attr_mask::IBV_QP_PORT
            | ibv_qp_attr_mask::IBV_QP_ACCESS_FLAGS;
        if unsafe { ibv_modify_qp(self.inner(), &mut attr, attr_mask.0.cast()) } != 0 {
            return Err(Error::last_os_error());
        }
        Ok(())
    }

    pub fn ready_to_receive(&self, remote_emp: EndPoint) -> Result<()> {
        let mut attr = unsafe { std::mem::zeroed::<ibv_qp_attr>() };
        attr.qp_state = ibv_qp_state::IBV_QPS_RTR;
        attr.path_mtu = ibv_mtu::IBV_MTU_512;
        attr.dest_qp_num = remote_emp.qpn;
        // qp_attr.rq_psn(X) must be equal to qp_attr.sq_psn(Y)
        attr.rq_psn = 0;
        attr.max_dest_rd_atomic = 1;
        attr.min_rnr_timer = 18;
        attr.ah_attr = new_ah(remote_emp);
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
}

impl Drop for QP<'_> {
    fn drop(&mut self) {
        unsafe {
            ibv_destroy_qp(self.inner());
        }
    }
}

unsafe impl Send for QP<'_> {}
unsafe impl Sync for QP<'_> {}

pub fn create_qp(pd: &PD, cq: &CQ, qp_cap: QPCap) -> NonNull<ibv_qp> {
    let mut qp_init_attr = unsafe { mem::zeroed::<ibv_qp_init_attr>() };
    qp_init_attr.send_cq = cq.inner();
    qp_init_attr.recv_cq = cq.inner();
    qp_init_attr.qp_type = ibv_qp_type::IBV_QPT_RC;
    qp_init_attr.sq_sig_all = 1;
    qp_init_attr.cap = qp_cap.into();
    // while send_flag in WR has IBV_SEND_SIGNALED. with sq_sig_all=0, a Work Completion will be generated when the processing of this WR will be ended.
    qp_init_attr.sq_sig_all = 0;

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
    ah_attr.is_global = 0;
    // If the destination is in same subnet, the LID of the port to which the subnet delivers the packets to.
    // If the destination is in another subnet, the LID of the Router
    ah_attr.dlid = enp.lid;
    // The local physical port that the packets will be sent from
    ah_attr.port_num = 1;

    //Gloabel route information about remote end. This is useful when sending packets to another subnet.
    ah_attr.grh.dgid = enp.gid;
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

pub struct EndPoint {
    qpn: u32,
    lid: u16,
    pub gid: ibv_gid,
}

impl EndPoint {
    pub fn new(qpn: u32, lid: u16, gid: [u8; 16]) -> Self {
        Self {
            qpn,
            lid,
            gid: ibv_gid { raw: gid },
        }
    }
}
