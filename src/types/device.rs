use rdma_sys::*;
use std::ptr::NonNull;

pub struct Device {
    pub context: NonNull<ibv_context>,
}

impl Device {
    pub fn new(context: NonNull<ibv_context>) -> Self {
        Self { context }
    }

    pub fn inner(&self) -> *mut ibv_context {
        self.context.as_ptr()
    }
}

unsafe impl Send for Device {}
unsafe impl Sync for Device {}

pub fn default_device() -> NonNull<ibv_context> {
    let mut x = 1;
    let context = unsafe { &mut *ibv_open_device(*ibv_get_device_list(&mut x)) };
    NonNull::new(context).unwrap()
}
