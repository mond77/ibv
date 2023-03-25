//! cargo run --example server
//!

use std::{io::IoSlice, sync::Arc};

use ibv::connection::{conn::Conn, server::Server};
extern crate tokio;

#[tokio::main]
async fn main() {
    let mut server = Server::new("127.0.0.1:7777".to_owned()).await;
    let conn = Arc::new(server.accept().await);

    println!("server ready to use");

    println!("start recving");
    handle(conn).await;

    println!("done");
}

// parse recv_msg and response
pub async fn handle(conn: Arc<Conn>) {
    let mut count: u32 = 0;
    loop {
        let msg = conn.recv_msg().await.unwrap();
        // handle data and response
        count += 1;
        let data = msg.to_vec();
        conn.release(msg).await;
        let conn = conn.clone();
        tokio::spawn(async move {
            let response = count.to_be_bytes();
            let response = &[IoSlice::new(&response)];
            match conn.send_msg(response).await {
                Ok(_) => (),
                Err(err) => println!("err: {}", err),
            }
        });
        println!("count: {}, msg: {:?}", count, data);
    }
}
