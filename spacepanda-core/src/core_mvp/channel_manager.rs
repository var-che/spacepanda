//! Channel Manager - Main Orchestrator for MVP
//!
//! This module coordinates MLS, CRDT, and DHT to provide high-level channel operations.
//!
//! # Responsibilities
//!
//! - **Channel Creation**: Creates MLS group + CRDT channel + DHT entry
//! - **Invite Management**: Generates Welcome messages with ratchet trees
//! - **Join Operations**: Processes invites and syncs state
//! - **Message Routing**: Encrypts/decrypts messages via MLS
//! - **Member Management**: Add/remove with permission checks
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────┐
//! │  ChannelManager     │
//! └────┬───┬───┬────────┘
//!      │   │   │
//!      ▼   ▼   ▼
//!    MLS CRDT DHT
//! ```

use crate::{
    config::Config,
    core_mls::{
        engine::GroupOperations,
        service::MlsService,
        types::{GroupId, GroupMetadata},
    },
    core_mvp::{
        errors::{MvpError, MvpResult},
        types::{ChannelDescriptor, ChatMessage, InviteToken},
    },
    core_store::{
        model::{
            channel::Channel,
            types::{ChannelId, ChannelType, Timestamp, UserId},
        },
        store::local_store::LocalStore,
    },
};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Channel Manager - orchestrates all channel operations
pub struct ChannelManager {
    /// MLS service for group encryption
    mls_service: Arc<MlsService>,

    /// CRDT store for channel metadata
    store: Arc<LocalStore>,

    /// Current user's identity
    identity: Arc<Identity>,

    /// Configuration
    config: Arc<Config>,
}

/// Simple identity holder (will integrate with core_identity later)
#[derive(Debug, Clone)]
pub struct Identity {
    /// User ID
    pub user_id: UserId,
    /// Display name
    pub display_name: String,
    /// Node ID for CRDT
    pub node_id: String,
}

impl Identity {
    /// Create a new identity
    pub fn new(user_id: UserId, display_name: String, node_id: String) -> Self {
        Self {
            user_id,
            display_name,
            node_id,
        }
    }

    /// Get identity bytes for MLS
    pub fn as_bytes(&self) -> Vec<u8> {
        self.user_id.0.as_bytes().to_vec()
    }
}

impl ChannelManager {
    /// Create a new channel manager
    ///
    /// # Arguments
    ///
    /// * `mls_service` - MLS service for encryption
    /// * `store` - CRDT store for metadata
    /// * `identity` - Current user identity
    /// * `config` - Application configuration
    pub fn new(
        mls_service: Arc<MlsService>,
        store: Arc<LocalStore>,
        identity: Arc<Identity>,
        config: Arc<Config>,
    ) -> Self {
        info!(
            user_id = %identity.user_id,
            "Creating ChannelManager"
        );

        Self {
            mls_service,
            store,
            identity,
            config,
        }
    }

    /// Get a reference to the user's identity
    ///
    /// # Returns
    ///
    /// Reference to the user's Identity
    pub fn identity(&self) -> &Identity {
        &self.identity
    }

    /// Generate a key package for joining channels
    ///
    /// Creates a KeyPackageBundle with cryptographic material and stores it
    /// in the MLS provider. Returns only the serialized public key package
    /// that can be shared with channel administrators for creating invites.
    ///
    /// # Returns
    ///
    /// Serialized public key package bytes (Vec<u8>) that can be shared
    /// with channel admins. The private KeyPackageBundle is stored internally
    /// and will be used automatically when joining from a Welcome message.
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Bob generates a key package to share with Alice
    /// let bob_key_package = bob_manager.generate_key_package().await?;
    ///
    /// // Bob shares key_package with Alice (via separate channel)
    /// // Alice creates an invite using Bob's key package
    /// let invite = alice_manager.create_invite(&channel_id, bob_key_package).await?;
    ///
    /// // Bob can now join using the invite (his bundle is already stored)
    /// let channel_id = bob_manager.join_channel(&invite).await?;
    /// ```
    pub async fn generate_key_package(&self) -> MvpResult<Vec<u8>> {
        info!(
            user_id = %self.identity.user_id,
            "Generating key package"
        );

        let key_package = self
            .mls_service
            .generate_key_package(self.identity.user_id.0.as_bytes().to_vec())
            .await?;

        debug!(
            "Generated key package: {} bytes",
            key_package.len()
        );

        Ok(key_package)
    }

    /// Create a new channel
    ///
    /// This performs three operations:
    /// 1. Creates an MLS group for encryption
    /// 2. Creates a CRDT Channel model for metadata
    /// 3. (If public) Publishes to DHT for discovery
    ///
    /// # Arguments
    ///
    /// * `name` - Human-readable channel name
    /// * `is_public` - Whether channel is publicly discoverable
    ///
    /// # Returns
    ///
    /// The created channel's ID
    ///
    /// # Example
    ///
    /// ```ignore
    /// let channel_id = manager.create_channel("general", false).await?;
    /// ```
    pub async fn create_channel(&self, name: String, is_public: bool) -> MvpResult<ChannelId> {
        info!(
            name = %name,
            is_public = is_public,
            user_id = %self.identity.user_id,
            "Creating channel"
        );

        // Generate unique IDs
        let channel_id = ChannelId::generate();
        let group_id = GroupId::new(channel_id.0.as_bytes().to_vec());

        // Step 1: Create MLS group
        debug!(group_id = ?group_id, "Creating MLS group");
        let actual_group_id = self.mls_service
            .create_group(self.identity.as_bytes(), Some(group_id.clone()))
            .await
            .map_err(|e| {
                warn!(error = ?e, "Failed to create MLS group");
                MvpError::Mls(e)
            })?;

        // Verify the group ID matches
        if actual_group_id != group_id {
            warn!(expected = ?group_id, actual = ?actual_group_id, "Group ID mismatch");
        }

        // Step 2: Create CRDT channel model
        debug!(channel_id = %channel_id, "Creating CRDT channel");
        let channel = Channel::new(
            channel_id.clone(),
            name.clone(),
            ChannelType::Text,
            self.identity.user_id.clone(),
            Timestamp::now(),
            self.identity.node_id.clone(),
        );

        self.store.store_channel(&channel).map_err(|e| {
            warn!(error = ?e, "Failed to store channel");
            MvpError::Store(e.to_string())
        })?;

        // Step 3: Publish to DHT (if public)
        if is_public {
            debug!(channel_id = %channel_id, "Publishing channel to DHT");
            // TODO: Implement DHT publication
            // For MVP, we'll skip this and use local discovery
        }

        info!(
            channel_id = %channel_id,
            group_id = ?group_id,
            "Channel created successfully"
        );

        Ok(channel_id)
    }

    /// Create an invite for a new member
    ///
    /// This generates an MLS Welcome message and exports the ratchet tree.
    ///
    /// # Arguments
    ///
    /// * `channel_id` - Target channel
    /// * `key_package` - Invitee's serialized key package
    ///
    /// # Returns
    ///
    /// An InviteToken containing the Welcome message and ratchet tree
    ///
    /// # Example
    ///
    /// ```ignore
    /// let invite = manager.create_invite(&channel_id, &bob_key_package).await?;
    /// ```
    pub async fn create_invite(
        &self,
        channel_id: &ChannelId,
        key_package: Vec<u8>,
    ) -> MvpResult<(InviteToken, Option<Vec<u8>>)> {
        info!(
            channel_id = %channel_id,
            inviter = %self.identity.user_id,
            "Creating invite"
        );

        // Get channel metadata to include in invite
        let channel = self
            .store
            .get_channel(channel_id)
            .map_err(|e| MvpError::Store(e.to_string()))?
            .ok_or_else(|| MvpError::ChannelNotFound(channel_id.to_string()))?;

        // Get group ID from channel
        let group_id = GroupId::new(channel_id.0.as_bytes().to_vec());

        // Add member via MLS service and get Welcome
        debug!("Adding member to MLS group");
        let (commit, welcome_bytes, ratchet_tree) = self
            .mls_service
            .add_members(&group_id, vec![key_package])
            .await?;

        if welcome_bytes.is_empty() {
            warn!("No Welcome message generated");
            return Err(MvpError::Internal("Failed to generate Welcome message".to_string()));
        }

        // Convert ratchet tree to Option (None if empty)
        let ratchet_tree_opt = if ratchet_tree.is_empty() {
            None
        } else {
            Some(ratchet_tree)
        };

        // Get channel name and is_public flag
        let channel_name = channel.get_name().cloned().unwrap_or_else(|| "Unnamed Channel".to_string());
        let is_public = false; // TODO: Get from channel metadata when implemented

        // Create invite token with channel metadata
        let invite = InviteToken::new(
            channel_id.clone(),
            welcome_bytes,
            ratchet_tree_opt,
            channel_name,
            is_public,
            self.identity.user_id.clone(),
        );

        info!(
            channel_id = %channel_id,
            invite_size = invite.welcome_blob.len(),
            "Invite created successfully"
        );

        // Return invite and the commit for existing members
        let commit_opt = if commit.is_empty() { None } else { Some(commit) };
        Ok((invite, commit_opt))
    }

    /// Join a channel from an invite
    ///
    /// This processes the Welcome message and syncs channel state.
    ///
    /// # Arguments
    ///
    /// * `invite` - The invite token to process
    ///
    /// # Returns
    ///
    /// The joined channel's ID
    ///
    /// # Example
    ///
    /// ```ignore
    /// let channel_id = manager.join_channel(&invite).await?;
    /// ```
    pub async fn join_channel(&self, invite: &InviteToken) -> MvpResult<ChannelId> {
        info!(
            channel_id = %invite.channel_id,
            user_id = %self.identity.user_id,
            "Joining channel"
        );

        // Check if invite is expired
        if invite.is_expired() {
            warn!("Invite has expired");
            return Err(MvpError::InvalidInvite("Invite has expired".to_string()));
        }

        // Join MLS group from Welcome
        debug!("Joining MLS group from Welcome");
        
        // Convert ratchet tree to Vec<u8> if present
        let ratchet_tree_vec = invite.ratchet_tree.clone();
        
        let group_id = self
            .mls_service
            .join_group(&invite.welcome_blob, ratchet_tree_vec)
            .await?;

        // Verify group ID matches channel ID
        let expected_group_id = GroupId::new(invite.channel_id.0.as_bytes().to_vec());
        if group_id != expected_group_id {
            warn!(
                expected = ?expected_group_id,
                got = ?group_id,
                "Group ID mismatch"
            );
            return Err(MvpError::Internal("Group ID mismatch after join".to_string()));
        }

        // Fetch or create channel metadata
        debug!(channel_id = %invite.channel_id, "Fetching channel metadata");
        match self.store.get_channel(&invite.channel_id) {
            Ok(Some(_channel)) => {
                // Channel exists, we're good
                debug!("Channel metadata found in local store");
            }
            Ok(None) => {
                // Create placeholder channel using metadata from invite
                debug!("Channel metadata not found, creating from invite");
                let channel = Channel::new(
                    invite.channel_id.clone(),
                    invite.channel_name.clone(),
                    ChannelType::Text,
                    invite.inviter.clone(),
                    Timestamp::now(),
                    self.identity.node_id.clone(),
                );
                self.store.store_channel(&channel).map_err(|e| {
                    MvpError::Store(e.to_string())
                })?;
            }
            Err(e) => {
                return Err(MvpError::Store(e.to_string()));
            }
        }

        info!(
            channel_id = %invite.channel_id,
            "Successfully joined channel"
        );

        Ok(invite.channel_id.clone())
    }

    /// Send a message to a channel
    ///
    /// This encrypts the message via MLS and returns the ciphertext.
    ///
    /// # Arguments
    ///
    /// * `channel_id` - Target channel
    /// * `plaintext` - Message body
    ///
    /// # Returns
    ///
    /// The encrypted message ciphertext
    ///
    /// # Example
    ///
    /// ```ignore
    /// let ciphertext = manager.send_message(&channel_id, b"Hello!").await?;
    /// ```
    pub async fn send_message(
        &self,
        channel_id: &ChannelId,
        plaintext: &[u8],
    ) -> MvpResult<Vec<u8>> {
        debug!(
            channel_id = %channel_id,
            size = plaintext.len(),
            "Sending message"
        );

        // Get group ID
        let group_id = GroupId::new(channel_id.0.as_bytes().to_vec());

        // Encrypt message via MLS service
        let ciphertext = self.mls_service.send_message(&group_id, plaintext).await?;

        info!(
            channel_id = %channel_id,
            plaintext_size = plaintext.len(),
            ciphertext_size = ciphertext.len(),
            "Message encrypted successfully"
        );

        Ok(ciphertext)
    }

    /// Receive and decrypt a message
    ///
    /// This decrypts an MLS message and returns the plaintext.
    ///
    /// # Arguments
    ///
    /// * `ciphertext` - Encrypted message bytes
    ///
    /// # Returns
    ///
    /// The decrypted message plaintext
    ///
    /// # Example
    ///
    /// ```ignore
    /// let plaintext = manager.receive_message(&ciphertext).await?;
    /// ```
    pub async fn receive_message(&self, ciphertext: &[u8]) -> MvpResult<Vec<u8>> {
        debug!(size = ciphertext.len(), "Receiving message");

        // For MVP, we need to try all groups until we find the right one
        let groups = self.mls_service.list_groups().await;

        for group_id in groups.iter() {
            // Try to process message with this group
            match self.mls_service.process_message(group_id, ciphertext).await {
                Ok(Some(plaintext)) => {
                    info!(
                        group_id = ?group_id,
                        plaintext_size = plaintext.len(),
                        "Message decrypted successfully"
                    );
                    return Ok(plaintext);
                }
                Ok(None) => {
                    // Commit or proposal, continue trying
                }
                Err(_e) => {
                    // Failed with this group, try next
                }
            }
        }

        warn!("Failed to decrypt message with any of {} groups", groups.len());
        Err(MvpError::InvalidMessage(
            "Could not decrypt message".to_string(),
        ))
    }

    /// Process a commit message from the group
    ///
    /// This updates the member's group state when other members add/remove participants
    /// or perform other group operations. Essential for keeping epochs in sync.
    ///
    /// # Arguments
    ///
    /// * `commit` - Serialized commit message
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Alice adds Charlie, gets commit for Bob
    /// let (invite, Some(commit)) = alice.create_invite(&channel_id, charlie_kp).await?;
    /// // Bob processes commit to advance epoch
    /// bob.process_commit(&commit).await?;
    /// ```
    pub async fn process_commit(&self, commit: &[u8]) -> MvpResult<()> {
        debug!(size = commit.len(), "Processing commit");

        // Try all groups until we find the right one
        let groups = self.mls_service.list_groups().await;

        for group_id in groups.iter() {
            // Try to process commit with this group
            match self.mls_service.process_message(group_id, commit).await {
                Ok(Some(_)) => {
                    // This shouldn't happen for commits, but if it does, it worked
                    info!(group_id = ?group_id, "Commit processed (unexpected app message)");
                    return Ok(());
                }
                Ok(None) => {
                    // Commit or proposal processed successfully
                    info!(group_id = ?group_id, "Commit processed successfully");
                    return Ok(());
                }
                Err(_e) => {
                    // Failed with this group, try next
                }
            }
        }

        warn!("Failed to process commit with any of {} groups", groups.len());
        Err(MvpError::InvalidMessage(
            "Could not process commit".to_string(),
        ))
    }

    /// Remove a member from the channel
    ///
    /// This generates an MLS commit that removes the specified member.
    /// All remaining members must process this commit to stay in sync.
    ///
    /// # Arguments
    ///
    /// * `channel_id` - Target channel
    /// * `member_identity` - Identity bytes of the member to remove
    ///
    /// # Returns
    ///
    /// Serialized commit message to broadcast to remaining members
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Alice removes Bob from the channel
    /// let bob_identity = bob_user_id.0.as_bytes();
    /// let commit = alice.remove_member(&channel_id, bob_identity).await?;
    /// 
    /// // Charlie and Dave process the removal commit
    /// charlie.process_commit(&commit).await?;
    /// dave.process_commit(&commit).await?;
    /// ```
    pub async fn remove_member(
        &self,
        channel_id: &ChannelId,
        member_identity: &[u8],
    ) -> MvpResult<Vec<u8>> {
        info!(
            channel_id = %channel_id,
            member = ?std::str::from_utf8(member_identity).unwrap_or("<non-utf8>"),
            "Removing member from channel"
        );

        // Map channel ID to group ID
        let group_id = GroupId::new(channel_id.0.as_bytes().to_vec());

        // Get group metadata to find the member's leaf index
        let metadata = self
            .mls_service
            .get_group_metadata(&group_id)
            .await
            .map_err(|e| {
                warn!(error = ?e, "Failed to get group metadata");
                MvpError::Mls(e)
            })?;

        // Find the member's leaf index
        let leaf_index = metadata
            .members
            .iter()
            .find(|m| m.identity == member_identity)
            .map(|m| m.leaf_index)
            .ok_or_else(|| {
                warn!(
                    member = ?std::str::from_utf8(member_identity).unwrap_or("<non-utf8>"),
                    "Member not found in group"
                );
                MvpError::InvalidOperation(format!(
                    "Member {} not found in channel",
                    std::str::from_utf8(member_identity).unwrap_or("<non-utf8>")
                ))
            })?;

        debug!(leaf_index, "Found member's leaf index");

        // Remove the member via MLS service
        let commit = self
            .mls_service
            .remove_members(&group_id, vec![leaf_index])
            .await
            .map_err(|e| {
                warn!(error = ?e, "Failed to remove member");
                MvpError::Mls(e)
            })?;

        info!(
            channel_id = %channel_id,
            leaf_index,
            commit_size = commit.len(),
            "Member removed successfully"
        );

        Ok(commit)
    }

    /// Get channel metadata
    ///
    /// # Arguments
    ///
    /// * `channel_id` - Channel to query
    ///
    /// # Returns
    ///
    /// Channel descriptor if found
    pub async fn get_channel(&self, channel_id: &ChannelId) -> MvpResult<ChannelDescriptor> {
        let channel = self
            .store
            .get_channel(channel_id)
            .map_err(|e| MvpError::Store(e.to_string()))?
            .ok_or_else(|| MvpError::ChannelNotFound(channel_id.to_string()))?;

        let group_id = GroupId::new(channel_id.0.as_bytes().to_vec());

        Ok(ChannelDescriptor::new(
            channel_id.clone(),
            channel.created_by.clone(),
            channel.get_name().cloned().unwrap_or_default(),
            false, // TODO: Track public/private in Channel model
            group_id,
        ))
    }

    /// List all channels for current user
    pub async fn list_channels(&self) -> MvpResult<Vec<ChannelDescriptor>> {
        // For MVP, list all groups from MLS service
        let group_ids = self.mls_service.list_groups().await;

        let mut channels = Vec::new();
        for group_id in group_ids {
            // Convert group ID to channel ID
            // For MVP, we use a simple conversion (group ID bytes → string → ChannelId)
            let channel_id_str = String::from_utf8_lossy(group_id.as_bytes()).to_string();
            let channel_id = ChannelId(channel_id_str);

            if let Ok(descriptor) = self.get_channel(&channel_id).await {
                channels.push(descriptor);
            }
        }

        Ok(channels)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::Config,
        core_store::store::local_store::LocalStoreConfig,
        shutdown::ShutdownCoordinator,
    };
    use tempfile::tempdir;

    async fn create_test_manager() -> (Arc<ChannelManager>, tempfile::TempDir) {
        let temp_dir = tempdir().unwrap();
        let config = Arc::new(Config::default());
        let shutdown = Arc::new(ShutdownCoordinator::new(std::time::Duration::from_secs(5)));

        // Create MLS service
        let mls_service = Arc::new(MlsService::new(&config, shutdown));

        // Create store
        let store_config = LocalStoreConfig {
            data_dir: temp_dir.path().to_path_buf(),
            enable_encryption: false,
            require_signatures: false,
            authorized_keys: Vec::new(),
            ..Default::default()
        };
        let store = Arc::new(LocalStore::new(store_config).unwrap());

        // Create identity
        let identity = Arc::new(Identity::new(
            UserId::generate(),
            "test_user".to_string(),
            "node_1".to_string(),
        ));

        let manager = Arc::new(ChannelManager::new(
            mls_service,
            store,
            identity,
            config,
        ));

        (manager, temp_dir)
    }

    #[tokio::test]
    async fn test_create_channel() {
        let (manager, _temp_dir) = create_test_manager().await;

        let channel_id = manager
            .create_channel("test_channel".to_string(), false)
            .await
            .unwrap();

        // Verify channel exists
        let descriptor = manager.get_channel(&channel_id).await.unwrap();
        assert_eq!(descriptor.name, "test_channel");
    }

    #[tokio::test]
    async fn test_list_channels() {
        let (manager, _temp_dir) = create_test_manager().await;

        // Create multiple channels
        let _ch1 = manager.create_channel("channel1".to_string(), false).await.unwrap();
        let _ch2 = manager.create_channel("channel2".to_string(), true).await.unwrap();

        let channels = manager.list_channels().await.unwrap();
        assert_eq!(channels.len(), 2);
    }
}
