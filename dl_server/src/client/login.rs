use r2d2_postgres::{PostgresConnectionManager, r2d2};
use r2d2_postgres::postgres::NoTls;
use r2d2_postgres::r2d2::{Pool, PooledConnection};
use dl_network_common::{Connection, ExpectedPacket, Packet};
use crate::{debug, warn};

/// Handles login and signup attempts from the client
/// returns true if disconnecting
pub fn login_handler(connection: &mut Connection, db: r2d2::Pool<PostgresConnectionManager<NoTls>>) -> bool {

    let mut dbclient = db.get().unwrap();

    // store for logging purposes
    let mut uname = format!("");
    loop {
        // expect Login packet from client
        let expected = connection.expect(ExpectedPacket::LoginRequest);
        if let Err(e) = expected {
            warn!("Failed to get LoginRequest from a client: {}", e);
            return true;
        }
        let (username, password, signup) = match expected.unwrap() {
            Packet::LoginRequest { username, password, signup } => {
                (username, password, signup)
            }
            _ => unreachable!()
        };

        if signup {
            if signup_handler(connection, &mut dbclient, username, password) {
                continue;
            }
            break;
        }

        // get the password of the user from db to verify if the received password is correct
        let query_result = dbclient.query(format!("SELECT password FROM user_data WHERE username='{}'", username).as_str(),
                                          &[]);
        if let Err(e) = query_result {
            // query result sent an error
            warn!("Client login attempt with username {} sent a database error: {}", username, e);
            if connection.send(Packet::LoginResponse { valid: false, error: Some(format!("Invalid login credentials")) }).is_err() {
                warn!("Client login attempt with username {}: Failed to send failed login response!", username);
            }
            continue;
        }
        let query = query_result.unwrap();
        if query.len() > 1 {
            warn!("Client login attempt with username {} had multiple database entries!", username);
            if connection.send(Packet::LoginResponse {
                valid: false,
                error: Some(format!("This user is corrupted in the database. Please let the system admin know!"))
            }).is_err() {
                warn!("Client login attempt with username {}: Failed to send failed login response!", username);
                return true;
            }
            continue;
        }
        if query.len() == 0 {
            // user with that name does not exist.
            if connection.send(Packet::LoginResponse {
                valid: false,
                error: Some(format!("Invalid login credentials"))
            }).is_err() {
                warn!("Client login attempt with username {}: Failed to send failed login response!", username);
                return true;
            }
            continue;
        }
        let row = query.get(0).unwrap();
        let qpass: String = row.get(0);

        debug!("client attempting login under username {}", username);

        // password is invalid
        if password != qpass {
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

        break;
    }

    if connection.send(Packet::LoginResponse {
        valid: true,
        error: None
    }).is_err() {
        warn!("Failed to send Login Accept to {}", uname);
    }

    false
}

pub fn signup_handler(connection: &mut Connection, db: &mut PooledConnection<PostgresConnectionManager<NoTls>>, uname: String, password: String) -> bool {

    debug!("client attempting signup");

    // attempt to create new user
    let result = db.execute("INSERT INTO user_data(username, password) VALUES ($1, $2)",
                            &[&(uname.as_str()), &(password.as_str())]);

    if result.is_err() {
        if connection.send(Packet::LoginResponse {
            valid: false,
            error: Some(format!("Username is taken"))
        }).is_err() {
            warn!("Failed to send Login Accept to {}", uname);
        }
        debug!("client failed to sign up with username {}", uname);
        return true
    }

    debug!("client signed up; username: {}", uname);
    false
}