use std::{io::Result, net::TcpStream, sync::Arc};

use crate::types::{
    device::{default_device, Device},
    qp::{QPCap, QP},
};

use super::conn::Conn;
pub struct Client {}

impl Client {
    pub fn new() -> Self {
        Client {}
    }

    pub fn connect(&self, addr: &str) -> Result<Conn> {
        // connect to server
        let stream = TcpStream::connect(addr)?;

        let device = Arc::new(Device::new(default_device()));
        // Create a new QP
        let mut qp = QP::new(device, QPCap::new(1000, 1000, 5, 5));
        if let Err(err) = qp.init() {
            println!("err: {}", err);
        }
        qp.set_stream(stream);
        qp.handshake();
        println!("handshake done");
        // exchange recv_buf with client
        let (recv_buf, remote_mr) = qp.exchange_recv_buf();
        let conn = Conn::new(Arc::new(qp), recv_buf, remote_mr);

        Ok(conn)
    }
}
