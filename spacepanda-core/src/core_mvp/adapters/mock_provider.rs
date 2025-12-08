//! Mock GroupProvider for testing
//!
//! This mock implementation allows testing ChannelManager logic
//! without requiring a full MLS setup.

use crate::core_mvp::{
    errors::{MvpError, MvpResult},
    group_provider::{GroupConfig, GroupHandle, GroupProvider, Welcome},
};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
struct MockGroup {
    id: Vec<u8>,
    epoch: u64,
    members: Vec<Vec<u8>>,
}

/// Mock GroupProvider for testing without real MLS
pub struct MockGroupProvider {
    groups: Arc<Mutex<HashMap<Vec<u8>, MockGroup>>>,
}

impl MockGroupProvider {
    /// Create a new mock provider
    pub fn new() -> Self {
        Self { groups: Arc::new(Mutex::new(HashMap::new())) }
    }
}

impl Default for MockGroupProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl GroupProvider for MockGroupProvider {
    async fn generate_key_package(&self, identity: &[u8]) -> MvpResult<Vec<u8>> {
        // For mock: just return the identity as the key package
        // In a real implementation, this would generate actual crypto material
        Ok(identity.to_vec())
    }

    async fn create_group(&self, identity: &[u8], config: GroupConfig) -> MvpResult<GroupHandle> {
        let group_id = config
            .group_id
            .unwrap_or_else(|| format!("group-{}", uuid::Uuid::new_v4()).into_bytes());

        let group = MockGroup { id: group_id.clone(), epoch: 0, members: vec![identity.to_vec()] };

        self.groups.lock().unwrap().insert(group_id.clone(), group);

        Ok(GroupHandle::new(group_id))
    }

    async fn create_welcome(
        &self,
        handle: &GroupHandle,
        key_packages: Vec<Vec<u8>>,
    ) -> MvpResult<Welcome> {
        // Mock welcome: just serialize the group ID and new member identities
        let mut groups = self.groups.lock().unwrap();
        let group = groups
            .get_mut(handle.as_bytes())
            .ok_or_else(|| MvpError::ChannelNotFound("Group not found".to_string()))?;

        // Add members
        for kp in &key_packages {
            group.members.push(kp.clone());
        }
        group.epoch += 1;

        let welcome = Welcome { blob: handle.as_bytes().to_vec(), ratchet_tree: None };

        Ok(welcome)
    }

    async fn join_from_welcome(
        &self,
        welcome: &Welcome,
        identity: &[u8],
    ) -> MvpResult<GroupHandle> {
        // Mock join: the welcome blob contains the group ID
        let group_id = welcome.blob.clone();
        let handle = GroupHandle::new(group_id);

        // Verify group exists
        let groups = self.groups.lock().unwrap();
        if !groups.contains_key(handle.as_bytes()) {
            return Err(MvpError::InvalidInvite("Group not found".to_string()));
        }

        Ok(handle)
    }

    async fn seal_message(&self, handle: &GroupHandle, plaintext: &[u8]) -> MvpResult<Vec<u8>> {
        // Mock encryption: just prepend "ENCRYPTED:" prefix
        let mut ciphertext = b"ENCRYPTED:".to_vec();
        ciphertext.extend_from_slice(plaintext);
        Ok(ciphertext)
    }

    async fn open_message(&self, handle: &GroupHandle, ciphertext: &[u8]) -> MvpResult<Vec<u8>> {
        // Mock decryption: strip "ENCRYPTED:" prefix
        if !ciphertext.starts_with(b"ENCRYPTED:") {
            return Err(MvpError::InvalidMessage("Bad ciphertext".to_string()));
        }
        Ok(ciphertext[10..].to_vec())
    }

    async fn propose_add(
        &self,
        handle: &GroupHandle,
        key_packages: Vec<Vec<u8>>,
    ) -> MvpResult<Vec<u8>> {
        let mut groups = self.groups.lock().unwrap();
        let group = groups
            .get_mut(handle.as_bytes())
            .ok_or_else(|| MvpError::ChannelNotFound("Group not found".to_string()))?;

        for kp in key_packages {
            group.members.push(kp);
        }
        group.epoch += 1;

        Ok(b"COMMIT:ADD".to_vec())
    }

    async fn propose_remove(
        &self,
        handle: &GroupHandle,
        member_indices: Vec<u32>,
    ) -> MvpResult<Vec<u8>> {
        let mut groups = self.groups.lock().unwrap();
        let group = groups
            .get_mut(handle.as_bytes())
            .ok_or_else(|| MvpError::ChannelNotFound("Group not found".to_string()))?;

        // Remove members (in reverse order to maintain indices)
        let mut indices = member_indices;
        indices.sort_by(|a, b| b.cmp(a));
        for idx in indices {
            if (idx as usize) < group.members.len() {
                group.members.remove(idx as usize);
            }
        }
        group.epoch += 1;

        Ok(b"COMMIT:REMOVE".to_vec())
    }

    async fn epoch(&self, handle: &GroupHandle) -> MvpResult<u64> {
        let groups = self.groups.lock().unwrap();
        let group = groups
            .get(handle.as_bytes())
            .ok_or_else(|| MvpError::ChannelNotFound("Group not found".to_string()))?;

        Ok(group.epoch)
    }

    async fn member_count(&self, handle: &GroupHandle) -> MvpResult<usize> {
        let groups = self.groups.lock().unwrap();
        let group = groups
            .get(handle.as_bytes())
            .ok_or_else(|| MvpError::ChannelNotFound("Group not found".to_string()))?;

        Ok(group.members.len())
    }

    async fn list_groups(&self) -> MvpResult<Vec<GroupHandle>> {
        let groups = self.groups.lock().unwrap();
        let handles = groups.keys().map(|id| GroupHandle::new(id.clone())).collect();

        Ok(handles)
    }

    async fn export_ratchet_tree(&self, handle: &GroupHandle) -> MvpResult<Vec<u8>> {
        Ok(b"MOCK_TREE".to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_create_and_list() {
        let provider = MockGroupProvider::new();

        let handle1 = provider.create_group(b"alice", GroupConfig::default()).await.unwrap();
        let handle2 = provider.create_group(b"bob", GroupConfig::default()).await.unwrap();

        let groups = provider.list_groups().await.unwrap();
        assert_eq!(groups.len(), 2);
    }

    #[tokio::test]
    async fn test_mock_encrypt_decrypt() {
        let provider = MockGroupProvider::new();
        let handle = provider.create_group(b"test", GroupConfig::default()).await.unwrap();

        let plaintext = b"Hello, World!";
        let ciphertext = provider.seal_message(&handle, plaintext).await.unwrap();
        let decrypted = provider.open_message(&handle, &ciphertext).await.unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[tokio::test]
    async fn test_mock_welcome_join() {
        let provider = MockGroupProvider::new();
        let handle = provider.create_group(b"alice", GroupConfig::default()).await.unwrap();

        let welcome = provider.create_welcome(&handle, vec![b"bob".to_vec()]).await.unwrap();

        let joined = provider.join_from_welcome(&welcome, b"bob").await.unwrap();

        assert_eq!(joined.id, handle.id);
    }

    #[tokio::test]
    async fn test_mock_member_operations() {
        let provider = MockGroupProvider::new();
        let handle = provider.create_group(b"alice", GroupConfig::default()).await.unwrap();

        // Initial: 1 member
        let count1 = provider.member_count(&handle).await.unwrap();
        assert_eq!(count1, 1);

        // Add 2 members
        provider
            .propose_add(&handle, vec![b"bob".to_vec(), b"charlie".to_vec()])
            .await
            .unwrap();

        let count2 = provider.member_count(&handle).await.unwrap();
        assert_eq!(count2, 3);

        // Remove 1 member
        provider.propose_remove(&handle, vec![1]).await.unwrap();

        let count3 = provider.member_count(&handle).await.unwrap();
        assert_eq!(count3, 2);
    }

    #[tokio::test]
    async fn test_mock_epoch_tracking() {
        let provider = MockGroupProvider::new();
        let handle = provider.create_group(b"test", GroupConfig::default()).await.unwrap();

        let epoch0 = provider.epoch(&handle).await.unwrap();
        assert_eq!(epoch0, 0);

        provider.propose_add(&handle, vec![b"member".to_vec()]).await.unwrap();

        let epoch1 = provider.epoch(&handle).await.unwrap();
        assert_eq!(epoch1, 1);
    }
}
