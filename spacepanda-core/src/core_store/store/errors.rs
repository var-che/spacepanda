/*
    errors.rs - Error types for the store subsystem
    
    Defines all error types that can occur in:
    - CRDT operations
    - Storage operations
    - Validation
    - Synchronization
*/

use thiserror::Error;

/// Errors that can occur in the store subsystem
#[derive(Debug, Error)]
pub enum StoreError {
    /// CRDT operation failed
    #[error("CRDT error: {0}")]
    Crdt(String),
    
    /// Storage I/O error
    #[error("Storage error: {0}")]
    Storage(String),
    
    /// Validation failed
    #[error("Validation error: {0}")]
    Validation(String),
    
    /// Signature verification failed
    #[error("Signature verification failed: {0}")]
    SignatureVerification(String),
    
    /// Causal ordering violation
    #[error("Causal ordering violation: {0}")]
    CausalViolation(String),
    
    /// Entity not found
    #[error("Not found: {0}")]
    NotFound(String),
    
    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(String),
    
    /// Deserialization error
    #[error("Deserialization error: {0}")]
    Deserialization(String),
    
    /// Encryption error
    #[error("Encryption error: {0}")]
    Encryption(String),
    
    /// Encryption/Decryption error (alternative name for compatibility)
    #[error("Encryption error: {0}")]
    EncryptionError(String),
    
    /// Decryption error
    #[error("Decryption error: {0}")]
    Decryption(String),
    
    /// Corrupted data detected
    #[error("Corrupted data: {0}")]
    CorruptedData(String),
    
    /// Validation error (alternative name for compatibility)
    #[error("Validation error: {0}")]
    ValidationError(String),
    
    /// Permission denied
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
    
    /// MLS epoch mismatch
    #[error("MLS epoch mismatch: expected {expected}, got {actual}")]
    EpochMismatch { expected: u64, actual: u64 },
    
    /// Invalid operation
    #[error("Invalid operation: {0}")]
    InvalidOperation(String),
    
    /// DHT error
    #[error("DHT error: {0}")]
    Dht(String),
    
    /// Concurrent modification conflict
    #[error("Concurrent modification: {0}")]
    Conflict(String),
    
    /// Internal error
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Result type for store operations
pub type StoreResult<T> = Result<T, StoreError>;

/// Validation-specific errors
#[derive(Debug, Error)]
pub enum ValidationError {
    /// Missing required field
    #[error("Missing required field: {0}")]
    MissingField(String),
    
    /// Invalid field value
    #[error("Invalid field value: {field} - {reason}")]
    InvalidField { field: String, reason: String },
    
    /// Schema mismatch
    #[error("Schema mismatch: {0}")]
    SchemaMismatch(String),
    
    /// Duplicate entry
    #[error("Duplicate entry: {0}")]
    Duplicate(String),
}

impl From<ValidationError> for StoreError {
    fn from(err: ValidationError) -> Self {
        StoreError::Validation(err.to_string())
    }
}

/// CRDT-specific errors
#[derive(Debug, Error)]
pub enum CrdtError {
    /// Vector clock comparison failed
    #[error("Vector clock error: {0}")]
    VectorClock(String),
    
    /// Merge conflict that cannot be resolved
    #[error("Unresolvable merge conflict: {0}")]
    MergeConflict(String),
    
    /// Invalid operation for CRDT type
    #[error("Invalid CRDT operation: {0}")]
    InvalidOperation(String),
    
    /// Tombstone resurrection attempt
    #[error("Cannot resurrect tombstoned entry: {0}")]
    TombstoneViolation(String),
}

impl From<CrdtError> for StoreError {
    fn from(err: CrdtError) -> Self {
        StoreError::Crdt(err.to_string())
    }
}

impl From<std::io::Error> for StoreError {
    fn from(err: std::io::Error) -> Self {
        StoreError::Storage(err.to_string())
    }
}

impl From<bincode::Error> for StoreError {
    fn from(err: bincode::Error) -> Self {
        StoreError::Serialization(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_store_error_display() {
        let err = StoreError::NotFound("channel".to_string());
        assert_eq!(err.to_string(), "Not found: channel");
    }
    
    #[test]
    fn test_epoch_mismatch_error() {
        let err = StoreError::EpochMismatch {
            expected: 5,
            actual: 3,
        };
        assert!(err.to_string().contains("expected 5"));
        assert!(err.to_string().contains("got 3"));
    }
    
    #[test]
    fn test_validation_error_conversion() {
        let val_err = ValidationError::MissingField("name".to_string());
        let store_err: StoreError = val_err.into();
        assert!(matches!(store_err, StoreError::Validation(_)));
    }
    
    #[test]
    fn test_crdt_error_conversion() {
        let crdt_err = CrdtError::VectorClock("clock comparison failed".to_string());
        let store_err: StoreError = crdt_err.into();
        assert!(matches!(store_err, StoreError::Crdt(_)));
    }
    
    #[test]
    fn test_validation_error_types() {
        let err1 = ValidationError::MissingField("test".to_string());
        assert!(err1.to_string().contains("Missing required field"));
        
        let err2 = ValidationError::InvalidField {
            field: "age".to_string(),
            reason: "negative value".to_string(),
        };
        assert!(err2.to_string().contains("Invalid field value"));
        assert!(err2.to_string().contains("age"));
    }
}
