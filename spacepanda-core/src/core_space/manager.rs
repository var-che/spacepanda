//! Manager traits for Space and Channel operations

use super::channel::{Channel, ChannelError, ChannelVisibility};
use super::invite::{InviteError, SpaceInvite};
use super::space::{Space, SpaceError, SpaceRole, SpaceVisibility};
use super::types::{ChannelId, SpaceId};
use crate::core_store::model::types::{Timestamp, UserId};

/// Manager for Space operations
pub trait SpaceManager {
    /// Create a new Space
    fn create_space(
        &mut self,
        name: String,
        owner_id: UserId,
        visibility: SpaceVisibility,
    ) -> Result<Space, SpaceError>;

    /// Get a Space by ID
    fn get_space(&self, space_id: &SpaceId) -> Result<Space, SpaceError>;

    /// Update Space metadata
    fn update_space(
        &mut self,
        space_id: &SpaceId,
        name: Option<String>,
        description: Option<String>,
        icon_url: Option<String>,
    ) -> Result<(), SpaceError>;

    /// Update Space visibility
    fn update_space_visibility(
        &mut self,
        space_id: &SpaceId,
        visibility: SpaceVisibility,
    ) -> Result<(), SpaceError>;

    /// Delete a Space (only owner can delete)
    fn delete_space(&mut self, space_id: &SpaceId, user_id: &UserId) -> Result<(), SpaceError>;

    /// List all public Spaces (for directory)
    fn list_public_spaces(&self) -> Result<Vec<Space>, SpaceError>;

    /// List Spaces a user is a member of
    fn list_user_spaces(&self, user_id: &UserId) -> Result<Vec<Space>, SpaceError>;
}

/// Manager for Space membership operations
pub trait MembershipManager {
    /// Create an invite link
    fn create_invite(
        &mut self,
        space_id: SpaceId,
        created_by: UserId,
        max_uses: Option<u32>,
        expires_at: Option<Timestamp>,
    ) -> Result<SpaceInvite, InviteError>;

    /// Create a direct invite to a specific user
    fn create_direct_invite(
        &mut self,
        space_id: SpaceId,
        created_by: UserId,
        target_user: UserId,
        expires_at: Option<Timestamp>,
    ) -> Result<SpaceInvite, InviteError>;

    /// Join a Space using an invite code
    fn join_space(
        &mut self,
        user_id: UserId,
        invite_code: String,
    ) -> Result<Space, MembershipError>;

    /// Join a public Space directly (no invite needed)
    fn join_public_space(&mut self, user_id: UserId, space_id: SpaceId)
        -> Result<Space, MembershipError>;

    /// Leave a Space (cannot leave if owner)
    fn leave_space(&mut self, user_id: &UserId, space_id: &SpaceId) -> Result<(), MembershipError>;

    /// Kick a member from a Space (admin only)
    fn kick_member(
        &mut self,
        space_id: &SpaceId,
        admin_id: &UserId,
        target_user_id: &UserId,
    ) -> Result<(), MembershipError>;

    /// Update a member's role (admin only)
    fn update_member_role(
        &mut self,
        space_id: &SpaceId,
        admin_id: &UserId,
        target_user_id: &UserId,
        new_role: SpaceRole,
    ) -> Result<(), MembershipError>;

    /// Revoke an invite
    fn revoke_invite(&mut self, invite_id: &str, user_id: &UserId) -> Result<(), InviteError>;

    /// List active invites for a Space
    fn list_invites(&self, space_id: &SpaceId, user_id: &UserId) -> Result<Vec<SpaceInvite>, InviteError>;
}

/// Manager for Channel operations
pub trait ChannelManager {
    /// Create a new Channel in a Space
    fn create_channel(
        &mut self,
        space_id: SpaceId,
        name: String,
        visibility: ChannelVisibility,
        creator_id: UserId,
    ) -> Result<Channel, ChannelError>;

    /// Get a Channel by ID
    fn get_channel(&self, channel_id: &ChannelId) -> Result<Channel, ChannelError>;

    /// Update Channel metadata
    fn update_channel(
        &mut self,
        channel_id: &ChannelId,
        name: Option<String>,
        description: Option<String>,
    ) -> Result<(), ChannelError>;

    /// Update Channel visibility
    fn update_channel_visibility(
        &mut self,
        channel_id: &ChannelId,
        visibility: ChannelVisibility,
    ) -> Result<(), ChannelError>;

    /// Delete a Channel (admin only)
    fn delete_channel(
        &mut self,
        channel_id: &ChannelId,
        user_id: &UserId,
    ) -> Result<(), ChannelError>;

    /// Add a member to a channel
    fn add_channel_member(
        &mut self,
        channel_id: &ChannelId,
        user_id: UserId,
    ) -> Result<(), ChannelError>;

    /// Remove a member from a channel
    fn remove_channel_member(
        &mut self,
        channel_id: &ChannelId,
        user_id: &UserId,
    ) -> Result<(), ChannelError>;

    /// List all channels in a Space
    fn list_space_channels(&self, space_id: &SpaceId) -> Result<Vec<Channel>, ChannelError>;

    /// List channels a user is a member of
    fn list_user_channels(
        &self,
        space_id: &SpaceId,
        user_id: &UserId,
    ) -> Result<Vec<Channel>, ChannelError>;

    /// Auto-join user to all public channels in a Space
    fn auto_join_public_channels(
        &mut self,
        space_id: &SpaceId,
        user_id: UserId,
    ) -> Result<Vec<ChannelId>, ChannelError>;
}

/// Membership operation errors
#[derive(Debug, thiserror::Error)]
pub enum MembershipError {
    #[error("Space not found")]
    SpaceNotFound,

    #[error("User is already a member")]
    AlreadyMember,

    #[error("User is not a member")]
    NotMember,

    #[error("Permission denied")]
    PermissionDenied,

    #[error("Cannot leave Space as owner")]
    CannotLeaveAsOwner,

    #[error("Invite error: {0}")]
    InviteError(#[from] InviteError),

    #[error("Space error: {0}")]
    SpaceError(#[from] SpaceError),

    #[error("Invalid operation")]
    InvalidOperation,
}

#[cfg(test)]
mod tests {
    use super::*;

    // These are trait definitions, so we just verify they compile
    // Actual implementation tests will be in the concrete implementations

    #[test]
    fn test_traits_compile() {
        // This test just ensures the traits are well-formed
        // Real tests will be in the implementation modules
    }
}
