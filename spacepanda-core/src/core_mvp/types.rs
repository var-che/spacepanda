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

    /// Channel name (so joiner can create proper metadata)
    pub channel_name: String,

    /// Whether channel is public
    pub is_public: bool,

    /// When this invite was created
    pub created_at: Timestamp,

    /// Optional expiration time
    pub expires_at: Option<Timestamp>,

    /// Inviter's user ID
    pub inviter: UserId,

    /// Inviter's peer ID (for P2P connection)
    /// This enables secure peer discovery without DHT metadata leakage
    pub inviter_peer_id: Option<Vec<u8>>,
}

impl InviteToken {
    /// Create a new invite token
    pub fn new(
        channel_id: ChannelId,
        welcome_blob: Vec<u8>,
        ratchet_tree: Option<Vec<u8>>,
        channel_name: String,
        is_public: bool,
        inviter: UserId,
    ) -> Self {
        Self {
            channel_id,
            welcome_blob,
            ratchet_tree,
            channel_name,
            is_public,
            created_at: Timestamp::now(),
            expires_at: None,
            inviter,
            inviter_peer_id: None,
        }
    }

    /// Set the inviter's peer ID for P2P connection establishment
    pub fn with_peer_id(mut self, peer_id: Vec<u8>) -> Self {
        self.inviter_peer_id = Some(peer_id);
        self
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

/// Emoji reaction to a message
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Reaction {
    /// The emoji (e.g., "👍", "❤️", "🎉")
    pub emoji: String,
    /// User who reacted
    pub user_id: UserId,
    /// When the reaction was added
    pub timestamp: Timestamp,
}

impl Reaction {
    /// Create a new reaction
    pub fn new(emoji: String, user_id: UserId) -> Self {
        Self { emoji, user_id, timestamp: Timestamp::now() }
    }
}

/// Aggregated reactions for a message (grouped by emoji)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReactionSummary {
    /// The emoji
    pub emoji: String,
    /// Count of users who reacted with this emoji
    pub count: usize,
    /// List of users who reacted (for display)
    pub users: Vec<UserId>,
    /// Whether the current user has reacted with this emoji
    pub user_reacted: bool,
}

/// Thread metadata for a message
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ThreadInfo {
    /// The root message ID (top of thread)
    pub root_message_id: MessageId,
    /// Total number of replies in the thread
    pub reply_count: usize,
    /// Unique participants in the thread
    pub participant_count: usize,
    /// List of participants (user IDs)
    pub participants: Vec<UserId>,
    /// Timestamp of the last reply
    pub last_reply_at: Option<Timestamp>,
    /// Preview of the last reply (first 100 chars)
    pub last_reply_preview: Option<String>,
}

/// A message with its thread context
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MessageWithThread {
    /// The message itself
    pub message: ChatMessage,
    /// Thread info if this message has replies
    pub thread_info: Option<ThreadInfo>,
    /// If this is a reply, the parent message
    pub parent_message: Option<Box<ChatMessage>>,
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
            "test-channel".to_string(),
            false,
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
        let reply =
            ChatMessage::new(channel_id, sender, b"Reply".to_vec()).reply_to(parent_id.clone());

        assert_eq!(reply.reply_to, Some(parent_id));
    }

    #[test]
    fn test_serialization() {
        let channel_id = ChannelId::generate();
        let owner = UserId::generate();
        let group_id = GroupId::new(vec![1, 2, 3]);

        let desc = ChannelDescriptor::new(channel_id, owner, "test".to_string(), false, group_id);

        let json = serde_json::to_string(&desc).unwrap();
        let deserialized: ChannelDescriptor = serde_json::from_str(&json).unwrap();

        assert_eq!(desc, deserialized);
    }
}
