// Delta Lima Server main file

use std::{io, thread};
use std::net::TcpListener;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use better_term::flush_styles;
use r2d2_postgres::postgres::NoTls;
use r2d2_postgres::{PostgresConnectionManager, r2d2};
use dl_network_common::{validate_ip, validate_port};
use crate::client::chandler;
use crate::config::{config_path, read_config};
use crate::database::get_db_address;

pub mod logging;
pub mod database;
pub mod config;
mod client;

pub const ACCEPTED_CLIENT_VERSION: &str = "0.1.1";
pub const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

// How long the main loop should wait between checking for incoming connections to save cpu resources
const MAIN_LOOP_WAIT_DELAY_MS: u64 = 20;

fn main() {
    info!("Reading IP and Port from config file...");
    // handle configuration
    let Ok(cfg_path) = config_path("config.toml") else { return; };
    let raw_path = Path::new(&cfg_path);
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
        } else {
            warn!("Failed to read ip value from `~/config/config.toml`");
        }
        if let Some(cfg_port) = server_conf.port {
            port = cfg_port;
        } else {
            warn!("Failed to read ip value from `~/config/config.toml`");
        }
    }

    if !validate_ip(ip.clone()) {
        error!("Invalid IP found in `~/config/config.toml`! If this issue persists, try deleting config.toml and re-running the program.");
        return;
    }

    if !validate_port(port.clone()) {
        error!("Invalid Port found in `~/config/config.toml`! If this issue persists, try deleting config.toml and re-running the program.");
        return;
    }

    let full_ip = format!("{}:{}", ip, port);

    // Create the listener for incoming connection attempts
    info!("Done! Starting server...");
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

    info!("Connecting to and setting up the database...");

    let database_info_result = get_db_address();
    if let Err(e) = database_info_result {
        error!("Failed to read database config: {}", e);
        return;
    }
    let dbinfo = database_info_result.unwrap();

    // create an r2d2 connection pool
    let db_manager = PostgresConnectionManager::new(
        format!("host={} port={} user={} password={}", dbinfo.ip, dbinfo.port, dbinfo.uname, dbinfo.pass).parse().unwrap(),
        NoTls
    );
    let pool = r2d2::Pool::new(db_manager).unwrap();

    let mut db_client = pool.get().unwrap();

    info!("Connected. Verifying tables...");

    // ensure the correct tables are created on the database
    db_client.execute(
        r"
    CREATE TABLE IF NOT EXISTS user_data (
        id       UUID,
        username VARCHAR UNIQUE NOT NULL,
        password VARCHAR NOT NULL
    );", &[]).expect("Failed to create database user_data table!");
    db_client.execute(
        r"
    CREATE TABLE IF NOT EXISTS unsent_msgs (
        id UUID,
        timestamp TIMESTAMP WITH TIME ZONE,
        message VARCHAR,
        sender UUID,
        recipient UUID
    );", &[]).expect("Failed to create database unsent_msgs table!");

    // this might not be needed later:
    db_client.execute(
        "SET timezone = \"America/Chicago\"", &[]).expect("Failed to set database timezone!");

    info!("Verified! Setting things up...");

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

    info!("Done! Listening on {}:{}", ip, port);

    // listen for incoming connections
    for stream in listener.incoming() {
        match stream {
            Ok(s) => {
                info!("New connection!");
                // create db reference and termination reference
                let tarc = Arc::clone(&terminate);
                let pool = pool.clone();

                handlers.push(thread::spawn(move || {
                    chandler(s, pool, tarc);
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
                    info!("Dropped a thread because it was finished.");
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
    flush_styles();
}
