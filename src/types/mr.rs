#![allow(unused)]
extern crate bincode;
use std::clone;
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::{mem::ManuallyDrop, ptr::NonNull, sync::Arc};
<<<<<<< HEAD

use crate::connection::conn::MAX_SENDING;

=======
>>>>>>> sendbuf release_task using AtomicBool
use super::default::{DEFAULT_SEND_BUFFER_SIZE, MIN_LENGTH_TO_NOTIFY_RELEASE};
use super::pd::PD;
use clippy_utilities::Cast;
use kanal;
use rdma_sys::{ibv_access_flags, ibv_dereg_mr, ibv_mr, ibv_reg_mr, ibv_sge};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
<<<<<<< HEAD
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::io;
use tokio::sync::mpsc::{channel, Receiver};
use tokio::sync::{oneshot, Mutex};
=======
use std::sync::atomic::{AtomicU64, Ordering, AtomicBool};
use tokio::io;
use tokio::sync::mpsc::Receiver;
use tokio::sync::Mutex;
>>>>>>> sendbuf release_task using AtomicBool
use tokio::task::JoinHandle;

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
    done: Arc<AtomicU64>,
    index: Mutex<u64>,
    left: u64,
    right: u64,
    to_release: MyDequeue,
    _release_task: JoinHandle<()>,
}

impl SendBuffer {
    pub async fn new(pd: &PD) -> Self {
        let mut send_buf = ManuallyDrop::new(vec![0u8; DEFAULT_SEND_BUFFER_SIZE]);
        let mr = Arc::new(MR::new(pd, &mut send_buf));
        let local_buf = LocalBuf::from(mr.clone());
        let done = Arc::new(AtomicU64::new(local_buf.addr));
        let index = Mutex::new(local_buf.addr);
        let left = local_buf.addr;
        let right = local_buf.addr + local_buf.length as u64;
        let to_release = MyDequeue(Box::into_raw(Box::new(VecDeque::new())));
        let done_clone = done.clone();
        let to_release_clone = to_release.clone();
        let release_task = tokio::spawn(async move {
            loop {
                // Receive the signal in order, only after the first rx receives the signal, the next one can receive it, and release the done in order
                while let Some((using, length)) = to_release_clone.pop_front() {
                    while using.load(Ordering::Relaxed) == true {
                        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                    }
                    // println!("release rx done");
                    if done_clone.load(Ordering::Relaxed) + length as u64 > right {
                        done_clone.store(left + length as u64, Ordering::Release);
                    } else {
                        done_clone.fetch_add(length as u64, Ordering::Relaxed);
                    }
                }
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }
        });
        Self {
            send_buf,
            mr,
            done,
            index,
            left,
            right,
            to_release,
            _release_task: release_task,
        }
    }

    pub async fn alloc(&self, length: u32) -> (LocalBuf, u64) {
        let lkey = self.mr.lkey;
        let mut index = self.index.lock().await;
        let done = self.done.clone();
        // notice: index could catch up done, that is a constraint.
        if *index + length as u64 > self.right {
            // wait for remote to release the space, until done > self.left + length as u64 (that means space is enough)
            while self.left + length as u64 >= done.load(Ordering::Acquire)
                || done.load(Ordering::Acquire) > *index
            {
                // task sleep for a while
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            }
            *index = self.left;
        } else {
            // wait for remote to release the space, until done > index + length as u64 (that means space is enough)
            while *index + length as u64 >= done.load(Ordering::Acquire)
                && done.load(Ordering::Acquire) > *index
            {
                // task sleep for a while
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            }
        }
        let addr = *index;
        *index += length as u64;
        (
            LocalBuf { addr, length, lkey },
            self.add_to_release(length).await,
        )
    }

    pub async fn add_to_release(&self, length: u32) -> u64 {
        let using = Arc::new(AtomicBool::new(true));
        let using_clone = using.clone();
        self.to_release.push_back(using, length);
        Arc::into_raw(using_clone) as u64
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

#[derive(Clone)]
pub struct MyDequeue(*mut VecDeque<(Arc<AtomicBool>, u32)>);

unsafe impl Send for MyDequeue {}
unsafe impl Sync for MyDequeue {}

impl MyDequeue {
    pub fn new() -> Self {
        Self(Box::into_raw(Box::new(VecDeque::new())))
    }

    pub fn length(&self) -> usize {
        let to_release = unsafe { &mut *self.0 };
        to_release.len()
    }

    pub fn push_back(&self, flag: Arc<AtomicBool>, length: u32) {
        let mut to_release = unsafe { &mut *self.0 };
        to_release.push_back((flag, length));
    }

    pub fn pop_front(&self) -> Option<(Arc<AtomicBool>, u32)> {
        let mut to_release = unsafe { &mut *self.0 };
        to_release.pop_front()
    }
}
