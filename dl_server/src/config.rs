use std::fs::{File, OpenOptions};
use std::fs;
use std::path::Path;
use std::io::{Read, Write};
use serde::Deserialize;
use crate::error;

#[derive(Debug, Deserialize)]
pub struct Server {
    pub ip: Option<String>,
    pub port: Option<String>
}

#[derive(Debug, Deserialize)]
pub struct DBCfg {
    pub ip: Option<String>,
    pub port: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Config {
    pub server: Option<Server>,
    pub database: Option<DBCfg>,
}

pub fn config_path<S: Into<String>>(file_name: S) -> Result<String, ()> {
    let cdir_r = std::env::current_dir();
    if let Err(e) = cdir_r {
        error!("Failed to create config file: no access! raw error: {}", e);
        return Err(());
    }
    let cdir = cdir_r.unwrap();
    let current_dir_r = cdir.as_path().to_str();
    if current_dir_r.is_none() {
        error!("Could not access the config file!");
        return Err(());
    }
    let current_dir = current_dir_r.unwrap();
    Ok(format!("{}/config/{}", current_dir, file_name.into()))
}

pub fn read_config_raw(file: &mut File) -> String {
    let mut config_content = String::new();
    file.read_to_string(&mut config_content)
        .expect("Failed to read config file. Please make sure that the server has permission to edit files.");
    config_content
}


pub fn read_config(path: &Path, default: String) -> Config {
    let dir = path.parent().expect("Failed to get parent location of config file. Invalid permissions?");
    if !dir.exists() {
        match fs::create_dir_all(dir) {
            Ok(_) => (),
            Err(e) => panic!("Failed to create parent directories: {}", e)
        }
    }

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .read(true)
        .open(path.clone())
        .expect("An error occurred in opening the config file.");

    let mut data = read_config_raw(&mut file);

    if data.is_empty() {
        match file.write_all(default.as_bytes()) {
            Ok(_) => (),
            Err(e) => {
                panic!("Failed to write defaults to config file: {}", e);
            }
        }
        data = default;
    }

    toml::from_str(data.as_str()).expect(format!("Could not read config file `{}`!", path.to_str().unwrap()).as_str())
}