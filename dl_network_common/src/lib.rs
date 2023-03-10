use std::io;
use std::net::TcpStream;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use capnp::message::Builder;
use capnp::{message, serialize};
use capnp::serialize::OwnedSegments;
use regex::Regex;

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

pub fn validate_ip<S: Into<String>>(ip: S) -> bool {
    let ip_pattern =
        Regex::new(r"^(?:(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.){3}(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)|localhost$")
            .expect("Failed to init regex");
    ip_pattern.is_match(ip.into().as_str())
}

pub fn validate_port<S: Into<String>>(port: S) -> bool {
    let port_pattern =
        Regex::new(r"^((6553[0-5])|(655[0-2][0-9])|(65[0-4][0-9]{2})|(6[0-4][0-9]{3})|([1-5][0-9]{4})|([0-5]{0,5})|([0-9]{1,4}))$")
            .expect("Failed to init regex");
    port_pattern.is_match(port.into().as_str())
}

pub struct SentMsg {
    message: String,
    sender: String,
    timestamp: String
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
    Message { message: String, sender: String, recipient: String, timestamp: String },
    /// Client --> Server | A request to see if a user with a specific name exists
    UserExistsRequest { username: String },
    /// Client --> Server | A request to see if a user with a specific name exists
    UserOnlineRequest { username: String },
    /// Server --> Client | A response responding to a UserExists or UserOnline with a true or false value
    UserResponse { response: bool },
    /// Client --> Server | A request to get the message history between self and a user
    MsgHistoryRequest { username: String },
    /// Server --> Client | The message history
    MsgHistory { history: Vec<SentMsg> },
    /// Client <-> Server | A way to announce a disconnection is required or imminent
    Disconnect,
    /// Client <-> Server | A way to announce an error has occurred, what the error is and if it requires a disconnection
    Error { should_disconnect: bool, error: String },
}

/// was here
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
            Packet::Message { message: msg, sender, recipient, timestamp } => {
                let ep = message.init_root::<packet_capnp::big_boi_chonk::Builder>();
                let mut minit = ep.init_message();
                minit.set_message(msg.as_str());
                minit.set_sender(sender.as_str());
                minit.set_recipient(recipient.as_str());
                minit.set_timestamp(timestamp.as_str());
            }
            Packet::UserExistsRequest { username } => {
                let ep = message.init_root::<packet_capnp::big_boi_chonk::Builder>();
                let mut init = ep.init_info_request();
                init.set_username_exists(username.as_str());
            }
            Packet::UserOnlineRequest { username } => {
                let ep = message.init_root::<packet_capnp::big_boi_chonk::Builder>();
                let mut init = ep.init_info_request();
                init.set_username_online(username.as_str());
            }
            Packet::UserResponse { response } => {
                let ep = message.init_root::<packet_capnp::big_boi_chonk::Builder>();
                let mut init = ep.init_info_response();
                init.set_user_response(response);
            }
            Packet::MsgHistoryRequest { username } => {
                let ep = message.init_root::<packet_capnp::big_boi_chonk::Builder>();
                let mut init = ep.init_info_request();
                init.set_msg_history(username.as_str());
            }
            Packet::MsgHistory { history } => {
                let ep = message.init_root::<packet_capnp::big_boi_chonk::Builder>();
                let mut init = ep.init_info_response();
                // initialize the message history list
                let mut list = init.init_msg_history(history.len() as u32);

                // build the list from the history vector
                // there is probably a better way to do this, but I cant find it.
                for x in 0..history.len() {
                    let msg = history.get(x).unwrap();
                    let index = x as u32;
                    list.reborrow().get(index).set_message(msg.message.as_str());
                    list.reborrow().get(index).set_timestamp(msg.timestamp.as_str());
                    list.reborrow().get(index).set_sender(msg.sender.as_str());
                    list.reborrow().get(index).set_recipient("");
                }
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
    pub fn expect(&mut self, expected: ExpectedPacket) -> Result<Packet, String> {
        let msg_reader_raw = serialize::read_message(&mut self.stream, ::capnp::message::ReaderOptions::default());
        if msg_reader_raw.is_err() {
            self.send_invalid_data_error()?;
            return Err(format!("Invalid or corrupt data was received!"));
        }
        let msg_reader = msg_reader_raw.unwrap();

        self.parse_received(expected, msg_reader)
    }

    /// Check if there is a packet to read of an expected type
    /// returns None if there is nothing to read, allowing the program to do other things
    pub fn check_expected(&mut self, expected: ExpectedPacket) -> Result<Option<Packet>, String> {
        let msg_reader_raw = serialize::try_read_message(&mut self.stream, ::capnp::message::ReaderOptions::default());
        if msg_reader_raw.is_err() {
            self.send_invalid_data_error()?;
            return Err(format!("Invalid or corrupt data was received!"));
        }
        let msg_reader = msg_reader_raw.unwrap();

        if msg_reader.is_none() {
            return Ok(None);
        }

        Ok(Some(self.parse_received(expected, msg_reader.unwrap())?))
    }

    fn parse_received(&mut self, expected: ExpectedPacket, reader: message::Reader<OwnedSegments>) -> Result<Packet, String> {
        match expected {
            ExpectedPacket::Ping => {
                let ep_raw = reader.get_root::<packet_capnp::ping::Reader>();
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
                let ep_raw = reader.get_root::<packet_capnp::ping_response::Reader>();
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
                let ep_raw = reader.get_root::<packet_capnp::login_request::Reader>();
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
                let ep_raw = reader.get_root::<packet_capnp::login_response::Reader>();
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
                let ep_raw = reader.get_root::<packet_capnp::big_boi_chonk::Reader>();
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
                            message: mr.get_message().unwrap().to_string(),
                            sender: mr.get_sender().unwrap().to_string(),
                            recipient: mr.get_recipient().unwrap().to_string(),
                            timestamp: mr.get_timestamp().unwrap().to_string(),
                        })
                    }
                    Ok(packet_capnp::big_boi_chonk::Disconnect(_)) => {
                        Ok(Packet::Disconnect)
                    }
                    Ok(packet_capnp::big_boi_chonk::InfoRequest(ireader)) => {
                        match ireader.unwrap().which() {
                            Ok(packet_capnp::info_request::UsernameOnline(ureader)) => {
                                let ur = ureader.unwrap();
                                Ok(Packet::UserOnlineRequest { username: ur.to_string() })
                            }
                            Ok(packet_capnp::info_request::UsernameExists(ureader)) => {
                                let ur = ureader.unwrap();
                                Ok(Packet::UserOnlineRequest { username: ur.to_string() })
                            }
                            Ok(packet_capnp::info_request::MsgHistory(ureader)) => {
                                let ur = ureader.unwrap();
                                Ok(Packet::UserOnlineRequest { username: ur.to_string() })
                            }
                            Err(::capnp::NotInSchema(_)) => {
                                self.send_invalid_data_error()?;
                                Err(format!("Invalid data received when expecting an info request! Disconnecting."))
                            }
                        }
                    }
                    Ok(packet_capnp::big_boi_chonk::InfoResponse(ireader)) => {
                        match ireader.unwrap().which() {
                            Ok(packet_capnp::info_response::UserResponse(response)) => {
                                Ok(Packet::UserResponse { response })
                            }
                            Ok(packet_capnp::info_response::MsgHistory(hreader)) => {
                                let reader = hreader.unwrap();
                                let mut history = Vec::new();
                                for msg in reader.into_iter() {
                                    history.push(SentMsg {
                                        message: msg.get_message().unwrap().to_string(),
                                        sender: msg.get_sender().unwrap().to_string(),
                                        timestamp: msg.get_timestamp().unwrap().to_string()
                                    });
                                }

                                Ok(Packet::MsgHistory { history })
                            }
                            Err(::capnp::NotInSchema(_)) => {
                                self.send_invalid_data_error()?;
                                Err(format!("Invalid data received when expecting an info response! Disconnecting."))
                            }
                        }
                    }
                    Ok(packet_capnp::big_boi_chonk::Error(ereader)) => {
                        let er = ereader.unwrap();
                        Ok(Packet::Error { should_disconnect: er.get_disconnect(), error: er.get_error().unwrap().to_string() })
                    }
                    Err(::capnp::NotInSchema(_)) => {
                        self.send_invalid_data_error()?;
                        Err(format!("Invalid data received when expecting a message! Disconnecting."))
                    }
                }
            } // end of ExpectedPacket::Message
        } // end of match expected
    } // end of parse_received
} // end of impl