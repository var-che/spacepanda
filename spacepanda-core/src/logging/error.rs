//! Error types for the logging subsystem

use std::fmt;

/// Errors that can occur in the logging subsystem
#[derive(Debug, Clone)]
pub enum LoggingError {
    /// Failed to initialize the logging system
    InitializationFailed(String),
    /// Invalid configuration provided
    InvalidConfiguration(String),
}

impl fmt::Display for LoggingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoggingError::InitializationFailed(msg) => {
                write!(f, "Failed to initialize logging: {}", msg)
            }
            LoggingError::InvalidConfiguration(msg) => {
                write!(f, "Invalid logging configuration: {}", msg)
            }
        }
    }
}

impl std::error::Error for LoggingError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_logging_error_display() {
        let err = LoggingError::InitializationFailed("test error".to_string());
        assert_eq!(
            format!("{}", err),
            "Failed to initialize logging: test error"
        );

        let err = LoggingError::InvalidConfiguration("bad config".to_string());
        assert_eq!(
            format!("{}", err),
            "Invalid logging configuration: bad config"
        );
    }

    #[test]
    fn test_logging_error_is_error_trait() {
        let err = LoggingError::InitializationFailed("test".to_string());
        let _: &dyn std::error::Error = &err;
    }
}
