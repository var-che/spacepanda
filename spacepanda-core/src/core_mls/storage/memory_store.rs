//! In-Memory Storage Provider
//!
//! Simple in-memory implementation for testing.

use crate::core_mls::errors::{MlsError, MlsResult};
use crate::core_mls::traits::storage::{GroupId, PersistedGroupSnapshot, StorageProvider};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// In-memory storage provider (for testing)
pub struct MemoryStorageProvider {
    snapshots: Arc<RwLock<HashMap<Vec<u8>, PersistedGroupSnapshot>>>,
    blobs: Arc<RwLock<HashMap<String, Vec<u8>>>>,
}

impl MemoryStorageProvider {
    /// Create a new in-memory storage provider
    pub fn new() -> Self {
        Self {
            snapshots: Arc::new(RwLock::new(HashMap::new())),
            blobs: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for MemoryStorageProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl StorageProvider for MemoryStorageProvider {
    async fn save_group_snapshot(&self, snapshot: PersistedGroupSnapshot) -> MlsResult<()> {
        let mut snapshots = self.snapshots.write().await;
        snapshots.insert(snapshot.group_id.clone(), snapshot);
        Ok(())
    }

    async fn load_group_snapshot(&self, group_id: &GroupId) -> MlsResult<PersistedGroupSnapshot> {
        let snapshots = self.snapshots.read().await;
        snapshots
            .get(group_id)
            .cloned()
            .ok_or_else(|| MlsError::NotFound(format!("Group not found: {}", hex::encode(group_id))))
    }

    async fn delete_group_snapshot(&self, group_id: &GroupId) -> MlsResult<()> {
        let mut snapshots = self.snapshots.write().await;
        snapshots
            .remove(group_id)
            .ok_or_else(|| MlsError::NotFound(format!("Group not found: {}", hex::encode(group_id))))?;
        Ok(())
    }

    async fn put_blob(&self, key: &str, data: &[u8]) -> MlsResult<()> {
        let mut blobs = self.blobs.write().await;
        blobs.insert(key.to_string(), data.to_vec());
        Ok(())
    }

    async fn get_blob(&self, key: &str) -> MlsResult<Vec<u8>> {
        let blobs = self.blobs.read().await;
        blobs
            .get(key)
            .cloned()
            .ok_or_else(|| MlsError::NotFound(format!("Blob not found: {}", key)))
    }

    async fn list_groups(&self) -> MlsResult<Vec<GroupId>> {
        let snapshots = self.snapshots.read().await;
        Ok(snapshots.keys().cloned().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_memory_storage() {
        let provider = MemoryStorageProvider::new();

        let snapshot = PersistedGroupSnapshot {
            group_id: vec![1, 2, 3],
            epoch: 5,
            serialized_group: vec![10, 20, 30],
        };

        provider.save_group_snapshot(snapshot.clone()).await.unwrap();
        let loaded = provider.load_group_snapshot(&snapshot.group_id).await.unwrap();

        assert_eq!(loaded.group_id, snapshot.group_id);
        assert_eq!(loaded.epoch, snapshot.epoch);
    }

    #[tokio::test]
    async fn test_memory_blob_storage() {
        let provider = MemoryStorageProvider::new();

        let data = vec![1, 2, 3, 4, 5];
        provider.put_blob("test", &data).await.unwrap();
        let retrieved = provider.get_blob("test").await.unwrap();

        assert_eq!(retrieved, data);
    }

    #[tokio::test]
    async fn test_memory_list_groups() {
        let provider = MemoryStorageProvider::new();

        let snapshot1 = PersistedGroupSnapshot {
            group_id: vec![1, 2, 3],
            epoch: 1,
            serialized_group: vec![10],
        };
        let snapshot2 = PersistedGroupSnapshot {
            group_id: vec![4, 5, 6],
            epoch: 2,
            serialized_group: vec![20],
        };

        provider.save_group_snapshot(snapshot1.clone()).await.unwrap();
        provider.save_group_snapshot(snapshot2.clone()).await.unwrap();

        let groups = provider.list_groups().await.unwrap();
        assert_eq!(groups.len(), 2);
    }
}
