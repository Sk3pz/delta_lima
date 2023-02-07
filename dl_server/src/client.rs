use std::net::TcpStream;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use r2d2_postgres::postgres::NoTls;
use r2d2_postgres::{PostgresConnectionManager, r2d2};
use dl_network_common::{Connection, ExpectedPacket, Packet};
use crate::client::login::login_handler;
use crate::{debug, error, info, warn};
use crate::client::ping::expect_ping;
use crate::database::{delete_msg, get_id_from_username, get_next_msg, insert_msg};

mod ping;
mod login;

/// Spawns a second thread
pub fn chandler(stream: TcpStream, db_pool: r2d2::Pool<PostgresConnectionManager<NoTls>>, tarc: Arc<AtomicBool>) {
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
    let mut db = db_pool.get().unwrap();

    let Some(id) = login_handler(&mut connection, &mut db) else {
        return;
    };
    debug!("Client logged in with ID: {}", id);

    // todo(skepz): add function that updates the client on how many messages are unread and send them before the client can send messages so
    //  messages dont get out of order

    'main: loop {
        // check if the server is shutting down
        if tarc.load(Ordering::SeqCst) {
            info!("Client received shutdown signal. Terminating connection");
            if let Err(_) = connection.send(Packet::Disconnect) {
                warn!("Failed to send disconnect to client, maybe they already disconnected?");
            }
            break;
        }

        // check for incoming messages from client
        if let Ok(pkt_opt) = connection.check_expected(ExpectedPacket::Message) {
            if let Some(packet) = pkt_opt {
                // handle incoming messages from client
                match packet {
                    Packet::Message { message, recipient, .. } => {
                        // get the recipient's ID from username
                        let rec_query = get_id_from_username(&mut db, recipient);
                        if let Err(e) = rec_query {
                            if connection.send(Packet::Error {
                                error: format!("Invalid recipient"),
                                should_disconnect: false
                            }).is_err() {
                                warn!("failed to send error message to client: {}", e);
                                break;
                            }
                            warn!("Failed to get recipient ID from username: {}", e);
                            continue;
                        }
                        let recipient_id = rec_query.unwrap();

                        if let Err(e) = insert_msg(&mut db, id, recipient_id, message) {
                            warn!("Failed to write message to database: {}", e);
                            if connection.send(Packet::Error {
                                error: format!("Database error"),
                                should_disconnect: false
                            }).is_err() {
                                warn!("failed to send error message to client.");
                                break;
                            }
                            continue;
                        }
                    }
                    Packet::Disconnect => {
                        break;
                    }
                    Packet::Error { error, should_disconnect } => {
                        warn!("Client with id {} sent an error: {}.{}", id, error, if should_disconnect { " Disconnecting." } else { "" });
                        if should_disconnect {
                            break;
                        }
                    }
                    _ => unreachable!()
                }
            }
        } else {
            warn!("Failed to read from client; disconnecting.");
            if connection.send(Packet::Error { error: format!("Invalid data received."),
                should_disconnect: true }).is_err() {
                warn!("Failed to send disconnect message to client. Most likely they already disconnected.");
            }
            break 'main;
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
            if connection.send(Packet::Message { message: msg.message, sender: msg.sender, recipient: format!("SELF") }).is_err() {
                warn!("Failed to send message to client!");
                continue;
            }

            if let Err(e) = delete_msg(&mut db, msg.id) {
                warn!("Failed to delete message from database after sending: {}", e);
            }
        }
    }

    info!("A client disconnected.");
}