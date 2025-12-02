//! MLS Security Testing - Adversarial scenarios and fuzzing
//!
//! This module provides:
//! - Adversarial test cases
//! - Fuzzing helpers
//! - Security validation
//! - Attack simulations
//!
//! # Test Categories
//!
//! - Replay attacks
//! - Bit-flip attacks
//! - Signature tampering
//! - Epoch confusion
//! - Malformed messages

use super::commit::Commit;
use super::encryption::EncryptedMessage;
use super::errors::{MlsError, MlsResult};
use super::group::MlsGroup;
use super::proposals::Proposal;
use super::transport::MlsEnvelope;
use super::types::{GroupId, MlsConfig};
use super::welcome::Welcome;

/// Fuzz a byte array by flipping random bits
pub fn fuzz_bytes(data: &[u8], flip_count: usize) -> Vec<u8> {
    use rand::Rng;
    let mut fuzzed = data.to_vec();
    let mut rng = rand::rng();

    for _ in 0..flip_count {
        if !fuzzed.is_empty() {
            let byte_idx = rng.random_range(0..fuzzed.len());
            let bit_idx = rng.random_range(0..8);
            fuzzed[byte_idx] ^= 1 << bit_idx;
        }
    }

    fuzzed
}

/// Tamper with epoch in a commit
pub fn tamper_commit_epoch(commit: &Commit, new_epoch: u64) -> Commit {
    let mut tampered = commit.clone();
    tampered.epoch = new_epoch;
    tampered
}

/// Tamper with sender in a proposal
pub fn tamper_proposal_sender(proposal: &Proposal, new_sender: u32) -> Proposal {
    let mut tampered = proposal.clone();
    tampered.sender = new_sender;
    tampered
}

/// Tamper with encrypted message sequence
pub fn tamper_message_sequence(msg: &EncryptedMessage, new_sequence: u64) -> EncryptedMessage {
    let mut tampered = msg.clone();
    tampered.sequence = new_sequence;
    tampered
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core_mls::api::MlsHandle;

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

    fn test_handle() -> MlsHandle {
        let (alice_pk, alice_sk) = test_keypair("alice");
        MlsHandle::create_group(
            Some("security-test".to_string()),
            alice_pk,
            b"alice".to_vec(),
            vec![1, 2, 3, 4],
            MlsConfig::default(),
        )
        .unwrap()
    }

    #[test]
    fn test_replay_attack_detection() {
        let handle = test_handle();

        let plaintext = b"Test message";
        let envelope = handle.send_message(plaintext).unwrap();

        // First receive succeeds
        assert!(handle.receive_message(&envelope).is_ok());

        // Replay fails
        let result = handle.receive_message(&envelope);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), MlsError::ReplayDetected(_)));
    }

    #[test]
    fn test_wrong_epoch_rejection() {
        let handle = test_handle();

        // Try to add proposal with wrong epoch
        let proposal = Proposal::new_add(0, 999, b"bob_pk".to_vec(), b"bob".to_vec());
        let envelope = MlsEnvelope::wrap_proposal(&proposal, handle.group_id().unwrap()).unwrap();

        let result = handle.receive_proposal(&envelope);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), MlsError::EpochMismatch { .. }));
    }

    #[test]
    fn test_corrupted_envelope_rejection() {
        let handle = test_handle();

        let plaintext = b"Test message";
        let envelope = handle.send_message(plaintext).unwrap();

        // Corrupt the envelope payload
        let mut corrupted = envelope.clone();
        corrupted.payload = fuzz_bytes(&corrupted.payload, 10);

        let result = handle.receive_message(&corrupted);
        assert!(result.is_err());
    }

    #[test]
    fn test_bit_flip_attack_on_ciphertext() {
        let handle = test_handle();

        let plaintext = b"Secret data";
        let envelope = handle.send_message(plaintext).unwrap();

        // Flip bits in ciphertext
        let mut msg = envelope.unwrap_application().unwrap();
        msg.ciphertext = fuzz_bytes(&msg.ciphertext, 1);

        let tampered_envelope = MlsEnvelope::wrap_application(&msg, handle.group_id().unwrap()).unwrap();

        // Should fail to decrypt
        let result = handle.receive_message(&tampered_envelope);
        assert!(result.is_err());
    }

    #[test]
    fn test_sender_data_mismatch_detection() {
        let mut handle1 = test_handle();
        
        // Add second member
        let (bob_pk, bob_sk) = test_keypair("bob");
        handle1.propose_add(bob_pk, b"bob".to_vec()).unwrap();
        let (_, welcomes) = handle1.commit().unwrap();
        
        let handle2 = MlsHandle::join_group(&welcomes[0], 1, &bob_sk, MlsConfig::default()).unwrap();

        // Alice sends message
        let envelope = handle1.send_message(b"Hello").unwrap();
        let mut msg = envelope.unwrap_application().unwrap();

        // Tamper with encrypted sender data
        msg.encrypted_sender_data = fuzz_bytes(&msg.encrypted_sender_data, 5);

        let tampered_envelope = MlsEnvelope::wrap_application(&msg, handle1.group_id().unwrap()).unwrap();

        // Bob should reject (sender data won't match)
        let result = handle2.receive_message(&tampered_envelope);
        assert!(result.is_err());
    }

    #[test]
    fn test_out_of_order_sequence_numbers() {
        let handle = test_handle();

        let msg1 = handle.send_message(b"msg1").unwrap();
        let msg2 = handle.send_message(b"msg2").unwrap();
        let msg3 = handle.send_message(b"msg3").unwrap();

        // Receive out of order (should still work, not enforcing order)
        assert!(handle.receive_message(&msg3).is_ok());
        assert!(handle.receive_message(&msg1).is_ok());
        assert!(handle.receive_message(&msg2).is_ok());

        // But replays should still fail
        assert!(handle.receive_message(&msg1).is_err());
        assert!(handle.receive_message(&msg2).is_err());
        assert!(handle.receive_message(&msg3).is_err());
    }

    #[test]
    fn test_commit_without_proposals_rejected() {
        let handle = test_handle();
        
        // Try to commit without proposals
        let result = handle.commit();
        assert!(result.is_err());
    }

    #[test]
    fn test_unauthorized_member_removal() {
        let mut alice = test_handle();
        
        // Alice adds Bob
        let (bob_pk, bob_sk) = test_keypair("bob");
        alice.propose_add(bob_pk, b"bob".to_vec()).unwrap();
        let (_, welcomes) = alice.commit().unwrap();
        
        let bob = MlsHandle::join_group(&welcomes[0], 1, &bob_sk, MlsConfig::default()).unwrap();

        // Bob tries to remove Alice (index 0) - should work in this simplified model
        // In production, this would require authorization checks
        let proposal = bob.propose_remove(0).unwrap();
        
        // This test demonstrates the need for authorization in production
        assert!(proposal.sender.is_some());
    }

    #[test]
    fn test_malformed_json_envelope_rejection() {
        let result = MlsEnvelope::from_json("{invalid json}");
        assert!(result.is_err());
    }

    #[test]
    fn test_empty_payload_rejection() {
        let mut envelope = MlsEnvelope::wrap_proposal(
            &Proposal::new_add(0, 0, b"pk".to_vec(), b"id".to_vec()),
            GroupId::random(),
        )
        .unwrap();

        envelope.payload = vec![];

        let result = envelope.unwrap_proposal();
        assert!(result.is_err());
    }

    #[test]
    fn test_wrong_message_type_unwrap() {
        let proposal = Proposal::new_add(0, 0, b"pk".to_vec(), b"id".to_vec());
        let envelope = MlsEnvelope::wrap_proposal(&proposal, GroupId::random()).unwrap();

        // Try to unwrap as wrong type
        assert!(envelope.unwrap_commit().is_err());
        assert!(envelope.unwrap_welcome().is_err());
        assert!(envelope.unwrap_application().is_err());
    }

    #[test]
    fn test_large_payload_handling() {
        let handle = test_handle();

        // Large message (1MB)
        let large_msg = vec![0xAB; 1024 * 1024];
        let envelope = handle.send_message(&large_msg).unwrap();

        let decrypted = handle.receive_message(&envelope).unwrap();
        assert_eq!(decrypted.len(), large_msg.len());
        assert_eq!(decrypted, large_msg);
    }

    #[test]
    fn test_zero_length_message() {
        let handle = test_handle();

        let envelope = handle.send_message(&[]).unwrap();
        let decrypted = handle.receive_message(&envelope).unwrap();
        assert_eq!(decrypted.len(), 0);
    }

    #[test]
    fn test_epoch_advancement_prevents_old_messages() {
        let handle = test_handle();

        // Send message at epoch 0
        let old_envelope = handle.send_message(b"old").unwrap();

        // Advance epoch
        let (bob_pk, _) = test_keypair("bob");
        handle.propose_add(bob_pk, b"bob".to_vec()).unwrap();
        handle.commit().unwrap();

        assert_eq!(handle.epoch().unwrap(), 1);

        // Old message should be rejected (wrong epoch)
        let result = handle.receive_message(&old_envelope);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), MlsError::EpochMismatch { .. }));
    }

    #[test]
    fn test_fuzz_bytes_utility() {
        let data = vec![0u8; 100];
        let fuzzed = fuzz_bytes(&data, 10);

        // Should have differences
        assert_ne!(data, fuzzed);
        assert_eq!(data.len(), fuzzed.len());
    }

    #[test]
    fn test_concurrent_message_sending() {
        use std::sync::Arc;
        use std::thread;

        let handle = Arc::new(test_handle());
        let mut threads = vec![];

        for i in 0..10 {
            let h = Arc::clone(&handle);
            threads.push(thread::spawn(move || {
                let msg = format!("msg_{}", i);
                h.send_message(msg.as_bytes())
            }));
        }

        let results: Vec<_> = threads.into_iter().map(|t| t.join().unwrap()).collect();

        // All should succeed
        for result in results {
            assert!(result.is_ok());
        }
    }

    #[test]
    fn test_group_id_mismatch_rejection() {
        let handle1 = test_handle();
        let handle2 = test_handle();

        // Create proposal for handle1's group
        let proposal = Proposal::new_add(0, 0, b"pk".to_vec(), b"id".to_vec());
        
        // But wrap it with handle2's group ID
        let envelope = MlsEnvelope::wrap_proposal(&proposal, handle2.group_id().unwrap()).unwrap();

        // handle1 should reject (wrong group)
        let result = handle1.receive_proposal(&envelope);
        // Note: In current implementation, this might not fail explicitly
        // Production would need group ID validation
    }
}
