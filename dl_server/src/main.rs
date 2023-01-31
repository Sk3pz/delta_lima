// Delta Lima Server main file

use std::{io, thread};
use std::net::TcpListener;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use dl_network_common::Connection;
use crate::client::handle_connection;
use crate::config::read_config;

pub mod logging;
mod config;
mod client;
pub mod database;

pub const ACCEPTED_CLIENT_VERSION: &str = "0.1.0";
pub const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

// How long the main loop should wait between checking for incoming connections to save cpu resources
const MAIN_LOOP_WAIT_DELAY_MS: u64 = 20;

fn main() {
    info!("Reading IP and Port from config file...");
    // handle configuration
    let cdir_r = std::env::current_dir();
    if let Err(e) = cdir_r {
        error!("Failed to create config file: no access! raw error: {}", e);
        return;
    }
    let cdir = cdir_r.unwrap();
    let current_dir_r = cdir.as_path().to_str();
    if current_dir_r.is_none() {
        error!("Could not access the config file!");
        return;
    }
    let current_dir = current_dir_r.unwrap();
    let config_path = format!("{}/config/config.toml", current_dir);
    let raw_path = Path::new(&config_path);
    let config = read_config(raw_path, format!("\
    [server]\
    \n# ip: the ip to listen on\
    \n# surround with '[' and ']' for Ipv6 addresses\
    \n# defaults to 0.0.0.0 and will listen on your machines current IP\
    \nip = \"0.0.0.0\"\
    \n# port: the port to listen on\
    \n# defaults to 2277\
    \nport = \"2277\""));

    // set default values for the config
    let mut ip = format!("0.0.0.0");
    let mut port = format!("2277");

    // if the configuration values are set, override defaults
    if let Some(server_conf) = config.server {
        if let Some(cfg_ip) = server_conf.ip {
            ip = cfg_ip;
        }
        if let Some(cfg_port) = server_conf.port {
            port = cfg_port;
        }
    }

    let full_ip = format!("{}:{}", ip, port);

    // Create the listener for incoming connection attempts
    info!("Done! Starting server at {} on port {}", ip, port);
    let listener_result = TcpListener::bind(full_ip.clone());
    let Ok(listener) = listener_result else {
        error!("Failed to bind listener to {}!", full_ip);
        return;
    };

    // set the listener to non-blocking to ensure safe exiting of the server
    if let Err(e) = listener.set_nonblocking(true) {
        error!("Failed to set the connection listener to non-blocking mode; safely exiting would not be possible.\n  Error: {}", e);
        return;
    }

    // todo(skepz) database code here

    // shutdown flag for threads
    let terminate = Arc::new(AtomicBool::new(false));

    // safely exit when ctrl+c is called
    let ctrlc_tarc = Arc::clone(&terminate);
    let cc_handler = ctrlc::set_handler(move || {
        ctrlc_tarc.store(true, Ordering::SeqCst);
    });
    if let Err(e) = cc_handler {
        error!("Failed to set exit handler; no safe way to exit\n  Error: {}", e);
        return;
    }

    let mut handlers = Vec::new();

    // listen for incoming connections
    for stream in listener.incoming() {
        match stream {
            Ok(s) => {
                // create db reference and termination reference
                let tarc = Arc::clone(&terminate);

                handlers.push(thread::spawn(move || {
                    // todo(skepz): create & call handle_connection function (takes s, link_arc and tarc)
                    handle_connection(s, tarc);
                }));
            }
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                // handle if the program needs to exit
                if terminate.load(Ordering::SeqCst) {
                    info!("Safely shutting down server...");
                    break;
                }

                // handle handlers no longer in use
                handlers.retain(|h| {
                    h.is_finished()
                });

                // save CPU resources with a sleep call
                thread::sleep(Duration::from_millis(MAIN_LOOP_WAIT_DELAY_MS));
                continue;
            }
            Err(e) => {
                error!("Encountered an error when polling for connections: {}", e);
                // safely exit
                break;
            }
        }
    }

    terminate.store(true, Ordering::SeqCst);

    info!("Shutting down all active connections...");

    for h in handlers {
        if let Err(_) = h.join() {
            warn!("A thread was unavailable when shutting down, this means a possible memory leak. Please report this alongside all other log messages!\n\
            (This will not harm your computer, but means the program is operating inefficiently)");
        }
    }

    // stop the listener
    drop(listener);

    info!("Server shut down!");

}
