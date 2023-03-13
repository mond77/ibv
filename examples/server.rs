//! cargo run --exampmle server
extern crate rdma_sys;

use ibv::connection::server::Server;


fn main() {

    let server = Server::new("10.211.55.3:7471".to_owned());
    let qp = server.accept();

    println!("qp: {:?}", qp.endpoint());
    
}
