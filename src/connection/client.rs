use crate::types::{
    device::{default_device, Device},
    qp::{QPCap, QP},
};
use std::{io::Result, sync::Arc};
use tokio::net::TcpStream;

use super::conn::Conn;
pub struct Client {}

impl Client {
    pub fn new() -> Self {
        Client {}
    }

    pub async fn connect(&self, addr: &str) -> Result<Conn> {
        // connect to server
        let stream = TcpStream::connect(addr).await?;

        let device = Arc::new(Device::new(default_device()));
        // Create a new QP
        let mut qp = QP::new(device, QPCap::new(1000, 1000, 5, 5));
        if let Err(err) = qp.init() {
            println!("err: {}", err);
        }
        qp.set_stream(stream);
        qp.handshake().await;
        println!("handshake done");
        // exchange recv_buf with client
        let (recv_buf, remote_mr, tx) = qp.exchange_recv_buf().await;
        let conn = Conn::new(Arc::new(qp), recv_buf, remote_mr, tx).await;

        Ok(conn)
    }
}
