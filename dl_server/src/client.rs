use std::net::TcpStream;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use r2d2_postgres::postgres::NoTls;
use r2d2_postgres::{PostgresConnectionManager, r2d2};
use dl_network_common::{Connection, ExpectedPacket, Packet};
use crate::client::login::login_handler;
use crate::{debug, error, info, warn};
use crate::client::ping::expect_ping;

mod ping;
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

    let Some(id) = login_handler(&mut connection, db.clone()) else {
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
                        let rec_query_result = dbclient.query(
                            format!("SELECT id FROM user_data WHERE username=$1").as_str(), &[&(recipient.as_str())]);
                        if rec_query_result.is_err() {
                            if connection.send(Packet::Error {
                                error: format!("Invalid recipient"),
                                should_disconnect: false
                            }).is_err() {
                                warn!("failed to send error message to client.");
                                break;
                            }
                        }
                        let rec_query = rec_query_result.unwrap();
                        let row = rec_query.get(0).unwrap();
                        let recipient_id: i32 = row.get(0);

                        if let Err(e) = dbclient.execute(
                            format!("INSERT INTO unsent_msgs(sender, recipient, message, timestamp) VALUES ({}, {}, '{}', null)",
                                    id, recipient_id, message).as_str(),
                            &[]) {
                            warn!("Failed to write message to database: {}", e);
                            if connection.send(Packet::Error {
                                error: format!("Database error"),
                                should_disconnect: false
                            }).is_err() {
                                warn!("failed to send error message to client.");
                                break;
                            }
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

        // check database for messages addressed to this user
        let msg_query_result = dbclient.query(
            format!("SELECT sender, message FROM unsent_msgs WHERE recipient={}", id).as_str(),
            &[]);
        if let Err(e) = msg_query_result {
            warn!("Database query error: {}", e);
            continue;
        }
        let msg_query = msg_query_result.unwrap();

        // todo(skepz): this could be optimized by only handling one unsent message per loop, allowing for the user to still send messages while the server is sending them?
        //  otherwise, depending on the amount of unread messages, it could back up.

        'msg_query: for msg in msg_query {
            let sender: i32 = msg.get(0);
            let message: String = msg.get(1);

            // todo(skepz): timestamps not yet implemented

            // get username of sender
            let sender_query_result = dbclient.query(
                format!("SELECT username FROM user_data WHERE id={}", sender).as_str(), &[]);
            if let Err(e) = sender_query_result {
                warn!("Failed to get sender of an unset message! {}", e);
                continue;
            }
            let sender_query = sender_query_result.unwrap();
            let sender_row = sender_query.get(0).unwrap();
            let sender_name: String = sender_row.get(0);

            // send the message
            if connection.send(Packet::Message { message: message.clone(), sender: sender_name, recipient: format!("SELF") }).is_err() {
                warn!("Failed to send message to client!");
                break 'msg_query;
            }

            // remove the message from the database as it has now been sent
            if let Err(e) = dbclient.execute(format!("DELETE FROM unsent_msgs WHERE recipient={} AND message='{}'", id, message).as_str(), &[]) {
                warn!("A message could not be removed from the database! {}\n  > query: {}", e, format!("DELETE FROM unsent_msgs WHERE recipient={} AND message='{}'", id, message));
                break;
            }

        }
    }

    info!("A client disconnected.");
}