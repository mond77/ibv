use rdma_sys::*;
use std::ptr::NonNull;

pub struct Device {
    pub context: NonNull<ibv_context>,
    pub port_attr: ibv_port_attr,
    pub device_attr: ibv_device_attr,
}

impl Device {
    pub fn new(context: NonNull<ibv_context>) -> Self {
        let mut port_attr = unsafe { std::mem::zeroed() };
        unsafe { rdma_sys::___ibv_query_port(context.as_ptr(), 1, &mut port_attr) };
        let mut device_attr = unsafe { std::mem::zeroed() };
        unsafe { rdma_sys::ibv_query_device(context.as_ptr(), &mut device_attr) };
        Self {
            context,
            port_attr,
            device_attr,
        }
    }

    pub fn inner(&self) -> *mut ibv_context {
        self.context.as_ptr()
    }

    pub fn lid(&self) -> u16 {
        self.port_attr.lid
    }

    pub fn gid(&self, idx: i8) -> [u8; 16] {
        let mut gid = unsafe { std::mem::zeroed::<ibv_gid>() };
        unsafe {
            ibv_query_gid(self.inner(), 1, idx as i32, &mut gid);
        }
        unsafe { gid.raw }
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        unsafe {
            ibv_close_device(self.inner());
        }
    }
}

unsafe impl Send for Device {}
unsafe impl Sync for Device {}

pub fn default_device() -> NonNull<ibv_context> {
    let mut x = 1;
    let context = unsafe { ibv_open_device(*ibv_get_device_list(&mut x)) };
    NonNull::new(context).unwrap()
}
