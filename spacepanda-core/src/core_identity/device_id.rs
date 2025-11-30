//! Device ID module
//!
//! Uniquely identify each device under the same user.

use blake2::{Blake2b512, Digest};
use serde::{Deserialize, Serialize};
use std::fmt;

/// DeviceId uniquely identifies a device (unique per device)
/// Derived from device public key or random bytes
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DeviceId(Vec<u8>);

impl DeviceId {
    /// Create a DeviceId from a device public key
    pub fn from_pubkey(pubkey: &[u8]) -> Self {
        let mut hasher = Blake2b512::new();
        hasher.update(pubkey);
        let hash = hasher.finalize();
        // Take first 16 bytes for device ID
        DeviceId(hash[0..16].to_vec())
    }

    /// Create a DeviceId from random bytes
    pub fn generate() -> Self {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let bytes: Vec<u8> = (0..16).map(|_| rng.random()).collect();
        DeviceId(bytes)
    }

    /// Create a DeviceId from raw bytes
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        DeviceId(bytes)
    }

    /// Get the raw bytes
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// Convert to hex string for display
    pub fn to_string(&self) -> String {
        hex::encode(&self.0)
    }

    /// Parse from hex string
    pub fn from_string(s: &str) -> Result<Self, String> {
        hex::decode(s)
            .map(DeviceId)
            .map_err(|e| format!("Invalid hex: {}", e))
    }
}

impl fmt::Display for DeviceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_id_from_pubkey_deterministic() {
        let pubkey = vec![1, 2, 3, 4, 5];
        let id1 = DeviceId::from_pubkey(&pubkey);
        let id2 = DeviceId::from_pubkey(&pubkey);
        assert_eq!(id1, id2);
        assert_eq!(id1.as_bytes().len(), 16);
    }

    #[test]
    fn test_device_id_generate_unique() {
        let id1 = DeviceId::generate();
        let id2 = DeviceId::generate();
        assert_ne!(id1, id2);
        assert_eq!(id1.as_bytes().len(), 16);
    }

    #[test]
    fn test_device_id_string_roundtrip() {
        let pubkey = vec![1, 2, 3, 4, 5];
        let id = DeviceId::from_pubkey(&pubkey);
        let s = id.to_string();
        let parsed = DeviceId::from_string(&s).unwrap();
        assert_eq!(id, parsed);
    }
}
