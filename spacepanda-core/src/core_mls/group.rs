//! MLS Group - High-level group state and operations
//!
//! MlsGroup ties together all MLS components:
//! - Tree management
//! - Epoch tracking
//! - Message encryption/decryption
//! - Proposal/Commit processing
//! - Replay protection
//!
//! This is the main API for interacting with an MLS group.

use super::commit::{Commit, CommitResult, CommitValidator, UpdatePath};
use super::encryption::{encrypt_message, decrypt_message, EncryptedMessage, KeySchedule, SenderData};
use super::errors::{MlsError, MlsResult};
use super::proposals::{Proposal, ProposalContent, ProposalQueue, ProposalRef, ProposalType};
use super::tree::{LeafIndex, MlsTree};
use super::types::{GroupId, GroupMetadata, MemberInfo, MlsConfig};
use super::welcome::{TreeSnapshot, Welcome, WelcomeGroupSecrets};
use std::collections::{HashMap, HashSet};

/// MLS Group state
pub struct MlsGroup {
    /// Group ID
    pub group_id: GroupId,
    /// Current epoch
    pub epoch: u64,
    /// Ratchet tree
    pub tree: MlsTree,
    /// Group metadata
    pub metadata: GroupMetadata,
    /// Configuration
    pub config: MlsConfig,
    /// Key schedule for current epoch
    key_schedule: KeySchedule,
    /// Pending proposals
    proposals: ProposalQueue,
    /// Replay protection: (sender, sequence) tuples seen
    replay_cache: HashSet<(u32, u64)>,
    /// Per-sender sequence numbers
    sender_sequences: HashMap<u32, u64>,
    /// This member's leaf index
    pub self_index: LeafIndex,
}

impl MlsGroup {
    /// Create a new group (as creator)
    pub fn new(
        group_id: GroupId,
        creator_public_key: Vec<u8>,
        creator_identity: Vec<u8>,
        application_secret: Vec<u8>,
        config: MlsConfig,
    ) -> MlsResult<Self> {
        let mut tree = MlsTree::new();
        let self_index = tree.add_leaf(creator_public_key.clone())?;

        let metadata = GroupMetadata {
            group_id: group_id.clone(),
            name: None,
            epoch: 0,
            members: vec![MemberInfo {
                identity: creator_identity,
                leaf_index: self_index,
                joined_at: current_timestamp(),
            }],
            created_at: current_timestamp(),
            updated_at: current_timestamp(),
        };

        let key_schedule = KeySchedule::new(0, application_secret);

        Ok(Self {
            group_id,
            epoch: 0,
            tree,
            metadata,
            config,
            key_schedule,
            proposals: ProposalQueue::new(),
            replay_cache: HashSet::new(),
            sender_sequences: HashMap::new(),
            self_index,
        })
    }

    /// Join a group via Welcome message
    pub fn from_welcome(
        welcome: &Welcome,
        member_index: LeafIndex,
        member_public_key: &[u8],
    ) -> MlsResult<Self> {
        let (secrets, tree) = welcome.process(member_index, member_public_key)?;

        let key_schedule = KeySchedule::new(secrets.epoch, secrets.application_secret);

        Ok(Self {
            group_id: welcome.group_id.clone(),
            epoch: secrets.epoch,
            tree,
            metadata: welcome.metadata.clone(),
            config: MlsConfig::default(),
            key_schedule,
            proposals: ProposalQueue::new(),
            replay_cache: HashSet::new(),
            sender_sequences: HashMap::new(),
            self_index: member_index,
        })
    }

    /// Add a proposal to the queue
    pub fn add_proposal(&mut self, proposal: Proposal) -> MlsResult<u32> {
        // Verify epoch matches
        if proposal.epoch != self.epoch {
            return Err(MlsError::EpochMismatch {
                expected: self.epoch,
                actual: proposal.epoch,
            });
        }

        self.proposals.add(proposal)
    }

    /// Create and commit pending proposals
    pub fn commit(&mut self, path: Option<UpdatePath>) -> MlsResult<(Commit, Vec<Welcome>)> {
        if self.proposals.is_empty() && path.is_none() {
            return Err(MlsError::InvalidState(
                "No proposals to commit".to_string(),
            ));
        }

        // Collect proposal references
        let proposal_refs: Vec<ProposalRef> = (0..self.proposals.len())
            .map(|i| ProposalRef::Index(i as u32))
            .collect();

        // Create commit
        let mut commit = Commit::new(
            self.group_id.clone(),
            self.epoch,
            self.self_index,
            proposal_refs,
            path,
        );

        // Compute confirmation tag (simplified - would use actual MAC)
        let confirmation_tag = self.compute_confirmation_tag();
        commit.set_confirmation_tag(confirmation_tag);

        // Apply proposals to get new members
        let result = self.apply_proposals_internal()?;

        // Advance epoch FIRST
        self.advance_epoch()?;

        // Create Welcome messages for new members (after epoch advancement)
        let mut welcomes = Vec::new();
        for added_idx in &result.added_members {
            if let Some(node) = self.tree.get_node(MlsTree::leaf_to_node_index(*added_idx)) {
                if let Some(ref public_key) = node.public_key {
                    let welcome = self.create_welcome_for_member(*added_idx, public_key.clone())?;
                    welcomes.push(welcome);
                }
            }
        }

        Ok((commit, welcomes))
    }

    /// Apply a commit from another member
    pub fn apply_commit(&mut self, commit: &Commit) -> MlsResult<CommitResult> {
        // Validate commit
        let valid_senders: Vec<u32> = self
            .metadata
            .members
            .iter()
            .map(|m| m.leaf_index)
            .collect();
        
        let validator = CommitValidator::new(self.epoch, valid_senders);
        validator.validate(commit)?;

        // Verify confirmation tag
        let expected_tag = self.compute_confirmation_tag();
        commit.verify_confirmation_tag(&expected_tag)?;

        // Apply proposals
        let result = self.apply_proposals_internal()?;

        // Advance epoch
        self.advance_epoch()?;

        Ok(result)
    }

    /// Seal an application message
    pub fn seal_message(&mut self, plaintext: &[u8]) -> MlsResult<EncryptedMessage> {
        // Get next sequence number for this sender
        let sequence = self.get_next_sequence(self.self_index);

        let sender_data = SenderData {
            leaf_index: self.self_index,
            sequence,
            epoch: self.epoch,
        };

        encrypt_message(&mut self.key_schedule, sender_data, plaintext)
    }

    /// Open an application message
    pub fn open_message(&mut self, encrypted: &EncryptedMessage) -> MlsResult<Vec<u8>> {
        // Check for replay
        let replay_key = (encrypted.sender_leaf, encrypted.sequence);
        if self.replay_cache.contains(&replay_key) {
            return Err(MlsError::ReplayDetected(format!(
                "Message from sender {} with sequence {} already processed",
                encrypted.sender_leaf, encrypted.sequence
            )));
        }

        // Decrypt
        let plaintext = decrypt_message(&mut self.key_schedule, encrypted)?;

        // Update replay cache
        self.replay_cache.insert(replay_key);

        // Trim cache if needed
        if self.replay_cache.len() > self.config.replay_cache_size {
            self.trim_replay_cache();
        }

        Ok(plaintext)
    }

    /// Export tree snapshot for Welcome
    pub fn export_tree_snapshot(&self) -> TreeSnapshot {
        TreeSnapshot::from_tree(&self.tree)
    }

    /// Get current epoch
    pub fn current_epoch(&self) -> u64 {
        self.epoch
    }

    /// Get member count
    pub fn member_count(&self) -> usize {
        self.metadata.members.len()
    }

    // Internal helpers

    fn apply_proposals_internal(&mut self) -> MlsResult<CommitResult> {
        let mut result = CommitResult::empty(self.epoch + 1);

        for proposal in self.proposals.all() {
            match &proposal.content {
                ProposalContent::Add { public_key, identity } => {
                    let leaf_idx = self.tree.add_leaf(public_key.clone())?;
                    
                    self.metadata.members.push(MemberInfo {
                        identity: identity.clone(),
                        leaf_index: leaf_idx,
                        joined_at: current_timestamp(),
                    });
                    
                    result.added_members.push(leaf_idx);
                }
                ProposalContent::Update { public_key } => {
                    self.tree.update_leaf(proposal.sender, public_key.clone())?;
                    result.updated_members.push(proposal.sender);
                }
                ProposalContent::Remove { removed } => {
                    self.tree.remove_leaf(*removed)?;
                    
                    self.metadata.members.retain(|m| m.leaf_index != *removed);
                    result.removed_members.push(*removed);
                }
                ProposalContent::PreSharedKey { .. } => {
                    // PSK proposals would update key schedule
                    // Not implemented in this prototype
                }
            }
        }

        // Clear proposals after applying
        self.proposals.clear();

        Ok(result)
    }

    fn advance_epoch(&mut self) -> MlsResult<()> {
        self.epoch += 1;
        self.metadata.epoch = self.epoch;
        self.metadata.updated_at = current_timestamp();

        // Derive new application secret (simplified)
        let new_app_secret = derive_next_epoch_secret(&self.key_schedule.application_secret);
        self.key_schedule = KeySchedule::new(self.epoch, new_app_secret);

        // Clear replay cache on epoch change
        self.replay_cache.clear();
        self.sender_sequences.clear();

        Ok(())
    }

    fn compute_confirmation_tag(&self) -> Vec<u8> {
        // Simplified: hash of tree root + epoch
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        
        if let Some(root_hash) = self.tree.root_hash() {
            hasher.update(&root_hash);
        }
        hasher.update(&self.epoch.to_be_bytes());
        
        hasher.finalize().to_vec()
    }

    fn create_welcome_for_member(
        &self,
        member_index: LeafIndex,
        public_key: Vec<u8>,
    ) -> MlsResult<Welcome> {
        let secrets = WelcomeGroupSecrets {
            epoch: self.epoch,
            application_secret: self.key_schedule.application_secret.clone(),
            epoch_authenticator: self.compute_confirmation_tag(),
        };

        Welcome::create(
            self.group_id.clone(),
            self.epoch,
            secrets,
            &self.tree,
            self.metadata.clone(),
            vec![(member_index, public_key)],
        )
    }

    fn get_next_sequence(&mut self, sender: LeafIndex) -> u64 {
        let seq = self.sender_sequences.entry(sender).or_insert(0);
        let current = *seq;
        *seq += 1;
        current
    }

    fn trim_replay_cache(&mut self) {
        // Remove oldest entries (simplified - in production use LRU)
        let to_remove = self.replay_cache.len() - (self.config.replay_cache_size / 2);
        let keys_to_remove: Vec<_> = self.replay_cache.iter().take(to_remove).cloned().collect();
        
        for key in keys_to_remove {
            self.replay_cache.remove(&key);
        }
    }
}

// Helper functions

fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn derive_next_epoch_secret(current: &[u8]) -> Vec<u8> {
    // Simplified KDF: hash(current || "epoch")
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(current);
    hasher.update(b"epoch");
    hasher.finalize().to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_group_id() -> GroupId {
        GroupId::random()
    }

    #[test]
    fn test_group_creation() {
        let group_id = test_group_id();
        let group = MlsGroup::new(
            group_id.clone(),
            b"alice_pk".to_vec(),
            b"alice".to_vec(),
            vec![1, 2, 3, 4],
            MlsConfig::default(),
        )
        .unwrap();

        assert_eq!(group.group_id, group_id);
        assert_eq!(group.epoch, 0);
        assert_eq!(group.member_count(), 1);
        assert_eq!(group.self_index, 0);
    }

    #[test]
    fn test_add_proposal() {
        let mut group = MlsGroup::new(
            test_group_id(),
            b"alice_pk".to_vec(),
            b"alice".to_vec(),
            vec![1, 2, 3, 4],
            MlsConfig::default(),
        )
        .unwrap();

        let proposal = Proposal::new_add(
            0,
            0,
            b"bob_pk".to_vec(),
            b"bob".to_vec(),
        );

        let idx = group.add_proposal(proposal).unwrap();
        assert_eq!(idx, 0);
    }

    #[test]
    fn test_add_proposal_wrong_epoch() {
        let mut group = MlsGroup::new(
            test_group_id(),
            b"alice_pk".to_vec(),
            b"alice".to_vec(),
            vec![1, 2, 3, 4],
            MlsConfig::default(),
        )
        .unwrap();

        let proposal = Proposal::new_add(
            0,
            999, // Wrong epoch
            b"bob_pk".to_vec(),
            b"bob".to_vec(),
        );

        let result = group.add_proposal(proposal);
        assert!(matches!(result, Err(MlsError::EpochMismatch { .. })));
    }

    #[test]
    fn test_seal_and_open_message() {
        let mut group = MlsGroup::new(
            test_group_id(),
            b"alice_pk".to_vec(),
            b"alice".to_vec(),
            vec![1, 2, 3, 4],
            MlsConfig::default(),
        )
        .unwrap();

        let plaintext = b"Hello, group!";
        let encrypted = group.seal_message(plaintext).unwrap();

        assert_eq!(encrypted.epoch, 0);
        assert_eq!(encrypted.sender_leaf, 0);
        assert_eq!(encrypted.sequence, 0);

        // Open in same group
        let decrypted = group.open_message(&encrypted).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_replay_detection() {
        let mut group = MlsGroup::new(
            test_group_id(),
            b"alice_pk".to_vec(),
            b"alice".to_vec(),
            vec![1, 2, 3, 4],
            MlsConfig::default(),
        )
        .unwrap();

        let plaintext = b"Hello";
        let encrypted = group.seal_message(plaintext).unwrap();

        // First open succeeds
        assert!(group.open_message(&encrypted).is_ok());

        // Second open fails (replay)
        let result = group.open_message(&encrypted);
        assert!(matches!(result, Err(MlsError::ReplayDetected(_))));
    }

    #[test]
    fn test_sequence_numbers() {
        let mut group = MlsGroup::new(
            test_group_id(),
            b"alice_pk".to_vec(),
            b"alice".to_vec(),
            vec![1, 2, 3, 4],
            MlsConfig::default(),
        )
        .unwrap();

        let msg1 = group.seal_message(b"msg1").unwrap();
        assert_eq!(msg1.sequence, 0);

        let msg2 = group.seal_message(b"msg2").unwrap();
        assert_eq!(msg2.sequence, 1);

        let msg3 = group.seal_message(b"msg3").unwrap();
        assert_eq!(msg3.sequence, 2);
    }

    #[test]
    fn test_commit_add_member() {
        let mut group = MlsGroup::new(
            test_group_id(),
            b"alice_pk".to_vec(),
            b"alice".to_vec(),
            vec![1, 2, 3, 4],
            MlsConfig::default(),
        )
        .unwrap();

        // Add proposal
        let proposal = Proposal::new_add(
            0,
            0,
            b"bob_pk".to_vec(),
            b"bob".to_vec(),
        );
        group.add_proposal(proposal).unwrap();

        // Commit
        let (commit, welcomes) = group.commit(None).unwrap();

        assert_eq!(commit.epoch, 0); // Committed at epoch 0
        assert_eq!(group.epoch, 1); // Now at epoch 1
        assert_eq!(group.member_count(), 2);
        assert_eq!(welcomes.len(), 1);
    }

    #[test]
    fn test_commit_remove_member() {
        let mut group = MlsGroup::new(
            test_group_id(),
            b"alice_pk".to_vec(),
            b"alice".to_vec(),
            vec![1, 2, 3, 4],
            MlsConfig::default(),
        )
        .unwrap();

        // Add bob first
        let add_proposal = Proposal::new_add(
            0,
            0,
            b"bob_pk".to_vec(),
            b"bob".to_vec(),
        );
        group.add_proposal(add_proposal).unwrap();
        group.commit(None).unwrap();

        assert_eq!(group.member_count(), 2);

        // Remove bob
        let remove_proposal = Proposal::new_remove(0, 1, 1); // Remove leaf 1
        group.add_proposal(remove_proposal).unwrap();
        group.commit(None).unwrap();

        assert_eq!(group.member_count(), 1);
        assert_eq!(group.epoch, 2);
    }

    #[test]
    fn test_commit_update_member() {
        let mut group = MlsGroup::new(
            test_group_id(),
            b"alice_pk".to_vec(),
            b"alice".to_vec(),
            vec![1, 2, 3, 4],
            MlsConfig::default(),
        )
        .unwrap();

        // Update self
        let update_proposal = Proposal::new_update(
            0,
            0,
            b"alice_new_pk".to_vec(),
        );
        group.add_proposal(update_proposal).unwrap();

        let (commit, _) = group.commit(None).unwrap();

        assert_eq!(commit.proposals.len(), 1);
        assert_eq!(group.epoch, 1);
    }

    #[test]
    fn test_commit_without_proposals_fails() {
        let mut group = MlsGroup::new(
            test_group_id(),
            b"alice_pk".to_vec(),
            b"alice".to_vec(),
            vec![1, 2, 3, 4],
            MlsConfig::default(),
        )
        .unwrap();

        let result = group.commit(None);
        assert!(result.is_err());
    }

    #[test]
    fn test_epoch_advancement() {
        let mut group = MlsGroup::new(
            test_group_id(),
            b"alice_pk".to_vec(),
            b"alice".to_vec(),
            vec![1, 2, 3, 4],
            MlsConfig::default(),
        )
        .unwrap();

        assert_eq!(group.current_epoch(), 0);

        // Commit advances epoch
        let proposal = Proposal::new_add(0, 0, b"bob_pk".to_vec(), b"bob".to_vec());
        group.add_proposal(proposal).unwrap();
        group.commit(None).unwrap();

        assert_eq!(group.current_epoch(), 1);
    }

    #[test]
    fn test_from_welcome() {
        // Create group
        let mut creator = MlsGroup::new(
            test_group_id(),
            b"alice_pk".to_vec(),
            b"alice".to_vec(),
            vec![1, 2, 3, 4],
            MlsConfig::default(),
        )
        .unwrap();

        // Add bob
        let proposal = Proposal::new_add(
            0,
            0,
            b"bob_pk".to_vec(),
            b"bob".to_vec(),
        );
        creator.add_proposal(proposal).unwrap();
        let (_, welcomes) = creator.commit(None).unwrap();

        // Bob joins via welcome
        let bob_group = MlsGroup::from_welcome(
            &welcomes[0],
            1, // Bob is at leaf index 1
            b"bob_pk",
        )
        .unwrap();

        assert_eq!(bob_group.epoch, creator.epoch);
        assert_eq!(bob_group.group_id, creator.group_id);
        assert_eq!(bob_group.self_index, 1);
    }
}
