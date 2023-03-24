//! cargo run --example client

use std::sync::Arc;

use ibv::connection::conn::connect;

#[tokio::main]
async fn main() {
    let conn = Arc::new(connect("127.0.0.1:7777").await.unwrap());

    println!("client ready to use");
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    // tokio oneshot
    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    let conn1 = conn.clone();
    let total = 100000;
    tokio::spawn(async move {
        let mut count = 0;
        println!("start recving");
        loop {
            let _msg = match conn1.recv_msg().await {
                Ok(msg) => msg,
                Err(_) => break,
            };
            count += 1;
            // println!("count: {}, msg: {:?}", count, _msg);
            if count == total {
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
    for i in 0..total as u32 {
        let conn1 = conn.clone();
        handles.push(tokio::spawn(async move {
            let data = i.to_be_bytes();
            let data = &[std::io::IoSlice::new(&data)];
            match conn1.send_msg(data).await {
                Ok(_) => {
                    // println!("send msg done: {}", i);
                }
                Err(err) => println!("err: {}", err),
            }
        }));
    }

    rx.await.unwrap();
    let elapsed = start.elapsed();
    println!("elapsed: {:?}", elapsed);

    println!("done");
}
