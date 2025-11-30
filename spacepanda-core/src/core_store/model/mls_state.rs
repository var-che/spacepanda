/*
    mls_state.rs - MLS (Message Layer Security) State
    
    Per-channel MLS epoch state tracking.
    
    MLS provides end-to-end encryption for group messaging.
    This module tracks the cryptographic state needed for:
    - Group key ratcheting
    - Member addition/removal
    - Epoch transitions
    - Welcome messages for new members
    
    Note: This is metadata storage only. Actual MLS crypto
    is handled by the core_mls module.
*/

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// MLS epoch number (monotonically increasing)
pub type EpochId = u64;

/// MLS group state for a channel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MLSState {
    /// Current epoch number
    pub epoch: EpochId,
    
    /// Group ID (typically derived from channel ID)
    pub group_id: Vec<u8>,
    
    /// Cipher suite in use
    pub cipher_suite: CipherSuite,
    
    /// Current group members and their leaf indices
    pub members: HashMap<String, LeafIndex>,
    
    /// Pending proposals (adds, removes, updates)
    pub pending_proposals: Vec<Proposal>,
    
    /// Epoch secret (encrypted with user's private key)
    pub epoch_secret: Vec<u8>,
    
    /// Confirmation tag for current epoch
    pub confirmation_tag: Vec<u8>,
    
    /// Tree hash for consistency checking
    pub tree_hash: Vec<u8>,
    
    /// When this epoch started
    pub epoch_start: u64,
}

/// MLS cipher suite identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CipherSuite {
    /// MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519
    MLS128DHKEMX25519AES128GCM,
    
    /// MLS_128_DHKEMX25519_CHACHA20POLY1305_SHA256_Ed25519  
    MLS128DHKEMX25519CHACHA20POLY1305,
    
    /// MLS_256_DHKEMX448_AES256GCM_SHA512_Ed448
    MLS256DHKEMX448AES256GCM,
}

impl Default for CipherSuite {
    fn default() -> Self {
        CipherSuite::MLS128DHKEMX25519AES128GCM
    }
}

/// Leaf index in the MLS tree
pub type LeafIndex = u32;

/// MLS proposal (pending group change)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Proposal {
    /// Add a new member
    Add {
        key_package: Vec<u8>,
        proposed_by: String,
    },
    
    /// Remove a member
    Remove {
        removed: LeafIndex,
        proposed_by: String,
    },
    
    /// Update own leaf key
    Update {
        leaf_node: Vec<u8>,
    },
    
    /// Group context extension
    GroupContextExtension {
        extension_data: Vec<u8>,
    },
}

/// Welcome message for new members
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Welcome {
    /// MLS version
    pub version: u16,
    
    /// Cipher suite
    pub cipher_suite: CipherSuite,
    
    /// Encrypted group secrets
    pub secrets: Vec<u8>,
    
    /// Encrypted group info
    pub encrypted_group_info: Vec<u8>,
}

/// Commit message (finalizes proposals)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Commit {
    /// Proposals being committed
    pub proposals: Vec<Proposal>,
    
    /// Path for updating encryption keys
    pub path: Option<UpdatePath>,
    
    /// New epoch number
    pub new_epoch: EpochId,
}

/// Update path for key ratcheting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdatePath {
    /// New leaf node
    pub leaf_node: Vec<u8>,
    
    /// Encrypted path secrets
    pub nodes: Vec<Vec<u8>>,
}

impl MLSState {
    /// Create initial MLS state for a new group
    pub fn new(group_id: Vec<u8>, cipher_suite: CipherSuite) -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};
        
        let epoch_start = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        MLSState {
            epoch: 0,
            group_id,
            cipher_suite,
            members: HashMap::new(),
            pending_proposals: Vec::new(),
            epoch_secret: Vec::new(),
            confirmation_tag: Vec::new(),
            tree_hash: Vec::new(),
            epoch_start,
        }
    }
    
    /// Add a member to the group
    pub fn add_member(&mut self, user_id: String, leaf_index: LeafIndex) {
        self.members.insert(user_id, leaf_index);
    }
    
    /// Remove a member from the group
    pub fn remove_member(&mut self, user_id: &str) -> Option<LeafIndex> {
        self.members.remove(user_id)
    }
    
    /// Get member's leaf index
    pub fn get_leaf_index(&self, user_id: &str) -> Option<LeafIndex> {
        self.members.get(user_id).copied()
    }
    
    /// Check if user is a member
    pub fn is_member(&self, user_id: &str) -> bool {
        self.members.contains_key(user_id)
    }
    
    /// Get member count
    pub fn member_count(&self) -> usize {
        self.members.len()
    }
    
    /// Add a pending proposal
    pub fn add_proposal(&mut self, proposal: Proposal) {
        self.pending_proposals.push(proposal);
    }
    
    /// Clear pending proposals (after commit)
    pub fn clear_proposals(&mut self) {
        self.pending_proposals.clear();
    }
    
    /// Advance to next epoch
    pub fn advance_epoch(&mut self, new_secret: Vec<u8>, confirmation: Vec<u8>, tree_hash: Vec<u8>) {
        use std::time::{SystemTime, UNIX_EPOCH};
        
        self.epoch += 1;
        self.epoch_secret = new_secret;
        self.confirmation_tag = confirmation;
        self.tree_hash = tree_hash;
        self.epoch_start = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        self.clear_proposals();
    }
    
    /// Check if epoch secret is initialized
    pub fn has_secret(&self) -> bool {
        !self.epoch_secret.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_mls_state_creation() {
        let group_id = b"channel_123".to_vec();
        let state = MLSState::new(group_id.clone(), CipherSuite::default());
        
        assert_eq!(state.epoch, 0);
        assert_eq!(state.group_id, group_id);
        assert_eq!(state.member_count(), 0);
    }
    
    #[test]
    fn test_add_remove_member() {
        let group_id = b"channel_123".to_vec();
        let mut state = MLSState::new(group_id, CipherSuite::default());
        
        state.add_member("alice".to_string(), 0);
        state.add_member("bob".to_string(), 1);
        
        assert_eq!(state.member_count(), 2);
        assert!(state.is_member("alice"));
        assert!(state.is_member("bob"));
        
        let removed = state.remove_member("alice");
        assert_eq!(removed, Some(0));
        assert_eq!(state.member_count(), 1);
        assert!(!state.is_member("alice"));
    }
    
    #[test]
    fn test_get_leaf_index() {
        let group_id = b"channel_123".to_vec();
        let mut state = MLSState::new(group_id, CipherSuite::default());
        
        state.add_member("alice".to_string(), 5);
        
        assert_eq!(state.get_leaf_index("alice"), Some(5));
        assert_eq!(state.get_leaf_index("bob"), None);
    }
    
    #[test]
    fn test_proposals() {
        let group_id = b"channel_123".to_vec();
        let mut state = MLSState::new(group_id, CipherSuite::default());
        
        let proposal = Proposal::Add {
            key_package: vec![1, 2, 3],
            proposed_by: "alice".to_string(),
        };
        
        state.add_proposal(proposal);
        assert_eq!(state.pending_proposals.len(), 1);
        
        state.clear_proposals();
        assert_eq!(state.pending_proposals.len(), 0);
    }
    
    #[test]
    fn test_advance_epoch() {
        let group_id = b"channel_123".to_vec();
        let mut state = MLSState::new(group_id, CipherSuite::default());
        
        assert_eq!(state.epoch, 0);
        
        state.advance_epoch(
            vec![1, 2, 3],
            vec![4, 5, 6],
            vec![7, 8, 9],
        );
        
        assert_eq!(state.epoch, 1);
        assert!(state.has_secret());
    }
}
