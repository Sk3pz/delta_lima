use std::net::TcpStream;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use dl_network_common::{Connection, Packet};
use crate::{error, warn};
use crate::client::ping::expect_ping;

mod ping;
mod message_sender;
mod message_receiver;
mod login;

/// Spawns a second thread
pub fn handle_connection(stream: TcpStream, tarc: Arc<AtomicBool>) {
    // ensure the stream is non-blocking to match the listener
    if let Err(_) = stream.set_nonblocking(false) {
        error!("Failed to set stream to blocking, failed to properly handle connection!");
        return;
    }

    // convert the stream into a Connection wrapper for sending capnp packets
    let mut connection = Connection::new(stream);

    // handle ping commands
    if expect_ping(&mut connection) {
        return;
    }

    // todo(skepz): login / signup

    // todo(skepz): receiving and sending messages

    // Split into 2 threads and clone the connection
    // let connection2_result = connection.try_clone();
    // if let Err(e) = connection2_result {
    //     warn!("Failed to clone connection for a client.");
    //     return;
    // }
    // let connection2 = connection2_result.unwrap();
}