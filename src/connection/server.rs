use crate::connection::conn::Conn;
use crate::types::qp::{QPCap, QP};
use std::net::TcpListener;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;

use crate::types::device::{default_device, Device};
pub struct Server {
    pub addr: String,
    incoming: Receiver<Conn>,
}

unsafe impl<'a> Send for Server {}
unsafe impl<'a> Sync for Server {}

impl Server {
    pub fn new(addr: String) -> Self {
        let (tx, rx) = channel();
        let address = addr.clone();
        std::thread::spawn(move || {
            run(address, tx);
        });
        Server { addr, incoming: rx }
    }

    pub fn accept(&self) -> Conn {
        self.incoming.recv().unwrap()
    }
}

pub fn run(addr: String, sender: Sender<Conn>) {
    let listener = TcpListener::bind(addr.clone()).unwrap();
    let device = Arc::new(Device::new(default_device()));
    loop {
        match listener.accept() {
            Ok((stream, addr)) => {
                println!("New connection: {}", addr);
                // Create a QP for the new connection
                let mut qp = QP::new(device.clone(), QPCap::new(1000, 1000, 5, 5));
                if let Err(err) = qp.init() {
                    println!("err: {}", err);
                }
                qp.set_stream(stream);
                qp.handshake();
                println!("handshake done");
                // exchange recv_buf with client
                let (recv_buf, remote_mr) = qp.exchange_recv_buf();
                let conn = Conn::new(Arc::new(qp), recv_buf, remote_mr);

                sender.send(conn).unwrap();
            }
            Err(e) => {
                println!("Error: {}", e);
                break;
            }
        }
    }
}
