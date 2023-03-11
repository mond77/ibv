use std::ptr::NonNull;

use rdma_sys::*;

use super::device::Device;
const DEFAULT_CQ_SIZE: i32 = 100;

pub struct CQ<'a> {
    inner: NonNull<ibv_cq>,
    pub device: &'a Device,
}

unsafe impl Send for CQ<'_> {}
unsafe impl Sync for CQ<'_> {}

impl<'a> CQ<'a> {
    pub fn new(device: &'a Device) -> Self {
        Self {
            inner: create_cq(device, DEFAULT_CQ_SIZE),
            device,
        }
    }

    pub fn inner(&self) -> *mut ibv_cq {
        self.inner.as_ptr()
    }

    pub fn device(&self) -> *mut ibv_context {
        self.device.inner()
    }
}

impl Drop for CQ<'_> {
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
