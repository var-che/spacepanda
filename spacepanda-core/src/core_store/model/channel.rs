/*
    channel.rs - Channel model
    
    Represents a communication channel within a space.
    Uses CRDTs for all mutable fields to enable conflict-free replication.
    
    CRDT Design:
    - name, topic: LWWRegister for single-value fields
    - members, pinned_messages: OR-Set for membership
    - permissions: OR-Map with LWW values for deterministic permission changes
    - mls_identity: OR-Map tracking MLS leaf indices and credentials
    - messages: GList for causally-ordered message timeline (TODO: implement GList)
*/

use super::types::{ChannelId, UserId, MessageId, ChannelType, Timestamp, PermissionLevel, IdentityMeta};
use crate::core_store::crdt::{LWWRegister, ORSet, ORMap};
use serde::{Deserialize, Serialize};

/// Channel metadata and state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Channel {
    /// Unique channel ID
    pub id: ChannelId,
    
    /// Channel name (replicated via LWW)
    pub name: LWWRegister<String>,
    
    /// Channel topic/description (replicated via LWW)
    pub topic: LWWRegister<String>,
    
    /// Channel type (immutable after creation)
    pub channel_type: ChannelType,
    
    /// Creation timestamp (immutable)
    pub created_at: Timestamp,
    
    /// Creator user ID (immutable)
    pub created_by: UserId,
    
    /// Set of channel members (replicated via OR-Set)
    pub members: ORSet<UserId>,
    
    /// Set of pinned message IDs (replicated via OR-Set)
    pub pinned_messages: ORSet<MessageId>,
    
    /// Per-role permission levels (replicated via OR-Map with LWW values)
    /// Maps role_id -> LWWRegister<PermissionLevel> for deterministic permission changes
    pub permissions: ORMap<String, LWWRegister<PermissionLevel>>,
    
    /// MLS identity metadata (replicated via OR-Map)
    /// Maps user_id -> IdentityMeta for MLS tree reconciliation
    pub mls_identity: ORMap<UserId, IdentityMeta>,
    
    // TODO: Add when GList is implemented
    // /// Message timeline (replicated via GList/RGA for causal ordering)
    // pub messages: GList<MessageId>,
}

impl Channel {
    /// Create a new channel with initial values
    pub fn new(
        id: ChannelId,
        name: String,
        channel_type: ChannelType,
        created_by: UserId,
        created_at: Timestamp,
        node_id: String,
    ) -> Self {
        let name_register = LWWRegister::with_value(name, node_id.clone());
        let topic_register = LWWRegister::with_value(String::new(), node_id);
        
        let members = ORSet::new();
        let pinned_messages = ORSet::new();
        let permissions = ORMap::new();
        let mls_identity = ORMap::new();
        
        Channel {
            id,
            name: name_register,
            topic: topic_register,
            channel_type,
            created_at,
            created_by,
            members,
            pinned_messages,
            permissions,
            mls_identity,
        }
    }
    
    /// Get the current channel name
    pub fn get_name(&self) -> Option<&String> {
        self.name.get()
    }
    
    /// Get the current topic
    pub fn get_topic(&self) -> Option<&String> {
        self.topic.get()
    }
    
    /// Check if a user is a member
    pub fn has_member(&self, user_id: &UserId) -> bool {
        self.members.contains(user_id)
    }
    
    /// Get all current members
    pub fn get_members(&self) -> Vec<UserId> {
        self.members.elements()
    }
    
    /// Check if a message is pinned
    pub fn is_pinned(&self, message_id: &MessageId) -> bool {
        self.pinned_messages.contains(message_id)
    }
    
    /// Get all pinned messages
    pub fn get_pinned_messages(&self) -> Vec<MessageId> {
        self.pinned_messages.elements()
    }
    
    /// Get permission level for a role (returns the LWW register)
    pub fn get_permission(&self, role_id: &str) -> Option<&LWWRegister<PermissionLevel>> {
        self.permissions.get(&role_id.to_string())
    }
    
    /// Get current permission level for a role
    pub fn get_permission_level(&self, role_id: &str) -> Option<&PermissionLevel> {
        self.get_permission(role_id)?.get()
    }
    
    /// Get all role permissions
    pub fn get_all_permissions(&self) -> Vec<(String, LWWRegister<PermissionLevel>)> {
        self.permissions.entries()
    }
    
    /// Get MLS identity for a user
    pub fn get_mls_identity(&self, user_id: &UserId) -> Option<&IdentityMeta> {
        self.mls_identity.get(user_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_channel_creation() {
        let channel_id = ChannelId::generate();
        let user_id = UserId::generate();
        let now = Timestamp::now();
        let channel = Channel::new(
            channel_id.clone(),
            "general".to_string(),
            ChannelType::Text,
            user_id.clone(),
            now,
            "node1".to_string(),
        );
        
        assert_eq!(channel.id, channel_id);
        assert_eq!(channel.get_name(), Some(&"general".to_string()));
        assert_eq!(channel.get_topic(), Some(&String::new()));
        assert_eq!(channel.channel_type, ChannelType::Text);
        assert_eq!(channel.created_by, user_id);
    }
    
    #[test]
    fn test_channel_accessors() {
        let channel_id = ChannelId::generate();
        let user_id = UserId::generate();
        let now = Timestamp::now();
        let channel = Channel::new(
            channel_id,
            "general".to_string(),
            ChannelType::Text,
            user_id,
            now,
            "node1".to_string(),
        );
        
        assert_eq!(channel.get_name(), Some(&"general".to_string()));
        assert_eq!(channel.get_topic(), Some(&String::new()));
        assert_eq!(channel.get_members().len(), 0);
        assert_eq!(channel.get_pinned_messages().len(), 0);
        assert_eq!(channel.get_all_permissions().len(), 0);
    }
}
