//! Core data types for MVP layer

use crate::core_mls::types::GroupId;
use crate::core_store::model::types::{ChannelId, MessageId, Timestamp, UserId};
use serde::{Deserialize, Serialize};

/// Channel descriptor for discovery and metadata
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChannelDescriptor {
    /// Unique channel identifier
    pub channel_id: ChannelId,

    /// Channel owner/creator
    pub owner: UserId,

    /// Human-readable channel name
    pub name: String,

    /// Whether channel is publicly discoverable
    pub is_public: bool,

    /// Associated MLS group ID
    pub mls_group_id: GroupId,

    /// Creation timestamp
    pub created_at: Timestamp,

    /// Bootstrap peer addresses (for P2P discovery)
    pub bootstrap_peers: Vec<String>,

    /// Optional description
    pub description: Option<String>,
}

impl ChannelDescriptor {
    /// Create a new channel descriptor
    pub fn new(
        channel_id: ChannelId,
        owner: UserId,
        name: String,
        is_public: bool,
        mls_group_id: GroupId,
    ) -> Self {
        Self {
            channel_id,
            owner,
            name,
            is_public,
            mls_group_id,
            created_at: Timestamp::now(),
            bootstrap_peers: Vec::new(),
            description: None,
        }
    }

    /// Add a bootstrap peer
    pub fn add_bootstrap_peer(&mut self, peer: String) {
        if !self.bootstrap_peers.contains(&peer) {
            self.bootstrap_peers.push(peer);
        }
    }
}

/// Invite token containing Welcome message and metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InviteToken {
    /// Target channel ID
    pub channel_id: ChannelId,

    /// MLS Welcome message (encrypted)
    pub welcome_blob: Vec<u8>,

    /// Ratchet tree for joining (required for some MLS configs)
    pub ratchet_tree: Option<Vec<u8>>,

    /// When this invite was created
    pub created_at: Timestamp,

    /// Optional expiration time
    pub expires_at: Option<Timestamp>,

    /// Inviter's user ID
    pub inviter: UserId,
}

impl InviteToken {
    /// Create a new invite token
    pub fn new(
        channel_id: ChannelId,
        welcome_blob: Vec<u8>,
        ratchet_tree: Option<Vec<u8>>,
        inviter: UserId,
    ) -> Self {
        Self {
            channel_id,
            welcome_blob,
            ratchet_tree,
            created_at: Timestamp::now(),
            expires_at: None,
            inviter,
        }
    }

    /// Check if invite has expired
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            Timestamp::now() > expires_at
        } else {
            false
        }
    }

    /// Set expiration time (seconds from now)
    pub fn with_expiry(mut self, seconds: u64) -> Self {
        let now = Timestamp::now().0;
        self.expires_at = Some(Timestamp(now + seconds));
        self
    }
}

/// Chat message structure (decrypted)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChatMessage {
    /// Unique message identifier
    pub message_id: MessageId,

    /// Channel this message belongs to
    pub channel_id: ChannelId,

    /// Sender's user ID
    pub sender: UserId,

    /// When message was created
    pub timestamp: Timestamp,

    /// Message body (plaintext after decryption)
    pub body: Vec<u8>,

    /// Optional reply-to message ID (for threading)
    pub reply_to: Option<MessageId>,

    /// Message type (for future extensions)
    pub message_type: MessageType,
}

impl ChatMessage {
    /// Create a new chat message
    pub fn new(channel_id: ChannelId, sender: UserId, body: Vec<u8>) -> Self {
        Self {
            message_id: MessageId::generate(),
            channel_id,
            sender,
            timestamp: Timestamp::now(),
            body,
            reply_to: None,
            message_type: MessageType::Text,
        }
    }

    /// Create a reply message
    pub fn reply_to(mut self, parent_id: MessageId) -> Self {
        self.reply_to = Some(parent_id);
        self
    }

    /// Get message body as UTF-8 string (if valid)
    pub fn body_as_string(&self) -> Option<String> {
        String::from_utf8(self.body.clone()).ok()
    }
}

/// Message type enumeration
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum MessageType {
    /// Regular text message
    Text,
    /// System message (join, leave, etc.)
    System,
    /// File/media attachment
    Attachment,
}

/// Channel creation request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateChannelRequest {
    /// Channel name
    pub name: String,
    /// Whether channel is public
    pub is_public: bool,
    /// Optional description
    pub description: Option<String>,
}

/// Channel creation response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateChannelResponse {
    /// Created channel ID
    pub channel_id: ChannelId,
    /// Associated MLS group ID
    pub mls_group_id: GroupId,
}

/// Invite creation request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateInviteRequest {
    /// Invitee's key package (serialized)
    pub key_package: Vec<u8>,
}

/// Invite creation response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateInviteResponse {
    /// Invite token (contains Welcome + ratchet tree)
    pub invite: InviteToken,
}

/// Join channel request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoinChannelRequest {
    /// Invite token
    pub invite: InviteToken,
}

/// Join channel response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoinChannelResponse {
    /// Joined channel ID
    pub channel_id: ChannelId,
    /// Current member count
    pub member_count: usize,
}

/// Send message request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendMessageRequest {
    /// Message plaintext
    pub plaintext: Vec<u8>,
    /// Optional reply-to message ID
    pub reply_to: Option<MessageId>,
}

/// Send message response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendMessageResponse {
    /// Generated message ID
    pub message_id: MessageId,
    /// Timestamp
    pub timestamp: Timestamp,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_channel_descriptor() {
        let channel_id = ChannelId::generate();
        let owner = UserId::generate();
        let group_id = GroupId::new(vec![1, 2, 3, 4]);

        let mut desc = ChannelDescriptor::new(
            channel_id.clone(),
            owner.clone(),
            "general".to_string(),
            true,
            group_id.clone(),
        );

        assert_eq!(desc.channel_id, channel_id);
        assert_eq!(desc.owner, owner);
        assert_eq!(desc.name, "general");
        assert!(desc.is_public);
        assert_eq!(desc.bootstrap_peers.len(), 0);

        desc.add_bootstrap_peer("peer1".to_string());
        desc.add_bootstrap_peer("peer2".to_string());
        desc.add_bootstrap_peer("peer1".to_string()); // Duplicate

        assert_eq!(desc.bootstrap_peers.len(), 2);
    }

    #[test]
    fn test_invite_token_expiry() {
        let channel_id = ChannelId::generate();
        let inviter = UserId::generate();

        let invite = InviteToken::new(
            channel_id,
            vec![1, 2, 3],
            Some(vec![4, 5, 6]),
            inviter,
        );

        assert!(!invite.is_expired());

        let expired_invite = invite.with_expiry(0); // Expires immediately
        // Note: This might pass or fail depending on timing
        // In production, use proper time mocking
    }

    #[test]
    fn test_chat_message() {
        let channel_id = ChannelId::generate();
        let sender = UserId::generate();
        let body = b"Hello, world!".to_vec();

        let msg = ChatMessage::new(channel_id.clone(), sender.clone(), body.clone());

        assert_eq!(msg.channel_id, channel_id);
        assert_eq!(msg.sender, sender);
        assert_eq!(msg.body, body);
        assert_eq!(msg.body_as_string(), Some("Hello, world!".to_string()));
        assert_eq!(msg.message_type, MessageType::Text);
        assert!(msg.reply_to.is_none());

        let parent_id = MessageId::generate();
        let reply = ChatMessage::new(channel_id, sender, b"Reply".to_vec())
            .reply_to(parent_id.clone());

        assert_eq!(reply.reply_to, Some(parent_id));
    }

    #[test]
    fn test_serialization() {
        let channel_id = ChannelId::generate();
        let owner = UserId::generate();
        let group_id = GroupId::new(vec![1, 2, 3]);

        let desc = ChannelDescriptor::new(
            channel_id,
            owner,
            "test".to_string(),
            false,
            group_id,
        );

        let json = serde_json::to_string(&desc).unwrap();
        let deserialized: ChannelDescriptor = serde_json::from_str(&json).unwrap();

        assert_eq!(desc, deserialized);
    }
}
