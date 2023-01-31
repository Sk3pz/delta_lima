// Delta Lima Client main file

use std::net::TcpStream;
use dl_network_common::{Connection, ExpectedPacket, Packet};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    // attempt connection
    let ip = format!("127.0.0.1:2277");
    println!("Attempting to connect...");
    let stream_result = TcpStream::connect(ip.clone());
    if let Err(e) = stream_result {
        println!("Failed to connect to server: {}", e);
        return;
    }

    // now have a connection to the server
    let stream = stream_result.unwrap();
    // turn the stream into a Connection structure
    let mut connection = Connection::new(stream);

    // send a ping with version data to make the server happy
    if let Err(_) = connection.send(Packet::Ping { version: VERSION.to_string(), disconnecting: false }) {
        println!("ERROR: Failed to send version data to server! Disconnected.");
        return;
    }

    // expect a PingResponse from the server
    let response = connection.read(ExpectedPacket::PingResponse);
    if let Err(e) = response {
        println!("ERROR: Failed to read version data from server: {}", e);
        return;
    }
    match response.unwrap() {
        Packet::PingResponse { valid, accepted_version } => {
            if !valid {
                println!("Invalid version! The server only accepts version {}. Disconnected.", accepted_version);
                return;
            }
            println!("Valid version detected.")
        }
        _ => unreachable!()
    }

    println!("Nothing else here right now. Disconnecting.");

}
