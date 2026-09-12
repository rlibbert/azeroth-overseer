use serde::Deserialize;
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub database: DatabaseConfig,
    pub server: ServerConfig,
}

#[derive(Debug, Deserialize)]
pub struct DatabaseConfig {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    pub poll_interval_secs: u64,
    pub http_port: u16,
}

impl Config {
    pub fn load() -> Self {
        let path = Path::new("config.toml");
        if !path.exists() {
            eprintln!(
                "config.toml not found. Copy config.toml.example to config.toml and fill in your database credentials."
            );
            std::process::exit(1);
        }
        let raw = fs::read_to_string(path).expect("failed to read config.toml");
        toml::from_str(&raw).expect("failed to parse config.toml")
    }
}
