//! MlsHandle Adapter for OpenMLS Engine
//!
//! This module provides a compatibility layer that allows the existing MlsHandle
//! API to work with the new OpenMlsEngine backend, enabling gradual migration.

use crate::core_mls::{
    errors::{MlsError, MlsResult},
    types::{GroupId, GroupMetadata, MlsConfig},
    engine::openmls_engine::OpenMlsEngine,
};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Adapter that wraps OpenMlsEngine to provide MlsHandle-compatible API
///
/// This allows gradual migration from the legacy MlsGroup to OpenMLS
/// while maintaining backward compatibility with existing code.
pub struct OpenMlsHandleAdapter {
    /// The underlying OpenMLS engine
    engine: Arc<RwLock<OpenMlsEngine>>,
    
    /// Configuration
    config: MlsConfig,
}

impl OpenMlsHandleAdapter {
    /// Create a new group using OpenMLS engine
    ///
    /// # Arguments
    /// * `group_id` - Optional group ID (will generate random if None)
    /// * `identity` - Member identity (username/user ID)
    /// * `config` - Group configuration
    pub async fn create_group(
        group_id: Option<GroupId>,
        identity: Vec<u8>,
        config: MlsConfig,
    ) -> MlsResult<Self> {
        let gid = group_id.unwrap_or_else(GroupId::random);
        
        let engine = OpenMlsEngine::create_group(gid, identity, config.clone()).await?;
        
        Ok(Self {
            engine: Arc::new(RwLock::new(engine)),
            config,
        })
    }

    /// Join an existing group from a Welcome message
    ///
    /// # Arguments
    /// * `welcome_bytes` - Serialized Welcome message
    /// * `ratchet_tree` - Optional ratchet tree bytes (if not in Welcome)
    /// * `config` - MLS configuration
    pub async fn join_from_welcome(
        welcome_bytes: &[u8],
        ratchet_tree: Option<Vec<u8>>,
        config: MlsConfig,
    ) -> MlsResult<Self> {
        let engine = OpenMlsEngine::join_from_welcome(welcome_bytes, ratchet_tree, config.clone()).await?;
        
        Ok(Self {
            engine: Arc::new(RwLock::new(engine)),
            config,
        })
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
    pub fn engine(&self) -> Arc<RwLock<OpenMlsEngine>> {
        Arc::clone(&self.engine)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_adapter_create_group() {
        let config = MlsConfig::default();
        let identity = b"alice@example.com".to_vec();

        let adapter = OpenMlsHandleAdapter::create_group(None, identity, config)
            .await
            .expect("Failed to create group via adapter");

        assert_eq!(adapter.epoch().await, 0);
        
        let metadata = adapter.metadata().await.expect("Failed to get metadata");
        assert_eq!(metadata.epoch, 0);
        assert_eq!(metadata.members.len(), 1);
    }

    #[tokio::test]
    async fn test_adapter_with_custom_group_id() {
        let config = MlsConfig::default();
        let group_id = GroupId::random();
        let identity = b"bob@example.com".to_vec();

        let adapter = OpenMlsHandleAdapter::create_group(
            Some(group_id.clone()),
            identity,
            config,
        )
        .await
        .expect("Failed to create group");

        let actual_id = adapter.group_id().await;
        assert_eq!(actual_id, group_id);
    }

    #[tokio::test]
    async fn test_adapter_multiple_instances() {
        let config = MlsConfig::default();

        let adapter1 = OpenMlsHandleAdapter::create_group(
            None,
            b"user1@example.com".to_vec(),
            config.clone(),
        )
        .await
        .expect("Failed to create adapter 1");

        let adapter2 = OpenMlsHandleAdapter::create_group(
            None,
            b"user2@example.com".to_vec(),
            config,
        )
        .await
        .expect("Failed to create adapter 2");

        let id1 = adapter1.group_id().await;
        let id2 = adapter2.group_id().await;

        assert_ne!(id1, id2, "Different adapters should have different group IDs");
    }
}
