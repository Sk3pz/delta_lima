use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use r2d2_postgres::{PostgresConnectionManager, r2d2};
use r2d2_postgres::postgres::NoTls;
use dl_network_common::Connection;

/// this function handles messages incoming from the client and stores them in the database
pub fn read_incoming(connection: &mut Connection, db: r2d2::Pool<PostgresConnectionManager<NoTls>>, tarc: Arc<AtomicBool>) {
    // todo(skepz)
}