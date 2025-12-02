//! MLS Commits for applying group state changes
//!
//! A Commit message:
//! - References a set of Proposals to apply
//! - Updates the ratchet tree and epoch
//! - Is signed by the committer
//! - Advances the group to a new epoch
//!
//! Commits ensure atomic state transitions with proper authentication.

use super::errors::{MlsError, MlsResult};
use super::proposals::{Proposal, ProposalRef};
use super::types::GroupId;
use serde::{Deserialize, Serialize};

/// A commit message that applies proposals and advances epoch
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Commit {
    /// Group ID
    pub group_id: GroupId,
    /// Current epoch (before commit)
    pub epoch: u64,
    /// Committer's leaf index
    pub sender: u32,
    /// Proposals being committed
    pub proposals: Vec<ProposalRef>,
    /// Path update (optional - only if committer updates their key)
    pub path: Option<UpdatePath>,
    /// Confirmation tag (MAC over group state)
    pub confirmation_tag: Vec<u8>,
    /// Signature over commit (committer's signature key)
    pub signature: Vec<u8>,
}

/// Update path for key material refresh
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdatePath {
    /// New leaf public key
    pub leaf_public_key: Vec<u8>,
    /// Encrypted path secrets for resolution
    pub encrypted_path_secrets: Vec<Vec<u8>>,
}

impl Commit {
    /// Create a new commit
    pub fn new(
        group_id: GroupId,
        epoch: u64,
        sender: u32,
        proposals: Vec<ProposalRef>,
        path: Option<UpdatePath>,
    ) -> Self {
        Self {
            group_id,
            epoch,
            sender,
            proposals,
            path,
            confirmation_tag: Vec::new(), // Set after computing
            signature: Vec::new(),         // Set after signing
        }
    }

    /// Set confirmation tag (MAC over new group state)
    pub fn set_confirmation_tag(&mut self, tag: Vec<u8>) {
        self.confirmation_tag = tag;
    }

    /// Get bytes to sign (commit without signature)
    pub fn to_be_signed(&self) -> MlsResult<Vec<u8>> {
        let mut unsigned = self.clone();
        unsigned.signature = Vec::new();

        bincode::serialize(&unsigned)
            .map_err(|e| MlsError::PersistenceError(format!("Failed to serialize commit: {}", e)))
    }

    /// Sign the commit
    pub fn sign<F>(&mut self, sign_fn: F) -> MlsResult<()>
    where
        F: FnOnce(&[u8]) -> MlsResult<Vec<u8>>,
    {
        let to_sign = self.to_be_signed()?;
        self.signature = sign_fn(&to_sign)?;
        Ok(())
    }

    /// Verify commit signature
    pub fn verify<F>(&self, verify_fn: F) -> MlsResult<()>
    where
        F: FnOnce(&[u8], &[u8]) -> MlsResult<bool>,
    {
        let to_verify = self.to_be_signed()?;
        let valid = verify_fn(&to_verify, &self.signature)?;

        if valid {
            Ok(())
        } else {
            Err(MlsError::VerifyFailed("Commit signature invalid".to_string()))
        }
    }

    /// Verify confirmation tag matches expected state
    pub fn verify_confirmation_tag(&self, expected: &[u8]) -> MlsResult<()> {
        if self.confirmation_tag == expected {
            Ok(())
        } else {
            Err(MlsError::VerifyFailed(
                "Confirmation tag mismatch".to_string(),
            ))
        }
    }

    /// Serialize to bytes
    pub fn to_bytes(&self) -> MlsResult<Vec<u8>> {
        bincode::serialize(self)
            .map_err(|e| MlsError::PersistenceError(format!("Failed to serialize commit: {}", e)))
    }

    /// Deserialize from bytes
    pub fn from_bytes(bytes: &[u8]) -> MlsResult<Self> {
        bincode::deserialize(bytes)
            .map_err(|e| MlsError::PersistenceError(format!("Failed to deserialize commit: {}", e)))
    }
}

/// Outcome of applying a commit
#[derive(Debug, Clone)]
pub struct CommitResult {
    /// New epoch after commit
    pub new_epoch: u64,
    /// Members added
    pub added_members: Vec<u32>,
    /// Members removed
    pub removed_members: Vec<u32>,
    /// Members who updated keys
    pub updated_members: Vec<u32>,
}

impl CommitResult {
    /// Create empty result
    pub fn empty(new_epoch: u64) -> Self {
        Self {
            new_epoch,
            added_members: Vec::new(),
            removed_members: Vec::new(),
            updated_members: Vec::new(),
        }
    }

    /// Check if commit made any changes
    pub fn has_changes(&self) -> bool {
        !self.added_members.is_empty()
            || !self.removed_members.is_empty()
            || !self.updated_members.is_empty()
    }
}

/// Validator for commits
pub struct CommitValidator {
    /// Current epoch
    current_epoch: u64,
    /// Valid sender indices
    valid_senders: Vec<u32>,
}

impl CommitValidator {
    /// Create new validator
    pub fn new(current_epoch: u64, valid_senders: Vec<u32>) -> Self {
        Self {
            current_epoch,
            valid_senders,
        }
    }

    /// Validate commit epoch
    pub fn validate_epoch(&self, commit: &Commit) -> MlsResult<()> {
        if commit.epoch != self.current_epoch {
            return Err(MlsError::EpochMismatch {
                expected: self.current_epoch,
                actual: commit.epoch,
            });
        }
        Ok(())
    }

    /// Validate commit sender is authorized
    pub fn validate_sender(&self, commit: &Commit) -> MlsResult<()> {
        if !self.valid_senders.contains(&commit.sender) {
            return Err(MlsError::InvalidState(format!(
                "Unauthorized commit sender: {}",
                commit.sender
            )));
        }
        Ok(())
    }

    /// Validate commit has non-empty proposals
    pub fn validate_proposals(&self, commit: &Commit) -> MlsResult<()> {
        if commit.proposals.is_empty() && commit.path.is_none() {
            return Err(MlsError::InvalidState(
                "Commit must have proposals or path update".to_string(),
            ));
        }
        Ok(())
    }

    /// Run all validations
    pub fn validate(&self, commit: &Commit) -> MlsResult<()> {
        self.validate_epoch(commit)?;
        self.validate_sender(commit)?;
        self.validate_proposals(commit)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core_mls::types::GroupId;

    fn dummy_sign(data: &[u8]) -> MlsResult<Vec<u8>> {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(data);
        Ok(hasher.finalize().to_vec())
    }

    fn dummy_verify(data: &[u8], sig: &[u8]) -> MlsResult<bool> {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(data);
        let expected = hasher.finalize();
        Ok(expected.as_slice() == sig)
    }

    fn test_group_id() -> GroupId {
        GroupId::random()
    }

    #[test]
    fn test_commit_creation() {
        let group_id = test_group_id();
        let proposals = vec![ProposalRef::Index(0), ProposalRef::Index(1)];

        let commit = Commit::new(group_id.clone(), 1, 0, proposals.clone(), None);

        assert_eq!(commit.group_id, group_id);
        assert_eq!(commit.epoch, 1);
        assert_eq!(commit.sender, 0);
        assert_eq!(commit.proposals.len(), 2);
        assert!(commit.path.is_none());
    }

    #[test]
    fn test_commit_with_path() {
        let group_id = test_group_id();
        let path = UpdatePath {
            leaf_public_key: b"new_key".to_vec(),
            encrypted_path_secrets: vec![b"secret1".to_vec(), b"secret2".to_vec()],
        };

        let commit = Commit::new(group_id, 1, 0, vec![], Some(path));

        assert!(commit.path.is_some());
        assert_eq!(commit.path.as_ref().unwrap().leaf_public_key, b"new_key");
    }

    #[test]
    fn test_commit_confirmation_tag() {
        let group_id = test_group_id();
        let mut commit = Commit::new(group_id, 1, 0, vec![ProposalRef::Index(0)], None);

        let tag = b"confirmation_tag_123".to_vec();
        commit.set_confirmation_tag(tag.clone());

        assert_eq!(commit.confirmation_tag, tag);
    }

    #[test]
    fn test_commit_sign_and_verify() {
        let group_id = test_group_id();
        let mut commit = Commit::new(group_id, 1, 0, vec![ProposalRef::Index(0)], None);

        commit.sign(dummy_sign).unwrap();
        assert!(!commit.signature.is_empty());

        assert!(commit.verify(dummy_verify).is_ok());
    }

    #[test]
    fn test_commit_verify_invalid_signature() {
        let group_id = test_group_id();
        let mut commit = Commit::new(group_id, 1, 0, vec![ProposalRef::Index(0)], None);

        commit.sign(dummy_sign).unwrap();
        commit.signature[0] ^= 0xFF; // Corrupt

        let result = commit.verify(dummy_verify);
        assert!(result.is_err());
    }

    #[test]
    fn test_commit_verify_confirmation_tag() {
        let group_id = test_group_id();
        let mut commit = Commit::new(group_id, 1, 0, vec![ProposalRef::Index(0)], None);

        let tag = b"tag123".to_vec();
        commit.set_confirmation_tag(tag.clone());

        assert!(commit.verify_confirmation_tag(&tag).is_ok());

        let wrong_tag = b"wrong_tag".to_vec();
        assert!(commit.verify_confirmation_tag(&wrong_tag).is_err());
    }

    #[test]
    fn test_commit_serialization() {
        let group_id = test_group_id();
        let mut commit = Commit::new(
            group_id.clone(),
            1,
            0,
            vec![ProposalRef::Index(0)],
            None,
        );
        commit.sign(dummy_sign).unwrap();

        let bytes = commit.to_bytes().unwrap();
        let decoded = Commit::from_bytes(&bytes).unwrap();

        assert_eq!(decoded.group_id, group_id);
        assert_eq!(decoded.epoch, 1);
        assert_eq!(decoded.sender, 0);
        assert_eq!(decoded.signature, commit.signature);
    }

    #[test]
    fn test_commit_result_empty() {
        let result = CommitResult::empty(2);

        assert_eq!(result.new_epoch, 2);
        assert!(!result.has_changes());
    }

    #[test]
    fn test_commit_result_has_changes() {
        let mut result = CommitResult::empty(2);
        result.added_members.push(5);

        assert!(result.has_changes());
    }

    #[test]
    fn test_commit_validator_epoch() {
        let validator = CommitValidator::new(1, vec![0, 1, 2]);

        let group_id = test_group_id();
        let valid_commit = Commit::new(group_id.clone(), 1, 0, vec![ProposalRef::Index(0)], None);
        assert!(validator.validate_epoch(&valid_commit).is_ok());

        let invalid_commit = Commit::new(group_id, 999, 0, vec![ProposalRef::Index(0)], None);
        assert!(validator.validate_epoch(&invalid_commit).is_err());
    }

    #[test]
    fn test_commit_validator_sender() {
        let validator = CommitValidator::new(1, vec![0, 1, 2]);

        let group_id = test_group_id();
        let valid_commit = Commit::new(group_id.clone(), 1, 1, vec![ProposalRef::Index(0)], None);
        assert!(validator.validate_sender(&valid_commit).is_ok());

        let invalid_commit = Commit::new(group_id, 1, 99, vec![ProposalRef::Index(0)], None);
        assert!(validator.validate_sender(&invalid_commit).is_err());
    }

    #[test]
    fn test_commit_validator_proposals() {
        let validator = CommitValidator::new(1, vec![0]);

        let group_id = test_group_id();
        
        // Valid: has proposals
        let valid_commit = Commit::new(group_id.clone(), 1, 0, vec![ProposalRef::Index(0)], None);
        assert!(validator.validate_proposals(&valid_commit).is_ok());

        // Valid: has path update
        let path = UpdatePath {
            leaf_public_key: b"key".to_vec(),
            encrypted_path_secrets: vec![],
        };
        let valid_commit2 = Commit::new(group_id.clone(), 1, 0, vec![], Some(path));
        assert!(validator.validate_proposals(&valid_commit2).is_ok());

        // Invalid: empty
        let invalid_commit = Commit::new(group_id, 1, 0, vec![], None);
        assert!(validator.validate_proposals(&invalid_commit).is_err());
    }

    #[test]
    fn test_commit_validator_full() {
        let validator = CommitValidator::new(1, vec![0, 1]);

        let group_id = test_group_id();
        let commit = Commit::new(group_id, 1, 0, vec![ProposalRef::Index(0)], None);

        assert!(validator.validate(&commit).is_ok());
    }

    #[test]
    fn test_commit_validator_full_fails() {
        let validator = CommitValidator::new(1, vec![0, 1]);

        let group_id = test_group_id();
        
        // Wrong epoch
        let commit1 = Commit::new(group_id.clone(), 2, 0, vec![ProposalRef::Index(0)], None);
        assert!(validator.validate(&commit1).is_err());

        // Unauthorized sender
        let commit2 = Commit::new(group_id.clone(), 1, 99, vec![ProposalRef::Index(0)], None);
        assert!(validator.validate(&commit2).is_err());

        // Empty commit
        let commit3 = Commit::new(group_id, 1, 0, vec![], None);
        assert!(validator.validate(&commit3).is_err());
    }
}
