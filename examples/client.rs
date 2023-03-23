//! cargo run --example client

use std::sync::Arc;

use ibv::connection::client::Client;

#[tokio::main]
async fn main() {
    let cli = Client::new();
    let conn = Arc::new(cli.connect("127.0.0.1:7777").await.unwrap());

    println!("client ready to use");
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    // tokio oneshot
    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    let conn1 = conn.clone();
    tokio::spawn(async move {
        let mut count = 0;
        loop {
            let msg = conn1.recv_msg().await;
            count += 1;
            println!("count: {}, msg: {:?}", count, msg);
            if count == 1000 {
                println!("recv response done");
                tx.send(()).unwrap();
                break;
            }
        }
    });
    let mut handles = vec![];
    println!("start sending");
    // time elapsed
    let start = std::time::Instant::now();
    for i in 0..1000 as u32 {
        let conn1 = conn.clone();
        handles.push(tokio::spawn(async move {
            let data = i.to_be_bytes();
            conn1.send_msg(&data).await;
        }));
    }

    rx.await.unwrap();
    let elapsed = start.elapsed();
    println!("elapsed: {:?}", elapsed);

    tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;

    println!("done");
}
