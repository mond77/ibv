pub static DEFAULT_GID_INDEX: u8 = 1;

// cq size
pub static DEFAULT_CQ_SIZE: i32 = 10000;

// seems like this is the max number of outstanding requests in RDMA-RoCE
pub static DEFAULT_RQE_COUNT: u32 = 1023;

// buffer size
// 16B, 64B, 256B, 1KB, 4KB, 16KB each size has 1024 buffers, total need space
pub static DEFAULT_PER_SIZE_BUFFER_COUNT: u32 = DEFAULT_RQE_COUNT + 100;
pub static DEFAULT_SEND_BUFFER_SIZE: usize = 32 * 1024 * 1024;
pub static DEFAULT_RECV_BUFFER_SIZE: usize = 4 * 1024 * 1024;

pub static MIN_LENGTH_TO_NOTIFY_RELEASE: u32 = 1024;
