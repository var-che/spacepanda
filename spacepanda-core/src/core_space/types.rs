//! Type definitions for Spaces and Channels

use serde::{Deserialize, Serialize};
use std::fmt;

/// Unique identifier for a Space
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SpaceId(pub [u8; 32]);

impl SpaceId {
    /// Create a new random SpaceId
    pub fn generate() -> Self {
        use rand::RngCore;
        let mut id = [0u8; 32];
        rand::rng().fill_bytes(&mut id);
        SpaceId(id)
    }

    /// Create SpaceId from bytes
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        SpaceId(bytes)
    }

    /// Get bytes representation
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Display for SpaceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", hex::encode(self.0))
    }
}

impl From<[u8; 32]> for SpaceId {
    fn from(bytes: [u8; 32]) -> Self {
        SpaceId(bytes)
    }
}

/// Unique identifier for a Channel
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChannelId(pub [u8; 32]);

impl ChannelId {
    /// Create a new random ChannelId
    pub fn generate() -> Self {
        use rand::RngCore;
        let mut id = [0u8; 32];
        rand::rng().fill_bytes(&mut id);
        ChannelId(id)
    }

    /// Create ChannelId from bytes
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        ChannelId(bytes)
    }

    /// Get bytes representation
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Display for ChannelId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", hex::encode(self.0))
    }
}

impl From<[u8; 32]> for ChannelId {
    fn from(bytes: [u8; 32]) -> Self {
        ChannelId(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_space_id_generation() {
        let id1 = SpaceId::generate();
        let id2 = SpaceId::generate();
        assert_ne!(id1, id2, "Generated IDs should be unique");
    }

    #[test]
    fn test_space_id_round_trip() {
        let original = SpaceId::generate();
        let bytes = *original.as_bytes();
        let restored = SpaceId::from_bytes(bytes);
        assert_eq!(original, restored);
    }

    #[test]
    fn test_channel_id_generation() {
        let id1 = ChannelId::generate();
        let id2 = ChannelId::generate();
        assert_ne!(id1, id2, "Generated IDs should be unique");
    }

    #[test]
    fn test_channel_id_display() {
        let id = ChannelId::from_bytes([0xAB; 32]);
        let display = format!("{}", id);
        assert_eq!(display.len(), 64); // 32 bytes * 2 hex chars
        assert!(display.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
