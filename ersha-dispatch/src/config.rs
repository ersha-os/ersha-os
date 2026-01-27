use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub dispatcher: DispatcherConfig,
    pub server: ServerConfig,
    pub storage: StorageConfig,
    pub prime: PrimeConfig,
    pub edge: EdgeConfig,
}

#[derive(Debug, Deserialize)]
pub struct DispatcherConfig {
    /// Dispatcher ID (ULID format)
    pub id: String,
    /// H3 cell location
    pub location: u64,
}

#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    /// Address for the HTTP server to listen on
    pub http_addr: SocketAddr,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum StorageConfig {
    Memory,
    Sqlite { path: PathBuf },
}

#[derive(Debug, Deserialize)]
pub struct PrimeConfig {
    /// Address of the ersha-prime RPC server
    pub rpc_addr: SocketAddr,
    /// Interval in seconds between upload attempts
    pub upload_interval_secs: u64,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum EdgeConfig {
    Mock {
        /// Interval in seconds between sensor readings
        reading_interval_secs: u64,
        /// Interval in seconds between status updates
        status_interval_secs: u64,
        /// Number of simulated devices
        device_count: usize,
    },
    Tcp {
        addr: SocketAddr,
    },
}

impl Config {
    pub fn load(path: &Path) -> color_eyre::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            dispatcher: DispatcherConfig {
                id: "01JJNQ1KQCNZ8X9PQRV5ABCD12".to_string(),
                location: 0x8a2a1072b59ffff,
            },
            server: ServerConfig {
                http_addr: "0.0.0.0:8081".parse().unwrap(),
            },
            storage: StorageConfig::Memory,
            prime: PrimeConfig {
                rpc_addr: "127.0.0.1:9000".parse().unwrap(),
                upload_interval_secs: 60,
            },
            edge: EdgeConfig::Mock {
                reading_interval_secs: 5,
                status_interval_secs: 30,
                device_count: 3,
            },
        }
    }
}
