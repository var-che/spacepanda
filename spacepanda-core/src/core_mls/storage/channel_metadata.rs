//! Privacy-Focused Channel Metadata Storage
//!
//! Design Principles:
//! 1. Minimize metadata exposure - store only what's essential
//! 2. Encrypt sensitive fields - names, topics, member lists
//! 3. No timestamps beyond creation (prevents traffic analysis)
//! 4. No read receipts or typing indicators in storage
//! 5. Member identities stored as hashes, not plaintext
//!
//! What we DON'T store:
//! - Last read timestamps
//! - Typing indicators
//! - Read receipts
//! - Online/offline status
//! - Message delivery timestamps
//! - IP addresses or network metadata

use crate::core_mls::errors::{MlsError, MlsResult};
use serde::{Deserialize, Serialize};

/// Minimal channel metadata for persistence
///
/// Privacy notes:
/// - Only group_id is indexed (for lookup)
/// - All other fields are encrypted blobs
/// - No timing metadata beyond creation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelMetadata {
    /// MLS group ID (not encrypted - needed for lookups)
    pub group_id: Vec<u8>,

    /// Encrypted channel name
    /// Only decryptable by group members with MLS key
    pub encrypted_name: Vec<u8>,

    /// Encrypted channel topic/description
    pub encrypted_topic: Vec<u8>,

    /// Creation timestamp (Unix epoch seconds)
    /// Note: Only creation time, no "last activity" to prevent traffic analysis
    pub created_at: i64,

    /// Encrypted member list
    /// Stored as encrypted blob, not individual rows (prevents membership analysis)
    pub encrypted_members: Vec<u8>,

    /// Channel type (0=private, 1=group, 2=public)
    /// Not encrypted as it's structural metadata
    pub channel_type: u8,

    /// Whether this is an archived channel (local only, not synced)
    pub archived: bool,
}

impl ChannelMetadata {
    /// Create new channel metadata
    pub fn new(
        group_id: Vec<u8>,
        encrypted_name: Vec<u8>,
        encrypted_topic: Vec<u8>,
        created_at: i64,
        encrypted_members: Vec<u8>,
        channel_type: u8,
    ) -> Self {
        Self {
            group_id,
            encrypted_name,
            encrypted_topic,
            created_at,
            encrypted_members,
            channel_type,
            archived: false,
        }
    }
}

/// Minimal message metadata for history
///
/// Privacy notes:
/// - Messages are encrypted end-to-end
/// - Only stores sender hash (not plaintext identity)
/// - Sequence number for ordering, not timestamps
/// - No read receipts or delivery confirmations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageMetadata {
    /// Message ID (hash of content + sequence)
    pub message_id: Vec<u8>,

    /// Group ID this message belongs to
    pub group_id: Vec<u8>,

    /// Encrypted message content
    pub encrypted_content: Vec<u8>,

    /// Sender identity hash (not plaintext)
    pub sender_hash: Vec<u8>,

    /// Sequence number (for ordering, not timing)
    pub sequence: i64,

    /// Local-only: whether this device has processed this message
    /// (NOT synced - prevents correlation attacks)
    pub processed: bool,
}

impl MessageMetadata {
    /// Create new message metadata
    pub fn new(
        message_id: Vec<u8>,
        group_id: Vec<u8>,
        encrypted_content: Vec<u8>,
        sender_hash: Vec<u8>,
        sequence: i64,
    ) -> Self {
        Self { message_id, group_id, encrypted_content, sender_hash, sequence, processed: false }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_channel_metadata_creation() {
        let metadata = ChannelMetadata::new(
            b"group123".to_vec(),
            b"encrypted_name".to_vec(),
            b"encrypted_topic".to_vec(),
            1234567890,
            b"encrypted_members".to_vec(),
            1, // group type
        );

        assert_eq!(metadata.group_id, b"group123");
        assert_eq!(metadata.channel_type, 1);
        assert!(!metadata.archived);
    }

    #[test]
    fn test_message_metadata_creation() {
        let metadata = MessageMetadata::new(
            b"msg123".to_vec(),
            b"group123".to_vec(),
            b"encrypted_content".to_vec(),
            b"sender_hash".to_vec(),
            1,
        );

        assert_eq!(metadata.sequence, 1);
        assert!(!metadata.processed);
    }
}
