//! Error types for MLS operations

use thiserror::Error;

/// Result type for MLS operations
pub type MlsResult<T> = Result<T, MlsError>;

/// Errors that can occur in MLS operations
#[derive(Debug, Error)]
pub enum MlsError {
    /// Invalid MLS message format
    #[error("Invalid message: {0}")]
    InvalidMessage(String),

    /// Signature verification failed
    #[error("Verification failed: {0}")]
    VerifyFailed(String),

    /// Replay attack detected
    #[error("Replay detected: {0}")]
    ReplayDetected(String),

    /// Epoch mismatch (too old or too new)
    #[error("Epoch mismatch: expected {expected}, got {actual}")]
    EpochMismatch { expected: u64, actual: u64 },

    /// Persistence/storage error
    #[error("Persistence error: {0}")]
    PersistenceError(String),

    /// Unauthorized operation
    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    /// Cryptographic operation failed
    #[error("Crypto error: {0}")]
    CryptoError(String),

    /// Group not found
    #[error("Group not found: {0}")]
    GroupNotFound(String),

    /// Member not found
    #[error("Member not found: {0}")]
    MemberNotFound(String),

    /// Invalid group state
    #[error("Invalid group state: {0}")]
    InvalidState(String),

    /// Invalid proposal
    #[error("Invalid proposal: {0}")]
    InvalidProposal(String),

    /// Configuration error
    #[error("Invalid config: {0}")]
    InvalidConfig(String),

    /// Serialization/deserialization error
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// OpenMLS library error
    #[error("OpenMLS error: {0}")]
    OpenMls(String),

    /// Internal error (bug)
    #[error("Internal error: {0}")]
    Internal(String),

    /// Storage operation failed
    #[error("Storage error: {0}")]
    Storage(String),

    /// Item not found
    #[error("Not found: {0}")]
    NotFound(String),

    /// Permission denied
    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    /// Invalid input
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    /// Other error
    #[error("Other: {0}")]
    Other(String),
}

impl From<openmls::prelude::LibraryError> for MlsError {
    fn from(e: openmls::prelude::LibraryError) -> Self {
        MlsError::OpenMls(e.to_string())
    }
}

impl From<openmls::prelude::KeyPackageNewError> for MlsError {
    fn from(e: openmls::prelude::KeyPackageNewError) -> Self {
        MlsError::OpenMls(e.to_string())
    }
}

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

impl From<std::io::Error> for MlsError {
    fn from(e: std::io::Error) -> Self {
        MlsError::PersistenceError(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = MlsError::GroupNotFound("test_group".to_string());
        assert_eq!(err.to_string(), "Group not found: test_group");

        let err = MlsError::EpochMismatch {
            expected: 5,
            actual: 3,
        };
        assert_eq!(err.to_string(), "Epoch mismatch: expected 5, got 3");
    }

    #[test]
    fn test_error_conversion() {
        let json_err = serde_json::from_str::<i32>("invalid").unwrap_err();
        let mls_err: MlsError = json_err.into();
        assert!(matches!(mls_err, MlsError::Serialization(_)));
    }

    #[test]
    fn test_io_error_conversion() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let mls_err: MlsError = io_err.into();
        assert!(matches!(mls_err, MlsError::PersistenceError(_)));
    }
}
