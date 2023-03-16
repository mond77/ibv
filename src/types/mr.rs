extern crate bincode;
use std::{ptr::NonNull, sync::Mutex};

use clippy_utilities::Cast;
use rdma_sys::{ibv_access_flags, ibv_mr, ibv_reg_mr, ibv_sge};
use serde::{Deserialize, Serialize};

use super::pd::PD;

#[derive(Clone)]
pub struct MR {
    inner: NonNull<ibv_mr>,
    pub addr: u64,
    pub length: u32,
    pub lkey: u32,
    pub rkey: u32,
}

impl MR {
    pub fn new(pd: &PD, data: &mut [u8]) -> Self {
        // todo: access control
        let access = (ibv_access_flags::IBV_ACCESS_LOCAL_WRITE
            | ibv_access_flags::IBV_ACCESS_REMOTE_WRITE
            | ibv_access_flags::IBV_ACCESS_REMOTE_READ
            | ibv_access_flags::IBV_ACCESS_REMOTE_ATOMIC)
            .0
            .cast();
        let mr =
            &mut unsafe { *ibv_reg_mr(pd.inner(), data.as_mut_ptr().cast(), data.len(), access) };
        Self {
            inner: NonNull::new(mr).unwrap(),
            addr: mr.addr as u64,
            length: mr.length.cast(),
            lkey: mr.lkey,
            rkey: mr.rkey,
        }
    }

    pub fn inner(&self) -> *const ibv_mr {
        self.inner.as_ptr()
    }

    pub fn sge(&self) -> ibv_sge {
        ibv_sge {
            addr: self.addr,
            length: self.length,
            lkey: self.lkey,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RemoteMR {
    pub addr: u64,
    pub length: u32,
    pub rkey: u32,
    // todo: access flags
}

impl RemoteMR {
    // get MR form ibv_mr
    pub fn from_ibv_mr(mr: *const ibv_mr) -> Self {
        let mr = &unsafe { *mr };
        Self {
            addr: mr.addr as u64,
            length: mr.length as u32,
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

impl From<&MR> for RemoteMR {
    fn from(mr: &MR) -> Self {
        Self {
            addr: mr.addr,
            length: mr.length,
            rkey: mr.rkey,
        }
    }
}

// a section of remote MR, alloced
#[derive(Clone)]
pub struct RemoteBuf {
    pub addr: u64,
    pub length: u32,
    pub rkey: u32,
}

pub struct RemoteBufManager {
    lock: Mutex<()>,
    // todo: index
    mr: RemoteMR,
}

impl RemoteBufManager {
    pub fn new(mr: RemoteMR) -> Self {
        Self {
            lock: Mutex::new(()),
            mr,
        }
    }

    // todo: block it when the space is not enough
    // cann't work!
    pub fn alloc(&self, length: u32) -> Option<RemoteBuf> {
        let _lock = self.lock.lock().unwrap();
        if length > self.mr.length {
            return None;
        }
        let addr = self.mr.addr;
        let rkey = self.mr.rkey;
        Some(RemoteBuf { addr, length, rkey })
    }
}
