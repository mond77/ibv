use crate::types::qp::{QPCap, QP};
use std::net::TcpListener;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;

use crate::types::device::{default_device, Device};
pub struct Server {
    pub addr: String,
    incoming: Receiver<QP>,
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

    pub fn accept(&self) -> QP {
        self.incoming.recv().unwrap()
    }
}

pub fn run(addr: String, sender: Sender<QP>) {
    let listener = TcpListener::bind(addr.clone()).unwrap();
    let device = Arc::new(Device::new(default_device()));
    loop {
        match listener.accept() {
            Ok((stream, addr)) => {
                println!("New connection: {}", addr);
                // Create a QP for the new connection
                let mut qp = QP::new(device.clone(), QPCap::new(100, 100, 1, 1));
                if let Err(err) = qp.init() {
                    println!("err: {}", err);
                }
                qp.set_stream(stream);
                qp.handshake();
                println!("handshake done");

                sender.send(qp).unwrap();
            }
            Err(e) => {
                println!("Error: {}", e);
                break;
            }
        }
    }
}
