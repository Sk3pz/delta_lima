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

    println!("Connected!");

    // send a ping with version data to make the server happy
    if let Err(_) = connection.send(Packet::Ping { version: VERSION.to_string(), disconnecting: false }) {
        println!("ERROR: Failed to send version data to server! Disconnected.");
        return;
    }

    // expect a PingResponse from the server
    let response = connection.expect(ExpectedPacket::PingResponse);
    if let Err(e) = response {
        println!("ERROR: Failed to read version data from server: {}", e);
        return;
    }
    match response.unwrap() {
        Packet::PingResponse { valid, accepted_version } => {
            if !valid {
                println!("Invalid version! The server only accepts version {}, and you are on {}. Disconnected.", accepted_version, VERSION);
                return;
            }
            println!("Valid version detected.");
        }
        _ => unreachable!()
    }

    // DUMMY LOGIN INFO FOR TESTING
    if connection.send(Packet::LoginRequest { username: format!("skepz"), password: format!("test"), signup: false }).is_err() {
        println!("Failed to send dummy login info.");
        return;
    }

    // expect a LoginResponse from the server
    let response = connection.expect(ExpectedPacket::LoginResponse);
    if let Err(e) = response {
        println!("ERROR: Failed to login response data from server: {}", e);
        return;
    }
    match response.unwrap() {
        Packet::LoginResponse { valid, error } => {
            if !valid {
                let err = if let Some(e) = error {
                    e
                } else {
                    format!("none")
                };
                println!("Invalid login: {}", err);
                return;
            }
            println!("Logged in!");
        }
        _ => unreachable!()
    }

    // send a test message
    println!("Sending test message.");

    let msg_timestamp =

    connection.send(Packet::Message { message: format!("Test Message"), sender: format!(""), recipient: format!("test"), timestamp: format!("") }).expect("Failed to send test message");

    loop {
        let incoming = connection.check_expected(ExpectedPacket::Message).expect("Failed to get message");
        if let Some(packet) = incoming {
            match packet {
                Packet::Message { message, sender, timestamp, .. } => {
                    println!("MESSAGE from {} @ {} > {}", sender, timestamp, message);
                }
                Packet::Error { error, should_disconnect } => {
                    println!("Error from server: {}", error);
                    if should_disconnect {
                        break;
                    }
                }
                Packet::Disconnect => {
                    break;
                }
                _ => unreachable!()
            }
        }
    }

    println!("Disconnected");

}
