use std::{
    io::{Read, Write},
    net::TcpStream,
};

use crate::types::qp::{EndPoint, QP};

pub fn handshake(mut stream: TcpStream, qp: &QP) {
    // Exchange QP information withw the remote side (e.g. using sockets)
    let enp = qp.endpoint();
    println!("server enp: {:?}", enp);
    let bytes = enp.to_bytes();
    if let Err(_) = stream.write_all(&bytes) {
        println!("write stream error");
    }
    let mut buf = vec![0; bytes.len()];
    if let Err(_) = stream.read_exact(&mut buf) {
        println!("read stream error");
    }
    let remote_enp = EndPoint::from_bytes(&buf);
    if let Err(err) = qp.ready_to_receive(remote_enp) {
        println!("err: {}", err);
    }
    if let Err(err) = qp.ready_to_send() {
        println!("server err: {}", err);
    }
}
