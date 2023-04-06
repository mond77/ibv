//! cargo run --example devices
//!

use ibv::types::device::{default_device, Device};

fn main() {
    let device = Device::new(default_device());
    println!("max_qp_wr: {}", device.max_qp_wr());
    println!("max_mr_size: {}", device.max_mr_size());
}
