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
use crate::core_mls::sealed_sender;
use crate::core_mls::service::MlsService;
use crate::core_mls::timing_obfuscation;
use crate::core_mls::types::GroupId;
use crate::core_mvp::network::NetworkLayer;
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

    /// Optional network layer for P2P message distribution
    network_layer: Option<Arc<NetworkLayer>>,
}

impl AsyncSpaceManager {
    /// Create a new async manager
    pub fn new(store: SpaceSqlStore, mls_service: Arc<MlsService>) -> Self {
        Self {
            manager: Arc::new(RwLock::new(SpaceManagerImpl::new(store))),
            mls_service,
            network_layer: None,
        }
    }

    /// Create a new async manager with network layer for P2P messaging
    pub fn with_network(
        store: SpaceSqlStore,
        mls_service: Arc<MlsService>,
        network_layer: Arc<NetworkLayer>,
    ) -> Self {
        Self {
            manager: Arc::new(RwLock::new(SpaceManagerImpl::new(store))),
            mls_service,
            network_layer: Some(network_layer),
        }
    }

    /// Get the network layer (if available)
    pub fn network_layer(&self) -> Option<Arc<NetworkLayer>> {
        self.network_layer.clone()
    }

    /// Register a user as a channel member for P2P routing
    pub async fn register_channel_member(
        &self,
        channel_id: &ChannelId,
        user_id: &UserId,
        peer_id: crate::core_router::session_manager::PeerId,
    ) -> Result<(), ChannelError> {
        if let Some(network) = &self.network_layer {
            // Convert ChannelId to model::types::ChannelId for NetworkLayer
            let network_channel_id = crate::core_store::model::types::ChannelId::new(
                hex::encode(channel_id.as_bytes())
            );
            
            network.register_channel_member(&network_channel_id, user_id.clone(), peer_id).await;
        }
        Ok(())
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

        // Save channel metadata to MLS SQL store (required for message foreign key)
        self.mls_service
            .save_channel_metadata(&group_id, name.as_bytes(), None, &[creator_id.0.as_bytes()], 0)
            .await
            .map_err(|e| {
                ChannelError::MlsError(format!("Failed to save channel metadata: {:?}", e))
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
    ///
    /// This method:
    /// 1. Encrypts the message using MLS
    /// 2. Saves it locally to the sender's database (with sealed sender for privacy)
    /// 3. Broadcasts it to other channel members via P2P (if network layer is available)
    pub async fn send_channel_message(
        &self,
        channel_id: &ChannelId,
        sender_id: &UserId,
        content: &[u8],
    ) -> Result<Vec<u8>, ChannelError> {
        eprintln!("[P2P] send_channel_message called for channel {} by user {}", 
            hex::encode(channel_id.as_bytes()), sender_id.0);
        
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
        
        eprintln!("[P2P] Message encrypted, size: {} bytes", encrypted_message.len());

        // PRIVACY: Seal sender identity to prevent metadata leakage
        // Get group secret for sealing
        let group_secret = self
            .mls_service
            .export_secret(group_id, "sender_key", b"", 32)
            .await
            .map_err(|e| {
                ChannelError::MlsError(format!("Failed to export secret: {:?}", e))
            })?;

        // Get current epoch
        let epoch = self
            .mls_service
            .get_epoch(group_id)
            .await
            .map_err(|e| {
                ChannelError::MlsError(format!("Failed to get epoch: {:?}", e))
            })?;

        // Derive sender key and seal sender
        let sender_key = sealed_sender::derive_sender_key(&group_secret);
        let sealed = sealed_sender::seal_sender(sender_id.0.as_bytes(), &sender_key, epoch)
            .map_err(|e| {
                ChannelError::MlsError(format!("Failed to seal sender: {:?}", e))
            })?;

        // Serialize sealed sender
        let sealed_sender_bytes = serde_json::to_vec(&sealed)
            .map_err(|e| {
                ChannelError::MlsError(format!("Failed to serialize sealed sender: {:?}", e))
            })?;

        // Save message locally (sender sees their own message)
        let plaintext_bytes = content;
        let message_id = crate::core_store::model::types::MessageId::generate().0;
        // PRIVACY: Use obfuscated sequence to prevent timing correlation
        let sequence = timing_obfuscation::generate_obfuscated_sequence();
        
        self.save_message_with_plaintext(
            message_id.as_bytes(),
            channel_id,
            &encrypted_message,
            &sealed_sender_bytes,
            sequence,
            Some(plaintext_bytes),
        )
        .await?;

        // Broadcast to peers via P2P network (if available)
        if let Some(network) = &self.network_layer {
            eprintln!("[P2P] Network layer available, broadcasting message...");
            
            // Convert ChannelId to model::types::ChannelId for NetworkLayer
            let network_channel_id =crate::core_store::model::types::ChannelId::new(
                hex::encode(channel_id.as_bytes())
            );
            
            eprintln!("[P2P] Calling broadcast_message for channel {}", network_channel_id.0);
            
            network
                .broadcast_message(&network_channel_id, encrypted_message.clone(), sender_id)
                .await
                .map_err(|e| {
                    // Log but don't fail - message is saved locally
                    eprintln!("[P2P] Broadcast failed: {}", e);
                    tracing::warn!(
                        channel_id = %channel_id,
                        error = %e,
                        "Failed to broadcast message to peers, saved locally only"
                    );
                    ChannelError::MlsError(format!("Network broadcast failed: {}", e))
                })?;
            
            eprintln!("[P2P] Message broadcast successful");
        } else {
            eprintln!("[P2P] No network layer available, skipping P2P broadcast");
            tracing::debug!(
                channel_id = %channel_id,
                "No network layer configured, message saved locally only"
            );
        }

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

    /// Handle incoming message from P2P network
    ///
    /// This is called when a message arrives from another peer.
    /// It:
    /// 1. Decrypts the message using MLS
    /// 2. Saves it to the local database (with sealed sender for privacy)
    /// 3. Returns the decrypted plaintext
    pub async fn handle_incoming_message(
        &self,
        channel_id: &ChannelId,
        sender_id: &UserId,
        encrypted_message: &[u8],
    ) -> Result<Vec<u8>, ChannelError> {
        // Get channel to find MLS group ID
        let manager = self.manager.read().await;
        let channel = manager.get_channel(channel_id)?;
        let group_id = &channel.mls_group_id;
        drop(manager);

        // Decrypt the message
        let plaintext = self
            .receive_channel_message(channel_id, encrypted_message)
            .await?;

        // PRIVACY: Seal sender identity to prevent metadata leakage
        // Get group secret for sealing
        let group_secret = self
            .mls_service
            .export_secret(group_id, "sender_key", b"", 32)
            .await
            .map_err(|e| {
                ChannelError::MlsError(format!("Failed to export secret: {:?}", e))
            })?;

        // Get current epoch
        let epoch = self
            .mls_service
            .get_epoch(group_id)
            .await
            .map_err(|e| {
                ChannelError::MlsError(format!("Failed to get epoch: {:?}", e))
            })?;

        // Derive sender key and seal sender
        let sender_key = sealed_sender::derive_sender_key(&group_secret);
        let sealed = sealed_sender::seal_sender(sender_id.0.as_bytes(), &sender_key, epoch)
            .map_err(|e| {
                ChannelError::MlsError(format!("Failed to seal sender: {:?}", e))
            })?;

        // Serialize sealed sender
        let sealed_sender_bytes = serde_json::to_vec(&sealed)
            .map_err(|e| {
                ChannelError::MlsError(format!("Failed to serialize sealed sender: {:?}", e))
            })?;

        // Save to local database so recipient can read it
        let message_id = crate::core_store::model::types::MessageId::generate().0;
        // PRIVACY: Use obfuscated sequence to prevent timing correlation
        let sequence = timing_obfuscation::generate_obfuscated_sequence();
        
        self.save_message_with_plaintext(
            message_id.as_bytes(),
            channel_id,
            encrypted_message,
            &sealed_sender_bytes,
            sequence,
            Some(&plaintext),
        )
        .await?;

        tracing::info!(
            channel_id = %channel_id,
            sender_id = %sender_id,
            "Received and saved incoming message from peer (with sealed sender)"
        );

        Ok(plaintext)
    }

    /// Handle incoming MLS commit from P2P network
    ///
    /// Processes commits to keep MLS group state synchronized
    pub async fn handle_incoming_commit(
        &self,
        channel_id: &ChannelId,
        commit_data: &[u8],
    ) -> Result<(), ChannelError> {
        eprintln!("[P2P] Processing incoming commit for channel {}", hex::encode(channel_id.as_bytes()));
        
        // Get channel to find MLS group ID
        let manager = self.manager.read().await;
        let channel = manager.get_channel(channel_id)?;
        let group_id = &channel.mls_group_id;
        drop(manager);

        // Process the commit message (updates epoch and group state)
        match self.mls_service.process_message(group_id, commit_data).await {
            Ok(_) => {
                eprintln!("[P2P] ✓ Successfully processed commit for channel {}", hex::encode(channel_id.as_bytes()));
                tracing::info!(
                    channel_id = %channel_id,
                    "Processed incoming MLS commit from peer"
                );
                Ok(())
            }
            Err(e) => {
                eprintln!("[P2P] Failed to process commit: {}", e);
                Err(ChannelError::MlsError(format!("Failed to process commit: {:?}", e)))
            }
        }
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

impl AsyncSpaceManager {
    /// Save a message to MLS storage
    pub async fn save_message(
        &self,
        message_id: &[u8],
        channel_id: &ChannelId,
        encrypted_content: &[u8],
        sealed_sender_bytes: &[u8],
        sequence: i64,
    ) -> Result<(), ChannelError> {
        self.save_message_with_plaintext(message_id, channel_id, encrypted_content, sealed_sender_bytes, sequence, None).await
    }

    /// Save a message to MLS storage with optional plaintext (for sent messages)
    pub async fn save_message_with_plaintext(
        &self,
        message_id: &[u8],
        channel_id: &ChannelId,
        encrypted_content: &[u8],
        sealed_sender_bytes: &[u8],
        sequence: i64,
        plaintext_content: Option<&[u8]>,
    ) -> Result<(), ChannelError> {
        // Get channel to find MLS group ID
        let manager = self.manager.read().await;
        let channel = manager.get_channel(channel_id)?;
        let group_id = &channel.mls_group_id;
        drop(manager);

        // Save to MLS storage
        self.mls_service
            .save_message_to_storage_with_plaintext(message_id, group_id, encrypted_content, sealed_sender_bytes, sequence, plaintext_content)
            .await
            .map_err(|e| ChannelError::MlsError(format!("Failed to save message: {:?}", e)))?;

        Ok(())
    }

    /// Load messages from MLS storage
    pub async fn load_messages(
        &self,
        channel_id: &ChannelId,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<(Vec<u8>, Vec<u8>, Vec<u8>, i64, bool, Option<Vec<u8>>)>, ChannelError> {
        // Get channel to find MLS group ID
        let manager = self.manager.read().await;
        let channel = manager.get_channel(channel_id)?;
        let group_id = &channel.mls_group_id;
        drop(manager);

        // Load from MLS storage
        self.mls_service
            .load_messages_from_storage(group_id, limit, offset)
            .await
            .map_err(|e| ChannelError::MlsError(format!("Failed to load messages: {:?}", e)))
    }

    /// Add a member to a channel's MLS group
    pub async fn add_member_to_channel(
        &self,
        channel_id: &ChannelId,
        user_id: &UserId,
    ) -> Result<(), ChannelError> {
        // Get channel to find MLS group ID
        let manager = self.manager.read().await;
        let channel = manager.get_channel(channel_id)?;
        let group_id = channel.mls_group_id.clone();
        drop(manager);

        // Generate key package for the new member
        let key_package = self
            .mls_service
            .generate_key_package(user_id.0.as_bytes().to_vec())
            .await
            .map_err(|e| ChannelError::MlsError(format!("Failed to generate key package: {:?}", e)))?;

        // Add member to MLS group
        let (commit, _welcome, _ratchet_tree) = self
            .mls_service
            .add_members(&group_id, vec![key_package])
            .await
            .map_err(|e| ChannelError::MlsError(format!("Failed to add member: {:?}", e)))?;

        // Broadcast commit to all channel members for MLS state synchronization
        if let Some(ref network_layer) = self.network_layer {
            eprintln!("[P2P] Broadcasting MLS commit for channel {} after adding member", hex::encode(channel_id.as_bytes()));
            // Convert bytes-based ChannelId to string-based ChannelId for network layer
            let network_channel_id = crate::core_store::model::types::ChannelId(hex::encode(channel_id.as_bytes()));
            if let Err(e) = network_layer.broadcast_commit(&network_channel_id, commit).await {
                eprintln!("[P2P] Warning: Failed to broadcast commit: {}", e);
                // Don't fail the operation if broadcast fails - member is already added locally
            } else {
                eprintln!("[P2P] ✓ Commit broadcasted successfully");
            }
        } else {
            eprintln!("[P2P] Warning: No network layer available, commit not broadcasted");
        }

        Ok(())
    }

    /// Remove a member from a channel's MLS group
    pub async fn remove_member_from_channel(
        &self,
        channel_id: &ChannelId,
        _user_id: &UserId,
    ) -> Result<(), ChannelError> {
        // Get channel to find MLS group ID
        let manager = self.manager.read().await;
        let channel = manager.get_channel(channel_id)?;
        let _group_id = channel.mls_group_id.clone();
        drop(manager);

        // TODO: Find the leaf index for the user_id in the MLS group
        // TODO: Call mls_service.remove_members with the leaf indices

        // For now, return error as this needs more implementation
        Err(ChannelError::MlsError(
            "Remove member not yet fully implemented".to_string(),
        ))
    }

    /// Generate a key package for this user to join channels
    pub async fn generate_key_package(&self) -> Result<Vec<u8>, ChannelError> {
        self.mls_service
            .generate_key_package(vec![])  // Empty identity - will use credential from provider
            .await
            .map_err(|e| ChannelError::MlsError(format!("Failed to generate key package: {:?}", e)))
    }

    /// Create an invite for a user to join a channel
    /// Returns (invite_token, optional_commit_for_existing_members, optional_ratchet_tree)
    pub async fn create_channel_invite(
        &self,
        channel_id: &ChannelId,
        key_package: Vec<u8>,
    ) -> Result<(Vec<u8>, Option<Vec<u8>>, Option<Vec<u8>>), ChannelError> {
        // Get channel to find MLS group ID
        let manager = self.manager.read().await;
        let channel = manager.get_channel(channel_id)?;
        let group_id = channel.mls_group_id.clone();
        drop(manager);

        // Add member to MLS group and get Welcome
        let (commit, welcome, ratchet_tree) = self
            .mls_service
            .add_members(&group_id, vec![key_package])
            .await
            .map_err(|e| ChannelError::MlsError(format!("Failed to add member: {:?}", e)))?;

        if welcome.is_empty() {
            return Err(ChannelError::MlsError("No Welcome message generated".to_string()));
        }

        // Return Welcome, commit, and ratchet tree
        let commit_opt = if commit.is_empty() { None } else { Some(commit) };
        let ratchet_tree_opt = if ratchet_tree.is_empty() { None } else { Some(ratchet_tree) };
        
        Ok((welcome, commit_opt, ratchet_tree_opt))
    }

    /// Join a channel using an invite token (containing Welcome message)
    pub async fn join_channel_from_invite(
        &self,
        invite_token: Vec<u8>,  // Welcome message
        ratchet_tree: Option<Vec<u8>>,
        user_id: &UserId,
        space_id: &SpaceId,
        channel_name: &str,
        original_channel_id: Option<ChannelId>, // Original channel ID from creator
    ) -> Result<ChannelId, ChannelError> {
        // Join the MLS group from Welcome
        let group_id = self
            .mls_service
            .join_group(&invite_token, ratchet_tree)
            .await
            .map_err(|e| ChannelError::MlsError(format!("Failed to join group: {:?}", e)))?;

        // Use original channel ID if provided (for P2P routing consistency)
        // Otherwise fall back to converting group ID to channel ID
        let channel_id = if let Some(original_id) = original_channel_id {
            eprintln!("[P2P] Using original channel ID from invite: {}", hex::encode(original_id.as_bytes()));
            original_id
        } else {
            // Fallback: Convert group ID to channel ID
            let id = ChannelId::from_bytes(
                group_id.0.as_slice().try_into()
                    .map_err(|_| ChannelError::MlsError("Invalid group ID length".to_string()))?
            );
            eprintln!("[P2P] No original channel ID, using converted group ID: {}", hex::encode(id.as_bytes()));
            id
        };

        // Save channel metadata to MLS SQL store (required for message foreign key)
        self.mls_service
            .save_channel_metadata(&group_id, channel_name.as_bytes(), None, &[user_id.0.as_bytes()], 0)
            .await
            .map_err(|e| {
                ChannelError::MlsError(format!("Failed to save channel metadata during join: {:?}", e))
            })?;

        // Create minimal space and channel records for messaging
        // This allows send/receive to work without full space sync
        use crate::core_space::space::{Space, SpaceVisibility};
        use crate::core_space::channel::{Channel, ChannelVisibility};
        use crate::core_store::model::types::Timestamp;
        use std::collections::{HashSet, HashMap};
        use crate::core_space::space::SpaceMember;
        
        let now = Timestamp::now();
        
        // Create minimal space (just enough to satisfy foreign key)
        let mut space_members = HashMap::new();
        space_members.insert(
            user_id.clone(),
            SpaceMember {
                user_id: user_id.clone(),
                role: crate::core_space::space::SpaceRole::Member,
                joined_at: now.clone(),
                invited_by: None,
            }
        );
        
        let space = Space {
            id: space_id.clone(),
            name: format!("Space (via invite)"),  // Placeholder name
            description: None,
            icon_url: None,
            visibility: SpaceVisibility::Private,
            owner_id: user_id.clone(),  // Temporary - not the real owner
            members: space_members,
            channels: vec![channel_id.clone()],
            created_at: now.clone(),
            updated_at: now.clone(),
        };
        
        // Create minimal channel record
        let mut channel_members = HashSet::new();
        channel_members.insert(user_id.clone());
        
        let channel = Channel {
            id: channel_id.clone(),
            space_id: space_id.clone(),
            name: channel_name.to_string(),
            description: None,
            visibility: ChannelVisibility::Private,
            mls_group_id: group_id,
            members: channel_members,
            created_at: now.clone(),
            updated_at: now,
        };

        // Save space and channel to local database
        let mut manager = self.manager.write().await;
        manager.create_space_direct(&space)
            .map_err(|e| ChannelError::MlsError(format!("Failed to create space during join: {:?}", e)))?;
        manager.create_channel_direct(&channel)
            .map_err(|e| ChannelError::MlsError(format!("Failed to create channel during join: {:?}", e)))?;
        drop(manager);

        Ok(channel_id)
    }
}