//! Manager trait implementations with business logic and MLS integration

use super::channel::{Channel, ChannelError, ChannelVisibility};
use super::invite::{InviteError, InviteType, SpaceInvite};
use super::manager::{ChannelManager, MembershipError, MembershipManager, SpaceManager};
use super::space::{Space, SpaceError, SpaceRole, SpaceVisibility};
use super::storage::SpaceSqlStore;
use super::types::{ChannelId, SpaceId};
use crate::core_mls::types::GroupId;
use crate::core_store::model::types::{Timestamp, UserId};

/// Manager implementation with business logic
///
/// Note: MLS integration is planned for async version.
/// Current implementation focuses on data model and storage operations.
pub struct SpaceManagerImpl {
    store: SpaceSqlStore,
}

impl SpaceManagerImpl {
    /// Create a new manager with storage
    pub fn new(store: SpaceSqlStore) -> Self {
        Self { store }
    }

    /// Validate Space name
    fn validate_space_name(name: &str) -> Result<(), SpaceError> {
        if name.is_empty() {
            return Err(SpaceError::PermissionDenied); // TODO: Add ValidationError
        }
        if name.len() > 100 {
            return Err(SpaceError::PermissionDenied);
        }
        Ok(())
    }

    /// Validate Channel name
    fn validate_channel_name(name: &str) -> Result<(), ChannelError> {
        if name.is_empty() {
            return Err(ChannelError::PermissionDenied); // TODO: Add ValidationError
        }
        if name.len() > 100 {
            return Err(ChannelError::PermissionDenied);
        }
        Ok(())
    }

    /// Check if user has admin privileges in a Space
    fn check_admin_permission(
        &self,
        space_id: &SpaceId,
        user_id: &UserId,
    ) -> Result<(), SpaceError> {
        let space = self.store.get_space(space_id)?;
        if !space.is_admin(user_id) {
            return Err(SpaceError::PermissionDenied);
        }
        Ok(())
    }

    /// Check if user is a member of a Space
    fn check_space_membership(
        &self,
        space_id: &SpaceId,
        user_id: &UserId,
    ) -> Result<(), SpaceError> {
        let space = self.store.get_space(space_id)?;
        if !space.is_member(user_id) {
            return Err(SpaceError::PermissionDenied);
        }
        Ok(())
    }
}

impl SpaceManager for SpaceManagerImpl {
    fn create_space(
        &mut self,
        name: String,
        owner_id: UserId,
        visibility: SpaceVisibility,
    ) -> Result<Space, SpaceError> {
        // Validate input
        Self::validate_space_name(&name)?;

        // Create Space data model
        let space = Space::new(name, owner_id, visibility);

        // Persist to database
        self.store.create_space(&space)?;

        Ok(space)
    }

    fn get_space(&self, space_id: &SpaceId) -> Result<Space, SpaceError> {
        self.store.get_space(space_id)
    }

    fn update_space(
        &mut self,
        space_id: &SpaceId,
        name: Option<String>,
        description: Option<String>,
        icon_url: Option<String>,
    ) -> Result<(), SpaceError> {
        let mut space = self.store.get_space(space_id)?;

        if let Some(new_name) = name {
            Self::validate_space_name(&new_name)?;
            space.name = new_name;
        }

        if let Some(new_desc) = description {
            space.description = Some(new_desc);
        }

        if let Some(new_icon) = icon_url {
            space.icon_url = Some(new_icon);
        }

        space.updated_at = Timestamp::now();

        self.store.update_space(&space)?;

        Ok(())
    }

    fn update_space_visibility(
        &mut self,
        space_id: &SpaceId,
        visibility: SpaceVisibility,
    ) -> Result<(), SpaceError> {
        let mut space = self.store.get_space(space_id)?;
        space.visibility = visibility;
        space.updated_at = Timestamp::now();

        self.store.update_space(&space)?;

        Ok(())
    }

    fn delete_space(&mut self, space_id: &SpaceId, user_id: &UserId) -> Result<(), SpaceError> {
        let space = self.store.get_space(space_id)?;

        // Only owner can delete
        if &space.owner_id != user_id {
            return Err(SpaceError::PermissionDenied);
        }

        // Delete (cascades to channels, members, invites)
        self.store.delete_space(space_id)?;

        Ok(())
    }

    fn list_public_spaces(&self) -> Result<Vec<Space>, SpaceError> {
        self.store.list_public_spaces()
    }

    fn list_user_spaces(&self, user_id: &UserId) -> Result<Vec<Space>, SpaceError> {
        self.store.list_user_spaces(user_id)
    }
}

impl MembershipManager for SpaceManagerImpl {
    fn create_invite(
        &mut self,
        space_id: SpaceId,
        created_by: UserId,
        max_uses: Option<u32>,
        expires_at: Option<Timestamp>,
    ) -> Result<SpaceInvite, InviteError> {
        // Verify creator is a member
        let space = self.store.get_space(&space_id).map_err(|_| InviteError::InvalidInviteCode)?;
        if !space.is_member(&created_by) {
            return Err(InviteError::UnauthorizedUser);
        }

        // Create invite
        let invite = SpaceInvite::new_link(space_id, created_by, max_uses, expires_at);

        // Persist to database
        self.store.create_invite(&invite)?;

        Ok(invite)
    }

    fn create_direct_invite(
        &mut self,
        space_id: SpaceId,
        created_by: UserId,
        target_user: UserId,
        expires_at: Option<Timestamp>,
    ) -> Result<SpaceInvite, InviteError> {
        // Verify creator is a member
        let space = self.store.get_space(&space_id).map_err(|_| InviteError::InvalidInviteCode)?;
        if !space.is_member(&created_by) {
            return Err(InviteError::UnauthorizedUser);
        }

        // Create direct invite
        let invite = SpaceInvite::new_direct(space_id, created_by, target_user, expires_at);

        // Persist to database
        self.store.create_invite(&invite)?;

        Ok(invite)
    }

    fn join_space(
        &mut self,
        user_id: UserId,
        invite_code: String,
    ) -> Result<Space, MembershipError> {
        // Find invite by code
        // Note: This requires a lookup - in production you'd have get_invite_by_code
        // For now, we'll return an error suggesting to use join_public_space or handle invite lookup differently
        Err(MembershipError::InviteError(InviteError::InviteNotFound))
    }

    fn join_public_space(
        &mut self,
        user_id: UserId,
        space_id: SpaceId,
    ) -> Result<Space, MembershipError> {
        let mut space = self.store.get_space(&space_id)?;

        // Verify Space is public
        if space.visibility != SpaceVisibility::Public {
            return Err(MembershipError::PermissionDenied);
        }

        // Check if already a member
        if space.is_member(&user_id) {
            return Err(MembershipError::AlreadyMember);
        }

        // Add user as member
        space.add_member(user_id.clone(), user_id.clone())?;

        // Update in database
        self.store.update_space(&space)?;

        // TODO: Auto-join public channels
        // This would require async context for MLS operations
        // For now, return the space and let caller handle channel joins

        Ok(space)
    }

    fn leave_space(&mut self, user_id: &UserId, space_id: &SpaceId) -> Result<(), MembershipError> {
        let mut space = self.store.get_space(space_id)?;

        // Cannot leave if owner
        if &space.owner_id == user_id {
            return Err(MembershipError::CannotLeaveAsOwner);
        }

        // Remove member
        space.remove_member(user_id)?;

        // Update in database
        self.store.update_space(&space)?;

        // TODO: Remove from all channels in this Space
        // This would require async context for MLS operations

        Ok(())
    }

    fn kick_member(
        &mut self,
        space_id: &SpaceId,
        admin_id: &UserId,
        target_user_id: &UserId,
    ) -> Result<(), MembershipError> {
        let mut space = self.store.get_space(space_id)?;

        // Verify admin has permission
        if !space.is_admin(admin_id) {
            return Err(MembershipError::PermissionDenied);
        }

        // Cannot kick owner
        if &space.owner_id == target_user_id {
            return Err(MembershipError::PermissionDenied);
        }

        // Remove member
        space.remove_member(target_user_id)?;

        // Update in database
        self.store.update_space(&space)?;

        // TODO: Remove from all channels in this Space
        // This would require async context for MLS operations

        Ok(())
    }

    fn update_member_role(
        &mut self,
        space_id: &SpaceId,
        admin_id: &UserId,
        target_user_id: &UserId,
        new_role: SpaceRole,
    ) -> Result<(), MembershipError> {
        let mut space = self.store.get_space(space_id)?;

        // Verify admin has permission
        if !space.is_admin(admin_id) {
            return Err(MembershipError::PermissionDenied);
        }

        // Update role
        space.update_member_role(target_user_id, new_role)?;

        // Update in database
        self.store.update_space(&space)?;

        Ok(())
    }

    fn revoke_invite(&mut self, invite_id: &str, user_id: &UserId) -> Result<(), InviteError> {
        let mut invite = self.store.get_invite(invite_id)?;

        // Verify user has permission (must be creator or Space admin)
        let space = self.store.get_space(&invite.space_id).map_err(|_| InviteError::InviteNotFound)?;
        if &invite.created_by != user_id && !space.is_admin(user_id) {
            return Err(InviteError::UnauthorizedUser);
        }

        // Revoke invite
        invite.revoke();

        // Update in database
        self.store.update_invite(&invite)?;

        Ok(())
    }

    fn list_invites(
        &self,
        space_id: &SpaceId,
        user_id: &UserId,
    ) -> Result<Vec<SpaceInvite>, InviteError> {
        // Verify user is a member
        let space = self.store.get_space(space_id).map_err(|_| InviteError::InviteNotFound)?;
        if !space.is_member(user_id) {
            return Err(InviteError::UnauthorizedUser);
        }

        self.store.list_space_invites(space_id)
    }
}

impl ChannelManager for SpaceManagerImpl {
    fn create_channel(
        &mut self,
        space_id: SpaceId,
        name: String,
        creator_id: UserId,
        visibility: ChannelVisibility,
        mls_group_id: Option<GroupId>,
    ) -> Result<Channel, ChannelError> {
        // Validate input
        Self::validate_channel_name(&name)?;

        // Verify creator is a Space member
        let space = self.store.get_space(&space_id).map_err(|_| ChannelError::PermissionDenied)?;
        if !space.is_member(&creator_id) {
            return Err(ChannelError::PermissionDenied);
        }

        // Use provided MLS group ID or create a placeholder
        let group_id = mls_group_id.unwrap_or_else(|| {
            // Generate a placeholder MLS group ID from ChannelId
            GroupId::from(ChannelId::generate().as_bytes().to_vec())
        });

        // Create Channel data model
        let channel = Channel::new(space_id, name, visibility, group_id, creator_id);

        // Persist to database
        self.store.create_channel(&channel)?;

        // TODO: Create actual MLS group
        // TODO: Add creator to MLS group
        // TODO: If public channel, auto-add all Space members

        Ok(channel)
    }

    fn get_channel(&self, channel_id: &ChannelId) -> Result<Channel, ChannelError> {
        self.store.get_channel(channel_id)
    }

    fn update_channel(
        &mut self,
        channel_id: &ChannelId,
        admin_id: &UserId,
        name: Option<String>,
        description: Option<String>,
    ) -> Result<(), ChannelError> {
        let channel = self.store.get_channel(channel_id)?;

        // Verify admin is a Space admin
        let space = self.store.get_space(&channel.space_id).map_err(|_| ChannelError::PermissionDenied)?;
        if !space.is_admin(admin_id) {
            return Err(ChannelError::PermissionDenied);
        }

        let mut channel = channel;

        if let Some(new_name) = name {
            Self::validate_channel_name(&new_name)?;
            channel.update_name(new_name);
        }

        if let Some(new_desc) = description {
            channel.update_description(Some(new_desc));
        }

        self.store.update_channel(&channel)?;

        Ok(())
    }

    fn update_channel_visibility(
        &mut self,
        channel_id: &ChannelId,
        visibility: ChannelVisibility,
    ) -> Result<(), ChannelError> {
        let mut channel = self.store.get_channel(channel_id)?;
        channel.update_visibility(visibility);

        self.store.update_channel(&channel)?;

        Ok(())
    }

    fn delete_channel(
        &mut self,
        channel_id: &ChannelId,
        admin_id: &UserId,
    ) -> Result<(), ChannelError> {
        let channel = self.store.get_channel(channel_id)?;

        // Verify user is Space admin
        let space = self.store.get_space(&channel.space_id).map_err(|_| ChannelError::PermissionDenied)?;
        if !space.is_admin(admin_id) {
            return Err(ChannelError::PermissionDenied);
        }

        // Delete channel (cascades to members)
        self.store.delete_channel(channel_id)?;

        // TODO: Delete MLS group
        // TODO: Send final messages to all members

        Ok(())
    }

    fn add_channel_member(
        &mut self,
        channel_id: &ChannelId,
        user_id: &UserId,
        admin_id: &UserId,
    ) -> Result<(), ChannelError> {
        let mut channel = self.store.get_channel(channel_id)?;

        // Verify admin is a Space admin
        let space = self.store.get_space(&channel.space_id).map_err(|_| ChannelError::PermissionDenied)?;
        if !space.is_admin(admin_id) {
            return Err(ChannelError::PermissionDenied);
        }

        // Verify user is a Space member
        if !space.is_member(user_id) {
            return Err(ChannelError::PermissionDenied);
        }

        // Add member to channel
        channel.add_member(user_id.clone())?;

        // Update database
        self.store.add_channel_member(channel_id, user_id)?;

        // TODO: Add user to MLS group
        // TODO: Generate and send welcome message

        Ok(())
    }

    fn remove_channel_member(
        &mut self,
        channel_id: &ChannelId,
        user_id: &UserId,
        admin_id: &UserId,
    ) -> Result<(), ChannelError> {
        let mut channel = self.store.get_channel(channel_id)?;

        // Verify admin is a Space admin
        let space = self.store.get_space(&channel.space_id).map_err(|_| ChannelError::PermissionDenied)?;
        if !space.is_admin(admin_id) {
            return Err(ChannelError::PermissionDenied);
        }

        // Remove member from channel
        channel.remove_member(user_id)?;

        // Update database
        self.store.remove_channel_member(channel_id, user_id)?;

        // TODO: Remove user from MLS group
        // TODO: Send commit to remaining members

        Ok(())
    }

    fn list_space_channels(&self, space_id: &SpaceId) -> Result<Vec<Channel>, ChannelError> {
        self.store.list_space_channels(space_id)
    }

    fn list_user_channels(
        &self,
        space_id: &SpaceId,
        user_id: &UserId,
    ) -> Result<Vec<Channel>, ChannelError> {
        // Get all channels in Space
        let all_channels = self.store.list_space_channels(space_id)?;

        // Filter to channels user is a member of
        let user_channels = all_channels
            .into_iter()
            .filter(|channel| channel.is_member(user_id))
            .collect();

        Ok(user_channels)
    }

    fn auto_join_public_channels(
        &mut self,
        space_id: &SpaceId,
        user_id: UserId,
    ) -> Result<Vec<ChannelId>, ChannelError> {
        // Get all channels in Space
        let channels = self.store.list_space_channels(space_id)?;

        let mut joined_channels = Vec::new();

        for channel in channels {
            // Skip if not public
            if channel.visibility != ChannelVisibility::Public {
                continue;
            }

            // Skip if already a member
            if channel.is_member(&user_id) {
                continue;
            }

            // Add member to channel
            self.store.add_channel_member(&channel.id, &user_id)?;

            // TODO: Add user to MLS group
            // TODO: Generate and send welcome message

            joined_channels.push(channel.id);
        }

        Ok(joined_channels)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_manager() -> SpaceManagerImpl {
        let store = SpaceSqlStore::memory().unwrap();
        SpaceManagerImpl::new(store)
    }

    #[test]
    fn test_create_and_get_space() {
        let mut manager = setup_manager();
        let owner = UserId::new("alice".to_string());

        let space = manager
            .create_space("Test Space".to_string(), owner.clone(), SpaceVisibility::Public)
            .unwrap();

        let retrieved = manager.get_space(&space.id).unwrap();
        assert_eq!(retrieved.name, "Test Space");
        assert_eq!(retrieved.owner_id, owner);
    }

    #[test]
    fn test_update_space() {
        let mut manager = setup_manager();
        let owner = UserId::new("alice".to_string());

        let space = manager
            .create_space("Test Space".to_string(), owner, SpaceVisibility::Public)
            .unwrap();

        manager
            .update_space(
                &space.id,
                Some("Updated Space".to_string()),
                Some("New description".to_string()),
                None,
            )
            .unwrap();

        let updated = manager.get_space(&space.id).unwrap();
        assert_eq!(updated.name, "Updated Space");
        assert_eq!(updated.description, Some("New description".to_string()));
    }

    #[test]
    fn test_delete_space_requires_owner() {
        let mut manager = setup_manager();
        let owner = UserId::new("alice".to_string());
        let other_user = UserId::new("bob".to_string());

        let space = manager
            .create_space("Test Space".to_string(), owner.clone(), SpaceVisibility::Public)
            .unwrap();

        // Non-owner cannot delete
        let result = manager.delete_space(&space.id, &other_user);
        assert!(result.is_err());

        // Owner can delete
        manager.delete_space(&space.id, &owner).unwrap();
    }

    #[test]
    fn test_create_invite() {
        let mut manager = setup_manager();
        let owner = UserId::new("alice".to_string());

        let space = manager
            .create_space("Test Space".to_string(), owner.clone(), SpaceVisibility::Public)
            .unwrap();

        let invite = manager
            .create_invite(space.id, owner, Some(10), None)
            .unwrap();

        assert_eq!(invite.space_id, space.id);
        assert_eq!(invite.max_uses, Some(10));
    }

    #[test]
    fn test_join_public_space() {
        let mut manager = setup_manager();
        let owner = UserId::new("alice".to_string());
        let new_user = UserId::new("bob".to_string());

        let space = manager
            .create_space("Test Space".to_string(), owner, SpaceVisibility::Public)
            .unwrap();

        let joined_space = manager.join_public_space(new_user.clone(), space.id).unwrap();

        assert!(joined_space.is_member(&new_user));
    }

    #[test]
    fn test_create_channel() {
        let mut manager = setup_manager();
        let owner = UserId::new("alice".to_string());

        let space = manager
            .create_space("Test Space".to_string(), owner.clone(), SpaceVisibility::Public)
            .unwrap();

        let channel = manager
            .create_channel(
                space.id,
                "general".to_string(),
                owner.clone(),
                ChannelVisibility::Public,
                None,
            )
            .unwrap();

        assert_eq!(channel.name, "general");
        assert_eq!(channel.space_id, space.id);
        assert!(channel.is_member(&owner));
    }

    #[test]
    fn test_auto_join_public_channels() {
        let mut manager = setup_manager();
        let owner = UserId::new("alice".to_string());
        let new_user = UserId::new("bob".to_string());

        let space = manager
            .create_space("Test Space".to_string(), owner.clone(), SpaceVisibility::Public)
            .unwrap();

        // Create public and private channels
        manager
            .create_channel(
                space.id,
                "general".to_string(),
                owner.clone(),
                ChannelVisibility::Public,
                None,
            )
            .unwrap();

        manager
            .create_channel(
                space.id,
                "private".to_string(),
                owner.clone(),
                ChannelVisibility::Private,
                None,
            )
            .unwrap();

        // Add new user to space
        manager.join_public_space(new_user.clone(), space.id).unwrap();

        // Auto-join public channels
        let joined = manager.auto_join_public_channels(&space.id, new_user.clone()).unwrap();

        assert_eq!(joined.len(), 1); // Should only join the public channel

        // Verify user is in public channel
        let channels = manager.list_user_channels(&space.id, &new_user).unwrap();
        assert_eq!(channels.len(), 1);
        assert_eq!(channels[0].name, "general");
    }
}
