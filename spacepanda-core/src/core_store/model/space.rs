/*
    space.rs - Space (Server) model

    Represents a "server" or "space" containing multiple channels.
    Uses CRDTs for all mutable fields to enable conflict-free replication.

    CRDT Design:
    - name, description: LWWRegister for single-value fields
    - channels, members: OR-Set for membership
    - roles: OR-Map with CRDT-wrapped Role fields
    - member_roles: OR-Map with LWW values for deterministic role assignment
    - mls_identity: OR-Map tracking MLS leaf indices and credentials
*/

use super::types::{ChannelId, IdentityMeta, PermissionLevel, SpaceId, Timestamp, UserId};
use crate::core_store::crdt::traits::Crdt;
use crate::core_store::crdt::{LWWRegister, ORMap, ORSet, VectorClock};
use crate::core_store::store::errors::StoreResult;
use serde::{Deserialize, Serialize};

/// Role definition within a space
/// All fields are CRDT-wrapped to allow safe concurrent modification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Role {
    /// Role name (LWW for concurrent renames)
    pub name: LWWRegister<String>,

    /// Permission level (LWW for concurrent permission changes)
    pub permission_level: LWWRegister<PermissionLevel>,

    /// Role color (LWW for concurrent color changes)
    pub color: LWWRegister<Option<String>>,
}

impl Role {
    pub fn new(name: String, permission_level: PermissionLevel, node_id: String) -> Self {
        Role {
            name: LWWRegister::with_value(name, node_id.clone()),
            permission_level: LWWRegister::with_value(permission_level, node_id.clone()),
            color: LWWRegister::with_value(None, node_id),
        }
    }

    pub fn get_name(&self) -> Option<&String> {
        self.name.get()
    }

    pub fn get_permission_level(&self) -> Option<&PermissionLevel> {
        self.permission_level.get()
    }

    pub fn get_color(&self) -> Option<&Option<String>> {
        self.color.get()
    }
}

/// Implement Crdt for Role to enable proper merging in ORMap
impl Crdt for Role {
    type Operation = (); // Role doesn't support standalone operations
    type Value = Self;

    fn apply(&mut self, _op: Self::Operation) -> StoreResult<()> {
        // Role doesn't support standalone operations
        Ok(())
    }

    fn merge(&mut self, other: &Self) -> StoreResult<()> {
        // Merge all CRDT fields
        self.name.merge(&other.name);
        self.permission_level.merge(&other.permission_level);
        self.color.merge(&other.color);
        Ok(())
    }

    fn value(&self) -> Self::Value {
        self.clone()
    }

    fn vector_clock(&self) -> &VectorClock {
        // Use the name's vector clock as the representative
        self.name.vector_clock()
    }
}

/// Space (Server) metadata and state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Space {
    /// Unique space ID
    pub id: SpaceId,

    /// Space name (replicated via LWW)
    pub name: LWWRegister<String>,

    /// Space description (replicated via LWW)
    pub description: LWWRegister<String>,

    /// Space owner (immutable)
    pub owner: UserId,

    /// Creation timestamp (immutable)
    pub created_at: Timestamp,

    /// Set of channel IDs (replicated via OR-Set)
    pub channels: ORSet<ChannelId>,

    /// Set of space members (replicated via OR-Set)
    pub members: ORSet<UserId>,

    /// Role definitions (replicated via OR-Map)
    /// Maps role_id -> Role (with CRDT-wrapped fields)
    pub roles: ORMap<String, Role>,

    /// User to role mappings (replicated via OR-Map with LWW values)
    /// Maps user_id -> LWWRegister<role_id> for deterministic role assignment
    pub member_roles: ORMap<UserId, LWWRegister<String>>,

    /// MLS identity metadata (replicated via OR-Map)
    /// Maps user_id -> IdentityMeta for MLS tree reconciliation
    pub mls_identity: ORMap<UserId, IdentityMeta>,
}

impl Space {
    /// Create a new space
    pub fn new(
        id: SpaceId,
        name: String,
        owner: UserId,
        created_at: Timestamp,
        node_id: String,
    ) -> Self {
        let name_register = LWWRegister::with_value(name, node_id.clone());
        let description_register = LWWRegister::with_value(String::new(), node_id.clone());

        let channels = ORSet::new();
        let members = ORSet::new();
        let roles = ORMap::new();
        let member_roles = ORMap::new();
        let mls_identity = ORMap::new();

        Space {
            id,
            name: name_register,
            description: description_register,
            owner,
            created_at,
            channels,
            members,
            roles,
            member_roles,
            mls_identity,
        }
    }

    /// Get the current space name
    pub fn get_name(&self) -> Option<&String> {
        self.name.get()
    }

    /// Get the current description
    pub fn get_description(&self) -> Option<&String> {
        self.description.get()
    }

    /// Check if a channel is in this space
    pub fn has_channel(&self, channel_id: &ChannelId) -> bool {
        self.channels.contains(channel_id)
    }

    /// Get all channels in this space
    pub fn get_channels(&self) -> Vec<ChannelId> {
        self.channels.elements()
    }

    /// Check if a user is a member
    pub fn has_member(&self, user_id: &UserId) -> bool {
        self.members.contains(user_id)
    }

    /// Get all members
    pub fn get_members(&self) -> Vec<UserId> {
        self.members.elements()
    }

    /// Get a role definition
    pub fn get_role(&self, role_id: &str) -> Option<&Role> {
        self.roles.get(&role_id.to_string())
    }

    /// Get all roles
    pub fn get_all_roles(&self) -> Vec<(String, Role)> {
        self.roles.entries()
    }

    /// Get a user's role (returns the LWWRegister containing the role_id)
    pub fn get_user_role(&self, user_id: &UserId) -> Option<&LWWRegister<String>> {
        self.member_roles.get(user_id)
    }

    /// Get a user's current role ID
    pub fn get_user_role_id(&self, user_id: &UserId) -> Option<&String> {
        self.get_user_role(user_id)?.get()
    }

    /// Get the permission level for a user
    pub fn get_user_permission_level(&self, user_id: &UserId) -> Option<PermissionLevel> {
        // Owner always has maximum permission
        if user_id == &self.owner {
            return Some(PermissionLevel::admin());
        }

        // Look up user's role and return its permission level
        self.get_user_role_id(user_id)
            .and_then(|role_id| self.get_role(role_id))
            .and_then(|role| role.get_permission_level())
            .cloned()
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
    fn test_space_creation() {
        let space_id = SpaceId::generate();
        let owner_id = UserId::generate();
        let now = Timestamp::now();
        let space = Space::new(
            space_id.clone(),
            "My Server".to_string(),
            owner_id.clone(),
            now,
            "node1".to_string(),
        );

        assert_eq!(space.id, space_id);
        assert_eq!(space.get_name(), Some(&"My Server".to_string()));
        assert_eq!(space.get_description(), Some(&String::new()));
        assert_eq!(space.owner, owner_id);
    }

    #[test]
    fn test_space_accessors() {
        let space_id = SpaceId::generate();
        let owner_id = UserId::generate();
        let now = Timestamp::now();
        let space = Space::new(
            space_id,
            "My Server".to_string(),
            owner_id.clone(),
            now,
            "node1".to_string(),
        );

        assert_eq!(space.get_name(), Some(&"My Server".to_string()));
        assert_eq!(space.get_description(), Some(&String::new()));
        assert_eq!(space.get_channels().len(), 0);
        assert_eq!(space.get_members().len(), 0);
        assert_eq!(space.get_all_roles().len(), 0);

        // Owner should have admin permissions
        assert_eq!(space.get_user_permission_level(&owner_id), Some(PermissionLevel::admin()));
    }

    #[test]
    fn test_role_creation() {
        let admin_role =
            Role::new("Admin".to_string(), PermissionLevel::admin(), "node1".to_string());

        assert_eq!(admin_role.get_name(), Some(&"Admin".to_string()));
        assert_eq!(admin_role.get_permission_level(), Some(&PermissionLevel::admin()));
        assert_eq!(admin_role.get_color(), Some(&None));
    }

    #[test]
    fn test_identity_meta_usage() {
        let space_id = SpaceId::generate();
        let owner_id = UserId::generate();
        let now = Timestamp::now();
        let space = Space::new(
            space_id,
            "My Server".to_string(),
            owner_id.clone(),
            now,
            "node1".to_string(),
        );

        // MLS identity should be empty initially
        assert_eq!(space.get_mls_identity(&owner_id), None);
    }
}
