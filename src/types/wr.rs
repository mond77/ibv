//! WR (work request) types.

use super::mr::RemoteBuf;
use rdma_sys::ibv_sge;
use std::io::Result;

#[derive(Clone)]
pub struct RDMA {
    r#type: RDMAType,
    remote_buf: RemoteBuf,
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

    pub fn post_to_qp(&self) -> Result<()> {
        match self.wr_type {
            WRType::SEND => {
                match self.rdma.clone() {
                    Some(rdma) => {
                        // todo: memory safety problem
                        match rdma.r#type {
                            RDMAType::READ => {
                                //READ
                            }
                            RDMAType::WRITE => {
                                //WRITE
                            }
                            RDMAType::WRITEIMM(_) => {
                                //WRITE_WITH_IMM
                            }
                        }
                    }
                    None => {
                        // SEND
                    }
                }
            }
            WRType::RECV => {
                // RECV
            }
        }
        Ok(())
    }
}

pub enum WRType {
    SEND,
    RECV,
}
