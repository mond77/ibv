//! cargo run --exampmle client

use ibv::connection::client::Client;
fn main() {
    let cli = Client::new();
    let qp = cli.connect("10.211.55.3:7471").unwrap();
    println!("qp: {:?}", qp.endpoint());
}