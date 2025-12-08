//! MlsHandle Adapter for OpenMLS Engine
//!
//! This module provides a compatibility layer that allows the existing MlsHandle
//! API to work with the new OpenMlsEngine backend, enabling gradual migration.

use crate::core_mls::{
    engine::openmls_engine::OpenMlsEngine,
    errors::MlsResult,
    state::GroupSnapshot,
    types::{GroupId, GroupMetadata, MlsConfig},
};
use openmls::prelude::KeyPackageBundle;
use openmls_traits::OpenMlsProvider;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Adapter that wraps OpenMlsEngine to provide MlsHandle-compatible API
///
/// This allows gradual migration from the legacy MlsGroup to OpenMLS
/// while maintaining backward compatibility with existing code.
pub struct OpenMlsHandleAdapter<P: OpenMlsProvider> {
    /// The underlying OpenMLS engine
    engine: Arc<RwLock<OpenMlsEngine<P>>>,

    /// Configuration
    config: MlsConfig,
}

impl<P: OpenMlsProvider + 'static> OpenMlsHandleAdapter<P> {
    /// Create a new group using OpenMLS engine
    ///
    /// # Arguments
    /// * `group_id` - Optional group ID (will generate random if None)
    /// * `identity` - Member identity (username/user ID)
    /// * `config` - Group configuration
    /// Create a new group
    ///
    /// # Arguments
    /// * `group_id` - Optional group ID (random if None)
    /// * `identity` - Creator's identity
    /// * `config` - MLS configuration
    /// * `provider` - Shared crypto provider for key continuity
    pub async fn create_group(
        group_id: Option<GroupId>,
        identity: Vec<u8>,
        config: MlsConfig,
        provider: Arc<P>,
    ) -> MlsResult<Self> {
        let gid = group_id.unwrap_or_else(GroupId::random);

        let engine = OpenMlsEngine::create_group(gid, identity, config.clone(), provider).await?;

        Ok(Self { engine: Arc::new(RwLock::new(engine)), config })
    }

    /// Create adapter from an existing OpenMlsEngine
    ///
    /// This is used when restoring groups from persistence.
    pub fn from_engine(engine: OpenMlsEngine<P>, config: MlsConfig) -> Self {
        Self { engine: Arc::new(RwLock::new(engine)), config }
    }

    /// Join an existing group from a Welcome message
    ///
    /// # Arguments
    /// * `welcome_bytes` - Serialized Welcome message
    /// * `ratchet_tree` - Optional ratchet tree bytes (if not in Welcome)
    /// * `config` - MLS configuration
    /// * `key_package_bundle` - Optional KeyPackageBundle with private keys for decryption
    /// * `provider` - Shared crypto provider (must match the one used for key package generation)
    pub async fn join_from_welcome(
        welcome_bytes: &[u8],
        ratchet_tree: Option<Vec<u8>>,
        config: MlsConfig,
        key_package_bundle: Option<KeyPackageBundle>,
        provider: Arc<P>,
    ) -> MlsResult<Self> {
        let engine = OpenMlsEngine::join_from_welcome(
            welcome_bytes,
            ratchet_tree,
            config.clone(),
            key_package_bundle,
            provider,
        )
        .await?;

        Ok(Self { engine: Arc::new(RwLock::new(engine)), config })
    }

    /// Get the group ID
    pub async fn group_id(&self) -> GroupId {
        let engine = self.engine.read().await;
        engine.group_id().await
    }

    /// Get the current epoch
    pub async fn epoch(&self) -> u64 {
        let engine = self.engine.read().await;
        engine.epoch().await
    }

    /// Get group metadata
    pub async fn metadata(&self) -> MlsResult<GroupMetadata> {
        let engine = self.engine.read().await;
        engine.metadata().await
    }

    /// Get the configuration
    pub fn config(&self) -> &MlsConfig {
        &self.config
    }

    /// Get a clone of the underlying engine (for advanced operations)
    pub fn engine(&self) -> Arc<RwLock<OpenMlsEngine<P>>> {
        Arc::clone(&self.engine)
    }

    /// Export group state as snapshot
    ///
    /// Creates an atomic snapshot of the current group state,
    /// suitable for backup, disaster recovery, or CRDT integration.
    ///
    /// # Returns
    /// * `GroupSnapshot` - Complete group state at current epoch
    pub async fn export_snapshot(&self) -> MlsResult<GroupSnapshot> {
        let engine = self.engine.read().await;
        engine.export_snapshot().await
    }

    /// Save snapshot to bytes
    ///
    /// Convenience method that exports snapshot and serializes to bytes.
    ///
    /// # Returns
    /// * Serialized snapshot bytes suitable for storage
    pub async fn save_snapshot(&self) -> MlsResult<Vec<u8>> {
        let snapshot = self.export_snapshot().await?;
        snapshot.to_bytes()
    }

    /// Load snapshot from bytes
    ///
    /// Deserializes a snapshot from bytes. Note: This only deserializes,
    /// it does not restore the group state into the engine.
    ///
    /// # Arguments
    /// * `bytes` - Serialized snapshot bytes
    ///
    /// # Returns
    /// * `GroupSnapshot` - Deserialized snapshot
    pub fn load_snapshot(bytes: &[u8]) -> MlsResult<GroupSnapshot> {
        GroupSnapshot::from_bytes(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openmls_rust_crypto::OpenMlsRustCrypto;

    #[tokio::test]
    async fn test_adapter_create_group() {
        let config = MlsConfig::default();
        let identity = b"alice@example.com".to_vec();
        let provider = Arc::new(OpenMlsRustCrypto::default());

        let adapter = OpenMlsHandleAdapter::create_group(None, identity, config, provider)
            .await
            .expect("Failed to create group via adapter");

        assert_eq!(adapter.epoch().await, 0);

        let metadata = adapter.metadata().await.expect("Failed to get metadata");
        assert_eq!(metadata.epoch, 0);
        assert_eq!(metadata.members.len(), 1);
    }

    #[tokio::test]
    async fn test_adapter_get_group_id() {
        let config = MlsConfig::default();
        let group_id = GroupId::random();
        let identity = b"bob@example.com".to_vec();
        let provider = Arc::new(OpenMlsRustCrypto::default());

        let adapter =
            OpenMlsHandleAdapter::create_group(Some(group_id.clone()), identity, config, provider)
                .await
                .expect("Failed to create group");

        let actual_id = adapter.group_id().await;
        assert_eq!(actual_id, group_id);
    }

    #[tokio::test]
    async fn test_adapter_multiple_instances() {
        let config = MlsConfig::default();
        let provider = Arc::new(OpenMlsRustCrypto::default());

        let adapter1 = OpenMlsHandleAdapter::create_group(
            None,
            b"user1@example.com".to_vec(),
            config.clone(),
            provider.clone(),
        )
        .await
        .expect("Failed to create adapter 1");

        let adapter2 = OpenMlsHandleAdapter::create_group(
            None,
            b"user2@example.com".to_vec(),
            config,
            provider.clone(),
        )
        .await
        .expect("Failed to create adapter 2");

        let id1 = adapter1.group_id().await;
        let id2 = adapter2.group_id().await;

        assert_ne!(id1, id2, "Different adapters should have different group IDs");
    }

    #[tokio::test]
    async fn test_adapter_snapshot_export() {
        let config = MlsConfig::default();
        let identity = b"alice@example.com".to_vec();
        let provider = Arc::new(OpenMlsRustCrypto::default());

        let adapter = OpenMlsHandleAdapter::create_group(None, identity, config, provider)
            .await
            .expect("Failed to create group");

        // Export snapshot
        let snapshot = adapter.export_snapshot().await.expect("Failed to export snapshot");

        // Verify snapshot contents
        assert_eq!(snapshot.epoch(), 0);
        assert_eq!(snapshot.members().len(), 1);
        assert!(!snapshot.ratchet_tree_bytes.is_empty());
    }

    #[tokio::test]
    async fn test_adapter_snapshot_save_load() {
        let config = MlsConfig::default();
        let identity = b"bob@example.com".to_vec();
        let provider = Arc::new(OpenMlsRustCrypto::default());

        let adapter = OpenMlsHandleAdapter::create_group(None, identity, config, provider)
            .await
            .expect("Failed to create group");

        let group_id = adapter.group_id().await;

        // Save snapshot to bytes
        let bytes = adapter.save_snapshot().await.expect("Failed to save snapshot");
        assert!(!bytes.is_empty());

        // Load snapshot from bytes
        let loaded =
            OpenMlsHandleAdapter::<openmls_rust_crypto::OpenMlsRustCrypto>::load_snapshot(&bytes)
                .expect("Failed to load snapshot");

        // Verify loaded snapshot matches
        assert_eq!(loaded.group_id(), &group_id);
        assert_eq!(loaded.epoch(), 0);
        assert_eq!(loaded.members().len(), 1);
    }
}
