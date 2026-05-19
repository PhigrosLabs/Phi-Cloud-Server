use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub webhook_url: String,
    #[serde(default = "default_data_dir")]
    pub data_dir: PathBuf,
    #[serde(default = "default_server_url")]
    pub server_url: String,
}

fn default_data_dir() -> PathBuf {
    PathBuf::from("./data")
}

fn default_server_url() -> String {
    "https://rak3ffdi.cloud.tds1.tapapis.cn".to_string()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            webhook_url: String::new(),
            data_dir: default_data_dir(),
            server_url: default_server_url(),
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
