//! Error types for core_mvp module

use thiserror::Error;

/// Result type for MVP operations
pub type MvpResult<T> = Result<T, MvpError>;

/// Errors that can occur in MVP layer operations
#[derive(Error, Debug)]
pub enum MvpError {
    /// MLS operation failed
    #[error("MLS error: {0}")]
    Mls(#[from] crate::core_mls::errors::MlsError),

    /// Store operation failed
    #[error("Store error: {0}")]
    Store(String),

    /// DHT operation failed
    #[error("DHT error: {0}")]
    Dht(String),

    /// Channel not found
    #[error("Channel not found: {0}")]
    ChannelNotFound(String),

    /// Channel already exists
    #[error("Channel already exists: {0}")]
    ChannelExists(String),

    /// Member not found in channel
    #[error("Member not found in channel {channel}: {member}")]
    MemberNotFound { channel: String, member: String },

    /// Permission denied
    #[error("Permission denied: {user} cannot {action} in channel {channel}")]
    PermissionDenied { user: String, action: String, channel: String },

    /// Invalid invite token
    #[error("Invalid invite token: {0}")]
    InvalidInvite(String),

    /// Invalid message format
    #[error("Invalid message: {0}")]
    InvalidMessage(String),

    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// Serialization error (alias for network layer compatibility)
    #[error("Serialization error: {0}")]
    SerializationError(String),

    /// Invalid operation
    #[error("Invalid operation: {0}")]
    InvalidOperation(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    Config(String),

    /// Network error
    #[error("Network error: {0}")]
    NetworkError(String),

    /// Message not found
    #[error("Message not found: {0}")]
    MessageNotFound(String),

    /// Internal error (should not happen)
    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<serde_json::Error> for MvpError {
    fn from(e: serde_json::Error) -> Self {
        MvpError::Serialization(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = MvpError::ChannelNotFound("test-channel".to_string());
        assert_eq!(err.to_string(), "Channel not found: test-channel");

        let err = MvpError::PermissionDenied {
            user: "alice".to_string(),
            action: "delete_message".to_string(),
            channel: "general".to_string(),
        };
        assert!(err.to_string().contains("Permission denied"));
    }

    #[test]
    fn test_error_conversions() {
        let json_err = serde_json::from_str::<String>("invalid json").unwrap_err();
        let mvp_err: MvpError = json_err.into();
        assert!(matches!(mvp_err, MvpError::Serialization(_)));
    }
}
