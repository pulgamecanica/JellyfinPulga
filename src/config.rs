use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub jellyfin: JellyfinConfig,
    pub media: MediaConfig,
    pub server: ServerConfig,
    #[serde(default)]
    pub execution: ExecutionConfig,
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

#[derive(Debug, Deserialize, Clone)]
pub struct ExecutionConfig {
    #[serde(default = "default_mode")]
    pub mode: ExecutionMode,
    #[serde(default)]
    pub ssh: Option<SshConfig>,
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self {
            mode: ExecutionMode::Ssh,
            ssh: None,
        }
    }
}

fn default_mode() -> ExecutionMode {
    ExecutionMode::Ssh
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ExecutionMode {
    Local,
    Ssh,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SshConfig {
    pub host: String,
    #[serde(default = "default_ssh_user")]
    pub user: String,
    #[serde(default = "default_ssh_port")]
    pub port: u16,
    pub key_path: Option<PathBuf>,
}

fn default_ssh_user() -> String {
    whoami::username()
}

fn default_ssh_port() -> u16 {
    22
}

impl Config {
    pub fn load(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }
}
