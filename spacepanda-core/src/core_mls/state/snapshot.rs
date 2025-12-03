//! Group State Snapshot
//!
//! Provides atomic snapshot export/import for MLS group state.
//! This works alongside OpenMLS's native storage to provide:
//! - Atomic state export for CRDT integration
//! - Disaster recovery
//! - Migration between storage backends

use crate::core_mls::{
    errors::{MlsError, MlsResult},
    types::{GroupId, MemberInfo},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Atomic snapshot of complete MLS group state
///
/// This captures the full state of an MLS group at a specific epoch,
/// suitable for backup, export, or CRDT synchronization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupSnapshot {
    /// Group identifier
    pub group_id: GroupId,

    /// Current epoch number
    pub epoch: u64,

    /// Serialized ratchet tree (public state)
    /// Export via `MlsGroup::export_ratchet_tree()`
    pub ratchet_tree_bytes: Vec<u8>,

    /// Serialized group context
    /// Contains group metadata, extensions, epoch
    pub group_context_bytes: Vec<u8>,

    /// Current group members
    pub members: Vec<MemberInfo>,

    /// Own leaf index in the tree
    pub own_leaf_index: u32,

    /// Custom metadata (application-specific)
    pub metadata: HashMap<String, Vec<u8>>,

    /// Timestamp of snapshot creation
    pub created_at: u64,
}

impl GroupSnapshot {
    /// Create a new snapshot
    pub fn new(
        group_id: GroupId,
        epoch: u64,
        ratchet_tree_bytes: Vec<u8>,
        group_context_bytes: Vec<u8>,
        members: Vec<MemberInfo>,
        own_leaf_index: u32,
    ) -> Self {
        Self {
            group_id,
            epoch,
            ratchet_tree_bytes,
            group_context_bytes,
            members,
            own_leaf_index,
            metadata: HashMap::new(),
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }

    /// Get group ID
    pub fn group_id(&self) -> &GroupId {
        &self.group_id
    }

    /// Get epoch
    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    /// Get members
    pub fn members(&self) -> &[MemberInfo] {
        &self.members
    }

    /// Add custom metadata
    pub fn with_metadata(mut self, key: String, value: Vec<u8>) -> Self {
        self.metadata.insert(key, value);
        self
    }

    /// Serialize to bytes
    pub fn to_bytes(&self) -> MlsResult<Vec<u8>> {
        bincode::serialize(self)
            .map_err(|e| MlsError::PersistenceError(format!("Failed to serialize snapshot: {}", e)))
    }

    /// Deserialize from bytes
    pub fn from_bytes(bytes: &[u8]) -> MlsResult<Self> {
        bincode::deserialize(bytes).map_err(|e| {
            MlsError::PersistenceError(format!("Failed to deserialize snapshot: {}", e))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snapshot_creation() {
        let group_id = GroupId::random();
        let snapshot =
            GroupSnapshot::new(group_id.clone(), 5, vec![1, 2, 3], vec![4, 5, 6], vec![], 0);

        assert_eq!(snapshot.group_id(), &group_id);
        assert_eq!(snapshot.epoch(), 5);
        assert_eq!(snapshot.members().len(), 0);
    }

    #[test]
    fn test_snapshot_metadata() {
        let snapshot = GroupSnapshot::new(GroupId::random(), 1, vec![], vec![], vec![], 0)
            .with_metadata("key1".to_string(), vec![10, 20])
            .with_metadata("key2".to_string(), vec![30, 40]);

        assert_eq!(snapshot.metadata.len(), 2);
        assert_eq!(snapshot.metadata.get("key1"), Some(&vec![10, 20]));
    }

    #[test]
    fn test_snapshot_serialization() {
        let original = GroupSnapshot::new(
            GroupId::random(),
            10,
            vec![1, 2, 3, 4],
            vec![5, 6, 7, 8],
            vec![],
            2,
        );

        let bytes = original.to_bytes().unwrap();
        let deserialized = GroupSnapshot::from_bytes(&bytes).unwrap();

        assert_eq!(deserialized.epoch, original.epoch);
        assert_eq!(deserialized.ratchet_tree_bytes, original.ratchet_tree_bytes);
        assert_eq!(deserialized.own_leaf_index, original.own_leaf_index);
    }
}
