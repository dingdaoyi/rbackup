use std::fs;
use std::path::PathBuf;

use serde_derive::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub default: String,
    pub log: LogConfig,
    pub servers: Vec<Server>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LogConfig {
    pub level: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct S3Server {
    pub name: String,
    pub access_key: String,
    pub secret_key: String,
    pub bucket: String,
    pub region: String,
    pub endpoint: Option<String>,
    pub default_path: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SSHServer {
    pub name: String,
    pub username: String,
    pub password: String,
    pub server: String,
    pub port: u16,
    pub default_path: String,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(tag = "type")]
pub enum Server {
    S3(S3Server),
    SSH(SSHServer),
}
impl Server{
    pub fn get_name(&self)->String{
        match self {
            Server::S3(S3Server{ name,.. }) => name.clone(),
            Server::SSH(SSHServer {name,..}) => name.clone()
        }
    }
    pub fn get_default_path(&self)->String{
        match self {
            Server::S3(S3Server{ default_path,.. }) => default_path.clone(),
            Server::SSH(SSHServer {default_path,..}) => default_path.clone()
        }
    }
}
impl Config {
    pub fn get_default(&self) -> Option<Server> {
        let default_name = &self.default;
        self.get(Some(default_name))
    }


    pub fn get(&self, name: Option<&String>) -> Option<Server> {
        match name {
            None => None,
            Some(name) => {
                let res = self.servers.iter().find(|server| match server {
                    Server::S3 ( S3Server{name: server_name, .. }) => server_name == name,
                    Server::SSH (SSHServer {name: server_name, ..} ) => server_name == name,
                });
                res.cloned()
            }
        }
    }
}

pub fn load_config(filename: &PathBuf) -> Result<Config, Box<dyn std::error::Error>> {
    let content = fs::read_to_string(filename)?;
    let config: Config = toml::from_str(&content)?;
    Ok(config)
}
