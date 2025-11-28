//! Channel identity module
//!
//! This module handles per-channel pseudonymous identities for the p2p system.
//! Each channel is identified by its content hash rather than an ID.

use crate::core_identity::keys::Keypair;
use std::fmt;

/// A cryptographic hash representing a channel in the p2p system
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ChannelHash(pub Vec<u8>);

impl ChannelHash {
    pub fn new(bytes: Vec<u8>) -> Self {
        ChannelHash(bytes)
    }
    
    pub fn from_hex(hex: &str) -> Result<Self, String> {
        // TODO: Implement proper hex decoding
        Ok(ChannelHash(hex.as_bytes().to_vec()))
    }
    
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl fmt::Display for ChannelHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", String::from_utf8_lossy(&self.0))
    }
}

/// Identity for a specific channel, using content-addressable hasshing
pub struct ChannelIdentity {
    /// The content hash of the channel (p2p identifier)
    pub channel_hash: ChannelHash,
    /// Pseudonymous keypair for this channel
    pub keypair: Keypair,
    /// Optional nickname for this channel identity
    pub nickname: Option<String>,
}

impl ChannelIdentity {
    pub fn new(channel_hash: ChannelHash, nickname: Option<String>) -> Self {
        ChannelIdentity {
            channel_hash,
            keypair: Keypair::generate(),
            nickname,
        }
    }
}
