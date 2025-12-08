//! Configuration management for SpacePanda
//!
//! This module provides environment-based configuration management with
//! support for defaults, validation, and feature flags.

use serde::{Deserialize, Serialize};
use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

mod error;
mod feature_flags;

pub use error::ConfigError;
pub use feature_flags::{FeatureFlags, FeatureManager};

/// Main application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Server configuration
    pub server: ServerConfig,

    /// DHT configuration
    pub dht: DhtConfig,

    /// Store configuration
    pub store: StoreConfig,

    /// Logging configuration
    pub logging: LoggingConfig,

    /// Metrics configuration
    pub metrics: MetricsConfig,

    /// Feature flags
    pub features: FeatureFlags,
}

/// Server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Server bind address
    pub bind_address: SocketAddr,

    /// Maximum concurrent connections
    pub max_connections: usize,

    /// Connection timeout
    #[serde(with = "humantime_serde")]
    pub connection_timeout: Duration,

    /// Graceful shutdown timeout
    #[serde(with = "humantime_serde")]
    pub shutdown_timeout: Duration,

    /// Enable TLS
    pub enable_tls: bool,

    /// TLS certificate path (if TLS enabled)
    pub tls_cert_path: Option<PathBuf>,

    /// TLS key path (if TLS enabled)
    pub tls_key_path: Option<PathBuf>,
}

/// DHT configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DhtConfig {
    /// Number of buckets in routing table
    pub bucket_count: usize,

    /// Maximum entries per bucket
    pub bucket_size: usize,

    /// Replication factor
    pub replication_factor: usize,

    /// Anti-entropy sync interval
    #[serde(with = "humantime_serde")]
    pub sync_interval: Duration,

    /// Peer timeout
    #[serde(with = "humantime_serde")]
    pub peer_timeout: Duration,

    /// Bootstrap peers
    pub bootstrap_peers: Vec<SocketAddr>,
}

/// Store configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreConfig {
    /// Data directory for persistent storage
    pub data_dir: PathBuf,

    /// Enable WAL (Write-Ahead Log)
    pub enable_wal: bool,

    /// Snapshot interval
    #[serde(with = "humantime_serde")]
    pub snapshot_interval: Duration,

    /// Maximum snapshot size in bytes
    pub max_snapshot_size: usize,

    /// Enable compression
    pub enable_compression: bool,

    /// Tombstone cleanup interval
    #[serde(with = "humantime_serde")]
    pub tombstone_cleanup_interval: Duration,
}

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level (trace, debug, info, warn, error)
    pub level: String,

    /// Enable JSON formatting
    pub json_format: bool,

    /// Include timestamps
    pub with_timestamp: bool,

    /// Include target module
    pub with_target: bool,

    /// Log file path (optional)
    pub log_file: Option<PathBuf>,
}

/// Metrics configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsConfig {
    /// Enable metrics collection
    pub enabled: bool,

    /// Metrics bind address
    pub bind_address: SocketAddr,

    /// Metrics collection interval
    #[serde(with = "humantime_serde")]
    pub collection_interval: Duration,

    /// Enable Prometheus export
    pub enable_prometheus: bool,

    /// Enable OpenTelemetry export
    pub enable_opentelemetry: bool,

    /// OpenTelemetry endpoint
    pub otlp_endpoint: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            dht: DhtConfig::default(),
            store: StoreConfig::default(),
            logging: LoggingConfig::default(),
            metrics: MetricsConfig::default(),
            features: FeatureFlags::default(),
        }
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind_address: "127.0.0.1:8080".parse().unwrap(),
            max_connections: 10_000,
            connection_timeout: Duration::from_secs(30),
            shutdown_timeout: Duration::from_secs(30),
            enable_tls: false,
            tls_cert_path: None,
            tls_key_path: None,
        }
    }
}

impl Default for DhtConfig {
    fn default() -> Self {
        Self {
            bucket_count: 256,
            bucket_size: 20,
            replication_factor: 3,
            sync_interval: Duration::from_secs(60),
            peer_timeout: Duration::from_secs(300),
            bootstrap_peers: vec![],
        }
    }
}

impl Default for StoreConfig {
    fn default() -> Self {
        Self {
            data_dir: PathBuf::from("./data"),
            enable_wal: true,
            snapshot_interval: Duration::from_secs(300),
            max_snapshot_size: 100 * 1024 * 1024, // 100 MB
            enable_compression: true,
            tombstone_cleanup_interval: Duration::from_secs(3600),
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            json_format: false,
            with_timestamp: true,
            with_target: true,
            log_file: None,
        }
    }
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            bind_address: "127.0.0.1:9090".parse().unwrap(),
            collection_interval: Duration::from_secs(15),
            enable_prometheus: true,
            enable_opentelemetry: false,
            otlp_endpoint: None,
        }
    }
}

impl Config {
    /// Load configuration from environment variables
    ///
    /// Environment variables follow the pattern: SPACEPANDA_<SECTION>_<KEY>
    /// Example: SPACEPANDA_SERVER_BIND_ADDRESS=0.0.0.0:8080
    pub fn from_env() -> Result<Self, ConfigError> {
        let mut config = Self::default();

        // Server config
        if let Ok(addr) = env::var("SPACEPANDA_SERVER_BIND_ADDRESS") {
            config.server.bind_address = addr
                .parse()
                .map_err(|e| ConfigError::InvalidValue(format!("Invalid bind address: {}", e)))?;
        }
        if let Ok(max_conn) = env::var("SPACEPANDA_SERVER_MAX_CONNECTIONS") {
            config.server.max_connections = max_conn.parse().map_err(|e| {
                ConfigError::InvalidValue(format!("Invalid max connections: {}", e))
            })?;
        }
        if let Ok(enable_tls) = env::var("SPACEPANDA_SERVER_ENABLE_TLS") {
            config.server.enable_tls = enable_tls
                .parse()
                .map_err(|e| ConfigError::InvalidValue(format!("Invalid TLS flag: {}", e)))?;
        }

        // DHT config
        if let Ok(bucket_size) = env::var("SPACEPANDA_DHT_BUCKET_SIZE") {
            config.dht.bucket_size = bucket_size
                .parse()
                .map_err(|e| ConfigError::InvalidValue(format!("Invalid bucket size: {}", e)))?;
        }
        if let Ok(repl_factor) = env::var("SPACEPANDA_DHT_REPLICATION_FACTOR") {
            config.dht.replication_factor = repl_factor.parse().map_err(|e| {
                ConfigError::InvalidValue(format!("Invalid replication factor: {}", e))
            })?;
        }

        // Store config
        if let Ok(data_dir) = env::var("SPACEPANDA_STORE_DATA_DIR") {
            config.store.data_dir = PathBuf::from(data_dir);
        }
        if let Ok(enable_wal) = env::var("SPACEPANDA_STORE_ENABLE_WAL") {
            config.store.enable_wal = enable_wal
                .parse()
                .map_err(|e| ConfigError::InvalidValue(format!("Invalid WAL flag: {}", e)))?;
        }

        // Logging config
        if let Ok(level) = env::var("SPACEPANDA_LOG_LEVEL") {
            config.logging.level = level;
        }
        if let Ok(json) = env::var("SPACEPANDA_LOG_JSON") {
            config.logging.json_format = json
                .parse()
                .map_err(|e| ConfigError::InvalidValue(format!("Invalid JSON flag: {}", e)))?;
        }

        // Metrics config
        if let Ok(enabled) = env::var("SPACEPANDA_METRICS_ENABLED") {
            config.metrics.enabled = enabled
                .parse()
                .map_err(|e| ConfigError::InvalidValue(format!("Invalid metrics flag: {}", e)))?;
        }
        if let Ok(addr) = env::var("SPACEPANDA_METRICS_BIND_ADDRESS") {
            config.metrics.bind_address = addr.parse().map_err(|e| {
                ConfigError::InvalidValue(format!("Invalid metrics address: {}", e))
            })?;
        }

        config.validate()?;
        Ok(config)
    }

    /// Load configuration from file
    pub fn from_file(path: impl AsRef<std::path::Path>) -> Result<Self, ConfigError> {
        let contents =
            std::fs::read_to_string(path).map_err(|e| ConfigError::FileReadError(e.to_string()))?;

        let config: Self =
            toml::from_str(&contents).map_err(|e| ConfigError::ParseError(e.to_string()))?;

        config.validate()?;
        Ok(config)
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<(), ConfigError> {
        // Validate server config
        if self.server.max_connections == 0 {
            return Err(ConfigError::ValidationFailed(
                "max_connections must be greater than 0".to_string(),
            ));
        }

        if self.server.enable_tls {
            if self.server.tls_cert_path.is_none() || self.server.tls_key_path.is_none() {
                return Err(ConfigError::ValidationFailed(
                    "TLS enabled but cert/key paths not provided".to_string(),
                ));
            }
        }

        // Validate DHT config
        if self.dht.bucket_size == 0 {
            return Err(ConfigError::ValidationFailed(
                "bucket_size must be greater than 0".to_string(),
            ));
        }

        if self.dht.replication_factor == 0 {
            return Err(ConfigError::ValidationFailed(
                "replication_factor must be greater than 0".to_string(),
            ));
        }

        // Validate store config
        if self.store.max_snapshot_size == 0 {
            return Err(ConfigError::ValidationFailed(
                "max_snapshot_size must be greater than 0".to_string(),
            ));
        }

        // Validate logging config
        let valid_levels = ["trace", "debug", "info", "warn", "error"];
        if !valid_levels.contains(&self.logging.level.as_str()) {
            return Err(ConfigError::ValidationFailed(format!(
                "Invalid log level: {}",
                self.logging.level
            )));
        }

        Ok(())
    }

    /// Save configuration to file
    pub fn save_to_file(&self, path: impl AsRef<std::path::Path>) -> Result<(), ConfigError> {
        let contents =
            toml::to_string_pretty(self).map_err(|e| ConfigError::SerializeError(e.to_string()))?;

        std::fs::write(path, contents).map_err(|e| ConfigError::FileWriteError(e.to_string()))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validation() {
        let mut config = Config::default();

        // Test invalid max_connections
        config.server.max_connections = 0;
        assert!(config.validate().is_err());

        // Test TLS validation
        config = Config::default();
        config.server.enable_tls = true;
        assert!(config.validate().is_err());

        // Test invalid bucket_size
        config = Config::default();
        config.dht.bucket_size = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_log_level_validation() {
        let mut config = Config::default();

        config.logging.level = "invalid".to_string();
        assert!(config.validate().is_err());

        config.logging.level = "debug".to_string();
        assert!(config.validate().is_ok());
    }
}
