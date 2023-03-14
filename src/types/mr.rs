extern crate bincode;
use rdma_sys::ibv_mr;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy)]
pub struct MR {
    pub addr: u64,
    pub length: u32,
    pub lkey: u32,
    pub rkey: u32,
}

impl MR {
    // get MR form ibv_mr
    pub fn from_ibv_mr(mr: *const ibv_mr) -> Self {
        let mr = &unsafe { *mr };
        Self {
            addr: mr.addr as u64,
            length: mr.length as u32,
            lkey: mr.lkey,
            rkey: mr.rkey,
        }
    }

    //serialize MR to Vec<u8>
    pub fn serialize(&self) -> Vec<u8> {
        bincode::serialize(self).unwrap()
    }

    //deserialize Vec<u8> to MR
    pub fn deserialize(data: Vec<u8>) -> Self {
        bincode::deserialize(&data).unwrap()
    }
}
