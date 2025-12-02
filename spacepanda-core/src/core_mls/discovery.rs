//! MLS Discovery Integration - CRDT-based group discovery
//!
//! This module provides:
//! - Public group information publishing
//! - CRDT-based discovery (offline-capable)
//! - Signature verification for public info
//! - No secrets in public data
//!
//! # Design
//!
//! Groups publish `GroupPublicInfo` via CRDT:
//! - Group ID and name
//! - Current epoch
//! - Public tree snapshot
//! - Creator signature
//!
//! Members can discover groups without being in them.

use super::errors::{MlsError, MlsResult};
use super::tree::MlsTree;
use super::types::{GroupId, GroupMetadata};
use super::welcome::TreeSnapshot;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Public group information for discovery
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GroupPublicInfo {
    /// Group ID
    pub group_id: GroupId,
    /// Group name (optional)
    pub name: Option<String>,
    /// Current epoch
    pub epoch: u64,
    /// Number of members
    pub member_count: usize,
    /// Public tree snapshot (for verification)
    pub tree_snapshot: TreeSnapshot,
    /// Creation timestamp
    pub created_at: u64,
    /// Last update timestamp
    pub updated_at: u64,
    /// Signature over the public info (creator signs)
    pub signature: Vec<u8>,
}

impl GroupPublicInfo {
    /// Create public info from group metadata
    pub fn from_metadata(
        group_id: GroupId,
        metadata: &GroupMetadata,
        tree: &MlsTree,
        sign_fn: impl FnOnce(&[u8]) -> Vec<u8>,
    ) -> Self {
        let tree_snapshot = TreeSnapshot::from_tree(tree);
        
        let mut info = Self {
            group_id,
            name: metadata.name.clone(),
            epoch: metadata.epoch,
            member_count: metadata.members.len(),
            tree_snapshot,
            created_at: metadata.created_at,
            updated_at: metadata.updated_at,
            signature: vec![],
        };

        // Sign the canonical bytes
        let bytes = info.to_bytes_for_signature();
        info.signature = sign_fn(&bytes);

        info
    }

    /// Verify signature
    pub fn verify(&self, verify_fn: impl FnOnce(&[u8], &[u8]) -> bool) -> MlsResult<()> {
        let mut info_copy = self.clone();
        let sig = info_copy.signature.clone();
        info_copy.signature = vec![];

        let bytes = info_copy.to_bytes_for_signature();

        if verify_fn(&bytes, &sig) {
            Ok(())
        } else {
            Err(MlsError::VerifyFailed(
                "GroupPublicInfo signature invalid".to_string(),
            ))
        }
    }

    /// Serialize to bytes for signing
    fn to_bytes_for_signature(&self) -> Vec<u8> {
        // Deterministic serialization for signing
        let mut bytes = Vec::new();
        bytes.extend_from_slice(self.group_id.as_bytes());
        if let Some(ref name) = self.name {
            bytes.extend_from_slice(name.as_bytes());
        }
        bytes.extend_from_slice(&self.epoch.to_be_bytes());
        bytes.extend_from_slice(&(self.member_count as u64).to_be_bytes());
        bytes.extend_from_slice(&self.created_at.to_be_bytes());
        bytes.extend_from_slice(&self.updated_at.to_be_bytes());
        
        // Include tree snapshot hash
        if let Ok(snapshot_bytes) = bincode::serialize(&self.tree_snapshot) {
            let mut hasher = Sha256::new();
            hasher.update(&snapshot_bytes);
            bytes.extend_from_slice(&hasher.finalize());
        }

        bytes
    }

    /// Check if group info has been updated
    pub fn is_newer_than(&self, other: &GroupPublicInfo) -> bool {
        self.epoch > other.epoch || self.updated_at > other.updated_at
    }

    /// Merge with another public info (CRDT semantics)
    pub fn merge(&mut self, other: &GroupPublicInfo) -> MlsResult<()> {
        // Only merge if same group
        if self.group_id != other.group_id {
            return Err(MlsError::InvalidState(
                "Cannot merge different groups".to_string(),
            ));
        }

        // Keep newer version
        if other.is_newer_than(self) {
            self.epoch = other.epoch;
            self.name = other.name.clone();
            self.member_count = other.member_count;
            self.tree_snapshot = other.tree_snapshot.clone();
            self.updated_at = other.updated_at;
            self.signature = other.signature.clone();
        }

        Ok(())
    }

    /// Serialize to JSON for CRDT storage
    pub fn to_json(&self) -> MlsResult<String> {
        serde_json::to_string(self)
            .map_err(|e| MlsError::Serialization(e.to_string()))
    }

    /// Deserialize from JSON
    pub fn from_json(json: &str) -> MlsResult<Self> {
        serde_json::from_str(json)
            .map_err(|e| MlsError::Serialization(e.to_string()))
    }
}

/// Discovery query filters
#[derive(Debug, Clone)]
pub struct DiscoveryQuery {
    /// Filter by group name pattern
    pub name_pattern: Option<String>,
    /// Minimum member count
    pub min_members: Option<usize>,
    /// Maximum member count
    pub max_members: Option<usize>,
    /// Created after timestamp
    pub created_after: Option<u64>,
}

impl DiscoveryQuery {
    /// Create empty query (matches all)
    pub fn all() -> Self {
        Self {
            name_pattern: None,
            min_members: None,
            max_members: None,
            created_after: None,
        }
    }

    /// Check if group info matches query
    pub fn matches(&self, info: &GroupPublicInfo) -> bool {
        // Name pattern
        if let Some(ref pattern) = self.name_pattern {
            if let Some(ref name) = info.name {
                if !name.contains(pattern) {
                    return false;
                }
            } else {
                return false;
            }
        }

        // Min members
        if let Some(min) = self.min_members {
            if info.member_count < min {
                return false;
            }
        }

        // Max members
        if let Some(max) = self.max_members {
            if info.member_count > max {
                return false;
            }
        }

        // Created after
        if let Some(after) = self.created_after {
            if info.created_at <= after {
                return false;
            }
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core_mls::group::MlsGroup;
    use crate::core_mls::types::MlsConfig;

    fn test_group() -> MlsGroup {
        MlsGroup::new(
            GroupId::random(),
            b"alice_pk".to_vec(),
            b"alice".to_vec(),
            vec![1, 2, 3, 4],
            MlsConfig::default(),
        )
        .unwrap()
    }

    fn sign_fn(data: &[u8]) -> Vec<u8> {
        // Simplified: hash as signature
        let mut hasher = Sha256::new();
        hasher.update(data);
        hasher.finalize().to_vec()
    }

    fn verify_fn(data: &[u8], sig: &[u8]) -> bool {
        // Simplified: recompute hash
        let mut hasher = Sha256::new();
        hasher.update(data);
        &hasher.finalize()[..] == sig
    }

    #[test]
    fn test_create_public_info() {
        let group = test_group();
        
        let info = GroupPublicInfo::from_metadata(
            group.group_id.clone(),
            &group.metadata,
            &group.tree,
            sign_fn,
        );

        assert_eq!(info.group_id, group.group_id);
        assert_eq!(info.epoch, 0);
        assert_eq!(info.member_count, 1);
        assert!(!info.signature.is_empty());
    }

    #[test]
    fn test_verify_signature() {
        let group = test_group();
        
        let info = GroupPublicInfo::from_metadata(
            group.group_id.clone(),
            &group.metadata,
            &group.tree,
            sign_fn,
        );

        assert!(info.verify(verify_fn).is_ok());
    }

    #[test]
    fn test_verify_signature_fails_on_tamper() {
        let group = test_group();
        
        let mut info = GroupPublicInfo::from_metadata(
            group.group_id.clone(),
            &group.metadata,
            &group.tree,
            sign_fn,
        );

        // Tamper with epoch
        info.epoch = 999;

        assert!(info.verify(verify_fn).is_err());
    }

    #[test]
    fn test_json_serialization() {
        let group = test_group();
        
        let info = GroupPublicInfo::from_metadata(
            group.group_id.clone(),
            &group.metadata,
            &group.tree,
            sign_fn,
        );

        let json = info.to_json().unwrap();
        let deserialized = GroupPublicInfo::from_json(&json).unwrap();

        assert_eq!(deserialized, info);
    }

    #[test]
    fn test_is_newer_than() {
        let group = test_group();
        
        let info1 = GroupPublicInfo::from_metadata(
            group.group_id.clone(),
            &group.metadata,
            &group.tree,
            sign_fn,
        );

        let mut group2 = group.clone();
        group2.metadata.epoch = 1;
        group2.metadata.updated_at += 1000;
        
        let info2 = GroupPublicInfo::from_metadata(
            group2.group_id.clone(),
            &group2.metadata,
            &group2.tree,
            sign_fn,
        );

        assert!(info2.is_newer_than(&info1));
        assert!(!info1.is_newer_than(&info2));
    }

    #[test]
    fn test_merge() {
        let group = test_group();
        
        let mut info1 = GroupPublicInfo::from_metadata(
            group.group_id.clone(),
            &group.metadata,
            &group.tree,
            sign_fn,
        );

        let mut group2 = group.clone();
        group2.metadata.epoch = 1;
        group2.metadata.updated_at += 1000;
        
        let info2 = GroupPublicInfo::from_metadata(
            group2.group_id.clone(),
            &group2.metadata,
            &group2.tree,
            sign_fn,
        );

        info1.merge(&info2).unwrap();

        assert_eq!(info1.epoch, 1);
        assert_eq!(info1.updated_at, info2.updated_at);
    }

    #[test]
    fn test_merge_different_groups_fails() {
        let group1 = test_group();
        let group2 = test_group();
        
        let mut info1 = GroupPublicInfo::from_metadata(
            group1.group_id.clone(),
            &group1.metadata,
            &group1.tree,
            sign_fn,
        );

        let info2 = GroupPublicInfo::from_metadata(
            group2.group_id.clone(),
            &group2.metadata,
            &group2.tree,
            sign_fn,
        );

        assert!(info1.merge(&info2).is_err());
    }

    #[test]
    fn test_discovery_query_all() {
        let group = test_group();
        let info = GroupPublicInfo::from_metadata(
            group.group_id.clone(),
            &group.metadata,
            &group.tree,
            sign_fn,
        );

        let query = DiscoveryQuery::all();
        assert!(query.matches(&info));
    }

    #[test]
    fn test_discovery_query_name_pattern() {
        let mut group = test_group();
        group.metadata.name = Some("test-group".to_string());
        
        let info = GroupPublicInfo::from_metadata(
            group.group_id.clone(),
            &group.metadata,
            &group.tree,
            sign_fn,
        );

        let mut query = DiscoveryQuery::all();
        query.name_pattern = Some("test".to_string());
        assert!(query.matches(&info));

        query.name_pattern = Some("other".to_string());
        assert!(!query.matches(&info));
    }

    #[test]
    fn test_discovery_query_member_count() {
        let group = test_group();
        let info = GroupPublicInfo::from_metadata(
            group.group_id.clone(),
            &group.metadata,
            &group.tree,
            sign_fn,
        );

        let mut query = DiscoveryQuery::all();
        query.min_members = Some(1);
        query.max_members = Some(5);
        assert!(query.matches(&info));

        query.min_members = Some(2);
        assert!(!query.matches(&info));
    }

    #[test]
    fn test_discovery_query_created_after() {
        let group = test_group();
        let info = GroupPublicInfo::from_metadata(
            group.group_id.clone(),
            &group.metadata,
            &group.tree,
            sign_fn,
        );

        let mut query = DiscoveryQuery::all();
        query.created_after = Some(0);
        assert!(query.matches(&info));

        query.created_after = Some(u64::MAX);
        assert!(!query.matches(&info));
    }
}
