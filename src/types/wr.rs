//! WR (work request) types.

use super::qp::QP;
use rdma_sys::{
    ibv_wr_opcode::{IBV_WR_RDMA_READ, IBV_WR_RDMA_WRITE, IBV_WR_RDMA_WRITE_WITH_IMM, IBV_WR_SEND},
    *,
};
use std::io::Result;
#[derive(Clone)]
pub struct RDMA {
    r#type: RDMAType,
    addr: u64,
    rkey: u32,
}

impl RDMA {
    pub fn new(r#type: RDMAType, addr: u64, rkey: u32) -> Self {
        Self { r#type, addr, rkey }
    }
}

#[derive(Clone)]
pub enum RDMAType {
    READ,
    WRITE,
    // IBV_WR_RDMA_WRITE_WITH_IMM :
    // Receive Request will be consumed from the head of remote QP's Receive Queue and immediate data will be sent in the message.
    // This value will be available in the Work Completion that will be generated for the consumed Receive Request in the remote QP.
    WRITEIMM(u32),
}

pub struct WR {
    wr_type: WRType,
    // todo: unique wr_id
    wr_id: u64,
    // todo: add send_flags
    // include sg_list and num_sge
    sges: Vec<ibv_sge>,
    rdma: Option<RDMA>,
}

impl WR {
    // create WR, considering wr_id generating and how to manage it (in QP or global).
    pub fn new(wr_id: u64, wr_type: WRType, sges: Vec<ibv_sge>, rdma: Option<RDMA>) -> Self {
        Self {
            wr_type,
            wr_id,
            sges,
            rdma,
        }
    }

    // build WR, and post it to QP.
    pub fn post_to_qp(&mut self, qp: &QP) -> Result<()> {
        match self.wr_type {
            WRType::SEND => {
                let mut wr = unsafe { std::mem::zeroed::<ibv_send_wr>() };
                wr.wr_id = self.wr_id as u64;
                wr.num_sge = self.sges.len() as i32;
                wr.sg_list = self.sges.as_mut_ptr();
                match self.rdma.clone() {
                    Some(rdma) => {
                        // todo: memory safety problem
                        match rdma.r#type {
                            RDMAType::READ => {
                                //READ
                                wr.opcode = IBV_WR_RDMA_READ;
                            }
                            RDMAType::WRITE => {
                                //WRITE
                                wr.opcode = IBV_WR_RDMA_WRITE;
                            }
                            RDMAType::WRITEIMM(imm) => {
                                //WRITE_WITH_IMM
                                wr.opcode = IBV_WR_RDMA_WRITE_WITH_IMM;
                                wr.imm_data_invalidated_rkey_union.imm_data = imm;
                            }
                        }

                        wr.wr.rdma.remote_addr = rdma.addr;
                        wr.wr.rdma.rkey = rdma.rkey;
                    }
                    None => {
                        // SEND
                        wr.opcode = IBV_WR_SEND;
                    }
                }
                // send operation will be signaled
                // wr.send_flags = ibv_send_flags::IBV_SEND_SIGNALED.0.cast();
                let mut bad_send_wr = std::ptr::null_mut();
                let ret = unsafe { ibv_post_send(qp.inner(), &mut wr, &mut bad_send_wr) };
                if ret != 0 {
                    println!("ret: {}, qp_status: {:?}", ret, qp.status());
                    return Err(std::io::Error::last_os_error());
                }
            }
            WRType::RECV => {
                // RECV
                let mut wr = unsafe { std::mem::zeroed::<ibv_recv_wr>() };
                wr.wr_id = self.wr_id;
                wr.num_sge = self.sges.len() as i32;
                wr.next = std::ptr::null_mut();
                wr.sg_list = self.sges.as_mut_ptr();
                let mut bad_recv_wr = std::ptr::null_mut();
                let ret = unsafe { ibv_post_recv(qp.inner(), &mut wr, &mut bad_recv_wr) };
                if ret != 0 {
                    println!("ret: {}, qp_status: {:?}", ret, qp.status());
                }
                return Ok(());
            }
        }
        Ok(())
    }

    // only build a send WR, not post it to QP. Return ibv_send_wr.
    pub fn build_send_wr(&mut self) -> ibv_send_wr {
        let mut wr = unsafe { std::mem::zeroed::<ibv_send_wr>() };
        wr.wr_id = self.wr_id as u64;
        wr.num_sge = self.sges.len() as i32;
        wr.sg_list = self.sges.as_mut_ptr();
        match self.rdma.clone() {
            Some(rdma) => {
                // todo: memory safety problem
                match rdma.r#type {
                    RDMAType::READ => {
                        //READ
                        wr.opcode = IBV_WR_RDMA_READ;
                    }
                    RDMAType::WRITE => {
                        //WRITE
                        wr.opcode = IBV_WR_RDMA_WRITE;
                    }
                    RDMAType::WRITEIMM(imm) => {
                        //WRITE_WITH_IMM
                        wr.opcode = IBV_WR_RDMA_WRITE_WITH_IMM;
                        wr.imm_data_invalidated_rkey_union.imm_data = imm;
                    }
                }

                wr.wr.rdma.remote_addr = rdma.addr;
                wr.wr.rdma.rkey = rdma.rkey;
            }
            None => {
                // SEND
                wr.opcode = IBV_WR_SEND;
            }
        }
        // send operation will be signaled
        // wr.send_flags = ibv_send_flags::IBV_SEND_SIGNALED.0.cast();
        wr
    }

    // build a recv WR, not post it to QP. Return ibv_recv_wr.
    pub fn build_recv_wr(&mut self) -> *mut ibv_recv_wr {
        // todo: drop the box
        let mut wr = &mut unsafe { *Box::into_raw(Box::new(std::mem::zeroed::<ibv_recv_wr>())) };
        wr.wr_id = self.wr_id;
        wr.num_sge = self.sges.len() as i32;
        wr.next = std::ptr::null_mut();
        wr.sg_list = self.sges.as_mut_ptr();
        wr
    }

    // // build multi recv WRs in a list, and post it to QP. Return Result<()>.
    // pub fn post_multi_recv_wr(&mut self, qp: &QP, num: usize) {
    //     let head_wr = self.build_recv_wr();
    //     let mut prev_wr = &mut unsafe { *head_wr };
    //     for _ in 1..num {
    //         let next = self.build_recv_wr();
    //         prev_wr.next = next;
    //         prev_wr = unsafe { &mut *next };
    //     }
    //     let mut bad_recv_wr = std::ptr::null_mut();
    //     unsafe { ibv_post_recv(qp.inner(), head_wr, &mut bad_recv_wr) };
    // }
}

pub enum WRType {
    SEND,
    RECV,
}
