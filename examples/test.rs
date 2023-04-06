//! cargo run --example test
//!

use std::{
    alloc::{alloc, dealloc, Layout},
    ptr::slice_from_raw_parts_mut,
    sync::Arc,
};

use ibv::types::{
    device::{default_device, Device},
    mr::MR,
    pd::PD,
};

fn main() {
    let device = Arc::new(Device::new(default_device()));
    let pd = Arc::new(PD::new(device));
    let layout = Layout::new::<u64>();
    let data = unsafe {
        let ptr = alloc(layout);

        *(ptr as *mut u64) = 42;
        assert_eq!(*(ptr as *mut u64), 42);
        &mut *slice_from_raw_parts_mut(ptr, 8)
    };
    let mr = Arc::new(MR::new(pd, data));
    println!("{}", mr.inner() as u64);
    let errorno = mr.dereg();
    println!("errorno: {}", errorno);

    unsafe {
        dealloc(data.as_mut_ptr(), layout);
    }
}
