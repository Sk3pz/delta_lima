use r2d2_postgres::PostgresConnectionManager;
use r2d2_postgres::postgres::NoTls;
use r2d2_postgres::r2d2::PooledConnection;
use regex::Regex;
use dl_network_common::{Connection, ExpectedPacket, Packet};
use crate::{debug, warn};
use crate::database::{get_user_from_username, insert_user};

pub fn validate_username(name: String) -> bool {

    // ^([A-Za-z0-9_-]{2,17})$
    let Ok(username_patt) = Regex::new("^([A-Za-z0-9_-]{2,17})$") else {
        warn!("Failed to initialize regex pattern for username check.");
        return false;
    };

    username_patt.is_match(name.as_str())
}

pub fn validate_password(pass: String) -> bool {

    // old regex: ^([A-Za-z0-9~`!@#$%^&*()_+-=\[\]{}:;<>,.?/\\|]{2,33})$
    let Ok(pass_patt) = Regex::new(r"^.{2,33}$") else {
        warn!("Failed to initialize regex pattern for password check.");
        return false;
    };

    pass_patt.is_match(pass.as_str())
}

/// Handles login and signup attempts from the client
/// returns true if disconnecting
pub fn login_handler(connection: &mut Connection, db: &mut PooledConnection<PostgresConnectionManager<NoTls>>) -> Option<i32> {

    // for storing the username for debugging
    let mut uname = format!("");
    // store the id when received
    let mut id= 0;
    loop {
        // expect Login packet from client
        let expected = connection.expect(ExpectedPacket::LoginRequest);
        if let Err(e) = expected {
            warn!("Failed to get LoginRequest from a client: {}", e);
            return None;
        }
        let (username, password, signup) = match expected.unwrap() {
            Packet::LoginRequest { username, password, signup } => {
                (username, password, signup)
            }
            _ => unreachable!()
        };

        // Handle if the user is signing up
        if signup {
            // check username and password
            if !validate_username(uname.clone()) {
                if connection.send(Packet::LoginResponse {
                    valid: false,
                    error: Some(format!("Invalid characters in username"))
                }).is_err() {
                    warn!("Failed to send Login Accept to {}", uname);
                }
                debug!("client failed to sign up with username {}, invalid username.", uname);
                continue;
            }

            if !validate_password(password.clone()) {
                if connection.send(Packet::LoginResponse {
                    valid: false,
                    error: Some(format!("Invalid characters in password"))
                }).is_err() {
                    warn!("Failed to send Login Accept to {}", uname);
                }
                debug!("client failed to sign up with username {}, invalid username.", uname);
                continue;
            }

            if let Err(e) = insert_user(db, uname.clone(), password) {
                if connection.send(Packet::LoginResponse {
                    valid: false,
                    error: Some(format!("Username is taken"))
                }).is_err() {
                    warn!("Failed to send Login Accept to {}", uname);
                }
                debug!("client failed to sign up with username {}: {}", uname, e);
                continue;
            }
            break;
        }

        // get the password of the user from db to verify if the received password is correct
        // this also gives the user's id to reduce the amount of database queries
        let pass_query = get_user_from_username(db, username.clone());
        if let Err(e) = pass_query {
            // query result sent an error
            warn!("Client login attempt with username {} sent a database error: {}", username, e);
            if connection.send(Packet::LoginResponse { valid: false, error: Some(format!("Invalid login credentials")) }).is_err() {
                warn!("Client login attempt with username {}: Failed to send failed login response!", username);
            }
            continue;
        }
        let (qid, pass) = pass_query.unwrap();

        debug!("client attempting login under username {}", username);

        // password is invalid
        if password != pass {
            if connection.send(Packet::LoginResponse {
                valid: false,
                error: Some(format!("Invalid login credentials"))
            }).is_err() {
                warn!("Client login attempt with username {}: Failed to send failed login response!", username);
            }
            debug!("{} failed to log in", username);
            continue;
        }

        debug!("{} logged in.", username);
        uname = username;
        id = qid;

        break;
    };

    if connection.send(Packet::LoginResponse {
        valid: true,
        error: None
    }).is_err() {
        warn!("Failed to send Login Accept to {}", uname);
    }

    Some(id)
}