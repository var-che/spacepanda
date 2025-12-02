//! MLS Proposals for group state changes
//!
//! Proposals represent suggested changes to the group state:
//! - Add: Add a new member
//! - Update: Update a member's key material
//! - Remove: Remove a member from the group
//! - PreSharedKey: Inject external entropy
//!
//! Proposals are collected and applied atomically via Commits.

use super::errors::{MlsError, MlsResult};
use serde::{Deserialize, Serialize};

/// Type of proposal
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProposalType {
    /// Add a new member to the group
    Add,
    /// Update a member's key material (self-update)
    Update,
    /// Remove a member from the group
    Remove,
    /// Inject pre-shared key for extra entropy
    PreSharedKey,
}

/// A proposal to change group state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proposal {
    /// Type of proposal
    pub proposal_type: ProposalType,
    /// Proposer's leaf index
    pub sender: u32,
    /// Epoch when proposed
    pub epoch: u64,
    /// Proposal-specific data
    pub content: ProposalContent,
    /// Signature over proposal (proposer's signature key)
    pub signature: Vec<u8>,
}

/// Proposal-specific content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProposalContent {
    /// Add proposal: new member's KeyPackage
    Add {
        /// New member's public key
        public_key: Vec<u8>,
        /// New member's identity
        identity: Vec<u8>,
    },
    /// Update proposal: sender's new public key
    Update {
        /// Updated public key
        public_key: Vec<u8>,
    },
    /// Remove proposal: target member's leaf index
    Remove {
        /// Leaf index to remove
        removed: u32,
    },
    /// Pre-shared key proposal
    PreSharedKey {
        /// PSK ID
        psk_id: Vec<u8>,
    },
}

impl Proposal {
    /// Create a new Add proposal
    pub fn new_add(
        sender: u32,
        epoch: u64,
        public_key: Vec<u8>,
        identity: Vec<u8>,
    ) -> Self {
        Self {
            proposal_type: ProposalType::Add,
            sender,
            epoch,
            content: ProposalContent::Add {
                public_key,
                identity,
            },
            signature: Vec::new(), // Set after signing
        }
    }

    /// Create a new Update proposal
    pub fn new_update(sender: u32, epoch: u64, public_key: Vec<u8>) -> Self {
        Self {
            proposal_type: ProposalType::Update,
            sender,
            epoch,
            content: ProposalContent::Update { public_key },
            signature: Vec::new(),
        }
    }

    /// Create a new Remove proposal
    pub fn new_remove(sender: u32, epoch: u64, removed: u32) -> Self {
        Self {
            proposal_type: ProposalType::Remove,
            sender,
            epoch,
            content: ProposalContent::Remove { removed },
            signature: Vec::new(),
        }
    }

    /// Create a new PSK proposal
    pub fn new_psk(sender: u32, epoch: u64, psk_id: Vec<u8>) -> Self {
        Self {
            proposal_type: ProposalType::PreSharedKey,
            sender,
            epoch,
            content: ProposalContent::PreSharedKey { psk_id },
            signature: Vec::new(),
        }
    }

    /// Get bytes to sign (proposal without signature)
    pub fn to_be_signed(&self) -> MlsResult<Vec<u8>> {
        // Create a copy without signature
        let mut unsigned = self.clone();
        unsigned.signature = Vec::new();

        bincode::serialize(&unsigned)
            .map_err(|e| MlsError::PersistenceError(format!("Failed to serialize proposal: {}", e)))
    }

    /// Sign the proposal with a signing function
    pub fn sign<F>(&mut self, sign_fn: F) -> MlsResult<()>
    where
        F: FnOnce(&[u8]) -> MlsResult<Vec<u8>>,
    {
        let to_sign = self.to_be_signed()?;
        self.signature = sign_fn(&to_sign)?;
        Ok(())
    }

    /// Verify the proposal signature
    pub fn verify<F>(&self, verify_fn: F) -> MlsResult<()>
    where
        F: FnOnce(&[u8], &[u8]) -> MlsResult<bool>,
    {
        let to_verify = self.to_be_signed()?;
        let valid = verify_fn(&to_verify, &self.signature)?;

        if valid {
            Ok(())
        } else {
            Err(MlsError::VerifyFailed("Proposal signature invalid".to_string()))
        }
    }

    /// Serialize to bytes
    pub fn to_bytes(&self) -> MlsResult<Vec<u8>> {
        bincode::serialize(self)
            .map_err(|e| MlsError::PersistenceError(format!("Failed to serialize proposal: {}", e)))
    }

    /// Deserialize from bytes
    pub fn from_bytes(bytes: &[u8]) -> MlsResult<Self> {
        bincode::deserialize(bytes)
            .map_err(|e| MlsError::PersistenceError(format!("Failed to deserialize proposal: {}", e)))
    }
}

/// Reference to a proposal (by hash or index)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProposalRef {
    /// Reference by hash
    Hash(Vec<u8>),
    /// Reference by index in proposal list
    Index(u32),
}

/// Collection of pending proposals
#[derive(Debug, Clone, Default)]
pub struct ProposalQueue {
    /// Pending proposals
    proposals: Vec<Proposal>,
}

impl ProposalQueue {
    /// Create new empty queue
    pub fn new() -> Self {
        Self {
            proposals: Vec::new(),
        }
    }

    /// Add a proposal to the queue
    pub fn add(&mut self, proposal: Proposal) -> MlsResult<u32> {
        // Verify proposal signature before adding
        // (verification function would be passed by caller)
        
        let index = self.proposals.len() as u32;
        self.proposals.push(proposal);
        Ok(index)
    }

    /// Get proposal by index
    pub fn get(&self, index: u32) -> Option<&Proposal> {
        self.proposals.get(index as usize)
    }

    /// Remove proposal by index
    pub fn remove(&mut self, index: u32) -> Option<Proposal> {
        if (index as usize) < self.proposals.len() {
            Some(self.proposals.remove(index as usize))
        } else {
            None
        }
    }

    /// Get all proposals
    pub fn all(&self) -> &[Proposal] {
        &self.proposals
    }

    /// Clear all proposals
    pub fn clear(&mut self) {
        self.proposals.clear();
    }

    /// Number of pending proposals
    pub fn len(&self) -> usize {
        self.proposals.len()
    }

    /// Check if queue is empty
    pub fn is_empty(&self) -> bool {
        self.proposals.is_empty()
    }

    /// Filter proposals by type
    pub fn by_type(&self, proposal_type: ProposalType) -> Vec<&Proposal> {
        self.proposals
            .iter()
            .filter(|p| p.proposal_type == proposal_type)
            .collect()
    }

    /// Filter proposals by sender
    pub fn by_sender(&self, sender: u32) -> Vec<&Proposal> {
        self.proposals
            .iter()
            .filter(|p| p.sender == sender)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core_mls::crypto::{MlsSigningKey, sign_with_key, verify_with_key};

    fn test_signing_key() -> MlsSigningKey {
        // Use deterministic key for tests
        let seed = [42u8; 32];
        MlsSigningKey::from_bytes(&seed)
    }

    fn dummy_sign(data: &[u8]) -> MlsResult<Vec<u8>> {
        // Real Ed25519 signature
        let key = test_signing_key();
        sign_with_key(data, &key)
    }

    fn dummy_verify(data: &[u8], sig: &[u8]) -> MlsResult<bool> {
        // Real Ed25519 verification
        let key = test_signing_key();
        let verifying_key = key.verifying_key();
        verify_with_key(data, sig, &verifying_key)
    }

    #[test]
    fn test_add_proposal_creation() {
        let proposal = Proposal::new_add(
            0,
            1,
            b"public_key".to_vec(),
            b"alice".to_vec(),
        );

        assert_eq!(proposal.proposal_type, ProposalType::Add);
        assert_eq!(proposal.sender, 0);
        assert_eq!(proposal.epoch, 1);
        assert!(matches!(proposal.content, ProposalContent::Add { .. }));
    }

    #[test]
    fn test_update_proposal_creation() {
        let proposal = Proposal::new_update(1, 2, b"new_key".to_vec());

        assert_eq!(proposal.proposal_type, ProposalType::Update);
        assert_eq!(proposal.sender, 1);
        assert_eq!(proposal.epoch, 2);
        assert!(matches!(proposal.content, ProposalContent::Update { .. }));
    }

    #[test]
    fn test_remove_proposal_creation() {
        let proposal = Proposal::new_remove(0, 1, 5);

        assert_eq!(proposal.proposal_type, ProposalType::Remove);
        assert_eq!(proposal.sender, 0);
        assert_eq!(proposal.epoch, 1);
        
        if let ProposalContent::Remove { removed } = proposal.content {
            assert_eq!(removed, 5);
        } else {
            panic!("Wrong content type");
        }
    }

    #[test]
    fn test_psk_proposal_creation() {
        let proposal = Proposal::new_psk(0, 1, b"psk_id_123".to_vec());

        assert_eq!(proposal.proposal_type, ProposalType::PreSharedKey);
        assert!(matches!(proposal.content, ProposalContent::PreSharedKey { .. }));
    }

    #[test]
    fn test_proposal_sign_and_verify() {
        let mut proposal = Proposal::new_add(
            0,
            1,
            b"public_key".to_vec(),
            b"alice".to_vec(),
        );

        // Sign
        proposal.sign(dummy_sign).unwrap();
        assert!(!proposal.signature.is_empty());

        // Verify
        assert!(proposal.verify(dummy_verify).is_ok());
    }

    #[test]
    fn test_proposal_verify_invalid_signature() {
        let mut proposal = Proposal::new_add(
            0,
            1,
            b"public_key".to_vec(),
            b"alice".to_vec(),
        );

        proposal.sign(dummy_sign).unwrap();

        // Corrupt signature
        proposal.signature[0] ^= 0xFF;

        let result = proposal.verify(dummy_verify);
        assert!(result.is_err());
    }

    #[test]
    fn test_proposal_serialization() {
        let mut proposal = Proposal::new_add(
            0,
            1,
            b"public_key".to_vec(),
            b"alice".to_vec(),
        );
        proposal.sign(dummy_sign).unwrap();

        let bytes = proposal.to_bytes().unwrap();
        let decoded = Proposal::from_bytes(&bytes).unwrap();

        assert_eq!(decoded.sender, 0);
        assert_eq!(decoded.epoch, 1);
        assert_eq!(decoded.signature, proposal.signature);
    }

    #[test]
    fn test_proposal_queue_add() {
        let mut queue = ProposalQueue::new();
        
        let proposal = Proposal::new_add(
            0,
            1,
            b"public_key".to_vec(),
            b"alice".to_vec(),
        );

        let index = queue.add(proposal).unwrap();
        assert_eq!(index, 0);
        assert_eq!(queue.len(), 1);
    }

    #[test]
    fn test_proposal_queue_get() {
        let mut queue = ProposalQueue::new();
        
        let proposal = Proposal::new_add(
            0,
            1,
            b"public_key".to_vec(),
            b"alice".to_vec(),
        );

        queue.add(proposal.clone()).unwrap();
        
        let retrieved = queue.get(0).unwrap();
        assert_eq!(retrieved.sender, 0);
    }

    #[test]
    fn test_proposal_queue_remove() {
        let mut queue = ProposalQueue::new();
        
        let proposal1 = Proposal::new_add(
            0,
            1,
            b"key1".to_vec(),
            b"alice".to_vec(),
        );
        let proposal2 = Proposal::new_update(1, 1, b"key2".to_vec());

        queue.add(proposal1).unwrap();
        queue.add(proposal2).unwrap();

        assert_eq!(queue.len(), 2);

        let removed = queue.remove(0).unwrap();
        assert_eq!(removed.sender, 0);
        assert_eq!(queue.len(), 1);
    }

    #[test]
    fn test_proposal_queue_clear() {
        let mut queue = ProposalQueue::new();
        
        queue.add(Proposal::new_add(0, 1, b"k1".to_vec(), b"a".to_vec())).unwrap();
        queue.add(Proposal::new_update(1, 1, b"k2".to_vec())).unwrap();

        assert_eq!(queue.len(), 2);

        queue.clear();
        assert_eq!(queue.len(), 0);
        assert!(queue.is_empty());
    }

    #[test]
    fn test_proposal_queue_filter_by_type() {
        let mut queue = ProposalQueue::new();
        
        queue.add(Proposal::new_add(0, 1, b"k1".to_vec(), b"a".to_vec())).unwrap();
        queue.add(Proposal::new_update(1, 1, b"k2".to_vec())).unwrap();
        queue.add(Proposal::new_add(2, 1, b"k3".to_vec(), b"b".to_vec())).unwrap();
        queue.add(Proposal::new_remove(0, 1, 5)).unwrap();

        let adds = queue.by_type(ProposalType::Add);
        assert_eq!(adds.len(), 2);

        let updates = queue.by_type(ProposalType::Update);
        assert_eq!(updates.len(), 1);

        let removes = queue.by_type(ProposalType::Remove);
        assert_eq!(removes.len(), 1);
    }

    #[test]
    fn test_proposal_queue_filter_by_sender() {
        let mut queue = ProposalQueue::new();
        
        queue.add(Proposal::new_add(0, 1, b"k1".to_vec(), b"a".to_vec())).unwrap();
        queue.add(Proposal::new_update(1, 1, b"k2".to_vec())).unwrap();
        queue.add(Proposal::new_add(0, 1, b"k3".to_vec(), b"b".to_vec())).unwrap();

        let from_0 = queue.by_sender(0);
        assert_eq!(from_0.len(), 2);

        let from_1 = queue.by_sender(1);
        assert_eq!(from_1.len(), 1);
    }
}
