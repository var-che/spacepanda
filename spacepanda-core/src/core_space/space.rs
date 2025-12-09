//! Space data structures and operations

use super::types::{ChannelId, SpaceId};
use crate::core_store::model::types::{Timestamp, UserId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A Space is a container for channels (like Discord servers)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Space {
    /// Unique identifier
    pub id: SpaceId,

    /// Human-readable name
    pub name: String,

    /// Optional description
    pub description: Option<String>,

    /// Optional icon URL
    pub icon_url: Option<String>,

    /// Visibility mode (public or private)
    pub visibility: SpaceVisibility,

    /// Owner of the Space (has full control)
    pub owner_id: UserId,

    /// Space members with their roles
    pub members: HashMap<UserId, SpaceMember>,

    /// List of channels in this Space
    pub channels: Vec<ChannelId>,

    /// When the Space was created
    pub created_at: Timestamp,

    /// Last time Space metadata was updated
    pub updated_at: Timestamp,
}

impl Space {
    /// Create a new Space
    pub fn new(name: String, owner_id: UserId, visibility: SpaceVisibility) -> Self {
        let now = Timestamp::now();
        let space_id = SpaceId::generate();

        let mut members = HashMap::new();
        members.insert(
            owner_id.clone(),
            SpaceMember {
                user_id: owner_id.clone(),
                role: SpaceRole::Owner,
                joined_at: now,
                invited_by: None,
            },
        );

        Space {
            id: space_id,
            name,
            description: None,
            icon_url: None,
            visibility,
            owner_id,
            members,
            channels: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }

    /// Add a member to the Space
    pub fn add_member(&mut self, user_id: UserId, invited_by: UserId) -> Result<(), SpaceError> {
        if self.members.contains_key(&user_id) {
            return Err(SpaceError::MemberAlreadyExists);
        }

        self.members.insert(
            user_id.clone(),
            SpaceMember {
                user_id,
                role: SpaceRole::Member,
                joined_at: Timestamp::now(),
                invited_by: Some(invited_by),
            },
        );

        self.updated_at = Timestamp::now();
        Ok(())
    }

    /// Remove a member from the Space
    pub fn remove_member(&mut self, user_id: &UserId) -> Result<(), SpaceError> {
        if user_id == &self.owner_id {
            return Err(SpaceError::CannotRemoveOwner);
        }

        if self.members.remove(user_id).is_none() {
            return Err(SpaceError::MemberNotFound);
        }

        self.updated_at = Timestamp::now();
        Ok(())
    }

    /// Update a member's role
    pub fn update_member_role(
        &mut self,
        user_id: &UserId,
        new_role: SpaceRole,
    ) -> Result<(), SpaceError> {
        if user_id == &self.owner_id && new_role != SpaceRole::Owner {
            return Err(SpaceError::CannotChangeOwnerRole);
        }

        let member = self
            .members
            .get_mut(user_id)
            .ok_or(SpaceError::MemberNotFound)?;

        member.role = new_role;
        self.updated_at = Timestamp::now();
        Ok(())
    }

    /// Add a channel to the Space
    pub fn add_channel(&mut self, channel_id: ChannelId) {
        if !self.channels.contains(&channel_id) {
            self.channels.push(channel_id);
            self.updated_at = Timestamp::now();
        }
    }

    /// Remove a channel from the Space
    pub fn remove_channel(&mut self, channel_id: &ChannelId) -> Result<(), SpaceError> {
        if let Some(pos) = self.channels.iter().position(|id| id == channel_id) {
            self.channels.remove(pos);
            self.updated_at = Timestamp::now();
            Ok(())
        } else {
            Err(SpaceError::ChannelNotFound)
        }
    }

    /// Get a member's role
    pub fn get_member_role(&self, user_id: &UserId) -> Option<SpaceRole> {
        self.members.get(user_id).map(|m| m.role)
    }

    /// Check if a user is a member
    pub fn is_member(&self, user_id: &UserId) -> bool {
        self.members.contains_key(user_id)
    }

    /// Check if a user has admin privileges (Owner or Admin)
    pub fn is_admin(&self, user_id: &UserId) -> bool {
        matches!(
            self.get_member_role(user_id),
            Some(SpaceRole::Owner) | Some(SpaceRole::Admin)
        )
    }
}

/// Space visibility modes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpaceVisibility {
    /// Listed in global directory, anyone can join
    Public,
    /// Invite-only, not listed in directory
    Private,
}

/// Space member information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpaceMember {
    /// User ID of the member
    pub user_id: UserId,

    /// Role in the Space
    pub role: SpaceRole,

    /// When the member joined
    pub joined_at: Timestamp,

    /// Who invited this member (if applicable)
    pub invited_by: Option<UserId>,
}

/// Space-level roles
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpaceRole {
    /// Full control, can delete Space and transfer ownership
    Owner,
    /// Can manage channels, members, and roles
    Admin,
    /// Default role, can participate in channels
    Member,
}

/// Space operation errors
#[derive(Debug, thiserror::Error)]
pub enum SpaceError {
    #[error("Member already exists in Space")]
    MemberAlreadyExists,

    #[error("Member not found in Space")]
    MemberNotFound,

    #[error("Cannot remove Space owner")]
    CannotRemoveOwner,

    #[error("Cannot change owner's role")]
    CannotChangeOwnerRole,

    #[error("Channel not found in Space")]
    ChannelNotFound,

    #[error("Permission denied")]
    PermissionDenied,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_space() {
        let owner = UserId::new("alice".to_string());
        let space = Space::new(
            "Test Space".to_string(),
            owner.clone(),
            SpaceVisibility::Public,
        );

        assert_eq!(space.name, "Test Space");
        assert_eq!(space.owner_id, owner);
        assert_eq!(space.visibility, SpaceVisibility::Public);
        assert_eq!(space.members.len(), 1);
        assert_eq!(space.get_member_role(&owner), Some(SpaceRole::Owner));
    }

    #[test]
    fn test_add_member() {
        let owner = UserId::new("alice".to_string());
        let mut space = Space::new(
            "Test Space".to_string(),
            owner.clone(),
            SpaceVisibility::Public,
        );

        let new_member = UserId::new("bob".to_string());
        space.add_member(new_member.clone(), owner.clone()).unwrap();

        assert!(space.is_member(&new_member));
        assert_eq!(space.get_member_role(&new_member), Some(SpaceRole::Member));
        assert_eq!(space.members.len(), 2);
    }

    #[test]
    fn test_cannot_add_duplicate_member() {
        let owner = UserId::new("alice".to_string());
        let mut space = Space::new(
            "Test Space".to_string(),
            owner.clone(),
            SpaceVisibility::Public,
        );

        let new_member = UserId::new("bob".to_string());
        space.add_member(new_member.clone(), owner.clone()).unwrap();

        let result = space.add_member(new_member, owner);
        assert!(matches!(result, Err(SpaceError::MemberAlreadyExists)));
    }

    #[test]
    fn test_remove_member() {
        let owner = UserId::new("alice".to_string());
        let mut space = Space::new(
            "Test Space".to_string(),
            owner.clone(),
            SpaceVisibility::Public,
        );

        let member = UserId::new("bob".to_string());
        space.add_member(member.clone(), owner).unwrap();
        assert_eq!(space.members.len(), 2);

        space.remove_member(&member).unwrap();
        assert_eq!(space.members.len(), 1);
        assert!(!space.is_member(&member));
    }

    #[test]
    fn test_cannot_remove_owner() {
        let owner = UserId::new("alice".to_string());
        let mut space = Space::new(
            "Test Space".to_string(),
            owner.clone(),
            SpaceVisibility::Public,
        );

        let result = space.remove_member(&owner);
        assert!(matches!(result, Err(SpaceError::CannotRemoveOwner)));
    }

    #[test]
    fn test_update_member_role() {
        let owner = UserId::new("alice".to_string());
        let mut space = Space::new(
            "Test Space".to_string(),
            owner.clone(),
            SpaceVisibility::Public,
        );

        let member = UserId::new("bob".to_string());
        space.add_member(member.clone(), owner).unwrap();

        space.update_member_role(&member, SpaceRole::Admin).unwrap();
        assert_eq!(space.get_member_role(&member), Some(SpaceRole::Admin));
        assert!(space.is_admin(&member));
    }

    #[test]
    fn test_add_and_remove_channel() {
        let owner = UserId::new("alice".to_string());
        let mut space = Space::new(
            "Test Space".to_string(),
            owner,
            SpaceVisibility::Public,
        );

        let channel_id = ChannelId::generate();
        space.add_channel(channel_id);
        assert_eq!(space.channels.len(), 1);

        space.remove_channel(&channel_id).unwrap();
        assert_eq!(space.channels.len(), 0);
    }
}
