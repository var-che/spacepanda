//! Channel data structures and operations

use super::types::{ChannelId, SpaceId};
use crate::core_mls::types::GroupId;
use crate::core_store::model::types::{Timestamp, UserId};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// A Channel is a communication space within a Space (like Discord channels)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Channel {
    /// Unique identifier
    pub id: ChannelId,

    /// Parent Space identifier
    pub space_id: SpaceId,

    /// Human-readable name
    pub name: String,

    /// Optional description/topic
    pub description: Option<String>,

    /// Visibility mode (public or private)
    pub visibility: ChannelVisibility,

    /// Associated MLS group ID (one group per channel)
    pub mls_group_id: GroupId,

    /// Members in this channel (subset of Space members)
    pub members: HashSet<UserId>,

    /// When the channel was created
    pub created_at: Timestamp,

    /// Last time channel metadata was updated
    pub updated_at: Timestamp,
}

impl Channel {
    /// Create a new Channel
    pub fn new(
        space_id: SpaceId,
        name: String,
        visibility: ChannelVisibility,
        mls_group_id: GroupId,
        creator_id: UserId,
    ) -> Self {
        let now = Timestamp::now();
        let mut members = HashSet::new();
        members.insert(creator_id);

        Channel {
            id: ChannelId::generate(),
            space_id,
            name,
            description: None,
            visibility,
            mls_group_id,
            members,
            created_at: now,
            updated_at: now,
        }
    }

    /// Add a member to the channel
    pub fn add_member(&mut self, user_id: UserId) -> Result<(), ChannelError> {
        if self.members.contains(&user_id) {
            return Err(ChannelError::MemberAlreadyExists);
        }

        self.members.insert(user_id);
        self.updated_at = Timestamp::now();
        Ok(())
    }

    /// Remove a member from the channel
    pub fn remove_member(&mut self, user_id: &UserId) -> Result<(), ChannelError> {
        if !self.members.remove(user_id) {
            return Err(ChannelError::MemberNotFound);
        }

        self.updated_at = Timestamp::now();
        Ok(())
    }

    /// Check if a user is a member of the channel
    pub fn is_member(&self, user_id: &UserId) -> bool {
        self.members.contains(user_id)
    }

    /// Get the number of members in the channel
    pub fn member_count(&self) -> usize {
        self.members.len()
    }

    /// Update channel name
    pub fn update_name(&mut self, new_name: String) {
        self.name = new_name;
        self.updated_at = Timestamp::now();
    }

    /// Update channel description
    pub fn update_description(&mut self, new_description: Option<String>) {
        self.description = new_description;
        self.updated_at = Timestamp::now();
    }

    /// Update channel visibility
    pub fn update_visibility(&mut self, new_visibility: ChannelVisibility) {
        self.visibility = new_visibility;
        self.updated_at = Timestamp::now();
    }
}

/// Channel visibility modes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChannelVisibility {
    /// All Space members can see and join (auto-join on Space join)
    Public,
    /// Only invited members can see and join
    Private,
}

/// Channel operation errors
#[derive(Debug, thiserror::Error)]
pub enum ChannelError {
    #[error("Member already exists in channel")]
    MemberAlreadyExists,

    #[error("Member not found in channel")]
    MemberNotFound,

    #[error("Permission denied")]
    PermissionDenied,

    #[error("Channel is full")]
    ChannelFull,

    #[error("Invalid channel visibility transition")]
    InvalidVisibilityChange,

    #[error("Channel not found")]
    NotFound,

    #[error("MLS error: {0}")]
    MlsError(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_group_id() -> GroupId {
        GroupId::from(vec![1, 2, 3, 4])
    }

    #[test]
    fn test_create_channel() {
        let space_id = SpaceId::generate();
        let creator = UserId::new("alice".to_string());
        let group_id = create_test_group_id();

        let channel = Channel::new(
            space_id,
            "general".to_string(),
            ChannelVisibility::Public,
            group_id.clone(),
            creator.clone(),
        );

        assert_eq!(channel.name, "general");
        assert_eq!(channel.visibility, ChannelVisibility::Public);
        assert_eq!(channel.mls_group_id, group_id);
        assert!(channel.is_member(&creator));
        assert_eq!(channel.member_count(), 1);
    }

    #[test]
    fn test_add_member() {
        let space_id = SpaceId::generate();
        let creator = UserId::new("alice".to_string());
        let group_id = create_test_group_id();

        let mut channel = Channel::new(
            space_id,
            "general".to_string(),
            ChannelVisibility::Public,
            group_id,
            creator,
        );

        let new_member = UserId::new("bob".to_string());
        channel.add_member(new_member.clone()).unwrap();

        assert!(channel.is_member(&new_member));
        assert_eq!(channel.member_count(), 2);
    }

    #[test]
    fn test_cannot_add_duplicate_member() {
        let space_id = SpaceId::generate();
        let creator = UserId::new("alice".to_string());
        let group_id = create_test_group_id();

        let mut channel = Channel::new(
            space_id,
            "general".to_string(),
            ChannelVisibility::Public,
            group_id,
            creator,
        );

        let member = UserId::new("bob".to_string());
        channel.add_member(member.clone()).unwrap();

        let result = channel.add_member(member);
        assert!(matches!(result, Err(ChannelError::MemberAlreadyExists)));
    }

    #[test]
    fn test_remove_member() {
        let space_id = SpaceId::generate();
        let creator = UserId::new("alice".to_string());
        let group_id = create_test_group_id();

        let mut channel = Channel::new(
            space_id,
            "general".to_string(),
            ChannelVisibility::Public,
            group_id,
            creator,
        );

        let member = UserId::new("bob".to_string());
        channel.add_member(member.clone()).unwrap();
        assert_eq!(channel.member_count(), 2);

        channel.remove_member(&member).unwrap();
        assert_eq!(channel.member_count(), 1);
        assert!(!channel.is_member(&member));
    }

    #[test]
    fn test_remove_nonexistent_member() {
        let space_id = SpaceId::generate();
        let creator = UserId::new("alice".to_string());
        let group_id = create_test_group_id();

        let mut channel = Channel::new(
            space_id,
            "general".to_string(),
            ChannelVisibility::Public,
            group_id,
            creator,
        );

        let member = UserId::new("bob".to_string());
        let result = channel.remove_member(&member);
        assert!(matches!(result, Err(ChannelError::MemberNotFound)));
    }

    #[test]
    fn test_update_channel_metadata() {
        let space_id = SpaceId::generate();
        let creator = UserId::new("alice".to_string());
        let group_id = create_test_group_id();

        let mut channel = Channel::new(
            space_id,
            "general".to_string(),
            ChannelVisibility::Public,
            group_id,
            creator,
        );

        channel.update_name("announcements".to_string());
        assert_eq!(channel.name, "announcements");

        channel.update_description(Some("Important updates".to_string()));
        assert_eq!(channel.description, Some("Important updates".to_string()));

        channel.update_visibility(ChannelVisibility::Private);
        assert_eq!(channel.visibility, ChannelVisibility::Private);
    }
}
