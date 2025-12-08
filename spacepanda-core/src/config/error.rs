//! Configuration error types

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Failed to read configuration file: {0}")]
    FileReadError(String),

    #[error("Failed to write configuration file: {0}")]
    FileWriteError(String),

    #[error("Failed to parse configuration: {0}")]
    ParseError(String),

    #[error("Failed to serialize configuration: {0}")]
    SerializeError(String),

    #[error("Invalid configuration value: {0}")]
    InvalidValue(String),

    #[error("Configuration validation failed: {0}")]
    ValidationFailed(String),
}
