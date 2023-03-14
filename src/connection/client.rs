use std::{io::Result, net::TcpStream, sync::Arc};

use crate::types::{
    device::{default_device, Device},
    qp::{QPCap, QP},
};
pub struct Client {}

impl Client {
    pub fn new() -> Self {
        Client {}
    }

    pub fn connect(&self, addr: &str) -> Result<QP> {
        // connect to server
        let stream = TcpStream::connect(addr)?;

        let device = Arc::new(Device::new(default_device()));
        // Create a new QP
        let mut qp = QP::new(device, QPCap::new(10, 10, 1, 1));
        if let Err(err) = qp.init() {
            println!("err: {}", err);
        }
        qp.set_stream(stream);
        qp.handshake();
        println!("handshake done");

        Ok(qp)
    }
}
