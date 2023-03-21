extern crate bincode;
use std::{
    ptr::NonNull,
    sync::{Arc, Mutex},
};

use clippy_utilities::Cast;
use rdma_sys::{ibv_access_flags, ibv_dereg_mr, ibv_mr, ibv_reg_mr, ibv_sge};
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

    pub fn inner(&self) -> *mut ibv_mr {
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

impl Drop for MR {
    fn drop(&mut self) {
        unsafe { ibv_dereg_mr(self.inner()) };
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

#[derive(Clone)]
pub struct LocalBuf {
    pub addr: u64,
    pub length: u32,
    pub lkey: u32,
}

impl Into<ibv_sge> for LocalBuf {
    fn into(self) -> ibv_sge {
        ibv_sge {
            addr: self.addr,
            length: self.length,
            lkey: self.lkey,
        }
    }
}

pub struct RemoteBufManager {
    // do it in conn level
    _lock: Mutex<()>,
    index: u64,
    limit: u64,
    mr: RemoteMR,
}

impl RemoteBufManager {
    pub fn new(mr: RemoteMR) -> Self {
        Self {
            _lock: Mutex::new(()),
            index: mr.addr,
            limit: mr.addr + mr.length as u64,
            mr,
        }
    }

    // todo: block it when the space is not enough
    pub fn alloc(&mut self, length: u32) -> Option<RemoteBuf> {
        // let _lock = self._lock.lock().unwrap();
        if self.index + length as u64 > self.limit {
            return None;
        }
        let addr = self.index;
        let rkey = self.mr.rkey;
        self.index = self.index + length as u64;
        Some(RemoteBuf { addr, length, rkey })
    }
}

// use BufPoll instead
pub struct SendBuffer {
    lock: Mutex<()>,
    mr: Arc<MR>,
    index: u64,
    limit: u64,
}

impl SendBuffer {
    pub fn new(pd: &PD, size: usize) -> Self {
        let mut send_buf: Vec<u8> = vec![0u8; size];
        let mr = Arc::new(MR::new(pd, &mut send_buf));
        Self {
            lock: Mutex::new(()),
            mr: mr.clone(),
            index: mr.addr,
            limit: mr.addr + mr.length as u64,
        }
    }

    pub fn alloc(&mut self, length: u32) -> Option<LocalBuf> {
        let _lock = self.lock.lock().unwrap();
        if self.index + length as u64 > self.limit {
            return None;
        }
        let addr = self.index;
        let lkey = self.mr.lkey;
        self.index = self.index + length as u64;
        Some(LocalBuf { addr, length, lkey })
    }
}

pub struct RecvBuffer {
    _mr: Arc<MR>,
    index: u64,
    limit: u64,
}

impl RecvBuffer {
    pub fn new(mr: Arc<MR>) -> Self {
        Self {
            _mr: mr.clone(),
            index: mr.addr,
            limit: mr.addr + mr.length as u64,
        }
    }
}

impl RecvBuffer {
    pub fn read(&mut self, length: u32) -> Vec<u8> {
        let mut data = vec![0u8; length as usize];
        unsafe {
            std::ptr::copy(self.index as *const u8, data.as_mut_ptr(), length as usize);
        }
        if self.index + length as u64 > self.limit {
            panic!("recv buffer overflow");
        }
        self.index = self.index + length as u64;
        data
    }
}
