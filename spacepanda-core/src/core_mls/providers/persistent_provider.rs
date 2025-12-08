//! Persistent OpenMLS Provider
//!
//! Wraps OpenMlsRustCrypto with SQL-backed persistence for application state.
//! This keeps OpenMLS's internal state in memory (for crypto operations) while
//! persisting our application-level data (group snapshots, key packages, etc).

use crate::core_mls::{
    errors::{MlsError, MlsResult},
    storage::SqlStorageProvider,
};
use openmls_rust_crypto::{OpenMlsRustCrypto, RustCrypto};
use openmls_traits::OpenMlsProvider;
use std::sync::Arc;

/// Persistent provider combining OpenMLS crypto with SQL storage
///
/// This provider:
/// - Uses OpenMlsRustCrypto for all cryptographic operations
/// - Uses MemoryStorage for OpenMLS internal state (fast, ephemeral)
/// - Uses SqlStorageProvider for application state (durable, persistent)
///
/// The separation is intentional:
/// - OpenMLS needs fast in-memory storage for crypto operations
/// - Application needs durable SQL storage for group snapshots and metadata
pub struct PersistentProvider {
    /// OpenMLS provider (crypto + memory storage)
    inner: OpenMlsRustCrypto,

    /// SQL storage for application state
    sql_storage: Arc<SqlStorageProvider>,
}

impl PersistentProvider {
    /// Create a new persistent provider
    ///
    /// # Arguments
    /// * `db_path` - Path to SQLite database file (use `:memory:` for in-memory database)
    pub fn new(db_path: &str) -> MlsResult<Self> {
        let sql_storage = Arc::new(SqlStorageProvider::new(db_path)?);

        Ok(Self { inner: OpenMlsRustCrypto::default(), sql_storage })
    }

    /// Get the SQL storage provider
    pub fn sql_storage(&self) -> &Arc<SqlStorageProvider> {
        &self.sql_storage
    }

    /// Get a clone of the SQL storage Arc
    pub fn sql_storage_arc(&self) -> Arc<SqlStorageProvider> {
        Arc::clone(&self.sql_storage)
    }

    /// Save method for compatibility with old provider
    ///
    /// This is a no-op because SQL storage is automatically persisted via transactions.
    /// Kept for backward compatibility with existing service code.
    pub fn save(&self) -> MlsResult<()> {
        // No-op: SQL storage auto-persists on every operation
        Ok(())
    }
}

impl Default for PersistentProvider {
    /// Create a default provider with in-memory SQLite database
    fn default() -> Self {
        Self::new(":memory:").expect("Failed to create in-memory persistent provider")
    }
}

impl OpenMlsProvider for PersistentProvider {
    type CryptoProvider = RustCrypto;
    type RandProvider = RustCrypto;
    type StorageProvider = openmls_memory_storage::MemoryStorage;

    fn storage(&self) -> &Self::StorageProvider {
        self.inner.storage()
    }

    fn crypto(&self) -> &Self::CryptoProvider {
        self.inner.crypto()
    }

    fn rand(&self) -> &Self::RandProvider {
        self.inner.rand()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core_mls::{
        engine::OpenMlsEngine,
        traits::storage::{PersistedGroupSnapshot, StorageProvider},
        types::MlsConfig,
    };
    use tempfile::tempdir;

    #[test]
    fn test_persistent_provider_creation() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");

        let provider = PersistentProvider::new(db_path.to_str().unwrap()).unwrap();

        // Verify we can access both storage types
        assert!(provider.storage() as *const _ != std::ptr::null());
        assert!(provider.sql_storage() as *const _ != std::ptr::null());
    }

    #[tokio::test]
    async fn test_group_persistence_with_engine() {
        use crate::core_mls::types::GroupId;

        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test_engine.db");

        let group_id = GroupId(b"test_persistent_group".to_vec());
        let identity = b"alice".to_vec();
        let config = MlsConfig::default();

        // Create group with first provider instance
        {
            let provider = Arc::new(PersistentProvider::new(db_path.to_str().unwrap()).unwrap());
            let engine = OpenMlsEngine::create_group(
                group_id.clone(),
                identity.clone(),
                config.clone(),
                provider.clone(),
            )
            .await
            .unwrap();

            // Get group state
            let group = engine.group.read().await;
            let epoch = group.epoch().as_u64();

            // Manually persist the group snapshot to SQL storage
            let snapshot = PersistedGroupSnapshot {
                group_id: group_id.0.clone(),
                epoch,
                serialized_group: vec![1, 2, 3, 4, 5], // Mock serialized data for test
            };

            provider.sql_storage().save_group_snapshot(snapshot).await.unwrap();
        }

        // Verify persistence with second provider instance
        {
            let provider = Arc::new(PersistentProvider::new(db_path.to_str().unwrap()).unwrap());

            // Load the persisted snapshot
            let loaded = provider.sql_storage().load_group_snapshot(&group_id.0).await.unwrap();

            assert_eq!(loaded.group_id, group_id.0);
            assert_eq!(loaded.epoch, 0); // Initial epoch
            assert_eq!(loaded.serialized_group, vec![1, 2, 3, 4, 5]);
        }
    }

    #[tokio::test]
    async fn test_atomic_group_operations() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test_atomic.db");
        let provider = Arc::new(PersistentProvider::new(db_path.to_str().unwrap()).unwrap());

        // Create multiple group snapshots
        let snapshots = vec![
            PersistedGroupSnapshot {
                group_id: b"group_1".to_vec(),
                epoch: 1,
                serialized_group: vec![1, 1, 1],
            },
            PersistedGroupSnapshot {
                group_id: b"group_2".to_vec(),
                epoch: 2,
                serialized_group: vec![2, 2, 2],
            },
            PersistedGroupSnapshot {
                group_id: b"group_3".to_vec(),
                epoch: 3,
                serialized_group: vec![3, 3, 3],
            },
        ];

        // Save all atomically
        provider.sql_storage().save_group_snapshots_atomic(&snapshots).await.unwrap();

        // Verify all were saved
        for snapshot in &snapshots {
            let loaded =
                provider.sql_storage().load_group_snapshot(&snapshot.group_id).await.unwrap();
            assert_eq!(loaded.epoch, snapshot.epoch);
            assert_eq!(loaded.serialized_group, snapshot.serialized_group);
        }

        // Delete all atomically
        let group_ids: Vec<_> = snapshots.iter().map(|s| s.group_id.clone()).collect();
        provider.sql_storage().delete_groups_atomic(&group_ids).await.unwrap();

        // Verify all were deleted
        for group_id in group_ids {
            let result = provider.sql_storage().load_group_snapshot(&group_id).await;
            assert!(result.is_err());
        }
    }
}
