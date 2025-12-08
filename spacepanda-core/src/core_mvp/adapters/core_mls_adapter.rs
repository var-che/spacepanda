//! CoreMLS Adapter - GroupProvider implementation for our custom core_mls
//!
//! This adapter wraps our existing MlsService to implement the GroupProvider trait,
//! enabling ChannelManager to work with a clean abstraction.

use crate::core_mls::{service::MlsService, types::GroupId};
use crate::core_mvp::{
    errors::{MvpError, MvpResult},
    group_provider::{GroupConfig, GroupHandle, GroupProvider, Welcome},
};
use async_trait::async_trait;
use std::sync::Arc;

/// Adapter that implements GroupProvider using our core_mls::MlsService
pub struct CoreMlsAdapter {
    /// Underlying MLS service
    mls_service: Arc<MlsService>,
}

impl CoreMlsAdapter {
    /// Create a new CoreMLS adapter
    ///
    /// # Arguments
    ///
    /// * `mls_service` - MLS service instance
    pub fn new(mls_service: Arc<MlsService>) -> Self {
        Self { mls_service }
    }

    /// Convert GroupHandle to GroupId
    fn to_group_id(handle: &GroupHandle) -> GroupId {
        GroupId::new(handle.id.clone())
    }

    /// Convert GroupId to GroupHandle
    fn from_group_id(group_id: GroupId) -> GroupHandle {
        GroupHandle::new(group_id.as_bytes().to_vec())
    }
}

#[async_trait]
impl GroupProvider for CoreMlsAdapter {
    async fn generate_key_package(&self, identity: &[u8]) -> MvpResult<Vec<u8>> {
        self.mls_service
            .generate_key_package(identity.to_vec())
            .await
            .map_err(|e| MvpError::Mls(e))
    }

    async fn create_group(&self, identity: &[u8], config: GroupConfig) -> MvpResult<GroupHandle> {
        let group_id_opt = config.group_id.map(GroupId::new);

        let group_id = self
            .mls_service
            .create_group(identity.to_vec(), group_id_opt)
            .await
            .map_err(|e| MvpError::Mls(e))?;

        Ok(Self::from_group_id(group_id))
    }

    async fn create_welcome(
        &self,
        handle: &GroupHandle,
        key_packages: Vec<Vec<u8>>,
    ) -> MvpResult<Welcome> {
        let group_id = Self::to_group_id(handle);

        let (commit, welcome_blob, ratchet_tree) = self
            .mls_service
            .add_members(&group_id, key_packages)
            .await
            .map_err(|e| MvpError::Mls(e))?;

        // Include ratchet tree if it was exported (non-empty)
        let ratchet_tree_opt = if ratchet_tree.is_empty() {
            None
        } else {
            Some(ratchet_tree)
        };

        Ok(Welcome { blob: welcome_blob, ratchet_tree: ratchet_tree_opt })
    }

    async fn join_from_welcome(
        &self,
        welcome: &Welcome,
        identity: &[u8],
    ) -> MvpResult<GroupHandle> {
        let group_id = self
            .mls_service
            .join_group(&welcome.blob, welcome.ratchet_tree.clone())
            .await
            .map_err(|e| MvpError::Mls(e))?;

        Ok(Self::from_group_id(group_id))
    }

    async fn seal_message(&self, handle: &GroupHandle, plaintext: &[u8]) -> MvpResult<Vec<u8>> {
        let group_id = Self::to_group_id(handle);

        let ciphertext = self
            .mls_service
            .send_message(&group_id, plaintext)
            .await
            .map_err(|e| MvpError::Mls(e))?;

        Ok(ciphertext)
    }

    async fn open_message(&self, handle: &GroupHandle, ciphertext: &[u8]) -> MvpResult<Vec<u8>> {
        let group_id = Self::to_group_id(handle);

        let plaintext = self
            .mls_service
            .process_message(&group_id, ciphertext)
            .await
            .map_err(|e| MvpError::Mls(e))?
            .ok_or_else(|| MvpError::InvalidMessage("Failed to decrypt".to_string()))?;

        Ok(plaintext)
    }

    async fn propose_add(
        &self,
        handle: &GroupHandle,
        key_packages: Vec<Vec<u8>>,
    ) -> MvpResult<Vec<u8>> {
        let group_id = Self::to_group_id(handle);

        let (commit, _welcome, _ratchet_tree) = self
            .mls_service
            .add_members(&group_id, key_packages)
            .await
            .map_err(|e| MvpError::Mls(e))?;

        Ok(commit)
    }

    async fn propose_remove(
        &self,
        handle: &GroupHandle,
        member_indices: Vec<u32>,
    ) -> MvpResult<Vec<u8>> {
        let group_id = Self::to_group_id(handle);

        if member_indices.is_empty() {
            return Err(MvpError::InvalidMessage("No members to remove".to_string()));
        }

        let commit = self
            .mls_service
            .remove_members(&group_id, member_indices)
            .await
            .map_err(|e| MvpError::Mls(e))?;

        Ok(commit)
    }

    async fn epoch(&self, handle: &GroupHandle) -> MvpResult<u64> {
        let group_id = Self::to_group_id(handle);

        let metadata =
            self.mls_service.get_metadata(&group_id).await.map_err(|e| MvpError::Mls(e))?;

        Ok(metadata.epoch)
    }

    async fn member_count(&self, handle: &GroupHandle) -> MvpResult<usize> {
        let group_id = Self::to_group_id(handle);

        let metadata =
            self.mls_service.get_metadata(&group_id).await.map_err(|e| MvpError::Mls(e))?;

        Ok(metadata.members.len())
    }

    async fn list_groups(&self) -> MvpResult<Vec<GroupHandle>> {
        let group_ids = self.mls_service.list_groups().await;

        let handles = group_ids.into_iter().map(Self::from_group_id).collect();

        Ok(handles)
    }

    async fn export_ratchet_tree(&self, handle: &GroupHandle) -> MvpResult<Vec<u8>> {
        let group_id = Self::to_group_id(handle);

        // Get the group adapter
        let groups = self.mls_service.list_groups().await;
        if !groups.contains(&group_id) {
            return Err(MvpError::Mls(crate::core_mls::errors::MlsError::GroupNotFound(
                group_id.to_string(),
            )));
        }

        // Export ratchet tree via MlsService
        self.mls_service
            .export_ratchet_tree(&group_id)
            .await
            .map_err(|e| MvpError::Mls(e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{config::Config, shutdown::ShutdownCoordinator};
    use std::time::Duration;

    async fn create_test_adapter() -> CoreMlsAdapter {
        let config = Arc::new(Config::default());
        let shutdown = Arc::new(ShutdownCoordinator::new(Duration::from_secs(5)));
        let mls_service = Arc::new(MlsService::new(&config, shutdown));

        CoreMlsAdapter::new(mls_service)
    }

    #[tokio::test]
    async fn test_create_group() {
        let adapter = create_test_adapter().await;
        let identity = b"test@spacepanda.local";
        let config = GroupConfig::default();

        let handle = adapter.create_group(identity, config).await.unwrap();

        assert!(!handle.id.is_empty());
    }

    #[tokio::test]
    async fn test_list_groups() {
        let adapter = create_test_adapter().await;

        // Create two groups
        let _group1 = adapter.create_group(b"alice", GroupConfig::default()).await.unwrap();
        let _group2 = adapter.create_group(b"bob", GroupConfig::default()).await.unwrap();

        let groups = adapter.list_groups().await.unwrap();

        assert_eq!(groups.len(), 2);
    }

    #[tokio::test]
    async fn test_group_metadata() {
        let adapter = create_test_adapter().await;
        let handle = adapter.create_group(b"test", GroupConfig::default()).await.unwrap();

        let epoch = adapter.epoch(&handle).await.unwrap();
        let count = adapter.member_count(&handle).await.unwrap();

        assert_eq!(epoch, 0); // Initial epoch
        assert_eq!(count, 1); // Just the creator
    }
}
