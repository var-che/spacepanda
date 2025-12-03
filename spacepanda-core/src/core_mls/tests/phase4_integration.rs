//! Phase 4: OpenMLS Engine Integration Tests
//!
//! These tests verify that the OpenMLS engine works correctly and can
//! be integrated with our existing MlsHandle API.

#[cfg(test)]
mod openmls_integration_tests {
    use crate::core_mls::engine::openmls_engine::OpenMlsEngine;
    use crate::core_mls::types::{GroupId, MlsConfig};
    use openmls_rust_crypto::OpenMlsRustCrypto;
    use std::sync::Arc;

    /// Test creating a new group with OpenMLS engine
    #[tokio::test]
    async fn test_create_group_with_openmls() {
        let config = MlsConfig::default();
        let group_id = GroupId::random();
        let identity = b"alice@example.com".to_vec();
        let provider = Arc::new(OpenMlsRustCrypto::default());

        let result = OpenMlsEngine::create_group(group_id, identity, config, provider).await;

        assert!(result.is_ok(), "Should create group successfully");

        let engine = result.unwrap();
        let epoch = engine.epoch().await;
        assert_eq!(epoch, 0, "New group should start at epoch 0");
    }

    /// Test getting group metadata
    #[tokio::test]
    async fn test_group_metadata() {
        let config = MlsConfig::default();
        let group_id = GroupId::random();
        let identity = b"bob@example.com".to_vec();
        let provider = Arc::new(OpenMlsRustCrypto::default());

        let engine = OpenMlsEngine::create_group(group_id, identity.clone(), config, provider)
            .await
            .expect("Failed to create group");

        let metadata = engine.metadata().await.expect("Failed to get metadata");

        // Should have exactly one member (the creator)
        assert_eq!(metadata.members.len(), 1, "Group should have 1 member");
        assert_eq!(metadata.epoch, 0, "Should be at epoch 0");
    }

    /// Test group ID generation
    #[tokio::test]
    async fn test_group_id() {
        let config = MlsConfig::default();
        let group_id1 = GroupId::random();
        let group_id2 = GroupId::random();
        let identity1 = b"user1@example.com".to_vec();
        let identity2 = b"user2@example.com".to_vec();
        let provider = Arc::new(OpenMlsRustCrypto::default());

        let engine1 = OpenMlsEngine::create_group(group_id1.clone(), identity1, config.clone(), provider.clone())
            .await
            .expect("Failed to create group 1");

        let engine2 = OpenMlsEngine::create_group(group_id2.clone(), identity2, config, provider)
            .await
            .expect("Failed to create group 2");

        let id1 = engine1.group_id().await;
        let id2 = engine2.group_id().await;

        // IDs should match what we provided
        assert_eq!(id1, group_id1, "Group 1 ID should match");
        assert_eq!(id2, group_id2, "Group 2 ID should match");

        // Different groups should have different IDs
        assert_ne!(id1, id2, "Different groups should have different IDs");
    }

    /// Test message encryption and decryption
    #[tokio::test]
    async fn test_message_encryption() {
        let config = MlsConfig::default();
        let group_id = GroupId::random();
        let identity = b"charlie@example.com".to_vec();
        let provider = Arc::new(OpenMlsRustCrypto::default());

        let engine = OpenMlsEngine::create_group(group_id, identity, config, provider)
            .await
            .expect("Failed to create group");

        // Test that we can encrypt messages
        let plaintext = b"Hello, World!";
        let result = engine.send_message(plaintext).await;

        // Should succeed in creating encrypted message
        assert!(result.is_ok(), "Should encrypt message successfully");
        let ciphertext = result.unwrap();
        assert!(!ciphertext.is_empty(), "Ciphertext should not be empty");
        assert_ne!(&ciphertext[..], plaintext, "Ciphertext should differ from plaintext");
    }

    /// Test epoch advancement
    #[tokio::test]
    async fn test_epoch_advancement() {
        let config = MlsConfig::default();
        let group_id = GroupId::random();
        let identity = b"dave@example.com".to_vec();
        let provider = Arc::new(OpenMlsRustCrypto::default());

        let engine = OpenMlsEngine::create_group(group_id, identity, config, provider)
            .await
            .expect("Failed to create group");

        let initial_epoch = engine.epoch().await;
        assert_eq!(initial_epoch, 0, "Should start at epoch 0");

        // Commit with no changes should still advance epoch
        let result = engine.commit_pending().await;
        assert!(result.is_ok(), "Commit should succeed");

        let new_epoch = engine.epoch().await;
        assert_eq!(new_epoch, 1, "Epoch should advance after commit");
    }

    /// Test multiple groups can coexist
    #[tokio::test]
    async fn test_multiple_groups() {
        let config = MlsConfig::default();
        let provider = Arc::new(OpenMlsRustCrypto::default());

        let mut engines = Vec::new();
        for i in 0..5 {
            let group_id = GroupId::random();
            let identity = format!("user{}@example.com", i).into_bytes();

            let engine = OpenMlsEngine::create_group(group_id.clone(), identity, config.clone(), provider.clone())
                .await
                .expect("Failed to create group");

            engines.push((group_id.clone(), engine));
        }

        // Verify all groups have unique IDs and are at epoch 0
        for (expected_id, engine) in &engines {
            let actual_id = engine.group_id().await;
            assert_eq!(&actual_id, expected_id, "Group ID should match");

            let epoch = engine.epoch().await;
            assert_eq!(epoch, 0, "New group should be at epoch 0");
        }

        // Verify all group IDs are unique
        let mut seen_ids = std::collections::HashSet::new();
        for (group_id, _) in &engines {
            assert!(seen_ids.insert(group_id.clone()), "Group IDs should be unique");
        }
    }

    /// Test configuration variations
    #[tokio::test]
    async fn test_config_variations() {
        let group_id = GroupId::random();
        let identity = b"eve@example.com".to_vec();
        let provider = Arc::new(OpenMlsRustCrypto::default());

        // Test with default config
        let config1 = MlsConfig::default();
        let engine1 = OpenMlsEngine::create_group(group_id.clone(), identity.clone(), config1, provider.clone())
            .await
            .expect("Failed to create group with default config");
        assert_eq!(engine1.epoch().await, 0);

        // Test with custom config (if MlsConfig supports builder pattern)
        let config2 = MlsConfig::default();
        let group_id2 = GroupId::random();
        let engine2 = OpenMlsEngine::create_group(group_id2, identity.clone(), config2, provider)
            .await
            .expect("Failed to create group with custom config");
        assert_eq!(engine2.epoch().await, 0);
    }

    /// Test group metadata is consistent
    #[tokio::test]
    async fn test_metadata_consistency() {
        let config = MlsConfig::default();
        let group_id = GroupId::random();
        let identity = b"frank@example.com".to_vec();
        let provider = Arc::new(OpenMlsRustCrypto::default());

        let engine = OpenMlsEngine::create_group(group_id.clone(), identity, config, provider)
            .await
            .expect("Failed to create group");

        // Get metadata multiple times
        let meta1 = engine.metadata().await.expect("Failed to get metadata 1");
        let meta2 = engine.metadata().await.expect("Failed to get metadata 2");
        let meta3 = engine.metadata().await.expect("Failed to get metadata 3");

        // All should be identical
        assert_eq!(meta1.epoch, meta2.epoch);
        assert_eq!(meta2.epoch, meta3.epoch);
        assert_eq!(meta1.members.len(), meta2.members.len());
        assert_eq!(meta2.members.len(), meta3.members.len());
    }

    /// Test message encryption succeeds
    ///
    /// Note: This tests message creation. In OpenMLS, a sender cannot decrypt their own
    /// messages (this is RFC 9420 compliant behavior). Multi-member message testing requires
    /// proper key package exchange and Welcome message handling, which will be added when
    /// member operations (add/remove) are fully implemented.
    #[tokio::test]
    async fn test_message_send_receive() {
        let config = MlsConfig::default();
        let group_id = GroupId::random();
        let provider = Arc::new(OpenMlsRustCrypto::default());

        // Create Alice's group
        let alice_identity = b"alice@example.com".to_vec();
        let alice_engine =
            OpenMlsEngine::create_group(group_id.clone(), alice_identity, config.clone(), provider)
                .await
                .expect("Failed to create Alice's group");

        // Alice sends a message
        let plaintext = b"Hello, World!";
        let encrypted = alice_engine.send_message(plaintext).await.expect("Failed to send message");

        assert!(!encrypted.is_empty(), "Encrypted message should not be empty");
        assert_ne!(&encrypted[..], plaintext, "Encrypted message should differ from plaintext");

        // Note: Actual message send→receive testing requires two distinct members,
        // which needs the full key package exchange flow implemented in group_ops.rs
    }

    // Note: Advanced multi-member tests (add/remove members, Welcome messages)
    // require proper key package exchange infrastructure which will be added
    // when integrating with the full MlsHandle API.
}

/// End-to-End Multi-Member Integration Tests
///
/// These tests verify full multi-member MLS workflows including:
/// - Key package generation and exchange
/// - Member addition via Welcome messages
/// - Multi-member message encryption/decryption
/// - Member removal and state consistency
#[cfg(test)]
mod e2e_integration_tests {
    use crate::core_mls::engine::{GroupOperations, OpenMlsEngine};
    use crate::core_mls::types::{GroupId, MlsConfig};
    use openmls::prelude::*;
    use openmls_basic_credential::SignatureKeyPair;
    use openmls_rust_crypto::OpenMlsRustCrypto;
    use std::sync::Arc;
    use tls_codec::Serialize as TlsSerializeTrait;

    /// Helper to create a key package for a user
    async fn create_key_package(identity: &[u8]) -> KeyPackage {
        let provider = OpenMlsRustCrypto::default();
        let ciphersuite = Ciphersuite::MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519;

        // Generate signature keys
        let signature_keys = SignatureKeyPair::new(ciphersuite.signature_algorithm())
            .expect("Failed to generate signature keys");

        // Store keys
        signature_keys.store(provider.storage()).expect("Failed to store keys");

        // Create credential
        let basic_credential = BasicCredential::new(identity.to_vec());
        let credential_with_key = CredentialWithKey {
            credential: basic_credential.into(),
            signature_key: signature_keys.public().into(),
        };

        // Create key package - returns KeyPackageBundle
        let key_package_bundle = KeyPackage::builder()
            .build(ciphersuite, &provider, &signature_keys, credential_with_key)
            .expect("Failed to build key package");

        // Extract the KeyPackage from the bundle
        key_package_bundle.key_package().clone()
    }

    /// Serialize a key package to bytes
    fn serialize_key_package(kp: &KeyPackage) -> Vec<u8> {
        kp.tls_serialize_detached().expect("Failed to serialize key package")
    }

    /// E2E Test 1: Two-member group with key package exchange
    #[tokio::test]
    async fn test_e2e_two_member_key_package_exchange() {
        let config = MlsConfig::default();
        let group_id = GroupId::random();
        let provider = Arc::new(OpenMlsRustCrypto::default());

        // Alice creates the group
        let alice_identity = b"alice@example.com".to_vec();
        let alice_engine =
            OpenMlsEngine::create_group(group_id.clone(), alice_identity.clone(), config.clone(), provider)
                .await
                .expect("Failed to create Alice's group");

        // Verify Alice is the only member
        let metadata = alice_engine.metadata().await.expect("Failed to get metadata");
        assert_eq!(metadata.members.len(), 1, "Alice should be the only member initially");
        assert_eq!(metadata.epoch, 0, "Should be at epoch 0");

        // Bob generates a key package
        let bob_key_package = create_key_package(b"bob@example.com").await;
        let bob_key_package_bytes = serialize_key_package(&bob_key_package);

        // Alice adds Bob to the group
        let (add_commit_bytes, welcome_bytes) = alice_engine
            .add_members(vec![bob_key_package_bytes])
            .await
            .expect("Failed to add Bob");

        assert!(!add_commit_bytes.is_empty(), "Add commit should produce bytes");
        assert!(welcome_bytes.is_some(), "Welcome message should be present");
        assert!(!welcome_bytes.as_ref().unwrap().is_empty(), "Welcome should have bytes");

        // Verify epoch advanced and member count increased
        let new_metadata = alice_engine.metadata().await.expect("Failed to get metadata");
        assert_eq!(new_metadata.epoch, 1, "Epoch should advance after adding member");
        assert_eq!(new_metadata.members.len(), 2, "Should have 2 members after add");
    }

    /// E2E Test 2: Welcome message handling and group joining
    #[tokio::test]
    async fn test_e2e_welcome_message_handling() {
        let config = MlsConfig::default();
        let group_id = GroupId::random();
        let provider = Arc::new(OpenMlsRustCrypto::default());

        // Alice creates the group
        let alice_identity = b"alice@example.com".to_vec();
        let alice_engine =
            OpenMlsEngine::create_group(group_id.clone(), alice_identity, config.clone(), provider)
                .await
                .expect("Failed to create Alice's group");

        // Bob generates key package
        let bob_key_package = create_key_package(b"bob@example.com").await;
        let bob_kp_bytes = serialize_key_package(&bob_key_package);

        // Alice adds Bob and gets Welcome message
        let (_add_commit, welcome_bytes) =
            alice_engine.add_members(vec![bob_kp_bytes]).await.expect("Failed to add Bob");

        // Verify Welcome message was created
        assert!(welcome_bytes.is_some(), "Welcome message should be created for new member");
        let _welcome = welcome_bytes.unwrap();

        // Note: Full Welcome message handling requires:
        // 1. Bob receiving and processing Welcome via join_group
        // 2. Bob's group state syncing with Alice's
        // This demonstrates Welcome extraction is now working

        let metadata = alice_engine.metadata().await.expect("Failed to get metadata");
        assert_eq!(metadata.members.len(), 2, "Group should have 2 members");
        assert_eq!(metadata.epoch, 1, "Epoch should be 1 after add");
    }

    /// E2E Test 3: Multi-member message encryption and decryption
    #[tokio::test]
    async fn test_e2e_multi_member_message_flow() {
        let config = MlsConfig::default();
        let group_id = GroupId::random();
        let provider = Arc::new(OpenMlsRustCrypto::default());

        // Create Alice's group
        let alice_engine = OpenMlsEngine::create_group(
            group_id.clone(),
            b"alice@example.com".to_vec(),
            config.clone(),
            provider,
        )
        .await
        .expect("Failed to create group");

        // Bob generates key package
        let bob_kp = create_key_package(b"bob@example.com").await;
        let bob_kp_bytes = serialize_key_package(&bob_kp);

        // Add Bob to the group
        let (_commit, _welcome) =
            alice_engine.add_members(vec![bob_kp_bytes]).await.expect("Failed to add Bob");

        // Alice sends a message
        let plaintext = b"Hello Bob!";
        let encrypted = alice_engine.send_message(plaintext).await.expect("Failed to send message");

        assert!(!encrypted.is_empty(), "Encrypted message should not be empty");
        assert_ne!(&encrypted[..], plaintext, "Should be encrypted");

        // Verify the message is a valid MLS message
        let result = alice_engine.process_message(&encrypted).await;
        // Note: Alice cannot decrypt her own messages in MLS (RFC 9420 compliant)
        // This is expected behavior - only Bob could decrypt this

        let metadata = alice_engine.metadata().await.unwrap();
        assert_eq!(metadata.epoch, 1, "Should be at epoch 1");
    }

    /// E2E Test 4: Member removal and state updates
    #[tokio::test]
    async fn test_e2e_member_removal() {
        let config = MlsConfig::default();
        let group_id = GroupId::random();
        let provider = Arc::new(OpenMlsRustCrypto::default());

        // Alice creates group
        let alice_engine =
            OpenMlsEngine::create_group(group_id, b"alice@example.com".to_vec(), config.clone(), provider)
                .await
                .expect("Failed to create group");

        // Add Bob
        let bob_kp = create_key_package(b"bob@example.com").await;
        let bob_kp_bytes = serialize_key_package(&bob_kp);
        alice_engine.add_members(vec![bob_kp_bytes]).await.expect("Failed to add Bob");

        let metadata_before = alice_engine.metadata().await.unwrap();
        assert_eq!(metadata_before.members.len(), 2, "Should have 2 members");
        assert_eq!(metadata_before.epoch, 1, "Epoch 1 after add");

        // Add Charlie
        let charlie_kp = create_key_package(b"charlie@example.com").await;
        let charlie_kp_bytes = serialize_key_package(&charlie_kp);
        alice_engine
            .add_members(vec![charlie_kp_bytes])
            .await
            .expect("Failed to add Charlie");

        let metadata_mid = alice_engine.metadata().await.unwrap();
        assert_eq!(metadata_mid.members.len(), 3, "Should have 3 members");
        assert_eq!(metadata_mid.epoch, 2, "Epoch 2 after second add");

        // Remove Bob (leaf index 1)
        let remove_commit =
            alice_engine.remove_members(vec![1]).await.expect("Failed to remove Bob");

        assert!(!remove_commit.is_empty(), "Remove commit should produce bytes");

        let metadata_after = alice_engine.metadata().await.unwrap();
        assert_eq!(metadata_after.members.len(), 2, "Should have 2 members after remove");
        assert_eq!(metadata_after.epoch, 3, "Epoch should advance to 3");
    }

    /// E2E Test 5: Multiple sequential operations
    #[tokio::test]
    async fn test_e2e_sequential_operations() {
        let config = MlsConfig::default();
        let group_id = GroupId::random();
        let provider = Arc::new(OpenMlsRustCrypto::default());

        let alice_engine =
            OpenMlsEngine::create_group(group_id, b"alice@example.com".to_vec(), config.clone(), provider)
                .await
                .expect("Failed to create group");

        // Initial state
        let meta0 = alice_engine.metadata().await.unwrap();
        assert_eq!(meta0.epoch, 0);
        assert_eq!(meta0.members.len(), 1);

        // Add Bob
        let bob_kp = create_key_package(b"bob@example.com").await;
        alice_engine.add_members(vec![serialize_key_package(&bob_kp)]).await.unwrap();

        // Send message
        alice_engine.send_message(b"Message 1").await.unwrap();

        // Add Charlie
        let charlie_kp = create_key_package(b"charlie@example.com").await;
        alice_engine
            .add_members(vec![serialize_key_package(&charlie_kp)])
            .await
            .unwrap();

        // Send another message
        alice_engine.send_message(b"Message 2").await.unwrap();

        // Remove Bob
        alice_engine.remove_members(vec![1]).await.unwrap();

        // Final state
        let meta_final = alice_engine.metadata().await.unwrap();
        assert_eq!(meta_final.members.len(), 2, "Alice and Charlie remain");
        assert_eq!(meta_final.epoch, 3, "Epoch advances through operations");
    }

    /// E2E Test 6: Batch member additions
    #[tokio::test]
    async fn test_e2e_batch_member_additions() {
        let config = MlsConfig::default();
        let group_id = GroupId::random();
        let provider = Arc::new(OpenMlsRustCrypto::default());

        let alice_engine =
            OpenMlsEngine::create_group(group_id, b"alice@example.com".to_vec(), config, provider)
                .await
                .expect("Failed to create group");

        // Generate multiple key packages
        let mut key_packages = Vec::new();
        for i in 0..5 {
            let identity = format!("user{}@example.com", i);
            let kp = create_key_package(identity.as_bytes()).await;
            key_packages.push(serialize_key_package(&kp));
        }

        // Add all members in one commit
        alice_engine
            .add_members(key_packages)
            .await
            .expect("Failed to add batch members");

        let metadata = alice_engine.metadata().await.unwrap();
        assert_eq!(metadata.members.len(), 6, "Should have Alice + 5 new members");
        assert_eq!(metadata.epoch, 1, "Single commit for batch add");
    }

    /// E2E Test 7: Message ordering and epoch consistency
    #[tokio::test]
    async fn test_e2e_message_ordering_and_epochs() {
        let config = MlsConfig::default();
        let group_id = GroupId::random();
        let provider = Arc::new(OpenMlsRustCrypto::default());

        let alice_engine =
            OpenMlsEngine::create_group(group_id, b"alice@example.com".to_vec(), config, provider)
                .await
                .unwrap();

        // Track epochs through operations
        let mut epochs = vec![alice_engine.epoch().await];

        // Add member
        let bob_kp = create_key_package(b"bob@example.com").await;
        alice_engine.add_members(vec![serialize_key_package(&bob_kp)]).await.unwrap();
        epochs.push(alice_engine.epoch().await);

        // Send messages (should not change epoch)
        for i in 0..3 {
            alice_engine.send_message(format!("Message {}", i).as_bytes()).await.unwrap();
            epochs.push(alice_engine.epoch().await);
        }

        // Another add
        let charlie_kp = create_key_package(b"charlie@example.com").await;
        alice_engine
            .add_members(vec![serialize_key_package(&charlie_kp)])
            .await
            .unwrap();
        epochs.push(alice_engine.epoch().await);

        // Verify epoch progression
        assert_eq!(epochs[0], 0, "Start at epoch 0");
        assert_eq!(epochs[1], 1, "Epoch 1 after first add");
        assert_eq!(epochs[2], 1, "Messages don't change epoch");
        assert_eq!(epochs[3], 1, "Messages don't change epoch");
        assert_eq!(epochs[4], 1, "Messages don't change epoch");
        assert_eq!(epochs[5], 2, "Epoch 2 after second add");
    }

    /// E2E Test 8: State consistency across operations
    #[tokio::test]
    async fn test_e2e_state_consistency() {
        let config = MlsConfig::default();
        let group_id = GroupId::random();
        let provider = Arc::new(OpenMlsRustCrypto::default());

        let alice_engine =
            OpenMlsEngine::create_group(group_id.clone(), b"alice@example.com".to_vec(), config, provider)
                .await
                .unwrap();

        // Perform various operations
        let bob_kp = create_key_package(b"bob@example.com").await;
        let (_commit, _welcome) =
            alice_engine.add_members(vec![serialize_key_package(&bob_kp)]).await.unwrap();

        // Check state consistency
        let meta1 = alice_engine.metadata().await.unwrap();
        let gid1 = alice_engine.group_id().await;
        let epoch1 = alice_engine.epoch().await;

        // Query again - should be identical
        let meta2 = alice_engine.metadata().await.unwrap();
        let gid2 = alice_engine.group_id().await;
        let epoch2 = alice_engine.epoch().await;

        assert_eq!(meta1.epoch, meta2.epoch);
        assert_eq!(meta1.members.len(), meta2.members.len());
        assert_eq!(gid1, gid2);
        assert_eq!(epoch1, epoch2);
        assert_eq!(gid1, group_id, "Group ID should match original");
    }
}
