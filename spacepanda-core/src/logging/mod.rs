//! Logging subsystem for SpacePanda
//!
//! This module provides a unified logging interface using the `tracing` crate.
//! It supports different log levels and can be configured for various output formats.

use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

mod error;
mod level;

pub use error::LoggingError;
pub use level::LogLevel;

/// Configuration for the logging subsystem
#[derive(Debug, Clone)]
pub struct LogConfig {
    /// The minimum log level to display
    pub level: LogLevel,
    /// Whether to include timestamps
    pub with_timestamp: bool,
    /// Whether to include target module information
    pub with_target: bool,
    /// Whether to use JSON formatting
    pub json_format: bool,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            level: LogLevel::Info,
            with_timestamp: true,
            with_target: true,
            json_format: false,
        }
    }
}

impl LogConfig {
    /// Create a new LogConfig with specified level
    pub fn new(level: LogLevel) -> Self {
        Self {
            level,
            ..Default::default()
        }
    }

    /// Set whether to include timestamps
    pub fn with_timestamp(mut self, enabled: bool) -> Self {
        self.with_timestamp = enabled;
        self
    }

    /// Set whether to include target information
    pub fn with_target(mut self, enabled: bool) -> Self {
        self.with_target = enabled;
        self
    }

    /// Set whether to use JSON formatting
    pub fn json_format(mut self, enabled: bool) -> Self {
        self.json_format = enabled;
        self
    }
}

/// Initialize the logging subsystem with default configuration
///
/// # Example
/// ```
/// use spacepanda_core::logging::init_logging;
///
/// init_logging().expect("Failed to initialize logging");
/// ```
pub fn init_logging() -> Result<(), LoggingError> {
    init_logging_with_config(LogConfig::default())
}

/// Initialize the logging subsystem with custom configuration
///
/// # Example
/// ```
/// use spacepanda_core::logging::{init_logging_with_config, LogConfig, LogLevel};
///
/// let config = LogConfig::new(LogLevel::Debug)
///     .with_timestamp(true)
///     .with_target(false);
///
/// init_logging_with_config(config).expect("Failed to initialize logging");
/// ```
pub fn init_logging_with_config(config: LogConfig) -> Result<(), LoggingError> {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(config.level.as_str()));

    let fmt_layer = fmt::layer()
        .with_target(config.with_target)
        .with_timer(if config.with_timestamp {
            fmt::time::time()
        } else {
            fmt::time::time()
        });

    if config.json_format {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt_layer.json())
            .try_init()
            .map_err(|e| LoggingError::InitializationFailed(e.to_string()))?;
    } else {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt_layer)
            .try_init()
            .map_err(|e| LoggingError::InitializationFailed(e.to_string()))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracing::{debug, error, info, trace, warn};

    #[test]
    fn test_log_config_default() {
        let config = LogConfig::default();
        assert!(matches!(config.level, LogLevel::Info));
        assert!(config.with_timestamp);
        assert!(config.with_target);
        assert!(!config.json_format);
    }

    #[test]
    fn test_log_config_builder() {
        let config = LogConfig::new(LogLevel::Debug)
            .with_timestamp(false)
            .with_target(false)
            .json_format(true);

        assert!(matches!(config.level, LogLevel::Debug));
        assert!(!config.with_timestamp);
        assert!(!config.with_target);
        assert!(config.json_format);
    }

    #[test]
    fn test_logging_levels() {
        // This test verifies that we can create different log levels
        let levels = vec![
            LogLevel::Trace,
            LogLevel::Debug,
            LogLevel::Info,
            LogLevel::Warn,
            LogLevel::Error,
        ];

        for level in levels {
            let config = LogConfig::new(level.clone());
            assert_eq!(config.level.as_str(), level.as_str());
        }
    }

    #[test]
    fn test_log_level_conversion() {
        assert_eq!(LogLevel::Trace.as_str(), "trace");
        assert_eq!(LogLevel::Debug.as_str(), "debug");
        assert_eq!(LogLevel::Info.as_str(), "info");
        assert_eq!(LogLevel::Warn.as_str(), "warn");
        assert_eq!(LogLevel::Error.as_str(), "error");
    }

    // Note: We can't easily test actual logging output without capturing stdout,
    // but we can test that the initialization doesn't panic
    #[test]
    fn test_logging_macros_compile() {
        // This test just ensures the logging macros compile correctly
        // The actual output would need runtime initialization
        let _guard = || {
            trace!("This is a trace message");
            debug!("This is a debug message");
            info!("This is an info message");
            warn!("This is a warning message");
            error!("This is an error message");
        };
    }
}
