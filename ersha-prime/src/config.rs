use std::net::SocketAddr;
use std::path::PathBuf;

use serde::Deserialize;

use ersha_tls::TlsConfig;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub registry: RegistryConfig,
    pub tls: TlsConfig,
}

#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    /// Address for the RPC server to listen on
    pub rpc_addr: SocketAddr,
    /// Address for the HTTP server to listen on
    pub http_addr: SocketAddr,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum RegistryConfig {
    Memory,
    Sqlite { path: PathBuf },
    Clickhouse { url: String, database: String },
}

impl Config {
    pub fn load(path: &PathBuf) -> color_eyre::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                rpc_addr: "0.0.0.0:9000".parse().unwrap(),
                http_addr: "0.0.0.0:8080".parse().unwrap(),
            },
            registry: RegistryConfig::Memory,
            tls: TlsConfig::default(),
        }
    }
}
