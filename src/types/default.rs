pub static DEFAULT_GID_INDEX: u8 = 1;

// cq size
pub static MAX_CQE: i32 = 32767;
pub static MAX_QP_WR: u32 = 16384;

// seems like this is the max number of outstanding requests in RDMA-RoCE
pub static DEFAULT_RQE_COUNT: u32 = MAX_QP_WR;

pub static DEFAULT_SEND_BUFFER_SIZE: usize = 16 * 1024 * 1024;
pub static DEFAULT_RECV_BUFFER_SIZE: usize = 16 * 1024 * 1024;

pub static MIN_LENGTH_TO_NOTIFY_RELEASE: u32 = 8 * 1024;
