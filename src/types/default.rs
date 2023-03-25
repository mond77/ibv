pub static DEFAULT_GID_INDEX: u8 = 1;

// cq size
pub static DEFAULT_CQ_SIZE: i32 = 10000;

// seems like this is the max number of outstanding requests in RDMA-RoCE
pub static DEFAULT_RQE_COUNT: u32 = 1023;

// buffer size
pub static DEFAULT_SEND_BUFFER_SIZE: usize = 1024 * 1024;
pub static DEFAULT_RECV_BUFFER_SIZE: usize = 1024 * 1024;

pub static MIN_LENGTH_TO_NOTIFY_RELEASE: u32 = 1024;
