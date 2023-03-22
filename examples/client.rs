//! cargo run --example client

use std::sync::Arc;

use ibv::connection::client::Client;

#[tokio::main]
async fn main() {
    let cli = Client::new();
    let conn = Arc::new(cli.connect("127.0.0.1:7777").await.unwrap());

    println!("client ready to use");
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    let mut handles = vec![];
    println!("start sending");
    for i in 0..100 as u32 {
        let conn = conn.clone();
        handles.push(tokio::spawn(async move {
            // Get the number of i in each quantile
            let i1 = (i >> 24) as u8;
            let i2 = (i >> 16) as u8;
            let i3 = (i >> 8) as u8;
            let i4 = i as u8;
            let data = vec![i1, i2, i3, i4];
            conn.send_msg(&data).await;
        }));
    }
    for handle in handles {
        handle.await;
    }
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    println!("done");
}
