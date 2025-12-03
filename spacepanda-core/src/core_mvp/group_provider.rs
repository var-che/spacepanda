//! GroupProvider Trait - Abstraction over MLS implementations
//!
//! This trait provides a clean abstraction layer over MLS group operations,
//! enabling:
//! - Easy migration from custom core_mls to OpenMLS
//! - Testability via mock implementations
//! - Reduced coupling in ChannelManager
//!
//! # Architecture
//!
//! ```text
//! ChannelManager
//!       |
//!       v
//! GroupProvider (trait)
//!       |
//!       +---> CoreMlsAdapter (current implementation)
//!       |
//!       +---> OpenMlsAdapter (future migration)
//!       |
//!       +---> MockGroupProvider (for testing)
//! ```

use crate::core_mvp::errors::MvpResult;
use async_trait::async_trait;

/// Group handle that encapsulates MLS group state
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GroupHandle {
    /// Opaque group identifier
    pub id: Vec<u8>,
}

impl GroupHandle {
    /// Create a new group handle
    pub fn new(id: Vec<u8>) -> Self {
        Self { id }
    }

    /// Get the group ID bytes
    pub fn as_bytes(&self) -> &[u8] {
        &self.id
    }
}

/// Welcome message for inviting new members
#[derive(Debug, Clone)]
pub struct Welcome {
    /// Serialized Welcome message (MLS format)
    pub blob: Vec<u8>,
    
    /// Optional ratchet tree export for the group
    pub ratchet_tree: Option<Vec<u8>>,
}

/// Configuration for creating a new group
#[derive(Debug, Clone)]
pub struct GroupConfig {
    /// Optional specific group ID (None = auto-generate)
    pub group_id: Option<Vec<u8>>,
    
    /// Group ciphersuite (e.g., MLS10_128_DHKEMX25519_AES128GCM_SHA256_Ed25519)
    pub ciphersuite: u16,
    
    /// Enable ratchet tree export in Welcomes
    pub export_tree: bool,
}

impl Default for GroupConfig {
    fn default() -> Self {
        Self {
            group_id: None,
            ciphersuite: 1, // MLS default ciphersuite
            export_tree: false,
        }
    }
}

/// Abstraction over MLS group operations
///
/// This trait allows ChannelManager to work with different MLS implementations
/// without coupling to specific provider details.
#[async_trait]
pub trait GroupProvider: Send + Sync {
    /// Create a new MLS group
    ///
    /// # Arguments
    ///
    /// * `identity` - Creator's identity bytes
    /// * `config` - Group configuration
    ///
    /// # Returns
    ///
    /// Handle to the created group
    async fn create_group(
        &self,
        identity: &[u8],
        config: GroupConfig,
    ) -> MvpResult<GroupHandle>;

    /// Create a Welcome message for new members
    ///
    /// # Arguments
    ///
    /// * `handle` - Group handle
    /// * `key_packages` - Key packages of members to add
    ///
    /// # Returns
    ///
    /// Welcome message containing secrets for new members
    async fn create_welcome(
        &self,
        handle: &GroupHandle,
        key_packages: Vec<Vec<u8>>,
    ) -> MvpResult<Welcome>;

    /// Join a group from a Welcome message
    ///
    /// # Arguments
    ///
    /// * `welcome` - Welcome message from group admin
    /// * `identity` - Joiner's identity bytes
    ///
    /// # Returns
    ///
    /// Handle to the joined group
    async fn join_from_welcome(
        &self,
        welcome: &Welcome,
        identity: &[u8],
    ) -> MvpResult<GroupHandle>;

    /// Seal (encrypt) a message for the group
    ///
    /// # Arguments
    ///
    /// * `handle` - Group handle
    /// * `plaintext` - Message plaintext
    ///
    /// # Returns
    ///
    /// Encrypted message ciphertext
    async fn seal_message(
        &self,
        handle: &GroupHandle,
        plaintext: &[u8],
    ) -> MvpResult<Vec<u8>>;

    /// Open (decrypt) a message from the group
    ///
    /// # Arguments
    ///
    /// * `handle` - Group handle
    /// * `ciphertext` - Encrypted message
    ///
    /// # Returns
    ///
    /// Decrypted message plaintext
    async fn open_message(
        &self,
        handle: &GroupHandle,
        ciphertext: &[u8],
    ) -> MvpResult<Vec<u8>>;

    /// Add members to the group
    ///
    /// # Arguments
    ///
    /// * `handle` - Group handle
    /// * `key_packages` - Key packages of members to add
    ///
    /// # Returns
    ///
    /// Commit message to be sent to group members
    async fn propose_add(
        &self,
        handle: &GroupHandle,
        key_packages: Vec<Vec<u8>>,
    ) -> MvpResult<Vec<u8>>;

    /// Remove members from the group
    ///
    /// # Arguments
    ///
    /// * `handle` - Group handle
    /// * `member_indices` - Leaf indices of members to remove
    ///
    /// # Returns
    ///
    /// Commit message to be sent to group members
    async fn propose_remove(
        &self,
        handle: &GroupHandle,
        member_indices: Vec<u32>,
    ) -> MvpResult<Vec<u8>>;

    /// Get current epoch number for the group
    ///
    /// # Arguments
    ///
    /// * `handle` - Group handle
    ///
    /// # Returns
    ///
    /// Current epoch number
    async fn epoch(&self, handle: &GroupHandle) -> MvpResult<u64>;

    /// Get member count for the group
    ///
    /// # Arguments
    ///
    /// * `handle` - Group handle
    ///
    /// # Returns
    ///
    /// Number of members in the group
    async fn member_count(&self, handle: &GroupHandle) -> MvpResult<usize>;

    /// List all groups managed by this provider
    ///
    /// # Returns
    ///
    /// List of group handles
    async fn list_groups(&self) -> MvpResult<Vec<GroupHandle>>;

    /// Export ratchet tree for a group
    ///
    /// # Arguments
    ///
    /// * `handle` - Group handle
    ///
    /// # Returns
    ///
    /// Serialized ratchet tree
    async fn export_ratchet_tree(&self, handle: &GroupHandle) -> MvpResult<Vec<u8>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_group_handle() {
        let id = b"test-group-id".to_vec();
        let handle = GroupHandle::new(id.clone());
        
        assert_eq!(handle.as_bytes(), id.as_slice());
    }

    #[test]
    fn test_group_config_default() {
        let config = GroupConfig::default();
        
        assert!(config.group_id.is_none());
        assert_eq!(config.ciphersuite, 1);
        assert!(!config.export_tree);
    }

    #[test]
    fn test_welcome_structure() {
        let welcome = Welcome {
            blob: vec![1, 2, 3],
            ratchet_tree: Some(vec![4, 5, 6]),
        };
        
        assert_eq!(welcome.blob.len(), 3);
        assert!(welcome.ratchet_tree.is_some());
    }
}
