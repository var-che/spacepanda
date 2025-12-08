//! Welcome messages for onboarding new members to MLS groups
//!
//! When a new member is added to a group, they receive a Welcome message
//! that contains:
//! - Encrypted group secrets (using HPKE to their public key)
//! - Public tree state (ratchet tree without secrets)
//! - Group metadata (configuration, members)
//!
//! This allows the new member to initialize their local group state
//! and participate in the group.

use super::encryption::HpkeContext;
use super::errors::{MlsError, MlsResult};
use super::tree::{MlsTree, NodeIndex};
use super::types::{GroupId, GroupMetadata};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[cfg(test)]
use super::types::MemberInfo;

/// Welcome message for a new group member
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Welcome {
    /// Group ID
    pub group_id: GroupId,
    /// Epoch when member was added
    pub epoch: u64,
    /// Encrypted group secrets (one per new member)
    pub encrypted_secrets: Vec<EncryptedGroupSecrets>,
    /// Public tree state (no secrets)
    pub tree_snapshot: TreeSnapshot,
    /// Group metadata
    pub metadata: GroupMetadata,
}

/// Encrypted secrets for a specific new member
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedGroupSecrets {
    /// Index of the new member in the tree
    pub new_member_index: u32,
    /// HPKE-encrypted group secrets
    pub encrypted_payload: Vec<u8>,
}

/// Group secrets that are encrypted in the Welcome
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WelcomeGroupSecrets {
    /// Current epoch
    pub epoch: u64,
    /// Application secret for deriving message keys
    pub application_secret: Vec<u8>,
    /// Epoch authenticator (MAC of epoch data)
    pub epoch_authenticator: Vec<u8>,
}

impl WelcomeGroupSecrets {
    /// Serialize to bytes for encryption
    pub fn to_bytes(&self) -> MlsResult<Vec<u8>> {
        bincode::serialize(self)
            .map_err(|e| MlsError::PersistenceError(format!("Failed to serialize secrets: {}", e)))
    }

    /// Deserialize from bytes
    pub fn from_bytes(bytes: &[u8]) -> MlsResult<Self> {
        bincode::deserialize(bytes).map_err(|e| {
            MlsError::PersistenceError(format!("Failed to deserialize secrets: {}", e))
        })
    }
}

/// Snapshot of the public tree state (no secrets)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TreeSnapshot {
    /// Number of leaves in the tree
    pub leaf_count: u32,
    /// Public nodes (index -> public key)
    pub public_nodes: HashMap<NodeIndex, Vec<u8>>,
    /// Node hashes for verification
    pub node_hashes: HashMap<NodeIndex, Vec<u8>>,
}

impl TreeSnapshot {
    /// Create snapshot from an MLS tree
    pub fn from_tree(tree: &MlsTree) -> Self {
        let mut public_nodes = HashMap::new();
        let mut node_hashes = HashMap::new();

        for (idx, pk) in tree.export_public_nodes() {
            public_nodes.insert(idx, pk);
            // Get node hash if available
            if let Some(node) = tree.get_node(idx) {
                if let Some(hash) = &node.node_hash {
                    node_hashes.insert(idx, hash.clone());
                }
            }
        }

        Self { leaf_count: tree.leaf_count(), public_nodes, node_hashes }
    }

    /// Reconstruct tree from snapshot
    pub fn to_tree(&self) -> MlsResult<MlsTree> {
        let mut tree = MlsTree::new();

        // Add leaves in order
        for leaf_idx in 0..self.leaf_count {
            let node_idx = MlsTree::leaf_to_node_index(leaf_idx);

            if let Some(public_key) = self.public_nodes.get(&node_idx) {
                tree.add_leaf(public_key.clone())?;
            } else {
                return Err(MlsError::InvalidState(format!(
                    "Missing public key for leaf {}",
                    leaf_idx
                )));
            }
        }

        // Verify root hash if present
        if let Some(expected_hash) = self.node_hashes.get(&tree.root_index().unwrap_or(0)) {
            if let Some(actual_hash) = tree.root_hash() {
                if &actual_hash != expected_hash {
                    return Err(MlsError::VerifyFailed("Tree root hash mismatch".to_string()));
                }
            }
        }

        Ok(tree)
    }
}

impl Welcome {
    /// Create a Welcome message for new members
    ///
    /// # Arguments
    /// * `group_id` - ID of the group
    /// * `epoch` - Current epoch
    /// * `secrets` - Group secrets to encrypt
    /// * `tree` - Current tree state
    /// * `metadata` - Group metadata
    /// * `new_members` - List of (leaf_index, public_key) for new members
    pub fn create(
        group_id: GroupId,
        epoch: u64,
        secrets: WelcomeGroupSecrets,
        tree: &MlsTree,
        metadata: GroupMetadata,
        new_members: Vec<(u32, Vec<u8>)>,
    ) -> MlsResult<Self> {
        let mut encrypted_secrets = Vec::new();

        // Encrypt secrets for each new member
        let secrets_bytes = secrets.to_bytes()?;

        for (leaf_index, public_key) in new_members {
            // HPKE encrypt to member's public key
            let hpke = HpkeContext::new(public_key);

            // Use group_id + epoch as AAD for binding
            let mut aad = group_id.as_bytes().to_vec();
            aad.extend_from_slice(&epoch.to_be_bytes());

            let encrypted_payload = hpke
                .seal(&secrets_bytes, &aad)
                .map_err(|e| MlsError::CryptoError(format!("HPKE encryption failed: {}", e)))?;

            encrypted_secrets
                .push(EncryptedGroupSecrets { new_member_index: leaf_index, encrypted_payload });
        }

        let tree_snapshot = TreeSnapshot::from_tree(tree);

        Ok(Self { group_id, epoch, encrypted_secrets, tree_snapshot, metadata })
    }

    /// Process Welcome message as a new member
    ///
    /// # Arguments
    /// * `member_index` - The recipient's leaf index in the tree
    /// * `member_secret_key` - The recipient's X25519 secret key (for HPKE decryption)
    ///
    /// # Returns
    /// Tuple of (WelcomeGroupSecrets, MlsTree) for initializing local state
    pub fn process(
        &self,
        member_index: u32,
        member_secret_key: &[u8],
    ) -> MlsResult<(WelcomeGroupSecrets, MlsTree)> {
        // Find encrypted secrets for this member
        let encrypted = self
            .encrypted_secrets
            .iter()
            .find(|e| e.new_member_index == member_index)
            .ok_or_else(|| {
                MlsError::InvalidState(format!("No encrypted secrets for member {}", member_index))
            })?;

        // Decrypt group secrets using HPKE
        let mut aad = self.group_id.as_bytes().to_vec();
        aad.extend_from_slice(&self.epoch.to_be_bytes());

        let secrets_bytes =
            HpkeContext::open(member_secret_key, &encrypted.encrypted_payload, &aad)?;

        let secrets = WelcomeGroupSecrets::from_bytes(&secrets_bytes)?;

        // Verify epoch matches
        if secrets.epoch != self.epoch {
            return Err(MlsError::EpochMismatch { expected: self.epoch, actual: secrets.epoch });
        }

        // Reconstruct tree from snapshot
        let tree = self.tree_snapshot.to_tree()?;

        Ok((secrets, tree))
    }

    /// Serialize Welcome to bytes
    pub fn to_bytes(&self) -> MlsResult<Vec<u8>> {
        bincode::serialize(self)
            .map_err(|e| MlsError::PersistenceError(format!("Failed to serialize Welcome: {}", e)))
    }

    /// Deserialize Welcome from bytes
    pub fn from_bytes(bytes: &[u8]) -> MlsResult<Self> {
        bincode::deserialize(bytes).map_err(|e| {
            MlsError::PersistenceError(format!("Failed to deserialize Welcome: {}", e))
        })
    }
}

/// Builder for creating Welcome messages
pub struct WelcomeBuilder {
    group_id: GroupId,
    epoch: u64,
    secrets: Option<WelcomeGroupSecrets>,
    tree: Option<MlsTree>,
    metadata: Option<GroupMetadata>,
    new_members: Vec<(u32, Vec<u8>)>,
}

impl WelcomeBuilder {
    /// Create new Welcome builder
    pub fn new(group_id: GroupId, epoch: u64) -> Self {
        Self {
            group_id,
            epoch,
            secrets: None,
            tree: None,
            metadata: None,
            new_members: Vec::new(),
        }
    }

    /// Set group secrets
    pub fn secrets(mut self, secrets: WelcomeGroupSecrets) -> Self {
        self.secrets = Some(secrets);
        self
    }

    /// Set tree state
    pub fn tree(mut self, tree: MlsTree) -> Self {
        self.tree = Some(tree);
        self
    }

    /// Set group metadata
    pub fn metadata(mut self, metadata: GroupMetadata) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Add a new member to receive the Welcome
    pub fn add_member(mut self, leaf_index: u32, public_key: Vec<u8>) -> Self {
        self.new_members.push((leaf_index, public_key));
        self
    }

    /// Build the Welcome message
    pub fn build(self) -> MlsResult<Welcome> {
        let secrets = self
            .secrets
            .ok_or_else(|| MlsError::InvalidState("Welcome builder missing secrets".to_string()))?;

        let tree = self
            .tree
            .ok_or_else(|| MlsError::InvalidState("Welcome builder missing tree".to_string()))?;

        let metadata = self.metadata.ok_or_else(|| {
            MlsError::InvalidState("Welcome builder missing metadata".to_string())
        })?;

        if self.new_members.is_empty() {
            return Err(MlsError::InvalidState("Welcome builder has no new members".to_string()));
        }

        Welcome::create(self.group_id, self.epoch, secrets, &tree, metadata, self.new_members)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    use x25519_dalek::{PublicKey, StaticSecret};

    fn test_keypair(name: &str) -> (Vec<u8>, Vec<u8>) {
        let mut hasher = Sha256::new();
        hasher.update(name.as_bytes());
        let hash = hasher.finalize();

        let sk_bytes: [u8; 32] = hash.into();
        let sk = StaticSecret::from(sk_bytes);
        let pk = PublicKey::from(&sk);

        (pk.as_bytes().to_vec(), sk.as_bytes().to_vec())
    }

    fn test_group_id() -> GroupId {
        GroupId::random()
    }

    fn test_secrets(epoch: u64) -> WelcomeGroupSecrets {
        WelcomeGroupSecrets {
            epoch,
            application_secret: vec![1, 2, 3, 4],
            epoch_authenticator: vec![5, 6, 7, 8],
        }
    }

    fn test_tree() -> MlsTree {
        let mut tree = MlsTree::new();
        tree.add_leaf(b"alice".to_vec()).unwrap();
        tree.add_leaf(b"bob".to_vec()).unwrap();
        tree
    }

    fn test_metadata() -> GroupMetadata {
        GroupMetadata {
            group_id: test_group_id(),
            name: Some("Test Group".to_string()),
            epoch: 1,
            created_at: 1234567890,
            updated_at: 1234567890,
            members: vec![
                MemberInfo {
                    identity: b"alice".to_vec(),
                    leaf_index: 0,
                    joined_at: 1234567890,
                    role: crate::core_mls::types::MemberRole::Admin,
                },
                MemberInfo {
                    identity: b"bob".to_vec(),
                    leaf_index: 1,
                    joined_at: 1234567891,
                    role: crate::core_mls::types::MemberRole::Member,
                },
            ],
        }
    }

    #[test]
    fn test_group_secrets_serialization() {
        let secrets = test_secrets(1);
        let bytes = secrets.to_bytes().unwrap();
        let decoded = WelcomeGroupSecrets::from_bytes(&bytes).unwrap();

        assert_eq!(decoded.epoch, 1);
        assert_eq!(decoded.application_secret, vec![1, 2, 3, 4]);
        assert_eq!(decoded.epoch_authenticator, vec![5, 6, 7, 8]);
    }

    #[test]
    fn test_tree_snapshot_roundtrip() {
        let tree = test_tree();
        let snapshot = TreeSnapshot::from_tree(&tree);

        assert_eq!(snapshot.leaf_count, 2);
        assert!(snapshot.public_nodes.len() >= 2);

        let reconstructed = snapshot.to_tree().unwrap();
        assert_eq!(reconstructed.leaf_count(), 2);
        assert_eq!(reconstructed.root_hash(), tree.root_hash());
    }

    #[test]
    fn test_tree_snapshot_missing_leaf() {
        let mut snapshot = TreeSnapshot {
            leaf_count: 2,
            public_nodes: HashMap::new(),
            node_hashes: HashMap::new(),
        };

        // Only add one leaf, missing the second
        snapshot.public_nodes.insert(0, b"alice".to_vec());

        let result = snapshot.to_tree();
        assert!(result.is_err());
    }

    #[test]
    fn test_welcome_create_and_process() {
        let group_id = test_group_id();
        let epoch = 1;
        let secrets = test_secrets(epoch);
        let tree = test_tree();
        let metadata = test_metadata();

        // New member (charlie) joining at leaf index 2
        let (charlie_pk, charlie_sk) = test_keypair("charlie");
        let new_members = vec![(2, charlie_pk.clone())];

        let welcome = Welcome::create(
            group_id.clone(),
            epoch,
            secrets.clone(),
            &tree,
            metadata.clone(),
            new_members,
        )
        .unwrap();

        assert_eq!(welcome.group_id, group_id);
        assert_eq!(welcome.epoch, epoch);
        assert_eq!(welcome.encrypted_secrets.len(), 1);
        assert_eq!(welcome.encrypted_secrets[0].new_member_index, 2);

        // Process as charlie
        let (decoded_secrets, decoded_tree) = welcome.process(2, &charlie_sk).unwrap();

        assert_eq!(decoded_secrets.epoch, epoch);
        assert_eq!(decoded_secrets.application_secret, secrets.application_secret);
        assert_eq!(decoded_tree.leaf_count(), 2);
    }

    #[test]
    fn test_welcome_wrong_member_index() {
        let group_id = test_group_id();
        let epoch = 1;
        let secrets = test_secrets(epoch);
        let tree = test_tree();
        let metadata = test_metadata();

        let (charlie_pk, charlie_sk) = test_keypair("charlie");
        let new_members = vec![(2, charlie_pk.clone())];

        let welcome =
            Welcome::create(group_id, epoch, secrets, &tree, metadata, new_members).unwrap();

        // Try to process as wrong member index
        let result = welcome.process(99, &charlie_sk);
        assert!(result.is_err());
    }

    #[test]
    fn test_welcome_wrong_public_key() {
        let group_id = test_group_id();
        let epoch = 1;
        let secrets = test_secrets(epoch);
        let tree = test_tree();
        let metadata = test_metadata();

        let (charlie_pk, _charlie_sk) = test_keypair("charlie");
        let new_members = vec![(2, charlie_pk.clone())];

        let welcome =
            Welcome::create(group_id, epoch, secrets, &tree, metadata, new_members).unwrap();

        // Try to decrypt with wrong key
        let (_wrong_pk, wrong_sk) = test_keypair("wrong");
        let result = welcome.process(2, &wrong_sk);
        assert!(result.is_err());
    }

    #[test]
    fn test_welcome_epoch_mismatch() {
        let group_id = test_group_id();
        let epoch = 1;
        let mut secrets = test_secrets(epoch);
        secrets.epoch = 999; // Wrong epoch in secrets

        let tree = test_tree();
        let metadata = test_metadata();

        let (charlie_pk, charlie_sk) = test_keypair("charlie");
        let new_members = vec![(2, charlie_pk.clone())];

        let welcome =
            Welcome::create(group_id, epoch, secrets, &tree, metadata, new_members).unwrap();

        let result = welcome.process(2, &charlie_sk);
        assert!(matches!(result, Err(MlsError::EpochMismatch { .. })));
    }

    #[test]
    fn test_welcome_serialization() {
        let group_id = test_group_id();
        let epoch = 1;
        let secrets = test_secrets(epoch);
        let tree = test_tree();
        let metadata = test_metadata();

        let (charlie_pk, _charlie_sk) = test_keypair("charlie");
        let new_members = vec![(2, charlie_pk.clone())];

        let welcome =
            Welcome::create(group_id, epoch, secrets, &tree, metadata, new_members).unwrap();

        let bytes = welcome.to_bytes().unwrap();
        let decoded = Welcome::from_bytes(&bytes).unwrap();

        assert_eq!(decoded.group_id, welcome.group_id);
        assert_eq!(decoded.epoch, welcome.epoch);
        assert_eq!(decoded.encrypted_secrets.len(), 1);
    }

    #[test]
    fn test_welcome_multiple_members() {
        let group_id = test_group_id();
        let epoch = 1;
        let secrets = test_secrets(epoch);
        let tree = test_tree();
        let metadata = test_metadata();

        let (charlie_pk, charlie_sk) = test_keypair("charlie");
        let (dave_pk, dave_sk) = test_keypair("dave");
        let new_members = vec![(2, charlie_pk.clone()), (3, dave_pk.clone())];

        let welcome =
            Welcome::create(group_id, epoch, secrets.clone(), &tree, metadata, new_members)
                .unwrap();

        assert_eq!(welcome.encrypted_secrets.len(), 2);

        // Charlie can process
        let (charlie_secrets, _) = welcome.process(2, &charlie_sk).unwrap();
        assert_eq!(charlie_secrets.epoch, epoch);

        // Dave can process
        let (dave_secrets, _) = welcome.process(3, &dave_sk).unwrap();
        assert_eq!(dave_secrets.epoch, epoch);
    }

    #[test]
    fn test_welcome_builder() {
        let group_id = test_group_id();
        let secrets = test_secrets(1);
        let tree = test_tree();
        let metadata = test_metadata();

        let (charlie_pk, charlie_sk) = test_keypair("charlie");

        let welcome = WelcomeBuilder::new(group_id.clone(), 1)
            .secrets(secrets)
            .tree(tree)
            .metadata(metadata)
            .add_member(2, charlie_pk.clone())
            .build()
            .unwrap();

        assert_eq!(welcome.group_id, group_id);
        assert_eq!(welcome.epoch, 1);
        assert_eq!(welcome.encrypted_secrets.len(), 1);

        // Process the welcome
        let (decoded_secrets, _) = welcome.process(2, &charlie_sk).unwrap();
        assert_eq!(decoded_secrets.epoch, 1);
    }

    #[test]
    fn test_welcome_builder_missing_secrets() {
        let group_id = test_group_id();
        let tree = test_tree();
        let metadata = test_metadata();

        let result = WelcomeBuilder::new(group_id, 1)
            .tree(tree)
            .metadata(metadata)
            .add_member(2, b"charlie".to_vec())
            .build();

        assert!(result.is_err());
    }

    #[test]
    fn test_welcome_builder_no_members() {
        let group_id = test_group_id();
        let secrets = test_secrets(1);
        let tree = test_tree();
        let metadata = test_metadata();

        let result = WelcomeBuilder::new(group_id, 1)
            .secrets(secrets)
            .tree(tree)
            .metadata(metadata)
            .build();

        assert!(result.is_err());
    }
}
