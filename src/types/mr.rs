extern crate bincode;
use std::{
    mem::ManuallyDrop,
    ptr::NonNull,
    sync::{Arc, Mutex},
};

use clippy_utilities::Cast;
use rdma_sys::{ibv_access_flags, ibv_dereg_mr, ibv_mr, ibv_reg_mr, ibv_sge};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::Receiver;

use crate::connection::DEFAULT_BUFFER_SIZE;

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

impl From<Arc<MR>> for LocalBuf {
    fn from(mr: Arc<MR>) -> Self {
        Self {
            addr: mr.addr,
            length: mr.length,
            lkey: mr.lkey,
        }
    }
}

pub struct RemoteBufManager {
    // do it in conn level
    index: *mut u64,
    limit: u64,
    mr: RemoteMR,
}

impl RemoteBufManager {
    pub fn new(mr: RemoteMR) -> Self {
        Self {
            index: Box::into_raw(Box::new(mr.addr)),
            limit: mr.addr + mr.length as u64,
            mr,
        }
    }

    // todo: block it when the space is not enough
    pub fn alloc(&self, length: u32) -> Option<RemoteBuf> {
        unsafe {
            let index = self.index;
            if *index + length as u64 > self.limit {
                return None;
            }
            let addr = *index;
            let rkey = self.mr.rkey;
            *index = *index + length as u64;
            Some(RemoteBuf { addr, length, rkey })
        }
    }
}

// use BufPoll instead
pub struct SendBuffer {
    mr: Arc<MR>,
    send_buf: ManuallyDrop<Vec<u8>>,
    index: Mutex<u64>,
    limit: u64,
    local_buf: LocalBuf,
}

impl SendBuffer {
    pub fn new(pd: &PD) -> Self {
        let mut send_buf = ManuallyDrop::new(vec![0u8; DEFAULT_BUFFER_SIZE]);
        let mr = Arc::new(MR::new(pd, &mut send_buf));
        Self {
            index: Mutex::new(mr.addr),
            send_buf,
            limit: mr.addr + mr.length as u64,
            local_buf: mr.clone().into(),
            mr,
        }
    }

    pub fn alloc(&self, length: u32) -> Option<LocalBuf> {
        let mut idx = self.index.lock().unwrap();
        if *idx + length as u64 > self.limit {
            return None;
        }
        let addr = *idx;
        let lkey = self.local_buf.lkey;
        *idx = *idx + length as u64;

        Some(LocalBuf { addr, length, lkey })
    }
}

impl Drop for SendBuffer {
    fn drop(&mut self) {
        unsafe {
            ibv_dereg_mr(self.mr.inner());
            ManuallyDrop::drop(&mut self.send_buf);
        }
    }
}

pub struct RecvBuffer {
    mr: Arc<MR>,
    pub rx: *mut Receiver<u32>,
    recv_buffer: ManuallyDrop<Vec<u8>>,
    index: *mut u64,
    limit: u64,
}

unsafe impl Send for RecvBuffer {}
unsafe impl Sync for RecvBuffer {}

impl RecvBuffer {
    pub fn new(mr: Arc<MR>, recv_buffer: ManuallyDrop<Vec<u8>>, rx: Receiver<u32>) -> Self {
        Self {
            mr: mr.clone(),
            rx: Box::into_raw(Box::new(rx)),
            recv_buffer,
            index: Box::into_raw(Box::new(mr.addr)),
            limit: mr.addr + mr.length as u64,
        }
    }

    pub fn read(&self, length: u32) -> Vec<u8> {
        let mut data = vec![0u8; length as usize];
        unsafe {
            std::ptr::copy(*self.index as *const u8, data.as_mut_ptr(), length as usize);
        }
        let idx = unsafe { &mut *self.index };
        if *idx + length as u64 > self.limit {
            panic!("recv buffer overflow");
        }
        *idx = *idx + length as u64;
        data
    }

    pub fn rx(&self) -> &mut Receiver<u32> {
        unsafe { &mut *(self.rx) }
    }

    pub async fn recv(&self) -> Vec<u8> {
        let length = self.rx().recv().await.unwrap();
        self.read(length)
    }
}

impl Drop for RecvBuffer {
    fn drop(&mut self) {
        unsafe {
            ibv_dereg_mr(self.mr.inner());
            ManuallyDrop::drop(&mut self.recv_buffer);
        }
    }
}
