//! Type definitions for MLS operations

use serde::{Deserialize, Serialize};

/// Group identifier (32 bytes)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GroupId(pub Vec<u8>);

impl GroupId {
    /// Create a new group ID from bytes
    pub fn new(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    /// Generate a random group ID
    pub fn random() -> Self {
        use rand::Rng;
        let mut bytes = vec![0u8; 32];
        rand::rng().fill(&mut bytes[..]);
        Self(bytes)
    }

    /// Get the bytes of the group ID
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// Convert to hex string for display
    pub fn to_hex(&self) -> String {
        hex::encode(&self.0)
    }

    /// Parse from hex string
    pub fn from_hex(s: &str) -> Result<Self, hex::FromHexError> {
        hex::decode(s).map(Self::new)
    }
}

impl std::fmt::Display for GroupId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

impl From<Vec<u8>> for GroupId {
    fn from(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }
}

impl AsRef<[u8]> for GroupId {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

/// Configuration for MLS operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MlsConfig {
    /// Maximum group size (0 = unlimited)
    pub max_group_size: usize,
    /// Enable automatic key rotation
    pub auto_key_rotation: bool,
    /// Key rotation interval in seconds (if auto_key_rotation is true)
    pub key_rotation_interval_secs: u64,
    /// Replay cache size (number of (epoch, sender, seq) tuples to remember)
    pub replay_cache_size: usize,
}

impl Default for MlsConfig {
    fn default() -> Self {
        Self {
            max_group_size: 1000,
            auto_key_rotation: true,
            key_rotation_interval_secs: 86400, // 24 hours
            replay_cache_size: 10_000,
        }
    }
}

/// Role of a member in a channel
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MemberRole {
    /// Full admin permissions (can remove members, promote/demote, change settings)
    Admin,
    /// Regular member (can send/receive messages)
    Member,
    /// Read-only access (can view but not send messages)
    ReadOnly,
}

impl MemberRole {
    /// Check if this role can remove members
    pub fn can_remove_members(&self) -> bool {
        matches!(self, MemberRole::Admin)
    }

    /// Check if this role can promote/demote members
    pub fn can_manage_roles(&self) -> bool {
        matches!(self, MemberRole::Admin)
    }

    /// Check if this role can send messages
    pub fn can_send_messages(&self) -> bool {
        matches!(self, MemberRole::Admin | MemberRole::Member)
    }
}

impl Default for MemberRole {
    fn default() -> Self {
        MemberRole::Member
    }
}

/// Member information in a group
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemberInfo {
    /// Member's identity (user ID bytes)
    pub identity: Vec<u8>,
    /// Leaf index in the ratchet tree
    pub leaf_index: u32,
    /// When the member joined (Unix timestamp)
    pub joined_at: u64,
    /// Member's role in the channel
    pub role: MemberRole,
}

/// Public group metadata (safe to publish to CRDT/DHT)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupPublicInfo {
    /// Group ID
    pub group_id: GroupId,
    /// Current epoch
    pub epoch: u64,
    /// Root hash of the ratchet tree
    pub root_hash: Vec<u8>,
    /// Timestamp of last update
    pub updated_at: u64,
    /// Signature over (group_id || epoch || root_hash || updated_at)
    /// by the group creator's device key
    pub signature: Vec<u8>,
}

/// Private group metadata (never publish, store encrypted)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupMetadata {
    /// Group ID
    pub group_id: GroupId,
    /// Group name (optional)
    pub name: Option<String>,
    /// Current epoch
    pub epoch: u64,
    /// Members in the group
    pub members: Vec<MemberInfo>,
    /// Created timestamp
    pub created_at: u64,
    /// Last updated timestamp
    pub updated_at: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_group_id_hex_roundtrip() {
        let group_id = GroupId::random();
        let hex = group_id.to_hex();
        let parsed = GroupId::from_hex(&hex).unwrap();
        assert_eq!(group_id, parsed);
    }

    #[test]
    fn test_group_id_display() {
        let group_id = GroupId::new(vec![1, 2, 3, 4]);
        let display = format!("{}", group_id);
        assert_eq!(display, "01020304");
    }

    #[test]
    fn test_config_default() {
        let config = MlsConfig::default();
        assert_eq!(config.max_group_size, 1000);
        assert_eq!(config.auto_key_rotation, true);
        assert_eq!(config.key_rotation_interval_secs, 86400);
        assert_eq!(config.replay_cache_size, 10_000);
    }

    #[test]
    fn test_serialization() {
        let group_id = GroupId::new(vec![1, 2, 3, 4]);
        let json = serde_json::to_string(&group_id).unwrap();
        let deserialized: GroupId = serde_json::from_str(&json).unwrap();
        assert_eq!(group_id, deserialized);
    }

    #[test]
    fn test_member_info() {
        let member = MemberInfo { identity: vec![1, 2, 3], leaf_index: 0, joined_at: 1234567890, role: MemberRole::Member };

        let json = serde_json::to_string(&member).unwrap();
        let deserialized: MemberInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(member, deserialized);
    }
}
