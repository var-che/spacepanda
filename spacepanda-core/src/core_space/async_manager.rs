//! Async Manager with MLS Integration
//!
//! This module provides async versions of the manager traits that integrate
//! with the MLS service to create actual MLS groups for channels.

use super::channel::{Channel, ChannelError, ChannelVisibility};
use super::invite::{InviteError, SpaceInvite};
use super::manager::{ChannelManager, MembershipError, MembershipManager, SpaceManager};
use super::manager_impl::SpaceManagerImpl;
use super::space::{Space, SpaceError, SpaceRole, SpaceVisibility};
use super::storage::SpaceSqlStore;
use super::types::{ChannelId, SpaceId};
use crate::core_mls::service::MlsService;
use crate::core_mls::types::GroupId;
use crate::core_store::model::types::{Timestamp, UserId};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Async manager with MLS integration
///
/// This wraps the synchronous SpaceManagerImpl and adds MLS operations
/// for creating/managing encrypted channels.
pub struct AsyncSpaceManager {
    /// Synchronous manager for database operations
    manager: Arc<RwLock<SpaceManagerImpl>>,

    /// MLS service for creating/managing groups
    mls_service: Arc<MlsService>,
}

impl AsyncSpaceManager {
    /// Create a new async manager
    pub fn new(store: SpaceSqlStore, mls_service: Arc<MlsService>) -> Self {
        Self {
            manager: Arc::new(RwLock::new(SpaceManagerImpl::new(store))),
            mls_service,
        }
    }

    /// Create a new Space (async version)
    pub async fn create_space(
        &self,
        name: String,
        owner_id: UserId,
        visibility: SpaceVisibility,
    ) -> Result<Space, SpaceError> {
        let mut manager = self.manager.write().await;
        manager.create_space(name, owner_id, visibility)
    }

    /// Get a Space by ID
    pub async fn get_space(&self, space_id: &SpaceId) -> Result<Space, SpaceError> {
        let manager = self.manager.read().await;
        manager.get_space(space_id)
    }

    /// Update Space metadata
    pub async fn update_space(
        &self,
        space_id: &SpaceId,
        name: Option<String>,
        description: Option<String>,
        icon_url: Option<String>,
    ) -> Result<(), SpaceError> {
        let mut manager = self.manager.write().await;
        manager.update_space(space_id, name, description, icon_url)
    }

    /// Update Space visibility
    pub async fn update_space_visibility(
        &self,
        space_id: &SpaceId,
        visibility: SpaceVisibility,
    ) -> Result<(), SpaceError> {
        let mut manager = self.manager.write().await;
        manager.update_space_visibility(space_id, visibility)
    }

    /// Delete a Space (only owner can delete)
    pub async fn delete_space(
        &self,
        space_id: &SpaceId,
        user_id: &UserId,
    ) -> Result<(), SpaceError> {
        let mut manager = self.manager.write().await;
        manager.delete_space(space_id, user_id)
    }

    /// List all public Spaces (for directory)
    pub async fn list_public_spaces(&self) -> Result<Vec<Space>, SpaceError> {
        let manager = self.manager.read().await;
        manager.list_public_spaces()
    }

    /// List Spaces a user is a member of
    pub async fn list_user_spaces(&self, user_id: &UserId) -> Result<Vec<Space>, SpaceError> {
        let manager = self.manager.read().await;
        manager.list_user_spaces(user_id)
    }

    /// Create an invite link
    pub async fn create_invite(
        &self,
        space_id: SpaceId,
        created_by: UserId,
        max_uses: Option<u32>,
        expires_at: Option<Timestamp>,
    ) -> Result<SpaceInvite, InviteError> {
        let mut manager = self.manager.write().await;
        manager.create_invite(space_id, created_by, max_uses, expires_at)
    }

    /// Create a direct invite to a specific user
    pub async fn create_direct_invite(
        &self,
        space_id: SpaceId,
        created_by: UserId,
        target_user: UserId,
        expires_at: Option<Timestamp>,
    ) -> Result<SpaceInvite, InviteError> {
        let mut manager = self.manager.write().await;
        manager.create_direct_invite(space_id, created_by, target_user, expires_at)
    }

    /// Join a Space using an invite code
    pub async fn join_space(
        &self,
        user_id: UserId,
        invite_code: String,
    ) -> Result<Space, MembershipError> {
        let mut manager = self.manager.write().await;
        manager.join_space(user_id, invite_code)
    }

    /// Join a public Space directly (no invite needed)
    pub async fn join_public_space(
        &self,
        user_id: UserId,
        space_id: SpaceId,
    ) -> Result<Space, MembershipError> {
        let mut manager = self.manager.write().await;
        manager.join_public_space(user_id, space_id)
    }

    /// Leave a Space (cannot leave if owner)
    pub async fn leave_space(
        &self,
        user_id: &UserId,
        space_id: &SpaceId,
    ) -> Result<(), MembershipError> {
        let mut manager = self.manager.write().await;
        manager.leave_space(user_id, space_id)
    }

    /// Kick a member from a Space (admin only)
    pub async fn kick_member(
        &self,
        space_id: &SpaceId,
        admin_id: &UserId,
        target_user_id: &UserId,
    ) -> Result<(), MembershipError> {
        let mut manager = self.manager.write().await;
        manager.kick_member(space_id, admin_id, target_user_id)
    }

    /// Update a member's role (admin only)
    pub async fn update_member_role(
        &self,
        space_id: &SpaceId,
        admin_id: &UserId,
        target_user_id: &UserId,
        new_role: SpaceRole,
    ) -> Result<(), MembershipError> {
        let mut manager = self.manager.write().await;
        manager.update_member_role(space_id, admin_id, target_user_id, new_role)
    }

    /// Revoke an invite
    pub async fn revoke_invite(
        &self,
        invite_id: &str,
        user_id: &UserId,
    ) -> Result<(), InviteError> {
        let mut manager = self.manager.write().await;
        manager.revoke_invite(invite_id, user_id)
    }

    /// Create a new Channel with MLS group integration
    ///
    /// This creates both the Channel metadata and an actual MLS group for E2EE messaging.
    pub async fn create_channel(
        &self,
        space_id: SpaceId,
        name: String,
        creator_id: UserId,
        visibility: ChannelVisibility,
    ) -> Result<Channel, ChannelError> {
        // Create MLS group for the channel
        let group_id = self
            .mls_service
            .create_group(creator_id.0.as_bytes().to_vec(), None)
            .await
            .map_err(|e| {
                ChannelError::MlsError(format!("Failed to create MLS group: {:?}", e))
            })?;

        // Create channel metadata in database
        let mut manager = self.manager.write().await;
        manager.create_channel(space_id, name, creator_id, visibility, Some(group_id))
    }

    /// Add a user to a Channel (creates MLS group membership)
    pub async fn add_channel_member(
        &self,
        channel_id: &ChannelId,
        user_id: &UserId,
        admin_id: &UserId,
    ) -> Result<(), ChannelError> {
        // Get channel to find MLS group ID
        let manager = self.manager.read().await;
        let channel = manager.get_channel(channel_id)?;
        let group_id = &channel.mls_group_id;
        drop(manager); // Release read lock

        // Generate key package for the user
        let key_package = self
            .mls_service
            .generate_key_package(user_id.0.as_bytes().to_vec())
            .await
            .map_err(|e| {
                ChannelError::MlsError(format!("Failed to generate key package: {:?}", e))
            })?;

        // Add user to MLS group
        let (_commit, _welcome, _ratchet_tree) = self
            .mls_service
            .add_members(group_id, vec![key_package])
            .await
            .map_err(|e| {
                ChannelError::MlsError(format!("Failed to add member to MLS group: {:?}", e))
            })?;

        // Add to database
        let mut manager = self.manager.write().await;
        manager.add_channel_member(channel_id, user_id, admin_id)
    }

    /// Remove a user from a Channel (removes from MLS group)
    pub async fn remove_channel_member(
        &self,
        channel_id: &ChannelId,
        user_id: &UserId,
        admin_id: &UserId,
    ) -> Result<(), ChannelError> {
        // Get channel to find MLS group ID
        let manager = self.manager.read().await;
        let channel = manager.get_channel(channel_id)?;
        let _group_id = &channel.mls_group_id;
        drop(manager); // Release read lock

        // TODO: Get user's leaf index from MLS group state
        // For now, we'll skip the MLS removal and just do the database operation
        // This will be implemented when we add proper member tracking

        // Example (needs member tracking):
        // let leaf_index = get_member_leaf_index(group_id, user_id)?;
        // let _commit = self.mls_service.remove_members(group_id, vec![leaf_index]).await?;

        // Remove from database
        let mut manager = self.manager.write().await;
        manager.remove_channel_member(channel_id, user_id, admin_id)
    }

    /// Send a message to a Channel (encrypted via MLS)
    pub async fn send_channel_message(
        &self,
        channel_id: &ChannelId,
        sender_id: &UserId,
        content: &[u8],
    ) -> Result<Vec<u8>, ChannelError> {
        // Get channel to find MLS group ID
        let manager = self.manager.read().await;
        let channel = manager.get_channel(channel_id)?;
        let group_id = &channel.mls_group_id;
        drop(manager);

        // Encrypt message via MLS
        let encrypted_message = self
            .mls_service
            .send_message(group_id, content)
            .await
            .map_err(|e| {
                ChannelError::MlsError(format!("Failed to encrypt message: {:?}", e))
            })?;

        Ok(encrypted_message)
    }

    /// Receive and decrypt a message from a Channel
    pub async fn receive_channel_message(
        &self,
        channel_id: &ChannelId,
        encrypted_message: &[u8],
    ) -> Result<Vec<u8>, ChannelError> {
        // Get channel to find MLS group ID
        let manager = self.manager.read().await;
        let channel = manager.get_channel(channel_id)?;
        let group_id = &channel.mls_group_id;
        drop(manager);

        // Decrypt message via MLS
        let plaintext_opt = self
            .mls_service
            .process_message(group_id, encrypted_message)
            .await
            .map_err(|e| {
                ChannelError::MlsError(format!("Failed to decrypt message: {:?}", e))
            })?;

        // Extract plaintext or return error if it was a control message
        let plaintext = plaintext_opt.ok_or_else(|| {
            ChannelError::MlsError("Received control message instead of application data".to_string())
        })?;

        Ok(plaintext)
    }

    /// Get a Channel by ID
    pub async fn get_channel(&self, channel_id: &ChannelId) -> Result<Channel, ChannelError> {
        let manager = self.manager.read().await;
        manager.get_channel(channel_id)
    }

    /// Update Channel metadata
    pub async fn update_channel(
        &self,
        channel_id: &ChannelId,
        admin_id: &UserId,
        name: Option<String>,
        description: Option<String>,
    ) -> Result<(), ChannelError> {
        let mut manager = self.manager.write().await;
        manager.update_channel(channel_id, admin_id, name, description)
    }

    /// Delete a Channel (admin only)
    pub async fn delete_channel(
        &self,
        channel_id: &ChannelId,
        admin_id: &UserId,
    ) -> Result<(), ChannelError> {
        let mut manager = self.manager.write().await;
        manager.delete_channel(channel_id, admin_id)
    }

    /// List all Channels in a Space
    pub async fn list_space_channels(
        &self,
        space_id: &SpaceId,
    ) -> Result<Vec<Channel>, ChannelError> {
        let manager = self.manager.read().await;
        manager.list_space_channels(space_id)
    }

    /// List Channels a user has access to in a Space
    pub async fn list_user_channels(
        &self,
        space_id: &SpaceId,
        user_id: &UserId,
    ) -> Result<Vec<Channel>, ChannelError> {
        let manager = self.manager.read().await;
        manager.list_user_channels(space_id, user_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::shutdown::ShutdownCoordinator;

    async fn setup_async_manager() -> AsyncSpaceManager {
        let store = SpaceSqlStore::memory().unwrap();
        let config = Config::default();
        let shutdown = Arc::new(ShutdownCoordinator::new(std::time::Duration::from_secs(5)));
        let mls_service = Arc::new(MlsService::new(&config, shutdown));

        AsyncSpaceManager::new(store, mls_service)
    }

    #[tokio::test]
    async fn test_create_space_async() {
        let manager = setup_async_manager().await;

        let space = manager
            .create_space(
                "Test Space".to_string(),
                UserId::new("alice".to_string()),
                SpaceVisibility::Public,
            )
            .await
            .unwrap();

        assert_eq!(space.name, "Test Space");
        assert_eq!(space.owner_id, UserId::new("alice".to_string()));
    }

    #[tokio::test]
    async fn test_create_channel_with_mls() {
        let manager = setup_async_manager().await;

        // Create a Space first
        let space = manager
            .create_space(
                "Test Space".to_string(),
                UserId::new("alice".to_string()),
                SpaceVisibility::Public,
            )
            .await
            .unwrap();

        // Create a Channel with MLS group
        let channel = manager
            .create_channel(
                space.id.clone(),
                "general".to_string(),
                UserId::new("alice".to_string()),
                ChannelVisibility::Public,
            )
            .await
            .unwrap();

        assert_eq!(channel.name, "general");
        assert_eq!(channel.space_id, space.id);
        // MLS group is always created
        assert!(!channel.mls_group_id.as_bytes().is_empty(), "MLS group should be created");
    }

    #[tokio::test]
    async fn test_add_channel_member_with_mls() {
        let manager = setup_async_manager().await;

        // Create Space and Channel
        let space = manager
            .create_space(
                "Test Space".to_string(),
                UserId::new("alice".to_string()),
                SpaceVisibility::Public,
            )
            .await
            .unwrap();

        let channel = manager
            .create_channel(
                space.id.clone(),
                "general".to_string(),
                UserId::new("alice".to_string()),
                ChannelVisibility::Public,
            )
            .await
            .unwrap();

        // Add bob to the channel (will create MLS membership)
        let result = manager
            .add_channel_member(&channel.id, &UserId::new("bob".to_string()), &UserId::new("alice".to_string()))
            .await;

        assert!(result.is_ok(), "Should add member successfully");
    }

    #[tokio::test]
    async fn test_send_receive_channel_message() {
        let manager = setup_async_manager().await;

        // Create Space and Channel
        let space = manager
            .create_space(
                "Test Space".to_string(),
                UserId::new("alice".to_string()),
                SpaceVisibility::Public,
            )
            .await
            .unwrap();

        let channel = manager
            .create_channel(
                space.id.clone(),
                "general".to_string(),
                UserId::new("alice".to_string()),
                ChannelVisibility::Public,
            )
            .await
            .unwrap();

        // Send a message
        let plaintext = b"Hello, world!";
        let encrypted = manager
            .send_channel_message(&channel.id, &UserId::new("alice".to_string()), plaintext)
            .await
            .unwrap();

        assert!(!encrypted.is_empty(), "Should produce encrypted message");

        // Receive and decrypt the message
        let decrypted = manager
            .receive_channel_message(&channel.id, &encrypted)
            .await
            .unwrap();

        assert_eq!(decrypted, plaintext, "Decrypted message should match original");
    }
}
