//! User ID module
//!
//! Defines the stable global identity identifier derived from long-term public key.

use blake3;
use serde::{Deserialize, Serialize};
use std::fmt;

/// UserId is a deterministic stable ID for a user (derived from long-term public key)
/// Uses BLAKE2b hash truncated to 32 bytes
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UserId(Vec<u8>);

impl UserId {
    /// Create a UserId from a public key by hashing it
    pub fn from_public_key(pubkey: &[u8]) -> Self {
        let hash = blake3::hash(pubkey);
        // Take full 32 bytes
        UserId(hash.as_bytes().to_vec())
    }

    /// Create a UserId from raw bytes (for deserialization)
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        UserId(bytes)
    }

    /// Get the raw bytes
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// Convert to base58 string for display
    pub fn to_string(&self) -> String {
        bs58::encode(&self.0).into_string()
    }

    /// Parse from base58 string
    pub fn from_string(s: &str) -> Result<Self, String> {
        bs58::decode(s)
            .into_vec()
            .map(UserId)
            .map_err(|e| format!("Invalid base58: {}", e))
    }
}

impl fmt::Display for UserId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_id_derivation_is_deterministic() {
        let pubkey = vec![1, 2, 3, 4, 5];
        let id1 = UserId::from_public_key(&pubkey);
        let id2 = UserId::from_public_key(&pubkey);
        assert_eq!(id1, id2);
        assert_eq!(id1.as_bytes().len(), 32);
    }

    #[test]
    fn test_user_id_different_keys() {
        let pubkey1 = vec![1, 2, 3, 4, 5];
        let pubkey2 = vec![5, 4, 3, 2, 1];
        let id1 = UserId::from_public_key(&pubkey1);
        let id2 = UserId::from_public_key(&pubkey2);
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_user_id_string_roundtrip() {
        let pubkey = vec![1, 2, 3, 4, 5];
        let id = UserId::from_public_key(&pubkey);
        let s = id.to_string();
        let parsed = UserId::from_string(&s).unwrap();
        assert_eq!(id, parsed);
    }
}
