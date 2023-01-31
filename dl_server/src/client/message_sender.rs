use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use dl_network_common::Connection;

/// this function reads the database and if the client has any incoming messages, sends them.
pub fn send_messages(connection: &mut Connection, tarc: Arc<AtomicBool>) {
    // todo(skepz)
}