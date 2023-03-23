//! cargo run --example server
//!

use std::sync::Arc;

use ibv::connection::server::Server;
extern crate tokio;

#[tokio::main]
async fn main() {
    let mut server = Server::new("127.0.0.1:7777".to_owned()).await;
    let conn = Arc::new(server.accept().await);

    println!("server ready to use");

    println!("start recving");
    let mut count: u32 = 0;
    loop {
        let msg = conn.recv_msg().await;
        if msg.is_empty() {
            break;
        }
        // handle data and response
        count += 1;
        let conn = conn.clone();
        tokio::spawn(async move {
            let response = count.to_be_bytes();
            conn.send_msg(&response).await;
        });
        println!("count: {}, msg: {:?}", count, msg);
    }

    println!("done");
}
