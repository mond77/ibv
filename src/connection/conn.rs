use std::{net::TcpStream, io::{Write, Read}, mem::size_of};

use crate::types::qp::{QP, EndPoint};

pub fn handshake(mut stream: TcpStream, qp: &QP) {
    // Exchange QP information withw the remote side (e.g. using sockets)
    let enp = qp.endpoint();
    println!("server enp: {:?}", enp);
    stream.write_all(&enp.to_bytes());
    let mut buf = vec![0; size_of::<EndPoint>()];
    stream.read_exact(&mut buf);
    let remote_enp = EndPoint::from_bytes(&buf);
    if let Err(err) = qp.ready_to_receive(remote_enp) {
        println!("err: {}", err);
    }
    if let Err(err) = qp.ready_to_send() {
        println!("server err: {}", err);
    }
}