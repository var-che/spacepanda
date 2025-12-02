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

    // Note: Multi-member tests require proper key package exchange
    // and Welcome message handling, which needs more infrastructure.
    // These will be added as Phase 4 progresses.
}
