use std::{
    fmt::{self, Debug},
    ptr::NonNull,
    sync::Arc,
};

use rdma_sys::*;
use std::io::{Error, Result};

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

    pub fn poll_wc(&self, num_entries: u32) -> Result<Vec<WC>> {
        if num_entries == 0 {
            return Ok(Vec::new());
        }
        let mut wcs: Vec<WC> = Vec::with_capacity(num_entries as usize);
        unsafe { wcs.set_len(num_entries as usize) };
        let num_poll = unsafe { ibv_poll_cq(self.inner(), num_entries as i32, &mut wcs[0].0) };
        if num_poll < 0 {
            return Err(Error::last_os_error());
        }
        unsafe { wcs.set_len(num_poll as usize) };
        Ok(wcs)
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

pub struct WC(ibv_wc);

impl WC {
    pub fn new(ibv_wc: ibv_wc) -> Self {
        Self(ibv_wc)
    }

    pub fn wr_id(&self) -> u64 {
        self.0.wr_id
    }

    pub fn status(&self) -> u32 {
        self.0.status
    }

    pub fn opcode(&self) -> u32 {
        self.0.opcode
    }

    pub fn imm_data(&self) -> u32 {
        unsafe { self.0.imm_data_invalidated_rkey_union.imm_data }
    }

    pub fn wc_flags(&self) -> u32 {
        // IBV_WC_WITH_IMM - Indicator that imm_data is valid. Relevant for Receive Work Completions
        self.0.wc_flags
    }
}

impl Debug for WC {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("WC")
            .field(&self.wr_id())
            .field(&self.status())
            .field(&self.opcode())
            .field(&self.imm_data())
            .field(&self.wc_flags())
            .finish()
    }
}
