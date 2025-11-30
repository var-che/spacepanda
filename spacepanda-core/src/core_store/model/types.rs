/*
    types.rs - Common types for core_store models
    
    Defines:
    - Timestamps
    - IDs for channels, spaces, messages, users
    - Common enums
*/

use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

/// Lamport timestamp for causal ordering
pub type LamportTimestamp = u64;

/// Unix timestamp in milliseconds
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Timestamp(pub u64);

impl Timestamp {
    /// Create a timestamp representing the current time
    pub fn now() -> Self {
        let duration = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards");
        Timestamp(duration.as_millis() as u64)
    }
    
    /// Create a timestamp from milliseconds since epoch
    pub fn from_millis(millis: u64) -> Self {
        Timestamp(millis)
    }
    
    /// Get milliseconds since epoch
    pub fn as_millis(&self) -> u64 {
        self.0
    }
}

impl Default for Timestamp {
    fn default() -> Self {
        Timestamp::now()
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// MLS identity metadata for a user in a channel/space
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentityMeta {
    /// MLS leaf index in the tree
    pub leaf_index: u32,
    
    /// Public key bytes
    pub public_key: Vec<u8>,
    
    /// MLS credential bytes
    pub credential: Vec<u8>,
}

impl IdentityMeta {
    pub fn new(leaf_index: u32, public_key: Vec<u8>, credential: Vec<u8>) -> Self {
        IdentityMeta {
            leaf_index,
            public_key,
            credential,
        }
    }
}

/// Unique identifier for a space (server)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SpaceId(pub String);

impl SpaceId {
    pub fn new(id: String) -> Self {
        SpaceId(id)
    }
    
    pub fn generate() -> Self {
        use uuid::Uuid;
        let id = Uuid::new_v4().to_string();
        SpaceId(id)
    }
}

impl fmt::Display for SpaceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Unique identifier for a channel
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChannelId(pub String);

impl ChannelId {
    pub fn new(id: String) -> Self {
        ChannelId(id)
    }
    
    pub fn generate() -> Self {
        use uuid::Uuid;
        let id = Uuid::new_v4().to_string();
        ChannelId(id)
    }
}

impl fmt::Display for ChannelId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Unique identifier for a message
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MessageId(pub String);

impl MessageId {
    pub fn new(id: String) -> Self {
        MessageId(id)
    }
    
    pub fn generate() -> Self {
        use uuid::Uuid;
        let id = Uuid::new_v4().to_string();
        MessageId(id)
    }
}

impl fmt::Display for MessageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// User identifier (references identity from core_identity)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UserId(pub String);

impl UserId {
    pub fn new(id: String) -> Self {
        UserId(id)
    }
    
    pub fn generate() -> Self {
        use uuid::Uuid;
        let id = Uuid::new_v4().to_string();
        UserId(id)
    }
}

impl fmt::Display for UserId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Channel type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChannelType {
    /// Text channel for messages
    Text,
    /// Voice channel
    Voice,
    /// Forum-style threaded discussions
    Forum,
    /// Announcement channel (read-only for most)
    Announcement,
}

impl Default for ChannelType {
    fn default() -> Self {
        ChannelType::Text
    }
}

/// Permission level with granular capabilities
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PermissionLevel {
    pub read: bool,
    pub write: bool,
    pub admin: bool,
    pub ban_members: bool,
    pub manage_roles: bool,
    pub manage_channels: bool,
}

impl PermissionLevel {
    /// Create a new permission level with all permissions disabled
    pub fn none() -> Self {
        PermissionLevel {
            read: false,
            write: false,
            admin: false,
            ban_members: false,
            manage_roles: false,
            manage_channels: false,
        }
    }
    
    /// Create read-only permissions
    pub fn read_only() -> Self {
        PermissionLevel {
            read: true,
            write: false,
            admin: false,
            ban_members: false,
            manage_roles: false,
            manage_channels: false,
        }
    }
    
    /// Create read-write permissions (typical member)
    pub fn member() -> Self {
        PermissionLevel {
            read: true,
            write: true,
            admin: false,
            ban_members: false,
            manage_roles: false,
            manage_channels: false,
        }
    }
    
    /// Create moderator permissions
    pub fn moderator() -> Self {
        PermissionLevel {
            read: true,
            write: true,
            admin: false,
            ban_members: true,
            manage_roles: false,
            manage_channels: false,
        }
    }
    
    /// Create admin permissions
    pub fn admin() -> Self {
        PermissionLevel {
            read: true,
            write: true,
            admin: true,
            ban_members: true,
            manage_roles: true,
            manage_channels: true,
        }
    }
}

impl Default for PermissionLevel {
    fn default() -> Self {
        PermissionLevel::member()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_timestamp_creation() {
        let ts1 = Timestamp::now();
        let ts2 = Timestamp::now();
        assert!(ts2.as_millis() >= ts1.as_millis());
    }
    
    #[test]
    fn test_timestamp_from_millis() {
        let ts = Timestamp::from_millis(1234567890);
        assert_eq!(ts.as_millis(), 1234567890);
    }
    
    #[test]
    fn test_timestamp_ordering() {
        let ts1 = Timestamp::from_millis(100);
        let ts2 = Timestamp::from_millis(200);
        assert!(ts1 < ts2);
    }
    
    #[test]
    fn test_space_id_generation() {
        let id1 = SpaceId::generate();
        let id2 = SpaceId::generate();
        assert_ne!(id1, id2);
        assert!(id1.0.len() > 0);
    }
    
    #[test]
    fn test_channel_id_generation() {
        let id1 = ChannelId::generate();
        let id2 = ChannelId::generate();
        assert_ne!(id1, id2);
        assert!(id1.0.len() > 0);
    }
    
    #[test]
    fn test_message_id_generation() {
        let id1 = MessageId::generate();
        let id2 = MessageId::generate();
        assert_ne!(id1, id2);
        assert!(id1.0.len() > 0);
    }
    
    #[test]
    fn test_channel_types() {
        assert_eq!(ChannelType::default(), ChannelType::Text);
    }
    
    #[test]
    fn test_permission_levels() {
        let admin = PermissionLevel::admin();
        let moderator = PermissionLevel::moderator();
        let member = PermissionLevel::member();
        let read_only = PermissionLevel::read_only();
        
        assert!(admin.admin);
        assert!(admin.manage_roles);
        assert!(admin.manage_channels);
        
        assert!(!moderator.admin);
        assert!(moderator.ban_members);
        assert!(!moderator.manage_roles);
        
        assert!(member.read);
        assert!(member.write);
        assert!(!member.admin);
        
        assert!(read_only.read);
        assert!(!read_only.write);
    }
    
    #[test]
    fn test_identity_meta() {
        let meta = IdentityMeta::new(0, vec![1, 2, 3], vec![4, 5, 6]);
        assert_eq!(meta.leaf_index, 0);
        assert_eq!(meta.public_key, vec![1, 2, 3]);
        assert_eq!(meta.credential, vec![4, 5, 6]);
    }
}
