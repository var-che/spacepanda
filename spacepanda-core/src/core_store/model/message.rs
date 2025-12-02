/*
    message.rs - Message model with MLS encryption
    
    Represents a single message in a channel.
    Content is encrypted via MLS; metadata is in the clear.
    
    Security model:
    - message_id: unique identifier (hash of content + timestamp)
    - sender: authenticated via signature
    - content: MLS ciphertext (encrypted message body)
    - timestamp: when message was created
    - reply_to: optional thread/reply reference
    - attachments: metadata about files (actual files stored separately)
    - reactions: emoji reactions (CRDT OR-Map)
*/

use super::types::{MessageId, ChannelId, UserId, Timestamp};
use crate::core_store::crdt::ORMap;
use serde::{Deserialize, Serialize};

/// Attachment metadata (actual file stored in blob store)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Attachment {
    /// Unique ID for this attachment
    pub id: String,
    /// Original filename
    pub filename: String,
    /// MIME type
    pub mime_type: String,
    /// Size in bytes
    pub size_bytes: u64,
    /// Hash of encrypted content (for verification)
    pub content_hash: String,
}

/// Message in a channel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Unique message ID
    pub id: MessageId,
    
    /// Channel this message belongs to
    pub channel_id: ChannelId,
    
    /// User who sent this message
    pub sender: UserId,
    
    /// MLS-encrypted message content
    /// This is a ciphertext blob that can only be decrypted by group members
    pub content: Vec<u8>,
    
    /// When the message was created (sender's timestamp)
    pub timestamp: Timestamp,
    
    /// Optional reply-to message ID (for threading)
    pub reply_to: Option<MessageId>,
    
    /// Optional attachments (encrypted separately)
    pub attachments: Vec<Attachment>,
    
    /// Emoji reactions: emoji -> set of user IDs who reacted
    /// Uses OR-Map to handle concurrent reactions
    pub reactions: ORMap<String, ORSet<UserId>>,
    
    /// Edit history: tracks if message was edited
    /// Maps timestamp -> (editor_id, new_encrypted_content)
    pub edits: Vec<(Timestamp, UserId, Vec<u8>)>,
    
    /// Whether this message has been deleted
    pub deleted: bool,
}

/// For OR-Set of user IDs in reactions
use crate::core_store::crdt::ORSet;

impl Message {
    /// Create a new message
    pub fn new(
        id: MessageId,
        channel_id: ChannelId,
        sender: UserId,
        content: Vec<u8>,
        timestamp: Timestamp,
    ) -> Self {
        Message {
            id,
            channel_id,
            sender,
            content,
            timestamp,
            reply_to: None,
            attachments: Vec::new(),
            reactions: ORMap::new(),
            edits: Vec::new(),
            deleted: false,
        }
    }
    
    /// Create a message with a reply reference
    pub fn new_reply(
        id: MessageId,
        channel_id: ChannelId,
        sender: UserId,
        content: Vec<u8>,
        timestamp: Timestamp,
        reply_to: MessageId,
    ) -> Self {
        let mut msg = Self::new(id, channel_id, sender, content, timestamp);
        msg.reply_to = Some(reply_to);
        msg
    }
    
    /// Add an attachment
    pub fn add_attachment(&mut self, attachment: Attachment) {
        self.attachments.push(attachment);
    }
    
    /// Mark as deleted (tombstone)
    pub fn delete(&mut self) {
        self.deleted = true;
    }
    
    /// Add an edit
    pub fn add_edit(&mut self, timestamp: Timestamp, editor: UserId, new_content: Vec<u8>) {
        self.edits.push((timestamp, editor, new_content));
    }
    
    /// Get the current content (latest edit or original)
    pub fn current_content(&self) -> &[u8] {
        self.edits
            .last()
            .map(|(_, _, content)| content.as_slice())
            .unwrap_or(&self.content)
    }
    
    /// Check if message was edited
    pub fn is_edited(&self) -> bool {
        !self.edits.is_empty()
    }
    
    /// Get who last edited (or original sender)
    pub fn last_editor(&self) -> &UserId {
        self.edits
            .last()
            .map(|(_, editor, _)| editor)
            .unwrap_or(&self.sender)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_message_creation() {
        let msg_id = MessageId::generate();
        let channel_id = ChannelId::generate();
        let sender = UserId::generate();
        let content = b"Hello, world!".to_vec();
        let timestamp = Timestamp::now();
        
        let msg = Message::new(
            msg_id.clone(),
            channel_id.clone(),
            sender.clone(),
            content.clone(),
            timestamp,
        );
        
        assert_eq!(msg.id, msg_id);
        assert_eq!(msg.channel_id, channel_id);
        assert_eq!(msg.sender, sender);
        assert_eq!(msg.content, content);
        assert!(!msg.is_edited());
        assert!(!msg.deleted);
    }
    
    #[test]
    fn test_message_reply() {
        let msg_id = MessageId::generate();
        let channel_id = ChannelId::generate();
        let sender = UserId::generate();
        let content = b"Reply!".to_vec();
        let timestamp = Timestamp::now();
        let reply_to = MessageId::generate();
        
        let msg = Message::new_reply(
            msg_id,
            channel_id,
            sender,
            content,
            timestamp,
            reply_to.clone(),
        );
        
        assert_eq!(msg.reply_to, Some(reply_to));
    }
    
    #[test]
    fn test_message_edit() {
        let msg_id = MessageId::generate();
        let channel_id = ChannelId::generate();
        let sender = UserId::generate();
        let content = b"Original".to_vec();
        let timestamp = Timestamp::now();
        
        let mut msg = Message::new(msg_id, channel_id, sender.clone(), content, timestamp);
        
        assert!(!msg.is_edited());
        
        let new_content = b"Edited".to_vec();
        msg.add_edit(Timestamp::now(), sender.clone(), new_content.clone());
        
        assert!(msg.is_edited());
        assert_eq!(msg.current_content(), new_content.as_slice());
        assert_eq!(msg.last_editor(), &sender);
    }
    
    #[test]
    fn test_message_delete() {
        let msg_id = MessageId::generate();
        let channel_id = ChannelId::generate();
        let sender = UserId::generate();
        let content = b"Delete me".to_vec();
        let timestamp = Timestamp::now();
        
        let mut msg = Message::new(msg_id, channel_id, sender, content, timestamp);
        
        assert!(!msg.deleted);
        msg.delete();
        assert!(msg.deleted);
    }
    
    #[test]
    fn test_message_attachments() {
        let msg_id = MessageId::generate();
        let channel_id = ChannelId::generate();
        let sender = UserId::generate();
        let content = b"Check attachment".to_vec();
        let timestamp = Timestamp::now();
        
        let mut msg = Message::new(msg_id, channel_id, sender, content, timestamp);
        
        let attachment = Attachment {
            id: "att1".to_string(),
            filename: "image.png".to_string(),
            mime_type: "image/png".to_string(),
            size_bytes: 1024,
            content_hash: "abc123".to_string(),
        };
        
        msg.add_attachment(attachment.clone());
        
        assert_eq!(msg.attachments.len(), 1);
        assert_eq!(msg.attachments[0], attachment);
    }
}
