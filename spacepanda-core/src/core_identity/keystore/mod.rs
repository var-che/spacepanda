//! Keystore module
//!
//! Abstract keystore API for storing and loading cryptographic keys.

use crate::core_identity::device_id::DeviceId;
use crate::core_identity::keypair::Keypair;
use thiserror::Error;

pub mod file_keystore;
pub mod memory_keystore;

/// Keystore errors
#[derive(Debug, Error)]
pub enum KeystoreError {
    #[error("Key not found: {0}")]
    NotFound(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Encryption error: {0}")]
    Encryption(String),

    #[error("Decryption error: {0}")]
    Decryption(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Invalid password")]
    InvalidPassword,
    
    #[error("Other error: {0}")]
    Other(String),
}

/// Abstract keystore trait
pub trait Keystore: Send + Sync {
    /// Load identity keypair
    fn load_identity_keypair(&self) -> Result<Keypair, KeystoreError>;

    /// Save identity keypair
    fn save_identity_keypair(&self, kp: &Keypair) -> Result<(), KeystoreError>;

    /// Load device keypair
    fn load_device_keypair(&self, device_id: &DeviceId) -> Result<Keypair, KeystoreError>;

    /// Save device keypair
    fn save_device_keypair(&self, device_id: &DeviceId, kp: &Keypair)
        -> Result<(), KeystoreError>;

    /// List all device IDs
    fn list_devices(&self) -> Result<Vec<DeviceId>, KeystoreError>;

    /// Rotate master key (optional, for password change)
    fn rotate_master_key(&self, password: &str) -> Result<(), KeystoreError> {
        let _ = password;
        Err(KeystoreError::Encryption(
            "Not implemented".to_string(),
        ))
    }
}
