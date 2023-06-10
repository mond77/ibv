use std::sync::Arc;
use std::time::{Duration, Instant};

use ibv::connection::conn::connect;
use ibv::connection::server::Server;
use tokio::sync::oneshot;
use tokio::task;

#[tokio::main]
async fn main() {
    let total = 100_000;
    let num_iterations = 10; // Number of iterations to repeat the benchmark
    let mut total_elapsed = Duration::default();
    let mut total_throughput = 0.0;
    let mut total_latency = Duration::default();
    let mut total_qps = 0.0;

    let (tx, rx) = oneshot::channel::<()>();

    let _server_task = task::spawn(async move {
        let mut server = Server::new("127.0.0.1:7777".to_owned()).await;
        println!("Server ready to use");
        let mut count: u32 = 0;
        loop {
            let conn = Arc::new(server.accept().await);
            tokio::spawn(async move {
                loop {
                    let conn = conn.clone(); // Clone conn before moving into the task

                    let msg = conn.recv_msg().await.unwrap();
                    // Handle data and response
                    count += 1;
                    let data = msg.to_vec();
                    conn.release(msg).await;

                    tokio::spawn(async move {
                        let response = count.to_be_bytes();
                        let response = &[std::io::IoSlice::new(&response)];

                        match conn.send_msg(response).await {
                            Ok(_) => (),
                            Err(err) => println!("Error: {}", err),
                        }
                    });
                }
            });

            if count == total * num_iterations {
                println!("Received all requests");
                break;
            }
        }
        tx.send(()).unwrap();
    });

    tokio::time::sleep(Duration::from_secs(1)).await;

    for i in 0..num_iterations {
        let conn = Arc::new(connect("127.0.0.1:7777").await.unwrap());

        println!("Client ready to use");

        let (tx2, rx2) = oneshot::channel::<()>();
        let conn1 = conn.clone(); // Clone conn before moving into the task
        let client_task = task::spawn(async move {
            let conn = conn1.clone();
            let mut count = 0;
            println!("Start receiving responses");
            loop {
                // Clone conn before moving into the task

                let msg = conn.recv_msg().await.unwrap();
                // Handle data and response
                count += 1;
                let data = msg.to_vec();
                conn.release(msg).await;
                // println!("Count: {}, Msg: {:?}", count, data);
                if count == total {
                    println!("Received all responses");
                    break;
                }
            }

            tx2.send(()).unwrap();
        });

        let mut handles = vec![];
        println!("Start sending requests");
        let start = Instant::now();

        let send_task = task::spawn(async move {
            for i in 0..total {
                let conn = conn.clone(); // Clone conn before moving into the task
                let send_task = task::spawn(async move {
                    let data = i.to_be_bytes();
                    let data = &[std::io::IoSlice::new(&data)];

                    match conn.send_msg(data).await {
                        Ok(_) => {
                            // println!("Send msg done: {}", i);
                        }
                        Err(err) => println!("Error: {}", err),
                    }
                });

                handles.push(send_task);
            }
        });

        tokio::try_join!(client_task, send_task).unwrap();

        let elapsed = start.elapsed();
        let elapsed_secs = elapsed.as_secs_f64();
        let throughput = total as f64 / elapsed_secs;
        let latency = elapsed / total as u32;
        let qps = total as f64 / elapsed_secs;

        total_elapsed += elapsed;
        total_throughput += throughput;
        total_latency += latency;
        total_qps += qps;

        rx2.await.unwrap();
        println!("Client {} done", i + 1)
    }

    // rx.await.unwrap();

    let avg_elapsed = total_elapsed / num_iterations as u32;
    let avg_throughput = total_throughput / num_iterations as f64;
    let avg_latency = total_latency / num_iterations as u32;
    let avg_qps = total_qps / num_iterations as f64;

    println!("Average Elapsed time: {:.2?}", avg_elapsed);
    println!("Average Throughput: {:.2} requests/s", avg_throughput);
    println!("Average Latency: {:?}", avg_latency);
    println!("Average QPS: {:.2}", avg_qps);

    println!("Benchmark done");
}
