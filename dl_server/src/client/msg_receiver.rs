use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use chrono::{DateTime, Utc};
use postgres::NoTls;
use r2d2_postgres::{PostgresConnectionManager, r2d2};
use uuid::Uuid;
use dl_network_common::{Connection, ExpectedPacket, Packet};
use crate::database::{get_id_from_username, insert_msg};
use crate::warn;

pub fn msg_receive_handler(connection: &mut Connection, db_pool: r2d2::Pool<PostgresConnectionManager<NoTls>>, id: Uuid, tarc: Arc<AtomicBool>) {

    let Ok(mut db) = db_pool.get() else {
        warn!("Failed to get database instance for msg_receive_handler!");
        tarc.store(true, Ordering::SeqCst);
        return;
    };

    loop {
        if tarc.load(Ordering::SeqCst) {
            if let Err(_) = connection.send(Packet::Disconnect) {
                warn!("Failed to send disconnect to client msg_receive_handler, maybe they already disconnected?");
            }
            return;
        }

        // check for incoming messages from client
        if let Ok(pkt_opt) = connection.check_expected(ExpectedPacket::Message) {
            if let Some(packet) = pkt_opt {
                // handle incoming messages from client
                match packet {
                    Packet::Message { message, recipient, timestamp, .. } => {
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

                        if let Err(e) = insert_msg(&mut db, id, recipient_id, message, DateTime::from(Utc::now())) {
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
            break;
        }
    }

    tarc.store(true, Ordering::SeqCst);
}