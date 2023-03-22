extern crate bincode;
use std::{
    cell::RefCell,
    ptr::NonNull,
    sync::{Arc, Mutex},
};
use tokio::sync::mpsc::Receiver;

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

unsafe impl Send for MR {}
unsafe impl Sync for MR {}

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

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RemoteMR {
    pub addr: u64,
    pub length: u32,
    pub rkey: u32,
    // todo: access flags
}

impl RemoteMR {
    // get MR form ibv_mr
    pub fn from_mr(mr: Arc<MR>) -> Self {
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

impl From<MR> for LocalBuf {
    fn from(mr: MR) -> Self {
        Self {
            addr: mr.addr,
            length: mr.length,
            lkey: mr.lkey,
        }
    }
}

pub struct RemoteBufManager {
    // do it in conn level
    _lock: Mutex<()>,
    index: RefCell<u64>,
    limit: u64,
    mr: RemoteMR,
}

impl RemoteBufManager {
    pub fn new(mr: RemoteMR) -> Self {
        Self {
            _lock: Mutex::new(()),
            index: RefCell::new(mr.addr),
            limit: mr.addr + mr.length as u64,
            mr,
        }
    }

    // todo: block it when the space is not enough
    pub fn alloc(&self, length: u32) -> Option<RemoteBuf> {
        // let _lock = self._lock.lock().unwrap();
        let mut index = self.index.borrow_mut();
        if *index + length as u64 > self.limit {
            return None;
        }
        let addr = *index;
        let rkey = self.mr.rkey;
        *index = *index + length as u64;
        Some(RemoteBuf { addr, length, rkey })
    }
}

// use BufPoll instead
pub struct SendBuffer {
    lock: Mutex<u64>,
    limit: u64,
    send_buf: LocalBuf,
}

impl SendBuffer {
    pub fn new(pd: &PD, size: usize) -> Self {
        let mut send_buf: Vec<u8> = vec![0u8; size];
        let mr = MR::new(pd, &mut send_buf);
        Self {
            lock: Mutex::new(mr.addr),
            limit: mr.addr + mr.length as u64,
            send_buf: mr.into(),
        }
    }

    pub fn alloc(&self, length: u32) -> Option<LocalBuf> {
        let mut idx = self.lock.lock().unwrap();
        if *idx + length as u64 > self.limit {
            return None;
        }
        let addr = *idx;
        let lkey = self.send_buf.lkey;
        *idx = *idx + length as u64;
        Some(LocalBuf { addr, length, lkey })
    }
}

pub struct RecvBuffer {
    pub rx: Receiver<u32>,
    _mr: Arc<MR>,
    index: u64,
    limit: u64,
}

unsafe impl Send for RecvBuffer {}
unsafe impl Sync for RecvBuffer {}

impl RecvBuffer {
    pub fn new(mr: Arc<MR>, rx: Receiver<u32>) -> Self {
        Self {
            rx,
            _mr: mr.clone(),
            index: mr.addr,
            limit: mr.addr + mr.length as u64,
        }
    }

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
