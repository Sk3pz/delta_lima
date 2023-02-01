use std::net::TcpStream;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use r2d2_postgres::postgres::NoTls;
use r2d2_postgres::{PostgresConnectionManager, r2d2};
use dl_network_common::Connection;
use crate::client::login::login_handler;
use crate::error;
use crate::client::ping::expect_ping;

mod ping;
mod msg_sender;
mod msg_receiver;
mod login;

/// Spawns a second thread
pub fn chandler(stream: TcpStream, db: r2d2::Pool<PostgresConnectionManager<NoTls>>, tarc: Arc<AtomicBool>) {
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

    // create a connection to the database
    let mut dbclient = db.get().unwrap();

    if login_handler(&mut connection, db) {
        return;
    }

    // todo(skepz): receiving and sending messages

    // Split into 2 threads and clone the connection
    // let connection2_result = connection.try_clone();
    // if let Err(e) = connection2_result {
    //     warn!("Failed to clone connection for a client.");
    //     return;
    // }
    // let connection2 = connection2_result.unwrap();
}