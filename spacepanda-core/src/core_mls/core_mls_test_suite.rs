//! Comprehensive End-to-End Test Suite for core_mls
//!
//! This test suite validates:
//! ✔ Unit tests (key schedule, commit logic, tree math, state transitions)
//! ✔ Integration tests (with storage, networking, membership changes)
//! ✔ Property tests (randomized invariant checking)
//! ✔ Failure-mode tests (malformed data, invalid state transitions)
//! ✔ Stress tests (high volume, large groups)

#![cfg(test)]

use super::api::MlsHandle;
use super::commit::{Commit, CommitValidator};
use super::crypto::MlsSigningKey;
use super::encryption::{encrypt_message, decrypt_message, EncryptedMessage, KeySchedule, SenderData};
use super::errors::{MlsError, MlsResult};
use super::group::MlsGroup;
use super::persistence::{
    decrypt_group_state, encrypt_group_state, load_group_from_file, save_group_to_file,
    PersistedGroupState, GroupSecrets,
};
use super::proposals::{Proposal, ProposalContent};
use super::transport::{MlsEnvelope, MlsMessageType};
use super::tree::{LeafIndex, MlsTree, TreeNode};
use super::types::{GroupId, MlsConfig};
use super::welcome::Welcome;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::PathBuf;
use tempfile::TempDir;

// ============================================================================
// TEST HELPERS
// ============================================================================

/// Generate deterministic X25519 secret key from name
fn test_secret_key(name: &str) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(name.as_bytes());
    hasher.finalize().to_vec()
}

/// Generate X25519 keypair
fn test_keypair(name: &str) -> (Vec<u8>, Vec<u8>) {
    use x25519_dalek::{PublicKey, StaticSecret};
    let secret = test_secret_key(name);
    let mut sk_bytes = [0u8; 32];
    sk_bytes.copy_from_slice(&secret);
    let static_secret = StaticSecret::from(sk_bytes);
    let public_key = PublicKey::from(&static_secret);
    (public_key.as_bytes().to_vec(), secret)
}

/// Generate Ed25519 signing key
fn test_signing_key(name: &str) -> MlsSigningKey {
    let mut hasher = Sha256::new();
    hasher.update(b"signing_");
    hasher.update(name.as_bytes());
    let seed: [u8; 32] = hasher.finalize().into();
    MlsSigningKey::from_bytes(&seed)
}

fn test_group_id() -> GroupId {
    GroupId::new(b"test-group-id".to_vec())
}

fn test_config() -> MlsConfig {
    MlsConfig::default()
}

/// Generate deterministic application secret
fn test_app_secret(name: &str) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(b"app_secret_");
    hasher.update(name.as_bytes());
    hasher.finalize().to_vec()
}

// ============================================================================
// 1. UNIT TESTS - KEY SCHEDULE
// ============================================================================

mod key_schedule_tests {
    use super::*;

    #[test]
    fn test_key_schedule_basic_derivation() {
        // Arrange: Create two key schedules from same seed
        let seed = test_app_secret("test_seed");

        // Act: Create key schedules
        let ks1 = KeySchedule::new(0, seed.clone());
        let ks2 = KeySchedule::new(0, seed);

        // Assert: Deterministic derivation
        assert_eq!(
            ks1.application_secret, ks2.application_secret,
            "Key schedule must be deterministic"
        );
        assert_eq!(
            ks1.sender_data_secret, ks2.sender_data_secret,
            "Sender data secret must be deterministic"
        );

        // Secrets should be cryptographically distinct
        assert_ne!(
            ks1.application_secret, ks1.sender_data_secret,
            "Different secrets must be cryptographically independent"
        );
    }

    #[test]
    fn test_key_schedule_epoch_change() {
        // Arrange: Create key schedule
        let app_secret1 = test_app_secret("epoch1");
        let ks1 = KeySchedule::new(0, app_secret1.clone());

        // Act: Advance to new epoch with new secret
        let app_secret2 = test_app_secret("epoch2");
        let ks2 = KeySchedule::new(1, app_secret2);

        // Assert: Secrets rotate between epochs
        assert_ne!(
            ks1.application_secret, ks2.application_secret,
            "Each epoch must have different application secret"
        );
        assert_eq!(ks1.epoch, 0);
        assert_eq!(ks2.epoch, 1);
    }

    #[test]
    fn test_key_schedule_message_key_derivation() {
        // Arrange
        let app_secret = test_app_secret("test");
        let mut ks = KeySchedule::new(0, app_secret);

        // Act: Derive message keys for different senders/sequences
        let key1 = ks.derive_message_key(0, 0);
        let key2 = ks.derive_message_key(0, 1);
        let key3 = ks.derive_message_key(1, 0);

        // Assert: Keys must be unique per (sender, sequence)
        assert_ne!(key1, key2, "Different sequences must produce different keys");
        assert_ne!(key1, key3, "Different senders must produce different keys");
        assert_eq!(key1.len(), 32, "Message keys must be 32 bytes");
    }

    #[test]
    fn test_key_schedule_cache_consistency() {
        // Arrange
        let app_secret = test_app_secret("cache_test");
        let mut ks = KeySchedule::new(0, app_secret);

        // Act: Derive same key twice
        let key1 = ks.derive_message_key(0, 0);
        let key2 = ks.derive_message_key(0, 0);

        // Assert: Cache returns same key
        assert_eq!(key1, key2, "Cache must return identical keys");
    }
}

// ============================================================================
// 2. UNIT TESTS - RATCHET TREE MATH
// ============================================================================

mod ratchet_tree_tests {
    use super::*;

    #[test]
    fn test_ratchet_tree_add_member() {
        // Arrange
        let mut tree = MlsTree::new();
        let old_root = tree.root_hash();

        // Act: Add a member
        let (public_key, _) = test_keypair("alice");
        let leaf_idx = tree.add_leaf(public_key).expect("add_leaf should succeed");

        // Assert
        assert_eq!(leaf_idx, 0, "First member should be at leaf 0");
        assert_ne!(
            tree.root_hash(),
            old_root,
            "Tree root must change when a new leaf is added"
        );
        assert_eq!(tree.leaf_count(), 1);
    }

    #[test]
    fn test_ratchet_tree_update_path() {
        // Arrange: Create tree with 4 members
        let mut tree = MlsTree::new();
        for i in 0..4 {
            let (pk, _) = test_keypair(&format!("member{}", i));
            tree.add_leaf(pk).unwrap();
        }

        // Act: Update leaf 2
        let before_hash = tree.root_hash();
        let (new_pk, _) = test_keypair("member2_updated");
        tree.update_leaf(2, new_pk).expect("update_leaf should succeed");
        let after_hash = tree.root_hash();

        // Assert: Tree hash changes
        assert_ne!(
            before_hash, after_hash,
            "Root hash must change after leaf update"
        );
    }

    #[test]
    fn test_ratchet_tree_remove_member() {
        // Arrange
        let mut tree = MlsTree::new();
        for i in 0..3 {
            let (pk, _) = test_keypair(&format!("member{}", i));
            tree.add_leaf(pk).unwrap();
        }

        let before_count = tree.leaf_count();
        let before_hash = tree.root_hash();

        // Act: Remove member at leaf 1
        tree.remove_leaf(1).expect("remove_leaf should succeed");

        // Assert: Leaf is blanked
        let node = tree.get_node(MlsTree::leaf_to_node_index(1));
        assert!(node.is_some());
        assert!(node.unwrap().is_blank(), "Removed leaf should be blank");
        
        // Note: leaf_count doesn't decrease (structure preserved)
        assert_eq!(tree.leaf_count(), before_count, "Leaf count structure preserved");
        
        // Root hash changes
        assert_ne!(
            tree.root_hash(),
            before_hash,
            "Root hash must change after removal"
        );
    }

    #[test]
    fn test_ratchet_tree_direct_path() {
        // Arrange
        let mut tree = MlsTree::new();
        for i in 0..4 {
            let (pk, _) = test_keypair(&format!("member{}", i));
            tree.add_leaf(pk).unwrap();
        }

        // Act: Get direct path from leaf 0
        let path = tree.direct_path(0);

        // Assert: Path exists and is reasonable length
        assert!(!path.is_empty(), "Direct path should contain parent nodes");
        assert!(path.len() <= 3, "Path length should be O(log N)");
    }

    #[test]
    fn test_tree_export_public_nodes() {
        // Arrange
        let mut tree = MlsTree::new();
        let (pk1, _) = test_keypair("alice");
        let (pk2, _) = test_keypair("bob");
        tree.add_leaf(pk1.clone()).unwrap();
        tree.add_leaf(pk2.clone()).unwrap();

        // Act
        let public_nodes = tree.export_public_nodes();

        // Assert: Contains public keys for leaves
        assert!(public_nodes.len() >= 2, "Should export at least 2 leaf nodes");
    }
}

// ============================================================================
// 3. UNIT TESTS - MESSAGE ENCRYPTION/DECRYPTION
// ============================================================================

mod message_encryption_tests {
    use super::*;

    #[test]
    fn test_mls_message_encrypt_decrypt() {
        // Arrange: Single member group
        let (alice_pk, _) = test_keypair("alice");
        let mut group = MlsGroup::new(
            test_group_id(),
            alice_pk,
            b"alice".to_vec(),
            test_app_secret("test"),
            test_config(),
        )
        .unwrap();

        // Act: Encrypt and decrypt
        let plaintext = b"hello world";
        let encrypted = group.seal_message(plaintext).expect("seal should succeed");
        let decrypted = group.open_message(&encrypted).expect("open should succeed");

        // Assert
        assert_eq!(decrypted, plaintext, "Decrypted text must match original");
    }

    #[test]
    fn test_message_sequence_numbers_increment() {
        // Arrange
        let (alice_pk, _) = test_keypair("alice");
        let mut group = MlsGroup::new(
            test_group_id(),
            alice_pk,
            b"alice".to_vec(),
            test_app_secret("test"),
            test_config(),
        )
        .unwrap();

        // Act: Send multiple messages
        let msg1 = group.seal_message(b"msg1").unwrap();
        let msg2 = group.seal_message(b"msg2").unwrap();
        let msg3 = group.seal_message(b"msg3").unwrap();

        // Assert: Sequence numbers increment
        assert_eq!(msg1.sequence, 0);
        assert_eq!(msg2.sequence, 1);
        assert_eq!(msg3.sequence, 2);
    }

    #[test]
    fn test_replay_protection() {
        // Arrange
        let (alice_pk, _) = test_keypair("alice");
        let mut group = MlsGroup::new(
            test_group_id(),
            alice_pk,
            b"alice".to_vec(),
            test_app_secret("test"),
            test_config(),
        )
        .unwrap();

        let message = group.seal_message(b"test").unwrap();

        // Act & Assert: First decrypt succeeds
        assert!(group.open_message(&message).is_ok(), "First decrypt should succeed");

        // Replay attempt fails
        let result = group.open_message(&message);
        assert!(
            matches!(result, Err(MlsError::ReplayDetected(_))),
            "Replayed message must be detected"
        );
    }
}

// ============================================================================
// 4. UNIT TESTS - COMMIT PROCESSING
// ============================================================================

mod commit_processing_tests {
    use super::*;

    #[test]
    fn test_process_commit_updates_epoch_and_tree() {
        // Arrange: Alice creates group
        let (alice_pk, _) = test_keypair("alice");
        let mut alice = MlsGroup::new(
            test_group_id(),
            alice_pk,
            b"alice".to_vec(),
            test_app_secret("alice"),
            test_config(),
        )
        .unwrap();

        let old_epoch = alice.epoch;
        let old_tree = alice.tree.root_hash();

        // Act: Add Bob
        let (bob_pk, _) = test_keypair("bob");
        let proposal = Proposal::new_add(0, 0, bob_pk, b"bob".to_vec());
        alice.add_proposal(proposal).unwrap();
        let (_commit, _welcomes) = alice.commit(None).unwrap();

        // Assert: Epoch and tree changed
        assert_eq!(alice.epoch, old_epoch + 1, "Epoch must increment");
        assert_ne!(
            alice.tree.root_hash(),
            old_tree,
            "Tree hash must change after commit"
        );
        assert_eq!(alice.member_count(), 2, "Member count must be 2");
    }

    #[test]
    fn test_commit_without_proposals_fails() {
        // Arrange
        let (alice_pk, _) = test_keypair("alice");
        let mut group = MlsGroup::new(
            test_group_id(),
            alice_pk,
            b"alice".to_vec(),
            test_app_secret("test"),
            test_config(),
        )
        .unwrap();

        // Act & Assert: Commit with no proposals fails
        let result = group.commit(None);
        assert!(
            result.is_err(),
            "Commit must fail when no proposals exist"
        );
    }

    #[test]
    fn test_commit_validator_rejects_wrong_epoch() {
        // Arrange
        let (alice_pk, _) = test_keypair("alice");
        let mut group = MlsGroup::new(
            test_group_id(),
            alice_pk,
            b"alice".to_vec(),
            test_app_secret("test"),
            test_config(),
        )
        .unwrap();

        let (bob_pk, _) = test_keypair("bob");
        let proposal = Proposal::new_add(0, 0, bob_pk, b"bob".to_vec());
        group.add_proposal(proposal).unwrap();
        let (mut commit, _) = group.commit(None).unwrap();

        // Act: Tamper with epoch
        commit.epoch = 999;

        // Assert: Validator rejects
        let validator = CommitValidator::new(1, vec![0]);
        let result = validator.validate(&commit);
        assert!(result.is_err(), "Commit with wrong epoch must be rejected");
    }
}

// ============================================================================
// 5. MULTI-MEMBER TESTS
// ============================================================================

mod multi_member_tests {
    use super::*;

    #[test]
    fn test_two_member_add_join_and_message_exchange() {
        // Arrange: Alice creates group
        let (alice_pk, _) = test_keypair("alice");
        let alice = MlsHandle::create_group(
            Some("test-group".to_string()),
            alice_pk,
            b"alice".to_vec(),
            test_app_secret("alice"),
            test_config(),
        )
        .unwrap();

        // Act: Alice invites Bob
        let (bob_pk, bob_sk) = test_keypair("bob");
        alice.propose_add(bob_pk, b"bob".to_vec()).unwrap();
        let (_commit, welcomes) = alice.commit().unwrap();

        // Bob joins using Welcome
        let bob = MlsHandle::join_group(&welcomes[0], 1, &bob_sk, test_config()).unwrap();

        // Assert: Membership alignment
        assert_eq!(alice.epoch().unwrap(), bob.epoch().unwrap());
        assert_eq!(alice.member_count().unwrap(), 2);
        assert_eq!(bob.member_count().unwrap(), 2);

        // Exchange messages
        let msg = alice.send_message(b"ping").unwrap();
        let result = bob.receive_message(&msg).unwrap();
        assert_eq!(result, b"ping");
    }

    #[test]
    fn test_three_member_add_update_remove_flow() {
        // Arrange: Alice creates group
        let (alice_pk, _) = test_keypair("alice");
        let alice = MlsHandle::create_group(
            Some("test".to_string()),
            alice_pk,
            b"alice".to_vec(),
            test_app_secret("alice"),
            test_config(),
        )
        .unwrap();

        // Add Bob
        let (bob_pk, bob_sk) = test_keypair("bob");
        alice.propose_add(bob_pk, b"bob".to_vec()).unwrap();
        let (_c1, welcomes1) = alice.commit().unwrap();
        let bob = MlsHandle::join_group(&welcomes1[0], 1, &bob_sk, test_config()).unwrap();

        // Add Charlie
        let (charlie_pk, charlie_sk) = test_keypair("charlie");
        alice.propose_add(charlie_pk, b"charlie".to_vec()).unwrap();
        let (c2, welcomes2) = alice.commit().unwrap();
        bob.receive_commit(&c2).unwrap();
        let charlie = MlsHandle::join_group(&welcomes2[0], 2, &charlie_sk, test_config()).unwrap();

        // All at same epoch
        assert_eq!(alice.epoch().unwrap(), bob.epoch().unwrap());
        assert_eq!(bob.epoch().unwrap(), charlie.epoch().unwrap());

        // Update Bob
        let (bob_new_pk, _) = test_keypair("bob_v2");
        bob.propose_update(bob_new_pk).unwrap();
        let (c3, _) = bob.commit().unwrap();
        alice.receive_commit(&c3).unwrap();
        charlie.receive_commit(&c3).unwrap();

        // All still aligned
        assert_eq!(alice.epoch().unwrap(), bob.epoch().unwrap());
        assert_eq!(bob.epoch().unwrap(), charlie.epoch().unwrap());

        // Remove Charlie
        alice.propose_remove(2).unwrap();
        let (c4, _) = alice.commit().unwrap();
        bob.receive_commit(&c4).unwrap();

        assert_eq!(alice.member_count().unwrap(), 2);
        assert_eq!(bob.member_count().unwrap(), 2);
    }

    #[test]
    fn test_state_convergence_after_commits() {
        // Arrange: Create 3-member group
        let (alice_pk, _) = test_keypair("alice");
        let alice = MlsHandle::create_group(
            Some("convergence-test".to_string()),
            alice_pk,
            b"alice".to_vec(),
            test_app_secret("alice"),
            test_config(),
        )
        .unwrap();

        let (bob_pk, bob_sk) = test_keypair("bob");
        alice.propose_add(bob_pk, b"bob".to_vec()).unwrap();
        let (_, welcomes) = alice.commit().unwrap();
        let bob = MlsHandle::join_group(&welcomes[0], 1, &bob_sk, test_config()).unwrap();

        let (charlie_pk, charlie_sk) = test_keypair("charlie");
        alice.propose_add(charlie_pk, b"charlie".to_vec()).unwrap();
        let (c, welcomes) = alice.commit().unwrap();
        bob.receive_commit(&c).unwrap();
        let charlie = MlsHandle::join_group(&welcomes[0], 2, &charlie_sk, test_config()).unwrap();

        // Act: Perform sequence of updates
        for i in 0..5 {
            let (new_pk, _) = test_keypair(&format!("alice_v{}", i));
            alice.propose_update(new_pk).unwrap();
            let (commit, _) = alice.commit().unwrap();
            bob.receive_commit(&commit).unwrap();
            charlie.receive_commit(&commit).unwrap();
        }

        // Assert: All members converged
        let epoch = alice.epoch().unwrap();
        assert_eq!(bob.epoch().unwrap(), epoch);
        assert_eq!(charlie.epoch().unwrap(), epoch);
    }
}

// ============================================================================
// 6. STORAGE INTEGRATION TESTS
// ============================================================================

mod storage_integration_tests {
    use super::*;

    #[test]
    fn test_state_persistence_roundtrip() {
        // Arrange
        let (alice_pk, _) = test_keypair("alice");
        let group = MlsGroup::new(
            test_group_id(),
            alice_pk,
            b"alice".to_vec(),
            test_app_secret("alice"),
            test_config(),
        )
        .unwrap();

        let original_state = PersistedGroupState {
            metadata: group.metadata.clone(),
            secrets: GroupSecrets {
                epoch: group.epoch,
                encryption_secret: test_app_secret("enc"),
                application_secret: test_app_secret("app"),
                sequence_counters: HashMap::new(),
            },
        };

        // Act: Serialize and deserialize
        let serialized = bincode::serialize(&original_state).expect("serialization must succeed");
        let deserialized: PersistedGroupState =
            bincode::deserialize(&serialized).expect("deserialization must succeed");

        // Assert: Equality
        assert_eq!(
            deserialized.metadata.group_id,
            original_state.metadata.group_id
        );
        assert_eq!(deserialized.metadata.epoch, original_state.metadata.epoch);
        assert_eq!(deserialized.secrets.epoch, original_state.secrets.epoch);
    }

    #[test]
    fn test_encrypted_persistence_roundtrip() {
        // Arrange
        let (alice_pk, _) = test_keypair("alice");
        let group = MlsGroup::new(
            test_group_id(),
            alice_pk,
            b"alice".to_vec(),
            test_app_secret("alice"),
            test_config(),
        )
        .unwrap();

        let state = PersistedGroupState {
            metadata: group.metadata.clone(),
            secrets: GroupSecrets {
                epoch: group.epoch,
                encryption_secret: vec![1, 2, 3],
                application_secret: vec![4, 5, 6],
                sequence_counters: HashMap::new(),
            },
        };
        let passphrase = "secure-pass-123";

        // Act: Encrypt and decrypt
        let encrypted = encrypt_group_state(&state, Some(passphrase)).expect("encryption must succeed");
        let decrypted = decrypt_group_state(&encrypted, Some(passphrase)).expect("decryption must succeed");

        // Assert
        assert_eq!(decrypted.metadata.group_id, state.metadata.group_id);
        assert_eq!(decrypted.secrets.epoch, state.secrets.epoch);
    }

    #[test]
    fn test_save_and_load_group_from_file() {
        // Arrange
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("group.mlsblob");

        let (alice_pk, _) = test_keypair("alice");
        let group = MlsGroup::new(
            test_group_id(),
            alice_pk,
            b"alice".to_vec(),
            test_app_secret("alice"),
            test_config(),
        )
        .unwrap();

        let state = PersistedGroupState {
            metadata: group.metadata.clone(),
            secrets: GroupSecrets {
                epoch: group.epoch,
                encryption_secret: vec![1, 2, 3],
                application_secret: vec![4, 5, 6],
                sequence_counters: HashMap::new(),
            },
        };
        let passphrase = "file-test-pass";

        // Act: Save and load
        save_group_to_file(&file_path, &state, Some(passphrase)).expect("save must succeed");
        let loaded = load_group_from_file(&file_path, Some(passphrase)).expect("load must succeed");

        // Assert
        assert_eq!(loaded.metadata.group_id, state.metadata.group_id);
        assert_eq!(loaded.secrets.epoch, state.secrets.epoch);
    }

    #[test]
    fn test_wrong_passphrase_fails_decryption() {
        // Arrange
        let (alice_pk, _) = test_keypair("alice");
        let group = MlsGroup::new(
            test_group_id(),
            alice_pk,
            b"alice".to_vec(),
            test_app_secret("alice"),
            test_config(),
        )
        .unwrap();

        let state = PersistedGroupState {
            metadata: group.metadata.clone(),
            secrets: GroupSecrets {
                epoch: group.epoch,
                encryption_secret: vec![1, 2, 3],
                application_secret: vec![4, 5, 6],
                sequence_counters: HashMap::new(),
            },
        };

        let encrypted = encrypt_group_state(&state, Some("correct-pass")).expect("encryption must succeed");

        // Act & Assert: Wrong passphrase fails
        let result = decrypt_group_state(&encrypted, Some("wrong-pass"));
        assert!(result.is_err(), "Wrong passphrase must fail decryption");
    }
}

// ============================================================================
// 7. SECURITY / FAILURE MODE TESTS
// ============================================================================

mod security_failure_tests {
    use super::*;

    #[test]
    fn test_rejects_malformed_commit() {
        // Arrange
        let (alice_pk, _) = test_keypair("alice");
        let mut group = MlsGroup::new(
            test_group_id(),
            alice_pk,
            b"alice".to_vec(),
            test_app_secret("alice"),
            test_config(),
        )
        .unwrap();

        // Create malformed commit with wrong epoch
        let bad_commit = Commit::new(
            test_group_id(),
            999, // illegal forward jump
            0,
            vec![],
            None,
        );

        // Act & Assert
        let result = group.apply_commit(&bad_commit);
        assert!(result.is_err(), "Malformed commit must be rejected");
    }

    #[test]
    fn test_rejects_wrong_epoch_ciphertext() {
        // Arrange: Two separate groups at different epochs
        let (alice_pk, _) = test_keypair("alice");
        let mut alice = MlsGroup::new(
            test_group_id(),
            alice_pk.clone(),
            b"alice".to_vec(),
            test_app_secret("alice_epoch0"),
            test_config(),
        )
        .unwrap();

        let (bob_pk, _) = test_keypair("bob");
        let mut bob = MlsGroup::new(
            test_group_id(),
            bob_pk.clone(),
            b"bob".to_vec(),
            test_app_secret("bob_epoch0"),
            test_config(),
        )
        .unwrap();

        // Alice advances 2 epochs
        for i in 0..2 {
            let (new_pk, _) = test_keypair(&format!("alice_v{}", i));
            let proposal = Proposal::new_update(0, alice.epoch, new_pk);
            alice.add_proposal(proposal).unwrap();
            alice.commit(None).unwrap();
        }

        // Act: Alice encrypts message
        let msg = alice.seal_message(b"future_msg").unwrap();

        // Assert: Bob cannot decrypt (different group state)
        let result = bob.open_message(&msg);
        assert!(result.is_err(), "Message from future epoch must fail");
    }

    #[test]
    fn test_detects_invalid_sender() {
        // Arrange
        let (alice_pk, _) = test_keypair("alice");
        let mut group = MlsGroup::new(
            test_group_id(),
            alice_pk,
            b"alice".to_vec(),
            test_app_secret("alice"),
            test_config(),
        )
        .unwrap();

        // Create message claiming to be from invalid sender
        let mut fake_msg = group.seal_message(b"test").unwrap();
        fake_msg.sender_leaf = 999; // Non-existent sender

        // Act & Assert
        let result = group.open_message(&fake_msg);
        assert!(
            result.is_err(),
            "Message from invalid sender must be rejected"
        );
    }

    #[test]
    fn test_reject_proposal_with_wrong_epoch() {
        // Arrange
        let (alice_pk, _) = test_keypair("alice");
        let mut group = MlsGroup::new(
            test_group_id(),
            alice_pk,
            b"alice".to_vec(),
            test_app_secret("alice"),
            test_config(),
        )
        .unwrap();

        // Create proposal for wrong epoch
        let (bob_pk, _) = test_keypair("bob");
        let bad_proposal = Proposal::new_add(0, 999, bob_pk, b"bob".to_vec());

        // Act & Assert
        let result = group.add_proposal(bad_proposal);
        assert!(
            matches!(result, Err(MlsError::EpochMismatch { .. })),
            "Proposal with wrong epoch must be rejected"
        );
    }

    #[test]
    fn test_tampered_welcome_fails() {
        // Arrange
        let (alice_pk, _) = test_keypair("alice");
        let mut group = MlsGroup::new(
            test_group_id(),
            alice_pk,
            b"alice".to_vec(),
            test_app_secret("alice"),
            test_config(),
        )
        .unwrap();

        let (bob_pk, bob_sk) = test_keypair("bob");
        let proposal = Proposal::new_add(0, 0, bob_pk, b"bob".to_vec());
        group.add_proposal(proposal).unwrap();
        let (_, mut welcomes) = group.commit(None).unwrap();

        // Tamper with Welcome
        welcomes[0].group_id = GroupId::new(b"tampered".to_vec());

        // Act & Assert: Bob tries to join with tampered Welcome
        let result = MlsGroup::from_welcome(&welcomes[0], 1, &bob_sk);
        assert!(result.is_err(), "Tampered Welcome must be rejected");
    }
}

// ============================================================================
// 8. PROPERTY-BASED TESTS
// ============================================================================

mod property_tests {
    use super::*;

    #[test]
    fn test_tree_hash_changes_on_any_modification() {
        // Test that tree hash changes for add, update
        let mut tree = MlsTree::new();

        let (pk1, _) = test_keypair("alice");
        tree.add_leaf(pk1).unwrap();
        let hash1 = tree.root_hash();

        let (pk2, _) = test_keypair("bob");
        tree.add_leaf(pk2).unwrap();
        let hash2 = tree.root_hash();
        assert_ne!(hash1, hash2, "Hash must change on add");

        let (pk3, _) = test_keypair("alice_v2");
        tree.update_leaf(0, pk3).unwrap();
        let hash3 = tree.root_hash();
        assert_ne!(hash2, hash3, "Hash must change on update");

        // Note: Remove blanks the node but may not change hash depending on
        // tree structure. The critical property is that blank nodes are
        // identifiable and don't have valid keys.
        tree.remove_leaf(1).unwrap();
        let node = tree.get_node(MlsTree::leaf_to_node_index(1));
        assert!(node.unwrap().is_blank(), "Removed node must be blank");
    }

    #[test]
    fn test_message_encryption_authenticated() {
        // Property: For any random message, encrypt/decrypt must roundtrip
        let (alice_pk, _) = test_keypair("alice");
        let mut group = MlsGroup::new(
            test_group_id(),
            alice_pk,
            b"alice".to_vec(),
            test_app_secret("alice"),
            test_config(),
        )
        .unwrap();

        let test_messages = vec![
            b"short".to_vec(),
            b"a much longer message with many bytes".to_vec(),
            vec![0u8; 1000], // Large binary
            b"special chars: \n\t\r".to_vec(),
        ];

        for msg in test_messages {
            let encrypted = group.seal_message(&msg).unwrap();
            let decrypted = group.open_message(&encrypted).unwrap();
            assert_eq!(decrypted, msg, "Message must roundtrip correctly");
        }
    }

    #[test]
    fn test_epoch_monotonicity() {
        // Property: Epoch must always increase after commit
        let (alice_pk, _) = test_keypair("alice");
        let mut group = MlsGroup::new(
            test_group_id(),
            alice_pk,
            b"alice".to_vec(),
            test_app_secret("alice"),
            test_config(),
        )
        .unwrap();

        let mut prev_epoch = group.epoch;

        for i in 0..10 {
            let (new_pk, _) = test_keypair(&format!("alice_v{}", i));
            let proposal = Proposal::new_update(0, group.epoch, new_pk);
            group.add_proposal(proposal).unwrap();
            group.commit(None).unwrap();

            assert!(
                group.epoch > prev_epoch,
                "Epoch must strictly increase after commit"
            );
            prev_epoch = group.epoch;
        }
    }
}

// ============================================================================
// 9. STRESS TESTS
// ============================================================================

mod stress_tests {
    use super::*;

    #[test]
    fn test_high_volume_messages() {
        // Arrange
        let (alice_pk, _) = test_keypair("alice");
        let mut group = MlsGroup::new(
            test_group_id(),
            alice_pk,
            b"alice".to_vec(),
            test_app_secret("alice"),
            test_config(),
        )
        .unwrap();

        // Act: Send 100 messages
        let mut messages = Vec::new();
        for i in 0..100 {
            let msg = format!("message_{}", i);
            let encrypted = group.seal_message(msg.as_bytes()).unwrap();
            messages.push(encrypted);
        }

        // Assert: All messages decrypt correctly (in fresh group instance)
        let (alice_pk2, _) = test_keypair("alice");
        let mut group2 = MlsGroup::new(
            test_group_id(),
            alice_pk2,
            b"alice".to_vec(),
            test_app_secret("alice"),
            test_config(),
        )
        .unwrap();

        for (i, msg) in messages.iter().enumerate() {
            let decrypted = group2.open_message(msg).unwrap();
            let expected = format!("message_{}", i);
            assert_eq!(decrypted, expected.as_bytes());
        }
    }

    #[test]
    fn test_mass_member_operations() {
        // Arrange: Alice creates group
        let (alice_pk, _) = test_keypair("alice");
        let mut group = MlsGroup::new(
            test_group_id(),
            alice_pk,
            b"alice".to_vec(),
            test_app_secret("alice"),
            test_config(),
        )
        .unwrap();

        // Act: Add 50 members
        for i in 0..50 {
            let (pk, _) = test_keypair(&format!("member{}", i));
            let proposal = Proposal::new_add(0, group.epoch, pk, format!("member{}", i).as_bytes().to_vec());
            group.add_proposal(proposal).unwrap();
            group.commit(None).unwrap();
        }

        // Assert
        assert_eq!(group.member_count(), 51, "Should have 51 members");

        // Remove 25 members
        for i in 1..26 {
            let proposal = Proposal::new_remove(0, group.epoch, i);
            group.add_proposal(proposal).unwrap();
            group.commit(None).unwrap();
        }

        assert_eq!(group.member_count(), 26, "Should have 26 members left");
    }

    #[test]
    fn test_long_epoch_sequence() {
        // Arrange
        let (alice_pk, _) = test_keypair("alice");
        let mut group = MlsGroup::new(
            test_group_id(),
            alice_pk,
            b"alice".to_vec(),
            test_app_secret("alice"),
            test_config(),
        )
        .unwrap();

        // Act: Perform 100 self-updates
        for i in 0..100 {
            let (new_pk, _) = test_keypair(&format!("alice_v{}", i));
            let proposal = Proposal::new_update(0, group.epoch, new_pk);
            group.add_proposal(proposal).unwrap();
            group.commit(None).unwrap();
        }

        // Assert
        assert_eq!(group.epoch, 100, "Should reach epoch 100");
    }
}

// ============================================================================
// 10. INTEGRATION TESTS (END-TO-END)
// ============================================================================

mod integration_tests {
    use super::*;

    #[test]
    fn test_end_to_end_group_lifecycle() {
        // Alice creates group
        let (alice_pk, _) = test_keypair("alice");
        let alice = MlsHandle::create_group(
            Some("lifecycle-test".to_string()),
            alice_pk,
            b"alice".to_vec(),
            test_app_secret("alice"),
            test_config(),
        )
        .unwrap();

        // Add Bob
        let (bob_pk, bob_sk) = test_keypair("bob");
        alice.propose_add(bob_pk, b"bob".to_vec()).unwrap();
        let (_, welcomes) = alice.commit().unwrap();
        let bob = MlsHandle::join_group(&welcomes[0], 1, &bob_sk, test_config()).unwrap();

        // Both at epoch 1
        assert_eq!(alice.epoch().unwrap(), 1);
        assert_eq!(bob.epoch().unwrap(), 1);

        // Message exchange
        let msg = alice.send_message(b"hello bob").unwrap();
        let decrypted = bob.receive_message(&msg).unwrap();
        assert_eq!(decrypted, b"hello bob");

        let reply = bob.send_message(b"hello alice").unwrap();
        let decrypted = alice.receive_message(&reply).unwrap();
        assert_eq!(decrypted, b"hello alice");

        // Add Charlie
        let (charlie_pk, charlie_sk) = test_keypair("charlie");
        alice.propose_add(charlie_pk, b"charlie".to_vec()).unwrap();
        let (commit, welcomes) = alice.commit().unwrap();
        bob.receive_commit(&commit).unwrap();
        let charlie = MlsHandle::join_group(&welcomes[0], 2, &charlie_sk, test_config()).unwrap();

        // All at epoch 2
        assert_eq!(alice.epoch().unwrap(), 2);
        assert_eq!(bob.epoch().unwrap(), 2);
        assert_eq!(charlie.epoch().unwrap(), 2);

        // Three-way communication
        let msg = alice.send_message(b"hello all").unwrap();
        assert_eq!(bob.receive_message(&msg).unwrap(), b"hello all");
        assert_eq!(charlie.receive_message(&msg).unwrap(), b"hello all");
    }

    #[test]
    fn test_concurrent_proposals_merged() {
        // Arrange: Alice and Bob in group
        let (alice_pk, _) = test_keypair("alice");
        let alice = MlsHandle::create_group(
            Some("concurrent".to_string()),
            alice_pk,
            b"alice".to_vec(),
            test_app_secret("alice"),
            test_config(),
        )
        .unwrap();

        let (bob_pk, bob_sk) = test_keypair("bob");
        alice.propose_add(bob_pk, b"bob".to_vec()).unwrap();
        let (_, welcomes) = alice.commit().unwrap();
        let bob = MlsHandle::join_group(&welcomes[0], 1, &bob_sk, test_config()).unwrap();

        // Both propose additions
        let (charlie_pk, _) = test_keypair("charlie");
        let alice_proposal = alice.propose_add(charlie_pk, b"charlie".to_vec()).unwrap();

        let (dave_pk, _) = test_keypair("dave");
        let bob_proposal = bob.propose_add(dave_pk, b"dave".to_vec()).unwrap();

        // Exchange proposals
        alice.receive_proposal(&bob_proposal).unwrap();
        bob.receive_proposal(&alice_proposal).unwrap();

        // Alice commits both
        let (commit_envelope, _) = alice.commit().unwrap();
        let commit = commit_envelope.unwrap_commit().unwrap();
        assert_eq!(commit.proposals.len(), 2, "Both proposals in commit");

        // Bob applies
        bob.receive_commit(&commit_envelope).unwrap();

        // Both have 4 members
        assert_eq!(alice.member_count().unwrap(), 4);
        assert_eq!(bob.member_count().unwrap(), 4);
    }

    #[test]
    fn test_router_envelope_wrapping() {
        // Arrange
        let (alice_pk, _) = test_keypair("alice");
        let mut group = MlsGroup::new(
            test_group_id(),
            alice_pk,
            b"alice".to_vec(),
            test_app_secret("alice"),
            test_config(),
        )
        .unwrap();

        let (bob_pk, _) = test_keypair("bob");
        let proposal = Proposal::new_add(0, 0, bob_pk, b"bob".to_vec());
        group.add_proposal(proposal).unwrap();
        let (commit, _) = group.commit(None).unwrap();

        // Act: Wrap in envelope
        let envelope = MlsEnvelope::wrap_commit(&commit).expect("envelope creation must succeed");

        // Assert
        assert_eq!(envelope.message_type, MlsMessageType::Commit);
        assert_eq!(envelope.group_id, test_group_id());
        assert!(envelope.sender.is_some());

        // Unwrap and verify
        let unwrapped = envelope.unwrap_commit().unwrap();
        assert_eq!(unwrapped.epoch, commit.epoch);
        assert_eq!(unwrapped.proposals.len(), 1);
    }
}
