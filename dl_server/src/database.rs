use std::path::Path;
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