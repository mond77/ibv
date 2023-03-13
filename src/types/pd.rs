use super::device::Device;
use rdma_sys::*;
use std::{ptr::NonNull, sync::Arc};

pub struct PD {
    inner: NonNull<ibv_pd>,
    pub device: Arc<Device>,
}

unsafe impl Send for PD {}
unsafe impl Sync for PD {}

impl PD {
    pub fn new(device: Arc<Device>) -> Self {
        Self {
            inner: alloc_pd(&device),
            device,
        }
    }

    pub fn inner(&self) -> *mut ibv_pd {
        self.inner.as_ptr()
    }

    pub fn device(&self) -> &Device {
        self.device.as_ref()
    }
}

impl Drop for PD {
    fn drop(&mut self) {
        unsafe {
            ibv_dealloc_pd(self.inner());
        }
    }
}

pub fn alloc_pd(device: &Device) -> NonNull<ibv_pd> {
    let pd = unsafe { ibv_alloc_pd(device.inner()) };
    NonNull::new(pd).unwrap()
}
