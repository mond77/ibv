use std::{
    fmt::{self, Debug},
    ptr::NonNull,
    sync::Arc,
};

use rdma_sys::*;
use std::io::{Error, Result};

use super::device::Device;
const DEFAULT_CQ_SIZE: i32 = 10000;

pub struct CQ {
    inner: NonNull<ibv_cq>,
    pub device: Arc<Device>,
    channel: Option<NonNull<ibv_comp_channel>>,
}

unsafe impl Send for CQ {}
unsafe impl Sync for CQ {}

impl CQ {
    pub fn new(device: Arc<Device>) -> Self {
        Self {
            inner: create_cq(&device, DEFAULT_CQ_SIZE),
            device: device.clone(),
            channel: None,
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

    pub fn req_notify(&self, solicited_only: bool) -> Result<()> {
        let ret = unsafe { ibv_req_notify_cq(self.inner(), solicited_only as i32) };
        if ret < 0 {
            return Err(Error::last_os_error());
        }
        Ok(())
    }

    pub fn wait_for_wc(&mut self) {
        unsafe {
            ibv_get_cq_event(
                self.channel.unwrap().as_ptr(),
                &mut self.inner(),
                std::ptr::null_mut(),
            )
        };
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

    // 0: IBV_WC_SUCCESS - Work Request completed successfully.
    // 1: IBV_WC_LOC_LEN_ERR - Local length of the scatter/gather list is invalid.
    // 2: IBV_WC_LOC_QP_OP_ERR - Local QP operation error.
    // 3: IBV_WC_LOC_EEC_OP_ERR - Local EEC operation error.
    // 4: IBV_WC_LOC_PROT_ERR - Local protection error.
    // 5: IBV_WC_WR_FLUSH_ERR - Work Request Flushed Error.
    // 6: IBV_WC_MW_BIND_ERR - Memory Window Bind Error.
    // 7: IBV_WC_BAD_RESP_ERR - Bad Response Error.
    // 8: IBV_WC_LOC_ACCESS_ERR - Local access error.
    // 9: IBV_WC_REM_INV_REQ_ERR - Remote invalid request error.
    // 10: IBV_WC_REM_ACCESS_ERR - Remote access error.
    // 11: IBV_WC_REM_OP_ERR - Remote operation error.
    // 12: IBV_WC_RETRY_EXC_ERR - Retry counter exceeded.
    // 13: IBV_WC_RNR_RETRY_EXC_ERR - RNR Retry counter exceeded.
    // 14: IBV_WC_LOC_RDD_VIOL_ERR - Local RDD Violation Error.
    // 15: IBV_WC_REM_INV_RD_REQ_ERR - Remote invalid RD Request.
    // 16: IBV_WC_REM_ABORT_ERR - Remote Abort Error.
    // 17: IBV_WC_INV_EECN_ERR - Invalid EECN Error.
    // 18: IBV_WC_INV_EEC_STATE_ERR - Invalid EEC State Error.
    // 19: IBV_WC_FATAL_ERR - Fatal Error.
    // 20: IBV_WC_RESP_TIMEOUT_ERR - Response Timeout Error.
    // 21: IBV_WC_GENERAL_ERR - General Error.

    pub fn status(&self) -> WCStatus {
        WCStatus::from(self.0.status)
    }

    pub fn opcode(&self) -> Opcode {
        Opcode::from(self.0.opcode)
    }

    pub fn imm_data(&self) -> u32 {
        unsafe { self.0.imm_data_invalidated_rkey_union.imm_data }
    }

    pub fn wc_flags(&self) -> WCFlag {
        WCFlag::from(self.0.wc_flags)
    }

    pub fn byte_len(&self) -> u32 {
        self.0.byte_len
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

#[derive(Debug, PartialEq, Eq)]
pub enum WCStatus {
    // 0: IBV_WC_SUCCESS - Work Request completed successfully.
    Success,
    // 1: IBV_WC_LOC_LEN_ERR - Local length of the scatter/gather list is invalid.
    LocalLenErr,
    // 2: IBV_WC_LOC_QP_OP_ERR - Local QP operation error.
    LocalQpOpErr,
    // 3: IBV_WC_LOC_EEC_OP_ERR - Local EEC operation error.
    LocalEecOpErr,
    // 4: IBV_WC_LOC_PROT_ERR - Local protection error.
    LocalProtErr,
    // 5: IBV_WC_WR_FLUSH_ERR - Work Request Flushed Error.
    WrFlushErr,
    // 6: IBV_WC_MW_BIND_ERR - Memory Window Bind Error.
    MwBindErr,
    // 7: IBV_WC_BAD_RESP_ERR - Bad Response Error.
    BadRespErr,
    // 8: IBV_WC_LOC_ACCESS_ERR - Local access error.
    LocalAccessErr,
    // 9: IBV_WC_REM_INV_REQ_ERR - Remote invalid request error.
    RemInvReqErr,
    // 10: IBV_WC_REM_ACCESS_ERR - Remote access error.
    RemAccessErr,
    // 11: IBV_WC_REM_OP_ERR - Remote operation error.
    RemOpErr,
    // 12: IBV_WC_RETRY_EXC_ERR - Retry counter exceeded.
    RetryExcErr,
    // 13: IBV_WC_RNR_RETRY_EXC_ERR - RNR Retry counter exceeded.
    RnrRetryExcErr,
    // 14: IBV_WC_LOC_RDD_VIOL_ERR - Local RDD Violation Error.
    LocRddViolErr,
    // 15: IBV_WC_REM_INV_RD_REQ_ERR - Remote invalid RD Request.
    RemInvRdReqErr,
    // 16: IBV_WC_REM_ABORT_ERR - Remote Abort Error.
    RemAbortErr,
    // 17: IBV_WC_INV_EECN_ERR - Invalid EECN Error.
    InvEecnErr,
    // 18: IBV_WC_INV_EEC_STATE_ERR - Invalid EEC State Error.
    InvEecStateErr,
    // 19: IBV_WC_FATAL_ERR - Fatal Error.
    FatalErr,
    // 20: IBV_WC_RESP_TIMEOUT_ERR - Response Timeout Error.
    RespTimeoutErr,
    // 21: IBV_WC_GENERAL_ERR - General Error.
    GeneralErr,

    Unknown(u32),
}

impl From<u32> for WCStatus {
    fn from(value: u32) -> Self {
        match value {
            0 => Self::Success,
            1 => Self::LocalLenErr,
            2 => Self::LocalQpOpErr,
            3 => Self::LocalEecOpErr,
            4 => Self::LocalProtErr,
            5 => Self::WrFlushErr,
            6 => Self::MwBindErr,
            7 => Self::BadRespErr,
            8 => Self::LocalAccessErr,
            9 => Self::RemInvReqErr,
            10 => Self::RemAccessErr,
            11 => Self::RemOpErr,
            12 => Self::RetryExcErr,
            13 => Self::RnrRetryExcErr,
            14 => Self::LocRddViolErr,
            15 => Self::RemInvRdReqErr,
            16 => Self::RemAbortErr,
            17 => Self::InvEecnErr,
            18 => Self::InvEecStateErr,
            19 => Self::FatalErr,
            20 => Self::RespTimeoutErr,
            21 => Self::GeneralErr,
            _ => Self::Unknown(value),
        }
    }
}

#[derive(Debug)]
pub enum Opcode {
    Send,
    Recv,
    Read,
    Write,
    SendWithImm,
    WriteWithImm,
    Unknown(u32),
}

impl From<u32> for Opcode {
    fn from(value: u32) -> Self {
        match value {
            0 => Self::Send,
            128 => Self::Recv,
            2 => Self::Read,
            1 => Self::Write,
            129 => Self::WriteWithImm,
            _ => Self::Unknown(value),
        }
    }
}

#[derive(Debug)]
pub enum WCFlag {
    None,
    // IBV_WC_WITH_IMM - Indicator that imm_data is valid. Relevant for Receive Work Completions
    WithImm,
    Unknown(u32),
}

impl From<u32> for WCFlag {
    fn from(value: u32) -> Self {
        match value {
            0 => Self::None,
            3 => Self::WithImm,
            _ => Self::Unknown(value),
        }
    }
}
