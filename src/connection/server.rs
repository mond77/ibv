use crate::connection::conn::Conn;
use crate::types::qp::{QPCap, QP};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::mpsc::{channel, Receiver, Sender};

use crate::types::device::{default_device, Device};
pub struct Server {
    pub addr: String,
    incoming: Receiver<Conn>,
}

unsafe impl<'a> Send for Server {}
unsafe impl<'a> Sync for Server {}

impl Server {
    pub async fn new(addr: String) -> Self {
        let (tx, rx) = channel(10);
        let address = addr.clone();
        tokio::spawn(run(address, tx));
        Server { addr, incoming: rx }
    }

    pub async fn accept(&mut self) -> Conn {
        self.incoming.recv().await.unwrap()
    }
}

pub async fn run(addr: String, sender: Sender<Conn>) {
    let listener = TcpListener::bind(addr.clone()).await.unwrap();
    let device = Arc::new(Device::new(default_device()));
    loop {
        match listener.accept().await {
            Ok((stream, addr)) => {
                println!("New connection: {}", addr);
                // Create a QP for the new connection
                let mut qp = QP::new(device.clone(), QPCap::new(1000, 1000, 5, 5));
                if let Err(err) = qp.init() {
                    println!("err: {}", err);
                }
                qp.set_stream(stream);
                qp.handshake().await;
                println!("handshake done");
                // exchange recv_buf with client
                let (recv_buf, remote_mr) = qp.exchange_recv_buf().await;
                let conn = Conn::new(Arc::new(qp), recv_buf, remote_mr).await;

                if let Err(e) = sender.send(conn).await {
                    println!("Error: {}", e);
                    break;
                }
            }
            Err(e) => {
                println!("Error: {}", e);
                break;
            }
        }
    }
}
