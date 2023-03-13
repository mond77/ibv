use std::{ptr::NonNull, sync::Arc};

use rdma_sys::*;

use super::device::Device;
const DEFAULT_CQ_SIZE: i32 = 100;

pub struct CQ {
    inner: NonNull<ibv_cq>,
    pub device: Arc<Device>,
}

unsafe impl Send for CQ {}
unsafe impl Sync for CQ {}

impl CQ {
    pub fn new(device: Arc<Device>) -> Self {
        Self {
            inner: create_cq(&device, DEFAULT_CQ_SIZE),
            device: device.clone(),
        }
    }

    pub fn inner(&self) -> *mut ibv_cq {
        self.inner.as_ptr()
    }

    pub fn device(&self) -> *mut ibv_context {
        self.device.inner()
    }
}

impl Drop for CQ {
    fn drop(&mut self) {
        unsafe {
            ibv_destroy_cq(self.inner());
        }
    }
}

pub fn create_cq(device: &Device, size: i32) -> NonNull<ibv_cq> {
    let cq = unsafe {
        ibv_create_cq(
            device.inner(),
            size,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            0,
        )
    };
    NonNull::new(cq).unwrap()
}
