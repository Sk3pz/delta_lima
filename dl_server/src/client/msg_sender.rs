use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use r2d2_postgres::{PostgresConnectionManager, r2d2};
use r2d2_postgres::postgres::NoTls;
use dl_network_common::Connection;

/// this function reads the database and if the client has any incoming messages, sends them.
pub fn send_messages(connection: &mut Connection, db: r2d2::Pool<PostgresConnectionManager<NoTls>>, tarc: Arc<AtomicBool>) {
    // todo(skepz)
}