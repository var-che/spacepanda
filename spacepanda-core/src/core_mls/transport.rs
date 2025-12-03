//! MLS Transport - Wire MLS messages to Router/RPC layer
//!
//! ⚠️ **DEPRECATED**: This module contains the legacy custom MLS transport.
//! Use OpenMLS message handling instead. This module will be removed in v0.3.0.
//!
//! This module provides:
//! - Message envelope format for MLS over Router
//! - Integration with SessionCommand for delivery
//! - Serialization/deserialization of MLS messages
//! - Type-safe message routing
//!
//! # Message Flow
//!
//! 1. MlsGroup generates MLS messages (Welcome, Commit, App)
//! 2. MlsTransport wraps them in envelopes
//! 3. Router delivers to recipients
//! 4. Recipients unwrap and process

#![allow(deprecated)]

use super::commit::Commit;
use super::encryption::EncryptedMessage;
use super::errors::{MlsError, MlsResult};
use super::group::MlsGroup;
use super::proposals::Proposal;
use super::types::GroupId;
use super::welcome::Welcome;
use serde::{Deserialize, Serialize};

/// MLS message envelope for transport
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MlsEnvelope {
    /// Protocol version
    pub version: u8,
    /// Message type
    pub message_type: MlsMessageType,
    /// Group ID
    pub group_id: GroupId,
    /// Sender leaf index (for tracking)
    pub sender: Option<u32>,
    /// Message payload (serialized)
    pub payload: Vec<u8>,
}

/// MLS message types for routing
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum MlsMessageType {
    /// Welcome message for new members
    Welcome,
    /// Proposal for group state change
    Proposal,
    /// Commit applying proposals
    Commit,
    /// Application message (encrypted)
    Application,
}

impl MlsEnvelope {
    /// Current protocol version
    pub const VERSION: u8 = 1;

    /// Create envelope for Welcome message
    pub fn wrap_welcome(welcome: &Welcome) -> MlsResult<Self> {
        let payload = bincode::serialize(welcome)
            .map_err(|e| MlsError::Serialization(e.to_string()))?;

        Ok(Self {
            version: Self::VERSION,
            message_type: MlsMessageType::Welcome,
            group_id: welcome.group_id.clone(),
            sender: None, // Welcome has no single sender
            payload,
        })
    }

    /// Create envelope for Proposal
    pub fn wrap_proposal(proposal: &Proposal, group_id: GroupId) -> MlsResult<Self> {
        let payload = bincode::serialize(proposal)
            .map_err(|e| MlsError::Serialization(e.to_string()))?;

        Ok(Self {
            version: Self::VERSION,
            message_type: MlsMessageType::Proposal,
            group_id,
            sender: Some(proposal.sender),
            payload,
        })
    }

    /// Create envelope for Commit
    pub fn wrap_commit(commit: &Commit) -> MlsResult<Self> {
        let payload = bincode::serialize(commit)
            .map_err(|e| MlsError::Serialization(e.to_string()))?;

        Ok(Self {
            version: Self::VERSION,
            message_type: MlsMessageType::Commit,
            group_id: commit.group_id.clone(),
            sender: Some(commit.sender),
            payload,
        })
    }

    /// Create envelope for Application message
    pub fn wrap_application(msg: &EncryptedMessage, group_id: GroupId) -> MlsResult<Self> {
        let payload = bincode::serialize(msg)
            .map_err(|e| MlsError::Serialization(e.to_string()))?;

        Ok(Self {
            version: Self::VERSION,
            message_type: MlsMessageType::Application,
            group_id,
            sender: Some(msg.sender_leaf),
            payload,
        })
    }

    /// Unwrap Welcome message
    pub fn unwrap_welcome(&self) -> MlsResult<Welcome> {
        if self.message_type != MlsMessageType::Welcome {
            return Err(MlsError::InvalidState(format!(
                "Expected Welcome, got {:?}",
                self.message_type
            )));
        }

        bincode::deserialize(&self.payload)
            .map_err(|e| MlsError::Serialization(e.to_string()))
    }

    /// Unwrap Proposal
    pub fn unwrap_proposal(&self) -> MlsResult<Proposal> {
        if self.message_type != MlsMessageType::Proposal {
            return Err(MlsError::InvalidState(format!(
                "Expected Proposal, got {:?}",
                self.message_type
            )));
        }

        bincode::deserialize(&self.payload)
            .map_err(|e| MlsError::Serialization(e.to_string()))
    }

    /// Unwrap Commit
    pub fn unwrap_commit(&self) -> MlsResult<Commit> {
        if self.message_type != MlsMessageType::Commit {
            return Err(MlsError::InvalidState(format!(
                "Expected Commit, got {:?}",
                self.message_type
            )));
        }

        bincode::deserialize(&self.payload)
            .map_err(|e| MlsError::Serialization(e.to_string()))
    }

    /// Unwrap Application message
    pub fn unwrap_application(&self) -> MlsResult<EncryptedMessage> {
        if self.message_type != MlsMessageType::Application {
            return Err(MlsError::InvalidState(format!(
                "Expected Application, got {:?}",
                self.message_type
            )));
        }

        bincode::deserialize(&self.payload)
            .map_err(|e| MlsError::Serialization(e.to_string()))
    }

    /// Serialize to JSON for Router
    pub fn to_json(&self) -> MlsResult<String> {
        serde_json::to_string(self)
            .map_err(|e| MlsError::Serialization(e.to_string()))
    }

    /// Deserialize from JSON
    pub fn from_json(json: &str) -> MlsResult<Self> {
        serde_json::from_str(json)
            .map_err(|e| MlsError::Serialization(e.to_string()))
    }

    /// Serialize to bytes (more compact)
    pub fn to_bytes(&self) -> MlsResult<Vec<u8>> {
        bincode::serialize(self)
            .map_err(|e| MlsError::Serialization(e.to_string()))
    }

    /// Deserialize from bytes
    pub fn from_bytes(bytes: &[u8]) -> MlsResult<Self> {
        bincode::deserialize(bytes)
            .map_err(|e| MlsError::Serialization(e.to_string()))
    }
}

/// MLS Transport for Router integration
pub struct MlsTransport {
    /// Local group state
    group: MlsGroup,
}

impl MlsTransport {
    /// Create transport wrapper for a group
    pub fn new(group: MlsGroup) -> Self {
        Self { group }
    }

    /// Get group reference
    pub fn group(&self) -> &MlsGroup {
        &self.group
    }

    /// Get mutable group reference
    pub fn group_mut(&mut self) -> &mut MlsGroup {
        &mut self.group
    }

    /// Send a proposal (returns envelope for Router)
    pub fn send_proposal(&mut self, proposal: Proposal) -> MlsResult<MlsEnvelope> {
        // Add to local group
        self.group.add_proposal(proposal.clone())?;

        // Wrap for transport
        MlsEnvelope::wrap_proposal(&proposal, self.group.group_id.clone())
    }

    /// Receive and process a proposal
    pub fn receive_proposal(&mut self, envelope: &MlsEnvelope) -> MlsResult<u32> {
        let proposal = envelope.unwrap_proposal()?;
        self.group.add_proposal(proposal)
    }

    /// Commit pending proposals (returns commit envelope + welcome envelopes)
    pub fn send_commit(&mut self) -> MlsResult<(MlsEnvelope, Vec<MlsEnvelope>)> {
        let (commit, welcomes) = self.group.commit(None)?;

        // Wrap commit
        let commit_envelope = MlsEnvelope::wrap_commit(&commit)?;

        // Wrap welcomes
        let welcome_envelopes: MlsResult<Vec<_>> = welcomes
            .iter()
            .map(MlsEnvelope::wrap_welcome)
            .collect();

        Ok((commit_envelope, welcome_envelopes?))
    }

    /// Receive and apply a commit
    pub fn receive_commit(&mut self, envelope: &MlsEnvelope) -> MlsResult<()> {
        let commit = envelope.unwrap_commit()?;
        self.group.apply_commit(&commit)?;
        Ok(())
    }

    /// Send an application message
    pub fn send_application(&mut self, plaintext: &[u8]) -> MlsResult<MlsEnvelope> {
        let encrypted = self.group.seal_message(plaintext)?;
        MlsEnvelope::wrap_application(&encrypted, self.group.group_id.clone())
    }

    /// Receive and decrypt an application message
    pub fn receive_application(&mut self, envelope: &MlsEnvelope) -> MlsResult<Vec<u8>> {
        let encrypted = envelope.unwrap_application()?;
        self.group.open_message(&encrypted)
    }

    /// Process Welcome message to join a group
    ///
    /// # Arguments
    /// * `envelope` - MlsEnvelope containing the Welcome message
    /// * `member_index` - This member's leaf index in the group
    /// * `member_secret_key` - This member's X25519 secret key for HPKE decryption
    pub fn from_welcome(
        envelope: &MlsEnvelope,
        member_index: u32,
        member_secret_key: &[u8],
    ) -> MlsResult<Self> {
        let welcome = envelope.unwrap_welcome()?;
        let group = MlsGroup::from_welcome(&welcome, member_index, member_secret_key)?;
        Ok(Self { group })
    }

    /// Get group ID
    pub fn group_id(&self) -> &GroupId {
        &self.group.group_id
    }

    /// Get current epoch
    pub fn epoch(&self) -> u64 {
        self.group.current_epoch()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core_mls::proposals::Proposal;
    use crate::core_mls::types::{GroupId, MlsConfig};

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

    fn test_group() -> MlsGroup {
        let (alice_pk, _) = test_keypair("alice");
        MlsGroup::new(
            GroupId::random(),
            alice_pk,
            b"alice".to_vec(),
            vec![1, 2, 3, 4],
            MlsConfig::default(),
        )
        .unwrap()
    }

    #[test]
    fn test_envelope_welcome_roundtrip() {
        let group = test_group();
        let (bob_pk, _bob_sk) = test_keypair("bob");
        let proposal = Proposal::new_add(0, 0, bob_pk, b"bob".to_vec());
        
        let mut transport = MlsTransport::new(group);
        transport.group.add_proposal(proposal).unwrap();
        let (_, welcomes) = transport.group.commit(None).unwrap();

        let envelope = MlsEnvelope::wrap_welcome(&welcomes[0]).unwrap();
        assert_eq!(envelope.message_type, MlsMessageType::Welcome);
        assert_eq!(envelope.version, MlsEnvelope::VERSION);

        let unwrapped = envelope.unwrap_welcome().unwrap();
        assert_eq!(unwrapped.group_id, welcomes[0].group_id);
    }

    #[test]
    fn test_envelope_proposal_roundtrip() {
        let group_id = GroupId::random();
        let (bob_pk, _) = test_keypair("bob");
        let proposal = Proposal::new_add(0, 0, bob_pk, b"bob".to_vec());

        let envelope = MlsEnvelope::wrap_proposal(&proposal, group_id.clone()).unwrap();
        assert_eq!(envelope.message_type, MlsMessageType::Proposal);
        assert_eq!(envelope.sender, Some(0));

        let unwrapped = envelope.unwrap_proposal().unwrap();
        assert_eq!(unwrapped.sender, proposal.sender);
    }

    #[test]
    fn test_envelope_commit_roundtrip() {
        let mut group = test_group();
        let (bob_pk, _) = test_keypair("bob");
        let proposal = Proposal::new_add(0, 0, bob_pk, b"bob".to_vec());
        group.add_proposal(proposal).unwrap();
        let (commit, _) = group.commit(None).unwrap();

        let envelope = MlsEnvelope::wrap_commit(&commit).unwrap();
        assert_eq!(envelope.message_type, MlsMessageType::Commit);
        assert_eq!(envelope.sender, Some(commit.sender));

        let unwrapped = envelope.unwrap_commit().unwrap();
        assert_eq!(unwrapped.epoch, commit.epoch);
    }

    #[test]
    fn test_envelope_application_roundtrip() {
        let mut group = test_group();
        let plaintext = b"Hello, MLS!";
        let encrypted = group.seal_message(plaintext).unwrap();

        let envelope =
            MlsEnvelope::wrap_application(&encrypted, group.group_id.clone()).unwrap();
        assert_eq!(envelope.message_type, MlsMessageType::Application);
        assert_eq!(envelope.sender, Some(0));

        let unwrapped = envelope.unwrap_application().unwrap();
        assert_eq!(unwrapped.sequence, encrypted.sequence);
    }

    #[test]
    fn test_envelope_json_serialization() {
        let group_id = GroupId::random();
        let (bob_pk, _) = test_keypair("bob");
        let proposal = Proposal::new_add(0, 0, bob_pk, b"bob".to_vec());
        let envelope = MlsEnvelope::wrap_proposal(&proposal, group_id).unwrap();

        let json = envelope.to_json().unwrap();
        let deserialized = MlsEnvelope::from_json(&json).unwrap();

        assert_eq!(deserialized, envelope);
    }

    #[test]
    fn test_envelope_bytes_serialization() {
        let group_id = GroupId::random();
        let (bob_pk, _) = test_keypair("bob");
        let proposal = Proposal::new_add(0, 0, bob_pk, b"bob".to_vec());
        let envelope = MlsEnvelope::wrap_proposal(&proposal, group_id).unwrap();

        let bytes = envelope.to_bytes().unwrap();
        let deserialized = MlsEnvelope::from_bytes(&bytes).unwrap();

        assert_eq!(deserialized, envelope);
    }

    #[test]
    fn test_envelope_wrong_type_unwrap() {
        let group_id = GroupId::random();
        let (bob_pk, _) = test_keypair("bob");
        let proposal = Proposal::new_add(0, 0, bob_pk, b"bob".to_vec());
        let envelope = MlsEnvelope::wrap_proposal(&proposal, group_id).unwrap();

        // Try to unwrap as commit
        let result = envelope.unwrap_commit();
        assert!(result.is_err());
    }

    #[test]
    fn test_transport_send_receive_proposal() {
        let group = test_group();
        let mut transport = MlsTransport::new(group);

        let (bob_pk, _) = test_keypair("bob");
        let proposal = Proposal::new_add(0, 0, bob_pk, b"bob".to_vec());
        let envelope = transport.send_proposal(proposal).unwrap();

        // Simulate receiving on another member
        let mut group2 = test_group();
        let mut transport2 = MlsTransport::new(group2);
        let idx = transport2.receive_proposal(&envelope).unwrap();

        assert_eq!(idx, 0);
    }

    #[test]
    fn test_transport_send_receive_commit() {
        let group = test_group();
        let mut transport = MlsTransport::new(group);

        // Add proposal
        let (bob_pk, _) = test_keypair("bob");
        let proposal = Proposal::new_add(0, 0, bob_pk, b"bob".to_vec());
        transport.send_proposal(proposal).unwrap();

        // Commit
        let (commit_envelope, welcome_envelopes) = transport.send_commit().unwrap();

        assert_eq!(commit_envelope.message_type, MlsMessageType::Commit);
        assert_eq!(welcome_envelopes.len(), 1);
        assert_eq!(welcome_envelopes[0].message_type, MlsMessageType::Welcome);
    }

    #[test]
    fn test_transport_send_receive_application() {
        let group = test_group();
        let mut transport = MlsTransport::new(group);

        let plaintext = b"Secret message";
        let envelope = transport.send_application(plaintext).unwrap();

        // Decrypt in same transport (simulates same group)
        let decrypted = transport.receive_application(&envelope).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_transport_from_welcome() {
        // Creator sends welcome
        let mut creator = test_group();
        let (bob_pk, bob_sk) = test_keypair("bob");
        let proposal = Proposal::new_add(0, 0, bob_pk, b"bob".to_vec());
        creator.add_proposal(proposal).unwrap();
        let (_, welcomes) = creator.commit(None).unwrap();
        let welcome_envelope = MlsEnvelope::wrap_welcome(&welcomes[0]).unwrap();

        // Bob joins via welcome
        let bob_transport =
            MlsTransport::from_welcome(&welcome_envelope, 1, &bob_sk).unwrap();

        assert_eq!(bob_transport.group_id(), &creator.group_id);
        assert_eq!(bob_transport.epoch(), creator.current_epoch());
    }

    #[test]
    fn test_transport_full_workflow() {
        // Alice creates group
        let alice_group = test_group();
        let mut alice = MlsTransport::new(alice_group);

        // Alice proposes to add Bob
        let (bob_pk, bob_sk) = test_keypair("bob");
        let proposal = Proposal::new_add(0, 0, bob_pk, b"bob".to_vec());
        let _prop_envelope = alice.send_proposal(proposal).unwrap();

        // Alice commits
        let (_commit_envelope, welcome_envelopes) = alice.send_commit().unwrap();

        // Bob joins
        let mut bob =
            MlsTransport::from_welcome(&welcome_envelopes[0], 1, &bob_sk).unwrap();

        assert_eq!(alice.group_id(), bob.group_id());
        assert_eq!(alice.epoch(), bob.epoch());

        // Alice sends message
        let msg_envelope = alice.send_application(b"Hello Bob").unwrap();

        // Bob receives
        let decrypted = bob.receive_application(&msg_envelope).unwrap();
        assert_eq!(decrypted, b"Hello Bob");
    }
}
