//! MLS Integration Tests - End-to-end scenarios
//!
//! This module provides comprehensive integration tests covering:
//! - Multi-device scenarios
//! - Full member lifecycle
//! - Network simulation
//! - Error recovery
//! - Performance validation

use super::api::MlsHandle;
use super::discovery::{DiscoveryQuery, GroupPublicInfo};
use super::errors::MlsResult;
use super::transport::MlsEnvelope;
use super::types::MlsConfig;
use std::collections::HashMap;

/// Simulated network for testing
struct TestNetwork {
    /// Pending messages (recipient_index -> envelopes)
    mailboxes: HashMap<u32, Vec<MlsEnvelope>>,
}

impl TestNetwork {
    fn new() -> Self {
        Self {
            mailboxes: HashMap::new(),
        }
    }

    /// Send message to recipient
    fn send(&mut self, recipient: u32, envelope: MlsEnvelope) {
        self.mailboxes.entry(recipient).or_default().push(envelope);
    }

    /// Broadcast to all recipients
    fn broadcast(&mut self, recipients: &[u32], envelope: MlsEnvelope) {
        for &recipient in recipients {
            self.send(recipient, envelope.clone());
        }
    }

    /// Receive all pending messages for a recipient
    fn receive(&mut self, recipient: u32) -> Vec<MlsEnvelope> {
        self.mailboxes.remove(&recipient).unwrap_or_default()
    }

    /// Clear all mailboxes
    fn clear(&mut self) {
        self.mailboxes.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> MlsConfig {
        MlsConfig::default()
    }

    #[test]
    fn test_three_member_group_lifecycle() {
        let mut network = TestNetwork::new();

        // Alice creates group
        let alice = MlsHandle::create_group(
            Some("team".to_string()),
            b"alice_pk".to_vec(),
            b"alice".to_vec(),
            vec![1, 2, 3, 4],
            test_config(),
        )
        .unwrap();

        assert_eq!(alice.member_count().unwrap(), 1);
        assert_eq!(alice.epoch().unwrap(), 0);

        // Alice proposes adding Bob
        alice.propose_add(b"bob_pk".to_vec(), b"bob".to_vec()).unwrap();
        let (commit, welcomes) = alice.commit().unwrap();

        assert_eq!(alice.epoch().unwrap(), 1);
        assert_eq!(welcomes.len(), 1);

        // Bob joins via welcome
        let bob = MlsHandle::join_group(&welcomes[0], 1, b"bob_pk", test_config()).unwrap();
        assert_eq!(bob.epoch().unwrap(), 1);
        assert_eq!(bob.member_count().unwrap(), 2);

        // Alice proposes adding Charlie
        alice.propose_add(b"charlie_pk".to_vec(), b"charlie".to_vec()).unwrap();
        let (commit2, welcomes2) = alice.commit().unwrap();

        // Bob receives commit
        bob.receive_commit(&commit2).unwrap();

        // Charlie joins
        let charlie = MlsHandle::join_group(&welcomes2[0], 2, b"charlie_pk", test_config()).unwrap();

        // All at same epoch
        assert_eq!(alice.epoch().unwrap(), 2);
        assert_eq!(bob.epoch().unwrap(), 2);
        assert_eq!(charlie.epoch().unwrap(), 2);
        assert_eq!(alice.member_count().unwrap(), 3);

        // Alice sends message to all
        let msg = alice.send_message(b"Hello team!").unwrap();

        // Bob and Charlie can decrypt
        let bob_msg = bob.receive_message(&msg).unwrap();
        let charlie_msg = charlie.receive_message(&msg).unwrap();

        assert_eq!(bob_msg, b"Hello team!");
        assert_eq!(charlie_msg, b"Hello team!");
    }

    #[test]
    fn test_member_removal_flow() {
        // Test that remote commits (from other members) properly apply embedded proposals
        // This verifies the fix for commit processing bug where proposals weren't extracted

        let mut network = TestNetwork::new();

        // Create group with Alice and Bob
        let alice = MlsHandle::create_group(
            Some("test".to_string()),
            b"alice_pk".to_vec(),
            b"alice".to_vec(),
            vec![1, 2, 3, 4],
            test_config(),
        )
        .unwrap();

        // Add Bob
        alice.propose_add(b"bob_pk".to_vec(), b"bob".to_vec()).unwrap();
        let (commit1, welcomes1) = alice.commit().unwrap();
        let bob = MlsHandle::join_group(&welcomes1[0], 1, b"bob_pk", test_config()).unwrap();

        // Bob joins via Welcome, so he already has the state at epoch 1
        // No need to receive commit1 - he got that via the Welcome

        assert_eq!(alice.member_count().unwrap(), 2);
        assert_eq!(bob.member_count().unwrap(), 2);
        assert_eq!(alice.epoch().unwrap(), 1);
        assert_eq!(bob.epoch().unwrap(), 1);

        // Add Charlie via Alice
        alice.propose_add(b"charlie_pk".to_vec(), b"charlie".to_vec()).unwrap();
        let (commit2, welcomes2) = alice.commit().unwrap();
        let charlie = MlsHandle::join_group(&welcomes2[0], 2, b"charlie_pk", test_config()).unwrap();

        // Bob receives Alice's commit (with embedded Add proposal)
        bob.receive_commit(&commit2).unwrap();
        // Charlie already has epoch 2 state from Welcome

        // All members should have 3 members at epoch 2
        assert_eq!(alice.member_count().unwrap(), 3);
        assert_eq!(bob.member_count().unwrap(), 3);
        assert_eq!(charlie.member_count().unwrap(), 3);
        assert_eq!(alice.epoch().unwrap(), 2);

        // Now Bob removes Charlie (testing remote commit with Remove proposal)
        bob.propose_remove(2).unwrap();
        let (remove_commit, _) = bob.commit().unwrap();

        // Alice receives Bob's commit (Charlie is being removed)
        alice.receive_commit(&remove_commit).unwrap();
        
        // Alice and Bob now at epoch 3 with 2 members
        assert_eq!(alice.epoch().unwrap(), 3);
        assert_eq!(bob.epoch().unwrap(), 3);
        assert_eq!(alice.member_count().unwrap(), 2);
        assert_eq!(bob.member_count().unwrap(), 2);

        // Both can send messages in new epoch
        let msg = alice.send_message(b"Charlie is gone").unwrap();
        let decrypted = bob.receive_message(&msg).unwrap();
        assert_eq!(decrypted, b"Charlie is gone");
    }

    #[test]
    fn test_concurrent_proposals() {
        // Alice and Bob both propose changes
        let alice = MlsHandle::create_group(
            None,
            b"alice_pk".to_vec(),
            b"alice".to_vec(),
            vec![1, 2, 3, 4],
            test_config(),
        )
        .unwrap();

        alice.propose_add(b"bob_pk".to_vec(), b"bob".to_vec()).unwrap();
        let (_, welcomes) = alice.commit().unwrap();
        let bob = MlsHandle::join_group(&welcomes[0], 1, b"bob_pk", test_config()).unwrap();

        // Alice proposes adding Charlie
        let alice_proposal = alice.propose_add(b"charlie_pk".to_vec(), b"charlie".to_vec()).unwrap();

        // Bob proposes adding Dave
        let bob_proposal = bob.propose_add(b"dave_pk".to_vec(), b"dave".to_vec()).unwrap();

        // Alice receives Bob's proposal
        alice.receive_proposal(&bob_proposal).unwrap();

        // Bob receives Alice's proposal
        bob.receive_proposal(&alice_proposal).unwrap();

        // Alice commits (includes both proposals)
        let (commit, welcomes) = alice.commit().unwrap();

        // Bob applies commit
        bob.receive_commit(&commit).unwrap();

        // Both have 4 members now
        assert_eq!(alice.member_count().unwrap(), 4);
        assert_eq!(bob.member_count().unwrap(), 4);
    }

    #[test]
    fn test_message_ordering_and_replay() {
        let alice = MlsHandle::create_group(
            None,
            b"alice_pk".to_vec(),
            b"alice".to_vec(),
            vec![1, 2, 3, 4],
            test_config(),
        )
        .unwrap();

        // Send multiple messages
        let msg1 = alice.send_message(b"first").unwrap();
        let msg2 = alice.send_message(b"second").unwrap();
        let msg3 = alice.send_message(b"third").unwrap();

        // Receive in different order (allowed)
        assert!(alice.receive_message(&msg2).is_ok());
        assert!(alice.receive_message(&msg1).is_ok());
        assert!(alice.receive_message(&msg3).is_ok());

        // Replays fail
        assert!(alice.receive_message(&msg1).is_err());
        assert!(alice.receive_message(&msg2).is_err());
        assert!(alice.receive_message(&msg3).is_err());
    }

    #[test]
    fn test_epoch_isolation() {
        let alice = MlsHandle::create_group(
            None,
            b"alice_pk".to_vec(),
            b"alice".to_vec(),
            vec![1, 2, 3, 4],
            test_config(),
        )
        .unwrap();

        // Message at epoch 0
        let old_msg = alice.send_message(b"epoch 0").unwrap();

        // Advance epoch
        alice.propose_add(b"bob_pk".to_vec(), b"bob".to_vec()).unwrap();
        alice.commit().unwrap();

        assert_eq!(alice.epoch().unwrap(), 1);

        // Old message rejected
        assert!(alice.receive_message(&old_msg).is_err());

        // New message at epoch 1 works
        let new_msg = alice.send_message(b"epoch 1").unwrap();
        assert!(alice.receive_message(&new_msg).is_ok());
    }

    #[test]
    fn test_self_update_rotation() {
        let alice = MlsHandle::create_group(
            None,
            b"alice_pk".to_vec(),
            b"alice".to_vec(),
            vec![1, 2, 3, 4],
            test_config(),
        )
        .unwrap();

        // Alice rotates her key
        alice.propose_update(b"alice_new_pk".to_vec()).unwrap();
        let (commit, _) = alice.commit().unwrap();

        assert_eq!(alice.epoch().unwrap(), 1);

        // Can still send/receive
        let msg = alice.send_message(b"rotated").unwrap();
        assert!(alice.receive_message(&msg).is_ok());
    }

    #[test]
    fn test_batch_operations() {
        let alice = MlsHandle::create_group(
            None,
            b"alice_pk".to_vec(),
            b"alice".to_vec(),
            vec![1, 2, 3, 4],
            test_config(),
        )
        .unwrap();

        // Batch add 5 members
        let members = vec![
            (b"bob_pk".to_vec(), b"bob".to_vec()),
            (b"charlie_pk".to_vec(), b"charlie".to_vec()),
            (b"dave_pk".to_vec(), b"dave".to_vec()),
            (b"eve_pk".to_vec(), b"eve".to_vec()),
            (b"frank_pk".to_vec(), b"frank".to_vec()),
        ];

        let proposals = alice.propose_add_batch(members).unwrap();
        assert_eq!(proposals.len(), 5);

        let (commit, welcomes) = alice.commit().unwrap();
        assert_eq!(welcomes.len(), 5);
        assert_eq!(alice.member_count().unwrap(), 6);
    }

    #[test]
    fn test_discovery_publication() {
        let alice = MlsHandle::create_group(
            Some("public-group".to_string()),
            b"alice_pk".to_vec(),
            b"alice".to_vec(),
            vec![1, 2, 3, 4],
            test_config(),
        )
        .unwrap();

        // Get group info for publication
        let metadata = alice.metadata().unwrap();
        
        // Simulate creating public info (would use actual tree from handle)
        use crate::core_mls::discovery::GroupPublicInfo;
        use crate::core_mls::tree::MlsTree;
        use sha2::{Digest, Sha256};

        let tree = MlsTree::new(); // Simplified for test
        let public_info = GroupPublicInfo::from_metadata(
            alice.group_id().unwrap(),
            &metadata,
            &tree,
            |data| {
                let mut hasher = Sha256::new();
                hasher.update(data);
                hasher.finalize().to_vec()
            },
        );

        // Verify signature
        assert!(public_info.verify(|data, sig| {
            let mut hasher = Sha256::new();
            hasher.update(data);
            &hasher.finalize()[..] == sig
        }).is_ok());

        // Serialize for CRDT storage
        let json = public_info.to_json().unwrap();
        assert!(json.contains("public-group"));
    }

    #[test]
    fn test_discovery_query() {
        use crate::core_mls::discovery::{DiscoveryQuery, GroupPublicInfo};
        use crate::core_mls::tree::MlsTree;
        use sha2::{Digest, Sha256};

        let alice = MlsHandle::create_group(
            Some("team-alpha".to_string()),
            b"alice_pk".to_vec(),
            b"alice".to_vec(),
            vec![1, 2, 3, 4],
            test_config(),
        )
        .unwrap();

        let tree = MlsTree::new();
        let info = GroupPublicInfo::from_metadata(
            alice.group_id().unwrap(),
            &alice.metadata().unwrap(),
            &tree,
            |data| {
                let mut hasher = Sha256::new();
                hasher.update(data);
                hasher.finalize().to_vec()
            },
        );

        // Query by name
        let mut query = DiscoveryQuery::all();
        query.name_pattern = Some("team".to_string());
        assert!(query.matches(&info));

        query.name_pattern = Some("other".to_string());
        assert!(!query.matches(&info));

        // Query by member count
        query.name_pattern = None;
        query.min_members = Some(1);
        query.max_members = Some(5);
        assert!(query.matches(&info));
    }

    #[test]
    fn test_multi_device_same_user() {
        // Simulate one user with multiple devices
        let device1 = MlsHandle::create_group(
            Some("alice-devices".to_string()),
            b"alice_device1_pk".to_vec(),
            b"alice".to_vec(),
            vec![1, 2, 3, 4],
            test_config(),
        )
        .unwrap();

        // Device 1 adds device 2
        device1.propose_add(b"alice_device2_pk".to_vec(), b"alice".to_vec()).unwrap();
        let (_, welcomes) = device1.commit().unwrap();

        let device2 = MlsHandle::join_group(&welcomes[0], 1, b"alice_device2_pk", test_config()).unwrap();

        // Both devices can send messages
        let msg1 = device1.send_message(b"from device 1").unwrap();
        let msg2 = device2.send_message(b"from device 2").unwrap();

        // Both can receive from each other
        assert!(device2.receive_message(&msg1).is_ok());
        assert!(device1.receive_message(&msg2).is_ok());
    }

    #[test]
    fn test_stress_100_messages() {
        let alice = MlsHandle::create_group(
            None,
            b"alice_pk".to_vec(),
            b"alice".to_vec(),
            vec![1, 2, 3, 4],
            test_config(),
        )
        .unwrap();

        // Send 100 messages
        for i in 0..100 {
            let msg = format!("message_{}", i);
            let envelope = alice.send_message(msg.as_bytes()).unwrap();
            let decrypted = alice.receive_message(&envelope).unwrap();
            assert_eq!(decrypted, msg.as_bytes());
        }
    }

    #[test]
    fn test_large_group_performance() {
        let alice = MlsHandle::create_group(
            Some("large-group".to_string()),
            b"alice_pk".to_vec(),
            b"alice".to_vec(),
            vec![1, 2, 3, 4],
            test_config(),
        )
        .unwrap();

        // Add 20 members in batches
        let mut members = Vec::new();
        for i in 0..20 {
            let pk = format!("member_{}_pk", i);
            let id = format!("member_{}", i);
            members.push((pk.into_bytes(), id.into_bytes()));
        }

        let start = std::time::Instant::now();
        alice.propose_add_batch(members).unwrap();
        let (_, welcomes) = alice.commit().unwrap();
        let duration = start.elapsed();

        assert_eq!(welcomes.len(), 20);
        assert_eq!(alice.member_count().unwrap(), 21);
        
        // Should complete in reasonable time (< 1s)
        assert!(duration.as_millis() < 1000, "Took {}ms", duration.as_millis());
    }

    #[test]
    fn test_handle_cloning_shared_state() {
        let handle1 = MlsHandle::create_group(
            Some("shared".to_string()),
            b"alice_pk".to_vec(),
            b"alice".to_vec(),
            vec![1, 2, 3, 4],
            test_config(),
        )
        .unwrap();

        let handle2 = handle1.clone_handle();

        // Operate on handle1
        handle1.propose_add(b"bob_pk".to_vec(), b"bob".to_vec()).unwrap();
        handle1.commit().unwrap();

        // Changes visible in handle2
        assert_eq!(handle2.epoch().unwrap(), 1);
        assert_eq!(handle2.member_count().unwrap(), 2);

        // Both can send/receive
        let msg1 = handle1.send_message(b"from h1").unwrap();
        let msg2 = handle2.send_message(b"from h2").unwrap();

        assert!(handle2.receive_message(&msg1).is_ok());
        assert!(handle1.receive_message(&msg2).is_ok());
    }
}
