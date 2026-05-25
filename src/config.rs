use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub jellyfin: JellyfinConfig,
    pub media: MediaConfig,
    pub server: ServerConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct JellyfinConfig {
    pub url: String,
    pub api_key: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct MediaConfig {
    pub paths: Vec<PathBuf>,
    pub ffprobe_path: PathBuf,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

impl Config {
    pub fn load(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }
}
