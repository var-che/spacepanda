//! MLS High-Level API - MlsHandle facade
//!
//! ⚠️ **DEPRECATED**: This module contains the legacy custom MLS implementation.
//! Use `handle::MlsHandle` (OpenMLS-based) instead. This module will be removed in v0.3.0.
//!
//! This module provides a clean, ergonomic API for MLS operations:
//! - Group creation and joining
//! - Member management (add/remove)
//! - Message sending and receiving
//! - Group state queries
//!
//! # Example
//!
//! ```ignore
//! // Create a group
//! let handle = MlsHandle::create_group(
//!     "my-group",
//!     public_key,
//!     identity,
//!     app_secret,
//! )?;
//!
//! // Add a member
//! handle.propose_add(member_pk, member_id)?;
//! let (commit, welcomes) = handle.commit()?;
//!
//! // Send message
//! let envelope = handle.send_message(b"Hello!")?;
//! ```

#![allow(deprecated)]

use super::errors::{MlsError, MlsResult};
use super::group::MlsGroup;
use super::proposals::Proposal;
use super::transport::{MlsEnvelope, MlsTransport};
use super::tree::MlsTree;
use super::types::{GroupId, GroupMetadata, MlsConfig};
use std::sync::{Arc, RwLock};

/// High-level MLS API handle
///
/// Thread-safe wrapper around MlsTransport with:
/// - Internal state synchronization
/// - Ergonomic method names
/// - Batch operations
pub struct MlsHandle {
    /// Transport layer (with RwLock for thread safety)
    transport: Arc<RwLock<MlsTransport>>,
    /// Configuration
    config: MlsConfig,
}

impl MlsHandle {
    /// Create a new group
    pub fn create_group(
        group_name: Option<String>,
        creator_public_key: Vec<u8>,
        creator_identity: Vec<u8>,
        application_secret: Vec<u8>,
        config: MlsConfig,
    ) -> MlsResult<Self> {
        let group_id = GroupId::random();
        
        let mut group = MlsGroup::new(
            group_id.clone(),
            creator_public_key,
            creator_identity,
            application_secret,
            config.clone(),
        )?;

        // Set group name if provided
        if let Some(name) = group_name {
            group.metadata.name = Some(name);
        }

        let transport = MlsTransport::new(group);

        Ok(Self {
            transport: Arc::new(RwLock::new(transport)),
            config,
        })
    }

    /// Join an existing group via Welcome message
    ///
    /// # Arguments
    /// * `welcome` - MlsEnvelope containing the Welcome message
    /// * `member_index` - This member's leaf index
    /// * `member_secret_key` - This member's X25519 secret key for HPKE decryption
    /// * `config` - MLS configuration
    pub fn join_group(
        welcome: &MlsEnvelope,
        member_index: u32,
        member_secret_key: &[u8],
        config: MlsConfig,
    ) -> MlsResult<Self> {
        let transport = MlsTransport::from_welcome(
            welcome,
            member_index,
            member_secret_key,
        )?;

        Ok(Self {
            transport: Arc::new(RwLock::new(transport)),
            config,
        })
    }

    /// Propose adding a new member
    pub fn propose_add(
        &self,
        public_key: Vec<u8>,
        identity: Vec<u8>,
    ) -> MlsResult<MlsEnvelope> {
        let mut transport = self.transport.write()
            .map_err(|e| MlsError::InvalidState(format!("Lock poisoned: {}", e)))?;

        let group = transport.group();
        let proposal = Proposal::new_add(
            group.self_index,
            group.epoch,
            public_key,
            identity,
        );

        transport.send_proposal(proposal)
    }

    /// Propose removing a member
    pub fn propose_remove(&self, removed_index: u32) -> MlsResult<MlsEnvelope> {
        let mut transport = self.transport.write()
            .map_err(|e| MlsError::InvalidState(format!("Lock poisoned: {}", e)))?;

        let group = transport.group();
        let proposal = Proposal::new_remove(
            group.self_index,
            group.epoch,
            removed_index,
        );

        transport.send_proposal(proposal)
    }

    /// Propose updating own key material
    pub fn propose_update(&self, new_public_key: Vec<u8>) -> MlsResult<MlsEnvelope> {
        let mut transport = self.transport.write()
            .map_err(|e| MlsError::InvalidState(format!("Lock poisoned: {}", e)))?;

        let group = transport.group();
        
        // Validate: new key must be different from current key
        if let Some(node) = group.tree.get_node(MlsTree::leaf_to_node_index(group.self_index)) {
            if let Some(ref current_key) = node.public_key {
                if current_key == &new_public_key {
                    return Err(MlsError::InvalidProposal(
                        "Update must use a new public key (cannot reuse current key)".to_string()
                    ));
                }
            }
        }
        
        let proposal = Proposal::new_update(
            group.self_index,
            group.epoch,
            new_public_key,
        );

        transport.send_proposal(proposal)
    }

    /// Receive and process a proposal from another member
    pub fn receive_proposal(&self, envelope: &MlsEnvelope) -> MlsResult<u32> {
        let mut transport = self.transport.write()
            .map_err(|e| MlsError::InvalidState(format!("Lock poisoned: {}", e)))?;

        transport.receive_proposal(envelope)
    }

    /// Commit all pending proposals
    pub fn commit(&self) -> MlsResult<(MlsEnvelope, Vec<MlsEnvelope>)> {
        let mut transport = self.transport.write()
            .map_err(|e| MlsError::InvalidState(format!("Lock poisoned: {}", e)))?;

        transport.send_commit()
    }

    /// Receive and apply a commit from another member
    pub fn receive_commit(&self, envelope: &MlsEnvelope) -> MlsResult<()> {
        let mut transport = self.transport.write()
            .map_err(|e| MlsError::InvalidState(format!("Lock poisoned: {}", e)))?;

        transport.receive_commit(envelope)
    }

    /// Send an application message
    pub fn send_message(&self, plaintext: &[u8]) -> MlsResult<MlsEnvelope> {
        let mut transport = self.transport.write()
            .map_err(|e| MlsError::InvalidState(format!("Lock poisoned: {}", e)))?;

        transport.send_application(plaintext)
    }

    /// Receive and decrypt an application message
    pub fn receive_message(&self, envelope: &MlsEnvelope) -> MlsResult<Vec<u8>> {
        let mut transport = self.transport.write()
            .map_err(|e| MlsError::InvalidState(format!("Lock poisoned: {}", e)))?;

        transport.receive_application(envelope)
    }

    /// Get group ID
    pub fn group_id(&self) -> MlsResult<GroupId> {
        let transport = self.transport.read()
            .map_err(|e| MlsError::InvalidState(format!("Lock poisoned: {}", e)))?;

        Ok(transport.group_id().clone())
    }

    /// Get current epoch
    pub fn epoch(&self) -> MlsResult<u64> {
        let transport = self.transport.read()
            .map_err(|e| MlsError::InvalidState(format!("Lock poisoned: {}", e)))?;

        Ok(transport.epoch())
    }

    /// Get current epoch (alias for epoch())
    pub fn current_epoch(&self) -> MlsResult<u64> {
        self.epoch()
    }

    /// Get member count
    pub fn member_count(&self) -> MlsResult<usize> {
        let transport = self.transport.read()
            .map_err(|e| MlsError::InvalidState(format!("Lock poisoned: {}", e)))?;

        Ok(transport.group().member_count())
    }

    /// Get group metadata
    pub fn metadata(&self) -> MlsResult<GroupMetadata> {
        let transport = self.transport.read()
            .map_err(|e| MlsError::InvalidState(format!("Lock poisoned: {}", e)))?;

        Ok(transport.group().metadata.clone())
    }

    /// Get self (this member's) leaf index
    pub fn self_index(&self) -> MlsResult<u32> {
        let transport = self.transport.read()
            .map_err(|e| MlsError::InvalidState(format!("Lock poisoned: {}", e)))?;

        Ok(transport.group().self_index)
    }

    /// Get configuration
    pub fn config(&self) -> &MlsConfig {
        &self.config
    }

    /// Clone the handle (shares underlying transport)
    pub fn clone_handle(&self) -> Self {
        Self {
            transport: Arc::clone(&self.transport),
            config: self.config.clone(),
        }
    }
}

/// Batch operations for efficiency
impl MlsHandle {
    /// Propose adding multiple members at once
    pub fn propose_add_batch(
        &self,
        members: Vec<(Vec<u8>, Vec<u8>)>, // (public_key, identity) pairs
    ) -> MlsResult<Vec<MlsEnvelope>> {
        let mut envelopes = Vec::new();

        for (public_key, identity) in members {
            let envelope = self.propose_add(public_key, identity)?;
            envelopes.push(envelope);
        }

        Ok(envelopes)
    }

    /// Propose removing multiple members at once
    pub fn propose_remove_batch(
        &self,
        indices: Vec<u32>,
    ) -> MlsResult<Vec<MlsEnvelope>> {
        let mut envelopes = Vec::new();

        for index in indices {
            let envelope = self.propose_remove(index)?;
            envelopes.push(envelope);
        }

        Ok(envelopes)
    }
}

/// Group information queries
impl MlsHandle {
    /// Get all group members
    pub fn members(&self) -> MlsResult<Vec<super::types::MemberInfo>> {
        let transport = self.transport.read()
            .map_err(|e| MlsError::InvalidState(format!("Lock poisoned: {}", e)))?;

        Ok(transport.group().metadata.members.clone())
    }

    /// Check if a specific member is in the group
    pub fn has_member(&self, leaf_index: u32) -> MlsResult<bool> {
        let members = self.members()?;
        Ok(members.iter().any(|m| m.leaf_index == leaf_index))
    }

    /// Get group name
    pub fn group_name(&self) -> MlsResult<Option<String>> {
        let metadata = self.metadata()?;
        Ok(metadata.name)
    }

    /// Set group name (requires commit to take effect)
    pub fn set_group_name(&self, name: String) -> MlsResult<()> {
        let mut transport = self.transport.write()
            .map_err(|e| MlsError::InvalidState(format!("Lock poisoned: {}", e)))?;

        transport.group_mut().metadata.name = Some(name);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core_mls::transport::MlsMessageType;

    fn test_config() -> MlsConfig {
        MlsConfig::default()
    }

    /// Generate a valid 32-byte X25519 secret key for testing
    fn test_secret_key(name: &str) -> Vec<u8> {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(name.as_bytes());
        hasher.finalize().to_vec()
    }

    /// Generate matching public/secret keypair for testing
    fn test_keypair(name: &str) -> (Vec<u8>, Vec<u8>) {
        use x25519_dalek::{PublicKey, StaticSecret};
        let secret = test_secret_key(name);
        let mut sk_bytes = [0u8; 32];
        sk_bytes.copy_from_slice(&secret);
        let static_secret = StaticSecret::from(sk_bytes);
        let public_key = PublicKey::from(&static_secret);
        (public_key.as_bytes().to_vec(), secret)
    }

    #[test]
    fn test_create_group() {
        let (alice_pk, _) = test_keypair("alice");
        let handle = MlsHandle::create_group(
            Some("test-group".to_string()),
            alice_pk,
            b"alice".to_vec(),
            vec![1, 2, 3, 4],
            test_config(),
        )
        .unwrap();

        assert_eq!(handle.epoch().unwrap(), 0);
        assert_eq!(handle.member_count().unwrap(), 1);
        assert_eq!(handle.self_index().unwrap(), 0);
        assert_eq!(handle.group_name().unwrap(), Some("test-group".to_string()));
    }

    #[test]
    fn test_propose_add() {
        let (alice_pk, _) = test_keypair("alice");
        let handle = MlsHandle::create_group(
            None,
            alice_pk,
            b"alice".to_vec(),
            vec![1, 2, 3, 4],
            test_config(),
        )
        .unwrap();

        let (bob_pk, _) = test_keypair("bob");
        let envelope = handle.propose_add(bob_pk, b"bob".to_vec()).unwrap();
        assert_eq!(envelope.message_type, MlsMessageType::Proposal);
    }

    #[test]
    fn test_propose_remove() {
        let (alice_pk, _) = test_keypair("alice");
        let handle = MlsHandle::create_group(
            None,
            alice_pk,
            b"alice".to_vec(),
            vec![1, 2, 3, 4],
            test_config(),
        )
        .unwrap();

        // Add bob first
        let (bob_pk, _) = test_keypair("bob");
        handle.propose_add(bob_pk, b"bob".to_vec()).unwrap();
        handle.commit().unwrap();

        // Remove bob
        let envelope = handle.propose_remove(1).unwrap();
        assert_eq!(envelope.message_type, MlsMessageType::Proposal);
    }

    #[test]
    fn test_propose_update() {
        let handle = MlsHandle::create_group(
            None,
            b"alice_pk".to_vec(),
            b"alice".to_vec(),
            vec![1, 2, 3, 4],
            test_config(),
        )
        .unwrap();

        let envelope = handle.propose_update(b"alice_new_pk".to_vec()).unwrap();
        assert_eq!(envelope.message_type, MlsMessageType::Proposal);
    }

    #[test]
    fn test_commit() {
        let (alice_pk, _) = test_keypair("alice");
        let handle = MlsHandle::create_group(
            None,
            alice_pk,
            b"alice".to_vec(),
            vec![1, 2, 3, 4],
            test_config(),
        )
        .unwrap();

        let (bob_pk, _) = test_keypair("bob");
        handle.propose_add(bob_pk, b"bob".to_vec()).unwrap();
        let (commit_envelope, welcome_envelopes) = handle.commit().unwrap();

        assert_eq!(commit_envelope.message_type, MlsMessageType::Commit);
        assert_eq!(welcome_envelopes.len(), 1);
        assert_eq!(handle.epoch().unwrap(), 1);
    }

    #[test]
    fn test_send_receive_message() {
        let (alice_pk, _) = test_keypair("alice");
        let handle = MlsHandle::create_group(
            None,
            alice_pk,
            b"alice".to_vec(),
            vec![1, 2, 3, 4],
            test_config(),
        )
        .unwrap();

        let plaintext = b"Hello, world!";
        let envelope = handle.send_message(plaintext).unwrap();

        let decrypted = handle.receive_message(&envelope).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_join_group() {
        // Alice creates group
        let (alice_pk, _) = test_keypair("alice");
        let alice = MlsHandle::create_group(
            Some("alice-group".to_string()),
            alice_pk,
            b"alice".to_vec(),
            vec![1, 2, 3, 4],
            test_config(),
        )
        .unwrap();

        // Alice adds bob
        let (bob_pk, bob_sk) = test_keypair("bob");
        alice.propose_add(bob_pk, b"bob".to_vec()).unwrap();
        let (_, welcomes) = alice.commit().unwrap();

        // Bob joins
        let bob = MlsHandle::join_group(&welcomes[0], 1, &bob_sk, test_config()).unwrap();

        assert_eq!(bob.group_id().unwrap(), alice.group_id().unwrap());
        assert_eq!(bob.epoch().unwrap(), alice.epoch().unwrap());
        assert_eq!(bob.self_index().unwrap(), 1);
        assert_eq!(bob.group_name().unwrap(), Some("alice-group".to_string()));
    }

    #[test]
    fn test_full_workflow() {
        // Alice creates group
        let (alice_pk, _) = test_keypair("alice");
        let alice = MlsHandle::create_group(
            Some("team".to_string()),
            alice_pk,
            b"alice".to_vec(),
            vec![1, 2, 3, 4],
            test_config(),
        )
        .unwrap();

        assert_eq!(alice.member_count().unwrap(), 1);

        // Alice proposes adding Bob
        let (bob_pk, bob_sk) = test_keypair("bob");
        let proposal_envelope = alice.propose_add(bob_pk, b"bob".to_vec()).unwrap();
        assert_eq!(proposal_envelope.message_type, MlsMessageType::Proposal);

        // Alice commits
        let (commit_envelope, welcome_envelopes) = alice.commit().unwrap();
        assert_eq!(alice.member_count().unwrap(), 2);
        assert_eq!(alice.epoch().unwrap(), 1);

        // Bob joins
        let bob = MlsHandle::join_group(&welcome_envelopes[0], 1, &bob_sk, test_config()).unwrap();
        assert_eq!(bob.member_count().unwrap(), 2);

        // Alice sends message
        let msg = alice.send_message(b"Hello Bob").unwrap();

        // Bob receives
        let decrypted = bob.receive_message(&msg).unwrap();
        assert_eq!(decrypted, b"Hello Bob");

        // Bob sends back
        let reply = bob.send_message(b"Hi Alice").unwrap();
        let decrypted_reply = alice.receive_message(&reply).unwrap();
        assert_eq!(decrypted_reply, b"Hi Alice");
    }

    #[test]
    fn test_propose_add_batch() {
        let (alice_pk, _) = test_keypair("alice");
        let handle = MlsHandle::create_group(
            None,
            alice_pk,
            b"alice".to_vec(),
            vec![1, 2, 3, 4],
            test_config(),
        )
        .unwrap();

        let (bob_pk, _) = test_keypair("bob");
        let (charlie_pk, _) = test_keypair("charlie");
        let (dave_pk, _) = test_keypair("dave");
        let members = vec![
            (bob_pk, b"bob".to_vec()),
            (charlie_pk, b"charlie".to_vec()),
            (dave_pk, b"dave".to_vec()),
        ];

        let envelopes = handle.propose_add_batch(members).unwrap();
        assert_eq!(envelopes.len(), 3);

        handle.commit().unwrap();
        assert_eq!(handle.member_count().unwrap(), 4);
    }

    #[test]
    fn test_propose_remove_batch() {
        let (alice_pk, _) = test_keypair("alice");
        let handle = MlsHandle::create_group(
            None,
            alice_pk,
            b"alice".to_vec(),
            vec![1, 2, 3, 4],
            test_config(),
        )
        .unwrap();

        // Add members
        let (bob_pk, _) = test_keypair("bob");
        let (charlie_pk, _) = test_keypair("charlie");
        handle.propose_add(bob_pk, b"bob".to_vec()).unwrap();
        handle.propose_add(charlie_pk, b"charlie".to_vec()).unwrap();
        handle.commit().unwrap();
        assert_eq!(handle.member_count().unwrap(), 3);

        // Remove multiple
        let envelopes = handle.propose_remove_batch(vec![1, 2]).unwrap();
        assert_eq!(envelopes.len(), 2);

        handle.commit().unwrap();
        assert_eq!(handle.member_count().unwrap(), 1);
    }

    #[test]
    fn test_has_member() {
        let (alice_pk, _) = test_keypair("alice");
        let handle = MlsHandle::create_group(
            None,
            alice_pk,
            b"alice".to_vec(),
            vec![1, 2, 3, 4],
            test_config(),
        )
        .unwrap();

        assert!(handle.has_member(0).unwrap()); // Alice
        assert!(!handle.has_member(1).unwrap()); // Bob not added yet

        let (bob_pk, _) = test_keypair("bob");
        handle.propose_add(bob_pk, b"bob".to_vec()).unwrap();
        handle.commit().unwrap();

        assert!(handle.has_member(1).unwrap()); // Bob now added
    }

    #[test]
    fn test_set_group_name() {
        let handle = MlsHandle::create_group(
            None,
            b"alice_pk".to_vec(),
            b"alice".to_vec(),
            vec![1, 2, 3, 4],
            test_config(),
        )
        .unwrap();

        assert_eq!(handle.group_name().unwrap(), None);

        handle.set_group_name("My Group".to_string()).unwrap();
        assert_eq!(handle.group_name().unwrap(), Some("My Group".to_string()));
    }

    #[test]
    fn test_clone_handle() {
        let (alice_pk, _) = test_keypair("alice");
        let handle1 = MlsHandle::create_group(
            Some("shared".to_string()),
            alice_pk,
            b"alice".to_vec(),
            vec![1, 2, 3, 4],
            test_config(),
        )
        .unwrap();

        let handle2 = handle1.clone_handle();

        // Both handles see same state
        assert_eq!(handle1.group_id().unwrap(), handle2.group_id().unwrap());
        assert_eq!(handle1.epoch().unwrap(), handle2.epoch().unwrap());

        // Modifications via handle1 visible in handle2
        let (bob_pk, _) = test_keypair("bob");
        handle1.propose_add(bob_pk, b"bob".to_vec()).unwrap();
        handle1.commit().unwrap();

        assert_eq!(handle2.epoch().unwrap(), 1);
        assert_eq!(handle2.member_count().unwrap(), 2);
    }

    #[test]
    fn test_receive_proposal_from_another_member() {
        let (alice_pk, _) = test_keypair("alice");
        let alice = MlsHandle::create_group(
            None,
            alice_pk,
            b"alice".to_vec(),
            vec![1, 2, 3, 4],
            test_config(),
        )
        .unwrap();

        // Simulate bob sending proposal
        let (charlie_pk, _) = test_keypair("charlie");
        let proposal = Proposal::new_add(
            0, // Alice's index
            0, // Epoch 0
            charlie_pk,
            b"charlie".to_vec(),
        );
        let envelope = MlsEnvelope::wrap_proposal(&proposal, alice.group_id().unwrap()).unwrap();

        // Alice receives it
        let idx = alice.receive_proposal(&envelope).unwrap();
        assert_eq!(idx, 0);

        // Alice can commit it
        alice.commit().unwrap();
        assert_eq!(alice.member_count().unwrap(), 2);
    }

    #[test]
    fn test_concurrent_operations() {
        use std::sync::Arc;
        use std::thread;

        let handle = Arc::new(MlsHandle::create_group(
            None,
            b"alice_pk".to_vec(),
            b"alice".to_vec(),
            vec![1, 2, 3, 4],
            test_config(),
        ).unwrap());

        let handle1 = Arc::clone(&handle);
        let handle2 = Arc::clone(&handle);

        // Thread 1: Query operations
        let t1 = thread::spawn(move || {
            for _ in 0..10 {
                let _ = handle1.epoch();
                let _ = handle1.member_count();
                let _ = handle1.group_id();
            }
        });

        // Thread 2: Query operations
        let t2 = thread::spawn(move || {
            for _ in 0..10 {
                let _ = handle2.metadata();
                let _ = handle2.self_index();
                let _ = handle2.members();
            }
        });

        t1.join().unwrap();
        t2.join().unwrap();

        // No panics = success
    }
}
