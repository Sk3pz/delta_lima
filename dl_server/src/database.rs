use std::path::Path;
use chrono::{DateTime, Utc};
use r2d2_postgres::postgres::NoTls;
use r2d2_postgres::PostgresConnectionManager;
use r2d2_postgres::r2d2::PooledConnection;
use uuid::Uuid;
use crate::config::{config_path, read_config};
use crate::warn;

pub struct DBInfo {
    pub ip: String,
    pub port: String,
    pub uname: String,
    pub pass: String,
}

pub fn get_db_address() -> Result<DBInfo, String> {
    // get database configuration
    let Ok(cfg_path) = config_path("database.toml") else {
        return Err(format!("Failed to get config path to database.toml! Please ensure you run the server as administrator or superuser"));
    };
    let raw_path = Path::new(&cfg_path);
    let config = read_config(raw_path, format!("\
        [database]\
        \n# ip: the ip to connect to\
        \n# surround with '[' and ']' for Ipv6 addresses\
        \n# Can be set to localhost\
        \n# defaults to 0.0.0.0 and will listen on your machines current IP\
        \nip = \"localhost\"\
        \n# port: the port to listen on\
        \n# defaults to 5432\
        \nport = \"5432\"\
        \n# username: the root user\
        \nusername = \"postgres\"\
        \n# password: the password for the root user\
        \npassword = \"admin\""));

    // set default values for the config
    let mut ip = format!("localhost");
    let mut port = format!("5432");
    let mut username = format!("postgres");
    let mut password = format!("admin");

    if let Some(dbcfg) = config.database {
        if let Some(ipv) = dbcfg.ip {
            ip = ipv;
        } else {
            warn!("Failed to read ip value from `~/config/database.toml`");
        }
        if let Some(portv) = dbcfg.port {
            port = portv;
        } else {
            warn!("Failed to read port value from `~/config/database.toml`");
        }
        if let Some(usernamev) = dbcfg.username {
            username = usernamev;
        } else {
            warn!("Failed to read username value from `~/config/database.toml`");
        }
        if let Some(passv) = dbcfg.password {
            password = passv;
        } else {
            warn!("Failed to read password value from `~/config/database.toml`");
        }
    }

    Ok(DBInfo {
        ip, port,
        uname: username,
        pass: password,
    })
}

// == UNSENT_MSGS

pub fn insert_msg(db: &mut PooledConnection<PostgresConnectionManager<NoTls>>, sender: i32, recipient: i32, message: String, timestamp: DateTime<Utc>) -> Result<(), String> {
    if let Err(e) = db.execute(
        "INSERT INTO unsent_msgs(sender, recipient, message, timestamp, id) VALUES ($1, $2, $3, $4, $5)",
        &[&sender, &recipient, &(message.as_str()), &timestamp, &(Uuid::new_v4())]) {
        return Err(format!("insert_msg.{}", e));
    }

    Ok(())
}

pub struct DBMessageQuery {
    pub id: Uuid,
    pub sender: String,
    pub message: String,
    pub timestamp: DateTime<Utc>,
}

pub fn get_next_msg(db: &mut PooledConnection<PostgresConnectionManager<NoTls>>, receiver: i32) -> Result<Option<DBMessageQuery>, String> {
    // todo(skepz): timestamps need to be handled here
    let msg_query_result = db.query(
        "SELECT sender, message, id, timestamp FROM unsent_msgs WHERE recipient=$1",
        &[&receiver]);
    if let Err(e) = msg_query_result {
        warn!("{}", e);
        return Err(format!("get_next_msg.{}", e));
    }
    let msg_query = msg_query_result.unwrap();

    if msg_query.len() == 0 {
        return Ok(None);
    }
    let msg = msg_query.get(0).unwrap();

    let sender: i32 = msg.get(0);
    let message: String = msg.get(1);
    let id: Uuid = msg.get(2);
    let timestamp: DateTime<Utc> = msg.get(3);

    // get the username of the sender
    let sender_name = get_username_from_id(db, sender);
    if let Err(e) = sender_name {
        return Err(format!("Failed to get sender of an unset message: {}", e));
    }

    // return the message
    Ok(Some(DBMessageQuery {
        id, message, sender: sender_name.unwrap(), timestamp
    }))
}

pub fn delete_msg(db: &mut PooledConnection<PostgresConnectionManager<NoTls>>, id: Uuid) -> Result<(), String> {
    if let Err(e) = db.execute("DELETE FROM unsent_msgs WHERE id=$1",
                                     &[&id]) {
        return Err(format!("A message could not be removed from the database! {}", e));
    }

    Ok(())
}

// == USER_DATA

pub fn insert_user(db: &mut PooledConnection<PostgresConnectionManager<NoTls>>, username: String, password: String) -> Result<(), String> {
    if let Err(e) = db.execute("INSERT INTO user_data(username, password) VALUES ($1, $2)",
                               &[&(username.as_str()), &(password.as_str())]) {
        return Err(format!("insert_user.{}", e));
    }
    Ok(())
}

pub fn get_username_from_id(db: &mut PooledConnection<PostgresConnectionManager<NoTls>>, id: i32) -> Result<String, String> {
    let query_result = db.query(
        "SELECT username FROM user_data WHERE id=$1", &[&id]);
    if let Err(e) = query_result {
        return Err(format!("get_username_from_id.{}", e));
    }
    let user_rows = query_result.unwrap();
    if user_rows.len() > 1 {
        return Err(format!("Multiple users with the same username found!"));
    }
    if user_rows.len() == 0 {
        return Err(format!("Invalid id!"));
    }
    let user = user_rows.get(0).unwrap();

    Ok(user.get(0))
}

pub fn get_id_from_username(db: &mut PooledConnection<PostgresConnectionManager<NoTls>>, username: String) -> Result<i32, String> {
    let id_query = db.query(
        format!("SELECT id FROM user_data WHERE username=$1").as_str(), &[&(username.as_str())]);
    if let Err(e) = id_query {
        return Err(format!("get_id_from_username.{}", e));
    }
    let user_rows = id_query.unwrap();
    if user_rows.len() > 1 {
        return Err(format!("Multiple users with the same username found!"));
    }
    if user_rows.len() == 0 {
        return Err(format!("Invalid username"));
    }
    let row = user_rows.get(0).unwrap();

    Ok(row.get(0))
}

pub fn get_user_from_username(db: &mut PooledConnection<PostgresConnectionManager<NoTls>>, username: String) -> Result<(i32, String), String> {
    let password_query = db.query(
        format!("SELECT id, password FROM user_data WHERE username=$1").as_str(), &[&(username.as_str())]);
    if let Err(e) = password_query {
        return Err(format!("get_user_from_username.{}", e));
    }
    let user_rows = password_query.unwrap();
    if user_rows.len() > 1 {
        return Err(format!("Multiple users with the same username found"));
    }
    if user_rows.len() == 0 {
        return Err(format!("Invalid username"));
    }
    let row = user_rows.get(0).unwrap();

    Ok((row.get(0), row.get(1)))
}