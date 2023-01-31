use std::io;
use std::net::TcpStream;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use capnp::message::Builder;
use capnp::serialize;

pub(crate) mod packet_capnp;

pub fn systime() -> Duration {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Fatal error occurred: System time moved backwards! Are you a time traveler?")
}

pub fn to_epoch(time: SystemTime) -> Duration {
    time.duration_since(UNIX_EPOCH)
        .expect("Fatal error occurred: System time moved backwards! Are you a time traveler?")
}

pub struct LoginData {
    pub username: String,
    pub password: String,
    pub signup: bool
}

pub enum Packet {
    /// Client --> Server | Check if client's version is valid
    /// disconnecting determines if the client is just checking compatibility or attempting a full connection
    Ping { version: String, disconnecting: bool },
    /// Client <-- Server | Respond if the client's version is valid and the version the server is accepting
    PingResponse { valid: bool, accepted_version: String },
    /// Client --> Server | Send a login or signup attempt to the server
    LoginRequest { username: String, password: String, signup: bool, },
    /// Client <-- Server | Send if the login attempt was valid or not, and if not send an error
    LoginResponse { valid: bool, error: Option<String> },
    /// Client <-> Server | A message sent from a client intended for another user
    Message { messages: Vec<String>, sender: String, recipient: String },
    /// Client <-> Server | A way to announce a disconnection is required or imminent
    Disconnect,
    /// Client <-> Server | A way to announce an error has occurred, what the error is and if it requires a disconnection
    Error { should_disconnect: bool, error: String },
}


pub enum ExpectedPacket {
    // No reason to ever expect an Error or Disconnect packet, and they will only be received under a Message packet
    Ping, PingResponse, LoginRequest, LoginResponse, Message
}

pub struct Connection {
    stream: TcpStream,
}

impl Connection {
    pub fn new(stream: TcpStream) -> Self {
        Self {
            stream,
        }
    }

    pub fn try_clone(&mut self) -> io::Result<Connection> {
        Ok(Self {
            stream: self.stream.try_clone()?
        })
    }

    pub fn disconnect(&mut self) -> Result<(), String> {
        let err = self.send(Packet::Disconnect);
        if let Err(_) = err {
            return Err(format!("Failed to send disconnect packet: already disconnected!"));
        }
        Ok(())
    }

    /// Send a packet across the stream
    pub fn send(&mut self, packet: Packet) -> ::capnp::Result<()> {
        let mut message = Builder::new_default();
        match packet {
            Packet::Ping { version, disconnecting } => {
                let mut ep = message.init_root::<packet_capnp::ping::Builder>();
                ep.set_version(version.as_str());
                ep.set_disconnecting(disconnecting)
            }
            Packet::PingResponse { valid, accepted_version: version } => {
                let mut ep = message.init_root::<packet_capnp::ping_response::Builder>();
                ep.set_valid(valid);
                ep.set_version(version.as_str());
            }
            Packet::LoginRequest { username, password, signup } => {
                let mut ep = message.init_root::<packet_capnp::login_request::Builder>();
                ep.set_username(username.as_str());
                ep.set_password(password.as_str());
                ep.set_signup(signup);
            }
            Packet::LoginResponse { valid, error } => {
                let mut ep = message.init_root::<packet_capnp::login_response::Builder>();
                ep.set_valid(valid);
                if let Some(err) = error {
                    ep.set_error(err.as_str());
                }
            }
            Packet::Message { messages, sender, recipient } => {
                let ep = message.init_root::<packet_capnp::big_boi_chonk::Builder>();
                let mut minit = ep.init_message();
                let mut messages_builder = minit.reborrow().init_message(messages.len() as u32);
                for x in 0..messages.len() {
                    messages_builder.reborrow().set(x as u32, messages.get(x).unwrap().as_str())
                }
                minit.set_sender(sender.as_str());
                minit.set_recipient(recipient.as_str());
            }
            Packet::Disconnect => {
                let mut ep = message.init_root::<packet_capnp::big_boi_chonk::Builder>();
                ep.set_disconnect(true);
            }
            Packet::Error { should_disconnect, error } => {
                let ep = message.init_root::<packet_capnp::big_boi_chonk::Builder>();
                let mut err_init = ep.init_error();
                err_init.set_error(error.as_str());
                err_init.set_disconnect(should_disconnect);
            }
        }
        serialize::write_message(&mut self.stream, &message)
    }

    // sends an error if invalid data was received and handles if it was because of a disconnection
    fn send_invalid_data_error(&mut self) -> Result<(), String> {
        let send_result = self.send(Packet::Error { should_disconnect: true, error: format!("Invalid data received!") });
        if send_result.is_err() {
            return Err(format!("Client was disconnected while expecting data!"));
        }
        Ok(())
    }

    /// Expect a specific packet and read its data
    /// @param expected: the packet type to expect
    /// @return: Ok(..): the packet that was read, Err(..): An error message
    pub fn read(&mut self, expected: ExpectedPacket) -> Result<Packet, String> {
        let msg_reader_raw = serialize::read_message(&mut self.stream, ::capnp::message::ReaderOptions::new());
        if msg_reader_raw.is_err() {
            self.send_invalid_data_error()?;
            return Err(format!("Invalid or corrupt data was received!"));
        }
        let msg_reader = msg_reader_raw.unwrap();

        match expected {
            ExpectedPacket::Ping => {
                let ep_raw = msg_reader.get_root::<packet_capnp::ping::Reader>();
                if let Err(_) = ep_raw {
                    self.send_invalid_data_error()?;
                    return Err(format!("Invalid data received! Disconnecting."));
                }
                let ep = ep_raw.unwrap();

                Ok(Packet::Ping {
                    version: ep.get_version().unwrap().to_string(),
                    disconnecting: ep.get_disconnecting()
                })
            }
            ExpectedPacket::PingResponse => {
                let ep_raw = msg_reader.get_root::<packet_capnp::ping_response::Reader>();
                if let Err(_) = ep_raw {
                    self.send_invalid_data_error()?;
                    return Err(format!("Invalid data received! Disconnecting."));
                }
                let ep = ep_raw.unwrap();

                Ok(Packet::PingResponse {
                    valid: ep.get_valid(),
                    accepted_version: ep.get_version().unwrap().to_string()
                })
            }
            ExpectedPacket::LoginRequest => {
                let ep_raw = msg_reader.get_root::<packet_capnp::login_request::Reader>();
                if let Err(_) = ep_raw {
                    self.send_invalid_data_error()?;
                    return Err(format!("Invalid data received! Disconnecting."));
                }
                let ep = ep_raw.unwrap();

                Ok(Packet::LoginRequest {
                    username: ep.get_username().unwrap().to_string(),
                    password: ep.get_password().unwrap().to_string(),
                    signup: ep.get_signup(),
                })
            }
            ExpectedPacket::LoginResponse => {
                let ep_raw = msg_reader.get_root::<packet_capnp::login_response::Reader>();
                if let Err(_) = ep_raw {
                    self.send_invalid_data_error()?;
                    return Err(format!("Invalid data received! Disconnecting."));
                }
                let ep = ep_raw.unwrap();

               return match ep.which() {
                    Ok(packet_capnp::login_response::Valid(_)) => {
                        Ok(Packet::LoginResponse { valid: true, error: None })
                    }
                    Ok(packet_capnp::login_response::Error(e)) => {
                        Ok(Packet::LoginResponse { valid: false, error: Some(e.unwrap().to_string()) })
                    }
                    Err(::capnp::NotInSchema(_)) => {
                        self.send_invalid_data_error()?;
                        Err(format!("Invalid data received when expecting a login response! Disconnecting."))
                    }
                }
            }
            ExpectedPacket::Message => {
                let ep_raw = msg_reader.get_root::<packet_capnp::big_boi_chonk::Reader>();
                if let Err(_) = ep_raw {
                    self.send_invalid_data_error()?;
                    return Err(format!("Invalid data received! Disconnecting."));
                }
                let ep = ep_raw.unwrap();

                // This handles 3 "packets" in 1
                match ep.which() {
                    Ok(packet_capnp::big_boi_chonk::Message(mreader)) => {
                        let mr = mreader.unwrap();
                        Ok(Packet::Message {
                            messages: mr.get_message().unwrap()
                                .iter().map(|m| { m.unwrap().to_string() }).collect::<Vec<String>>(),
                            sender: mr.get_sender().unwrap().to_string(),
                            recipient: mr.get_recipient().unwrap().to_string(),
                        })
                    }
                    Ok(packet_capnp::big_boi_chonk::Disconnect(_)) => {
                        Ok(Packet::Disconnect)
                    }
                    Ok(packet_capnp::big_boi_chonk::Error(ereader)) => {
                        let er = ereader.unwrap();
                        Ok(Packet::Error { should_disconnect: er.get_disconnect(), error: er.get_error().unwrap().to_string() })
                    }
                    Err(::capnp::NotInSchema(_)) => {
                        self.send_invalid_data_error()?;
                        Err(format!("Invalid data received when expecting a login response! Disconnecting."))
                    }
                }
            } // end of ExpectedPacket::Message
        } // end of match expected
    } // end of read function
} // end of impl