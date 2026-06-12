use serde::{Deserialize, Serialize};
use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::PathBuf,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_addr")]
    pub addr: SocketAddr,
    #[serde(default)]
    pub tls_cert: String,
    #[serde(default)]
    pub tls_key: String,
    #[serde(default)]
    pub webhook_url: String,
    #[serde(default = "default_data_dir")]
    pub data_dir: PathBuf,
    #[serde(default = "default_server_url")]
    pub server_url: String,
    #[serde(default = "default_phi_info_url")]
    pub phi_info_url: String,
}

fn default_addr() -> SocketAddr {
    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 3000)
}

fn default_data_dir() -> PathBuf {
    PathBuf::from("./data")
}

fn default_server_url() -> String {
    "https://rak3ffdi.cloud.tds1.tapapis.cn".to_string()
}

fn default_phi_info_url() -> String {
    "http://127.0.0.1:41669".to_string()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            addr: default_addr(),
            tls_cert: String::new(),
            tls_key: String::new(),
            webhook_url: String::new(),
            data_dir: default_data_dir(),
            server_url: default_server_url(),
            phi_info_url: default_phi_info_url(),
        }
    }
}

impl Config {
    pub fn load(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        match std::fs::read_to_string(path) {
            Ok(content) => {
                let config: Config = serde_json::from_str(&content)?;
                Ok(config)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let config = Config::default();
                if let Some(parent) = PathBuf::from(path).parent()
                    && !parent.as_os_str().is_empty()
                {
                    std::fs::create_dir_all(parent)?;
                }

                std::fs::write(path, serde_json::to_string_pretty(&config)?)?;
                Ok(config)
            }
            Err(e) => Err(Box::new(e)),
        }
    }
}
