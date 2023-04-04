#![allow(unused)]
extern crate bincode;
use std::{mem::ManuallyDrop, ptr::NonNull, sync::Arc};

use clippy_utilities::Cast;
use rdma_sys::{ibv_access_flags, ibv_dereg_mr, ibv_mr, ibv_reg_mr, ibv_sge};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::io;
use tokio::sync::mpsc::Receiver;

use super::default::{
    DEFAULT_PER_SIZE_BUFFER_COUNT, DEFAULT_SEND_BUFFER_SIZE, MIN_LENGTH_TO_NOTIFY_RELEASE,
};
use super::pd::PD;
use kanal;

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

unsafe impl Send for RemoteBuf {}
unsafe impl Sync for RemoteBuf {}

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
    // update from remote, when done catch up index, it means the space is empty.
    done: AtomicU64,
    // index of the available space, index cannot catch up done.
    index: AtomicU64,
    left: u64,
    right: u64,
    mr: RemoteMR,
}

impl RemoteBufManager {
    pub fn new(mr: RemoteMR) -> Self {
        Self {
            done: AtomicU64::new(mr.addr),
            index: AtomicU64::new(mr.addr),
            left: mr.addr,
            right: mr.addr + mr.length as u64,
            mr,
        }
    }

    pub async fn alloc(&self, length: u32) -> RemoteBuf {
        let rkey = self.mr.rkey;
        let index = &self.index;
        let done = &self.done;
        // notice: index could catch up done, that is a constraint.
        if index.load(Ordering::Relaxed) + length as u64 > self.right {
            // wait for remote to release the space, until done > self.left + length as u64 (that means space is enough)
            while self.left + length as u64 >= done.load(Ordering::Acquire)
                || done.load(Ordering::Acquire) > index.load(Ordering::Relaxed)
            {
                // task sleep for a while
                tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
            }
            index.store(self.left, Ordering::Release);
        } else {
            // wait for remote to release the space, until done > index + length as u64 (that means space is enough)
            while index.load(Ordering::Relaxed) + length as u64 >= done.load(Ordering::Acquire)
                && done.load(Ordering::Acquire) > index.load(Ordering::Relaxed)
            {
                // task sleep for a while
                tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
            }
        }
        let addr = index.fetch_add(length as u64, Ordering::Relaxed);
        RemoteBuf { addr, length, rkey }
    }

    pub fn update(&self, length: u32) {
        if self.done.load(Ordering::Relaxed) + length as u64 > self.right {
            self.done
                .store(self.left + length as u64, Ordering::Release);
        } else {
            self.done.fetch_add(length as u64, Ordering::Relaxed);
        }
    }
}

// use BufPool instead
pub struct SendBuffer {
    mr: Arc<MR>,
    send_buf: ManuallyDrop<Vec<u8>>,
    buf_pool: Arc<BufPool>,
}

impl SendBuffer {
    pub async fn new(pd: &PD) -> Self {
        let mut send_buf = ManuallyDrop::new(vec![0u8; DEFAULT_SEND_BUFFER_SIZE]);
        let mr = Arc::new(MR::new(pd, &mut send_buf));
        let local_buf = LocalBuf::from(mr.clone());
        let buf_pool = Arc::new(BufPool::new(local_buf).await);
        Self {
            send_buf,
            buf_pool,
            mr,
        }
    }

    pub async fn alloc(&self, length: u32) -> LocalBuf {
        self.buf_pool.alloc(length).await
    }

    pub async fn release(&self, buf: LocalBuf) {
        self.buf_pool.release(buf).await;
    }
}

impl Drop for SendBuffer {
    fn drop(&mut self) {
        unsafe {
            // Thread 1 "client" received signal SIGBUS, Bus error.
            // 0x0000000000000051 in ?? ()
            // ibv_dereg_mr(self.mr.inner());
            ManuallyDrop::drop(&mut self.send_buf);
        }
    }
}

pub struct BufPool {
    allocated: AtomicU64,
    local_buf: LocalBuf,
    // 16 bytes
    buf_16: (kanal::AsyncSender<LocalBuf>, kanal::AsyncReceiver<LocalBuf>),
    // 64 bytes
    buf_64: (kanal::AsyncSender<LocalBuf>, kanal::AsyncReceiver<LocalBuf>),
    // 256 bytes
    buf_256: (kanal::AsyncSender<LocalBuf>, kanal::AsyncReceiver<LocalBuf>),
    // 1024 bytes
    buf_1024: (kanal::AsyncSender<LocalBuf>, kanal::AsyncReceiver<LocalBuf>),
    // 4096 bytes
    buf_4096: (kanal::AsyncSender<LocalBuf>, kanal::AsyncReceiver<LocalBuf>),
    // 16384 bytes
    buf_16384: (kanal::AsyncSender<LocalBuf>, kanal::AsyncReceiver<LocalBuf>),
}

impl BufPool {
    pub async fn new(local_buf: LocalBuf) -> Self {
        let allocated = AtomicU64::new(local_buf.addr);
        let (buf_16_sender, buf_16_receiver) = kanal::unbounded_async();
        for i in 0..DEFAULT_PER_SIZE_BUFFER_COUNT {
            let buf = LocalBuf {
                addr: allocated.fetch_add(i as u64 * 16, Ordering::Relaxed),
                length: 16,
                lkey: local_buf.lkey,
            };
            buf_16_sender.send(buf).await.unwrap();
        }
        let (buf_64_sender, buf_64_receiver) = kanal::unbounded_async();
        for i in 0..DEFAULT_PER_SIZE_BUFFER_COUNT {
            let buf = LocalBuf {
                addr: allocated.fetch_add(i as u64 * 64, Ordering::Relaxed),
                length: 64,
                lkey: local_buf.lkey,
            };
            buf_64_sender.send(buf).await.unwrap();
        }
        let (buf_256_sender, buf_256_receiver) = kanal::unbounded_async();
        for i in 0..DEFAULT_PER_SIZE_BUFFER_COUNT {
            let buf = LocalBuf {
                addr: allocated.fetch_add(i as u64 * 256, Ordering::Relaxed),
                length: 256,
                lkey: local_buf.lkey,
            };
            buf_256_sender.send(buf).await.unwrap();
        }
        let (buf_1024_sender, buf_1024_receiver) = kanal::unbounded_async();
        for i in 0..DEFAULT_PER_SIZE_BUFFER_COUNT {
            let buf = LocalBuf {
                addr: allocated.fetch_add(i as u64 * 1024, Ordering::Relaxed),
                length: 1024,
                lkey: local_buf.lkey,
            };
            buf_1024_sender.send(buf).await.unwrap();
        }
        let (buf_4096_sender, buf_4096_receiver) = kanal::unbounded_async();
        for i in 0..DEFAULT_PER_SIZE_BUFFER_COUNT {
            let buf = LocalBuf {
                addr: allocated.fetch_add(i as u64 * 4096, Ordering::Relaxed),
                length: 4096,
                lkey: local_buf.lkey,
            };
            buf_4096_sender.send(buf).await.unwrap();
        }
        let (buf_16384_sender, buf_16384_receiver) = kanal::unbounded_async();
        for i in 0..DEFAULT_PER_SIZE_BUFFER_COUNT {
            let buf = LocalBuf {
                addr: allocated.fetch_add(i as u64 * 16384, Ordering::Relaxed),
                length: 16384,
                lkey: local_buf.lkey,
            };
            buf_16384_sender.send(buf).await.unwrap();
        }
        Self {
            local_buf,
            buf_16: (buf_16_sender, buf_16_receiver),
            buf_64: (buf_64_sender, buf_64_receiver),
            buf_256: (buf_256_sender, buf_256_receiver),
            buf_1024: (buf_1024_sender, buf_1024_receiver),
            buf_4096: (buf_4096_sender, buf_4096_receiver),
            buf_16384: (buf_16384_sender, buf_16384_receiver),
            allocated,
        }
    }

    pub async fn alloc(&self, length: u32) -> LocalBuf {
        // todo: if length > 16384, we will alloc a new buffer

        // alloc a buffer from the pool
        // length <= 16, alloc from buf_16
        // length <= 64, alloc from buf_64
        // length <= 256, alloc from buf_256
        // length <= 1024, alloc from buf_1024
        // length <= 4096, alloc from buf_4096
        // length <= 16384, alloc from buf_16384
        // if buf of spec length not enough, try to alloc bigger buffer
        // if all buffer not enough, wait for the channel
        match length {
            0..=16 => self.buf_16.1.recv().await.unwrap(),
            17..=64 => self.buf_64.1.recv().await.unwrap(),
            65..=256 => self.buf_256.1.recv().await.unwrap(),
            257..=1024 => self.buf_1024.1.recv().await.unwrap(),
            1025..=4096 => self.buf_4096.1.recv().await.unwrap(),
            4097..=16384 => self.buf_16384.1.recv().await.unwrap(),
            _ => unreachable!(),
        }
    }

    pub async fn release(&self, buf: LocalBuf) {
        match buf.length {
            0..=16 => self.buf_16.0.send(buf).await.unwrap(),
            17..=64 => self.buf_64.0.send(buf).await.unwrap(),
            65..=256 => self.buf_256.0.send(buf).await.unwrap(),
            257..=1024 => self.buf_1024.0.send(buf).await.unwrap(),
            1025..=4096 => self.buf_4096.0.send(buf).await.unwrap(),
            4097..=16384 => self.buf_16384.0.send(buf).await.unwrap(),
            _ => unreachable!(),
        }
    }
}

pub struct RecvBuffer {
    mr: Arc<MR>,
    // from polling
    pub rx: *mut Receiver<(u32, u32)>,
    recv_buffer: ManuallyDrop<Vec<u8>>,
    // it the length of gathered buf to release
    released: *mut u32,
    // it the position of the buf have been notify to release
    done: *mut u64,
    index: *mut u64,
    left: u64,
    right: u64,
}

unsafe impl Send for RecvBuffer {}
unsafe impl Sync for RecvBuffer {}

impl RecvBuffer {
    pub fn new(mr: Arc<MR>, recv_buffer: ManuallyDrop<Vec<u8>>, rx: Receiver<(u32, u32)>) -> Self {
        Self {
            mr: mr.clone(),
            rx: Box::into_raw(Box::new(rx)),
            recv_buffer,
            released: Box::into_raw(Box::new(0)),
            done: Box::into_raw(Box::new(mr.addr)),
            index: Box::into_raw(Box::new(mr.addr)),
            left: mr.addr,
            right: mr.addr + mr.length as u64,
        }
    }

    // after recv data form the &[u8], need to call release_buf to release the buf
    pub fn read(&self, length: u32) -> io::Result<&[u8]> {
        // get slice form recv_buffer
        let index = unsafe { &mut *self.index };
        let mut start = *index;
        let mut end = start + length as u64;
        // right check
        if end > self.right {
            start = self.left;
            end = self.left + length as u64;
        }
        *index = end;
        let buf = &self.recv_buffer[(start - self.mr.addr) as usize..(end - self.mr.addr) as usize];
        Ok(buf)
    }

    pub fn rx(&self) -> &mut Receiver<(u32, u32)> {
        unsafe { &mut *(self.rx) }
    }

    pub async fn recv(&self) -> (u32, u32) {
        self.rx().recv().await.unwrap()
    }

    pub fn notify_release(&self, length: u32) -> Option<u32> {
        let released = unsafe { &mut *self.released };
        let done = unsafe { &mut *self.done };
        // if over self.right, reset done to self.left.
        if *done + *released as u64 + length as u64 > self.right {
            let ret = *released;
            // length may be over MIN_LENGTH_TO_NOTIFY_RELEASE
            *released = length;
            *done = self.left;
            return Some(ret);
        } else if *released + length >= MIN_LENGTH_TO_NOTIFY_RELEASE {
            let ret = *released + length;
            *done += ret as u64;
            *released = 0;
            return Some(ret);
        } else {
            *released += length;
            return None;
        }
    }
}

impl Drop for RecvBuffer {
    fn drop(&mut self) {
        unsafe {
            let _ = Box::from_raw(self.rx);
            let _ = Box::from_raw(self.released);
            let _ = Box::from_raw(self.done);
            let _ = Box::from_raw(self.index);
            // Thread 1 "client" received signal SIGBUS, Bus error.
            // 0x0000000000000051 in ?? ()
            // ibv_dereg_mr(self.mr.inner());
            ManuallyDrop::drop(&mut self.recv_buffer);
        }
    }
}
