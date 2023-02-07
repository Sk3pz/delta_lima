use std::net::TcpStream;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use r2d2_postgres::postgres::NoTls;
use r2d2_postgres::{PostgresConnectionManager, r2d2};
use dl_network_common::{Connection, Packet};
use crate::client::login::login_handler;
use crate::{debug, info, warn};
use crate::client::msg_receiver::msg_receive_handler;
use crate::client::ping::expect_ping;
use crate::database::{delete_msg, get_next_msg};

mod ping;
mod login;
mod msg_receiver;

/// Spawns a second thread
pub fn chandler(stream: TcpStream, db_pool: r2d2::Pool<PostgresConnectionManager<NoTls>>, tarc: Arc<AtomicBool>) {
    // ensure the stream is non-blocking
    if let Err(_) = stream.set_nonblocking(false) {
        warn!("Failed to set stream to blocking, failed to properly handle connection!");
        return;
    }

    // convert the stream into a Connection wrapper for sending capnp packets
    let mut connection = Connection::new(stream);

    // handle ping commands
    if expect_ping(&mut connection) {
        return;
    }

    // create a connection to the database
    let mut db = db_pool.get().unwrap();

    // handle before login to not waste time
    let Ok(mut cloned_connection) = connection.try_clone() else {
        warn!("Failed to create second connection reference for client handler!");
        return;
    };

    let Some(id) = login_handler(&mut connection, &mut db) else {
        return;
    };
    debug!("Client logged in with ID: {}", id);

    // todo(skepz): add function that updates the client on how many messages are unread and send them before the client can send messages so
    //  messages dont get out of order

    let local_tarc = Arc::new(AtomicBool::new(false));

    let ltarc_clone = Arc::clone(&local_tarc);

    let msg_receiver = thread::spawn(move || {
        msg_receive_handler(&mut cloned_connection, db_pool.clone(), id.clone(), ltarc_clone);
    });

    loop {
        // check if the server is shutting down
        if tarc.load(Ordering::SeqCst) {
            info!("Client received shutdown signal. Terminating connection");
            // store for the msg_receiver
            local_tarc.store(true, Ordering::SeqCst);
            if let Err(_) = connection.send(Packet::Disconnect) {
                warn!("Failed to send disconnect to client, maybe they already disconnected?");
            }
            break;
        }

        if local_tarc.load(Ordering::SeqCst) {
            if let Err(_) = connection.send(Packet::Disconnect) {
                warn!("Failed to send disconnect to client, maybe they already disconnected?");
            }
            break;
        }

        // get the next message addressed to this user
        let msg_query = get_next_msg(&mut db, id);
        if let Err(e) = msg_query {
            warn!("Failed to get next message: {}", e);
            continue;
        }

        // if there is a message, send it and remove it from the database
        if let Some(msg) = msg_query.unwrap() {
            // send the message
            if connection.send(Packet::Message { message: msg.message, sender: msg.sender, recipient: format!("SELF"), timestamp: msg.timestamp.to_string() }).is_err() {
                warn!("Failed to send message to client!");
                continue;
            }

            if let Err(e) = delete_msg(&mut db, msg.id) {
                warn!("Failed to delete message from database after sending: {}", e);
            }
        }
    }

    local_tarc.store(true, Ordering::SeqCst);
    if let Err(_) = msg_receiver.join() {
        warn!("Failed to join msg_receiver thread when shutting down client!");
    }

    info!("A client disconnected.");
}