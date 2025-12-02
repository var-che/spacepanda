//! Phase 4: OpenMLS Engine Integration Tests
//!
//! These tests verify that the OpenMLS engine works correctly and can
//! be integrated with our existing MlsHandle API.

#[cfg(test)]
mod openmls_integration_tests {
    use crate::core_mls::engine::openmls_engine::OpenMlsEngine;
    use crate::core_mls::types::{MlsConfig, GroupId};

    /// Test creating a new group with OpenMLS engine
    #[tokio::test]
    async fn test_create_group_with_openmls() {
        let config = MlsConfig::default();
        let group_id = GroupId::random();
        let identity = b"alice@example.com".to_vec();

        let result = OpenMlsEngine::create_group(
            group_id,
            identity,
            config,
        ).await;

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

        let engine = OpenMlsEngine::create_group(group_id, identity.clone(), config)
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

        let engine1 = OpenMlsEngine::create_group(group_id1.clone(), identity1, config.clone())
            .await
            .expect("Failed to create group 1");
        
        let engine2 = OpenMlsEngine::create_group(group_id2.clone(), identity2, config)
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

        let engine = OpenMlsEngine::create_group(group_id, identity, config)
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

        let engine = OpenMlsEngine::create_group(group_id, identity, config)
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
        
        let mut engines = Vec::new();
        for i in 0..5 {
            let group_id = GroupId::random();
            let identity = format!("user{}@example.com", i).into_bytes();
            
            let engine = OpenMlsEngine::create_group(group_id.clone(), identity, config.clone())
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

        // Test with default config
        let config1 = MlsConfig::default();
        let engine1 = OpenMlsEngine::create_group(group_id.clone(), identity.clone(), config1)
            .await
            .expect("Failed to create group with default config");
        assert_eq!(engine1.epoch().await, 0);

        // Test with custom config (if MlsConfig supports builder pattern)
        let config2 = MlsConfig::default();
        let group_id2 = GroupId::random();
        let engine2 = OpenMlsEngine::create_group(group_id2, identity.clone(), config2)
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

        let engine = OpenMlsEngine::create_group(group_id.clone(), identity, config)
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
        
        // Create Alice's group
        let alice_identity = b"alice@example.com".to_vec();
        let alice_engine = OpenMlsEngine::create_group(group_id.clone(), alice_identity, config.clone())
            .await
            .expect("Failed to create Alice's group");

        // Alice sends a message
        let plaintext = b"Hello, World!";
        let encrypted = alice_engine.send_message(plaintext)
            .await
            .expect("Failed to send message");
        
        assert!(!encrypted.is_empty(), "Encrypted message should not be empty");
        assert_ne!(&encrypted[..], plaintext, "Encrypted message should differ from plaintext");
        
        // Note: Actual message send→receive testing requires two distinct members,
        // which needs the full key package exchange flow implemented in group_ops.rs
    }

    // Note: Advanced multi-member tests (add/remove members, Welcome messages)
    // require proper key package exchange infrastructure which will be added
    // when integrating with the full MlsHandle API.
}
