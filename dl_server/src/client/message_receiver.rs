use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use dl_network_common::Connection;

/// this function handles messages incoming from the client and stores them in the database
pub fn read_incoming(connection: &mut Connection, tarc: Arc<AtomicBool>) {
    // todo(skepz)
}