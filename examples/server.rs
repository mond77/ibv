//! cargo run --example server
//!

use ibv::connection::server::Server;
extern crate tokio;

#[tokio::main]
async fn main() {
    let mut server = Server::new("127.0.0.1:7777".to_owned()).await;
    let conn = server.accept().await;

    println!("server ready to use");

    println!("start recving");
    if let Err(e) = conn.polling.join() {
        println!("recving error: {:?}", e);
    }

    println!("done");
}
