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
    // create a Connection wrapper for sending capnp packets
    let mut connection = Connection::new(&stream);

    // ensure the stream is non-blocking to match the listener
    if let Err(_) = stream.set_nonblocking(false) {
        error!("Failed to set stream to blocking, failed to properly handle connection!");
        if let Err(_) =
            connection.send(Packet::Error { error: format!(""), should_disconnect: true }) {
            warn!("Failed to send error message to server. Most likely already disconnected.");
        }
        return;
    }

    // handle ping commands
    if expect_ping(&mut connection) {
        return;
    }
}