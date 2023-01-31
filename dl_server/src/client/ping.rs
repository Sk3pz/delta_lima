use dl_network_common::{Connection, ExpectedPacket, Packet};
use crate::{ACCEPTED_CLIENT_VERSION, error, warn};

/// Expect, read, and reply to a Ping from the client at the start of a connection
/// returns true if disconnecting
pub fn expect_ping(connection: &mut Connection) -> bool {
    // read ping
    let mut response = connection.read(ExpectedPacket::Ping);
    if let Err(e) = response {
        error!("Failed to read ping request: {}", e);
        return true;
    }
    // reply to the ping
    match response.unwrap() {
        Packet::Ping { version, disconnecting } => {
            let valid = version.as_str() == ACCEPTED_CLIENT_VERSION;

            if let Err(_) = connection.send(Packet::PingResponse { valid, accepted_version: ACCEPTED_CLIENT_VERSION.to_string() }) {
                warn!("Failed to send ping response to client. They may have disconnected");
                return true;
            }
            return disconnecting;
        }
        _ => unreachable!()
    }
}