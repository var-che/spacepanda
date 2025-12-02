//! Error types for MLS operations

use thiserror::Error;

/// Result type for MLS operations
pub type MlsResult<T> = Result<T, MlsError>;

/// Errors that can occur in MLS operations
#[derive(Debug, Error)]
pub enum MlsError {
    /// OpenMLS library error
    #[error("OpenMLS error: {0}")]
    OpenMls(String),

    /// Storage operation failed
    #[error("Storage error: {0}")]
    Storage(String),

    /// Serialization/deserialization error
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// Invalid credential
    #[error("Invalid credential: {0}")]
    InvalidCredential(String),

    /// Group not found
    #[error("Group not found: {0}")]
    GroupNotFound(String),

    /// Member not found
    #[error("Member not found: {0}")]
    MemberNotFound(String),

    /// Invalid group state
    #[error("Invalid group state: {0}")]
    InvalidGroupState(String),

    /// Unauthorized operation
    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    /// Key package error
    #[error("Key package error: {0}")]
    KeyPackage(String),

    /// Message processing error
    #[error("Message processing error: {0}")]
    MessageProcessing(String),

    /// Cryptographic operation failed
    #[error("Crypto error: {0}")]
    Crypto(String),

    /// Invalid configuration
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    /// Internal error
    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<openmls::prelude::LibraryError> for MlsError {
    fn from(e: openmls::prelude::LibraryError) -> Self {
        MlsError::OpenMls(e.to_string())
    }
}

impl From<openmls::prelude::KeyPackageNewError> for MlsError {
    fn from(e: openmls::prelude::KeyPackageNewError) -> Self {
        MlsError::KeyPackage(e.to_string())
    }
}

// Note: Provider-specific error conversions will be handled contextually

impl From<serde_json::Error> for MlsError {
    fn from(e: serde_json::Error) -> Self {
        MlsError::Serialization(e.to_string())
    }
}

impl From<bincode::Error> for MlsError {
    fn from(e: bincode::Error) -> Self {
        MlsError::Serialization(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = MlsError::GroupNotFound("test_group".to_string());
        assert_eq!(err.to_string(), "Group not found: test_group");
    }

    #[test]
    fn test_error_conversion() {
        let json_err = serde_json::from_str::<i32>("invalid").unwrap_err();
        let mls_err: MlsError = json_err.into();
        assert!(matches!(mls_err, MlsError::Serialization(_)));
    }
}
