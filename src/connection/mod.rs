pub mod client;
pub mod conn;
pub mod server;

// to debug: bigger size cann't work
pub const DEFAULT_BUFFER_SIZE: usize = 1024 * 64;
