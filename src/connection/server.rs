use crate::connection::conn::Conn;
use tokio::sync::mpsc::{channel, Receiver};

use super::conn::run;
pub struct Server {
    pub addr: String,
    incoming: Receiver<Conn>,
}

unsafe impl<'a> Send for Server {}
unsafe impl<'a> Sync for Server {}

impl Server {
    pub async fn new(addr: String) -> Self {
        let (tx, rx) = channel(10);
        let address = addr.clone();
        tokio::spawn(run(address, tx));
        Server { addr, incoming: rx }
    }

    pub async fn accept(&mut self) -> Conn {
        self.incoming.recv().await.unwrap()
    }
}
