//! Storage Provider Trait
//!
//! Defines the interface for persisting MLS group state and related data.

use crate::core_mls::errors::MlsResult;
use async_trait::async_trait;

/// Group identifier
pub type GroupId = Vec<u8>;

/// Snapshot of group state for persistence
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PersistedGroupSnapshot {
    pub group_id: GroupId,
    pub epoch: u64,
    pub serialized_group: Vec<u8>, // Engine-specific bytes
}

/// Storage provider trait for MLS state persistence
///
/// Implementations must ensure:
/// - Atomic writes (no partial state)
/// - Durability (survive crashes)
/// - Consistency (reads see latest committed writes)
#[async_trait]
pub trait StorageProvider: Send + Sync {
    /// Persist serialized group snapshot atomically.
    ///
    /// Implementations should ensure durability and atomic replace.
    /// If a snapshot already exists for this group_id, it should be replaced atomically.
    async fn save_group_snapshot(&self, snapshot: PersistedGroupSnapshot) -> MlsResult<()>;

    /// Load group snapshot by GroupId.
    ///
    /// Returns `MlsError::NotFound` if missing.
    async fn load_group_snapshot(&self, group_id: &GroupId) -> MlsResult<PersistedGroupSnapshot>;

    /// Delete snapshot (used on group close/leave)
    async fn delete_group_snapshot(&self, group_id: &GroupId) -> MlsResult<()>;

    /// Store arbitrary binary blob with a key
    ///
    /// Optional: Used for storing key-package bundles or other artifacts.
    async fn put_blob(&self, key: &str, data: &[u8]) -> MlsResult<()>;

    /// Retrieve arbitrary binary blob by key
    ///
    /// Returns `MlsError::NotFound` if key doesn't exist.
    async fn get_blob(&self, key: &str) -> MlsResult<Vec<u8>>;

    /// List all group IDs currently stored
    ///
    /// Optional: Used for recovery and debugging.
    async fn list_groups(&self) -> MlsResult<Vec<GroupId>> {
        // Default implementation returns empty (can be overridden)
        Ok(Vec::new())
    }
}
