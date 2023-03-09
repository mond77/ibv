use rdma_sys::*;
use std::ptr::NonNull;
use super::device::Device;

pub struct PD<'a> {
    inner: NonNull<ibv_pd>,
    device: &'a Device,
}

unsafe impl Send for PD<'_> {}
unsafe impl Sync for PD<'_> {}

impl<'a> PD<'a> {
    pub fn new(device: &'a Device) -> Self {
        Self {
            inner: alloc_pd(&device),
            device,
        }
    }

    pub fn inner(&self) -> *mut ibv_pd {
        self.inner.as_ptr()
    }

    pub fn device(&self) -> &Device {
        self.device
    }
}

pub fn alloc_pd(device: &Device) -> NonNull<ibv_pd> {
    let pd = unsafe { ibv_alloc_pd(device.inner()) };
    NonNull::new(pd).unwrap()
}
