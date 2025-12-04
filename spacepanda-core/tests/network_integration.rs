/// Integration tests for P2P networking between ChannelManagers
///
/// Tests end-to-end message delivery over the network layer

use spacepanda_core::config::Config;
use spacepanda_core::core_mls::service::MlsService;
use spacepanda_core::core_mvp::network::NetworkLayer;
use spacepanda_core::core_router::session_manager::PeerId;
use spacepanda_core::core_router::RouterHandle;
use spacepanda_core::core_store::model::types::{ChannelId, UserId};
use spacepanda_core::core_store::store::{LocalStore, LocalStoreConfig};
use spacepanda_core::shutdown::ShutdownCoordinator;
use spacepanda_core::{ChannelManager, Identity};
use std::sync::Arc;
use tempfile::TempDir;

/// Helper to create a ChannelManager with network layer
async fn create_networked_manager(
    user_name: &str,
    peer_id_bytes: Vec<u8>,
) -> (Arc<ChannelManager>, Arc<NetworkLayer>, TempDir) {
    let config = Arc::new(Config::default());
    let shutdown = Arc::new(ShutdownCoordinator::new(std::time::Duration::from_secs(30)));
    let mls_service = Arc::new(MlsService::new(&config, shutdown.clone()));

    let temp_dir = tempfile::tempdir().unwrap();
    let store_config = LocalStoreConfig {
        data_dir: temp_dir.path().to_path_buf(),
        enable_encryption: false,
        snapshot_interval: 1000,
        max_log_size: 10_000_000,
        enable_compaction: false,
        require_signatures: false,
        authorized_keys: Vec::new(),
    };
    let store = Arc::new(LocalStore::new(store_config).unwrap());

    let identity = Arc::new(Identity::new(
        UserId(uuid::Uuid::new_v4().to_string()),
        user_name.to_string(),
        uuid::Uuid::new_v4().to_string(),
    ));

    let manager = ChannelManager::new(mls_service, store, identity, config.clone());

    let (router, _handle) = RouterHandle::new();
    let peer_id = PeerId(peer_id_bytes);
    let (network, _rx) = NetworkLayer::new(router, peer_id);
    let network = Arc::new(network);

    let manager = Arc::new(manager.with_network(network.clone()));

    (manager, network, temp_dir)
}

#[tokio::test]
async fn test_two_managers_network_enabled() {
    // Create two managers with network layers
    let (manager1, network1, _dir1) = create_networked_manager("alice", vec![1, 2, 3, 4]).await;
    let (manager2, network2, _dir2) = create_networked_manager("bob", vec![5, 6, 7, 8]).await;

    // Verify network is enabled
    assert!(manager1.is_network_enabled());
    assert!(manager2.is_network_enabled());

    // Verify peer IDs are set correctly
    assert_eq!(network1.local_peer_id(), &PeerId(vec![1, 2, 3, 4]));
    assert_eq!(network2.local_peer_id(), &PeerId(vec![5, 6, 7, 8]));
}

#[tokio::test]
async fn test_register_channel_members() {
    let (manager1, network1, _dir1) = create_networked_manager("alice", vec![1, 2, 3, 4]).await;
    let (manager2, _network2, _dir2) = create_networked_manager("bob", vec![5, 6, 7, 8]).await;

    // Create a channel on manager1
    let channel_id = manager1.create_channel("test-network-channel".to_string(), false).await.unwrap();

    // Register both users in the network layer
    let alice_user_id = manager1.identity().user_id.clone();
    let bob_user_id = manager2.identity().user_id.clone();

    network1
        .register_channel_member(&channel_id, alice_user_id.clone(), PeerId(vec![1, 2, 3, 4]))
        .await;
    network1
        .register_channel_member(&channel_id, bob_user_id.clone(), PeerId(vec![5, 6, 7, 8]))
        .await;

    // Verify peers are registered
    let peers = network1.get_channel_peers(&channel_id).await;
    assert_eq!(peers.len(), 2);
}

#[tokio::test]
async fn test_send_message_with_network() {
    let (manager1, network1, _dir1) = create_networked_manager("alice", vec![1, 2, 3, 4]).await;
    let (manager2, _network2, _dir2) = create_networked_manager("bob", vec![5, 6, 7, 8]).await;

    // Create channel
    let channel_id = manager1.create_channel("network-test".to_string(), false).await.unwrap();

    // Register members in network layer (simulating they're both in the channel)
    let alice_user_id = manager1.identity().user_id.clone();
    let bob_user_id = manager2.identity().user_id.clone();

    network1
        .register_channel_member(&channel_id, alice_user_id.clone(), PeerId(vec![1, 2, 3, 4]))
        .await;
    network1
        .register_channel_member(&channel_id, bob_user_id.clone(), PeerId(vec![5, 6, 7, 8]))
        .await;

    // Send a message - this will attempt to broadcast over network
    let result = manager1
        .send_message(&channel_id, "Hello from Alice!".as_bytes())
        .await;

    // The message should be sent successfully (even though router is mock)
    assert!(result.is_ok(), "Message send failed: {:?}", result.err());
}

#[tokio::test]
async fn test_network_broadcast_to_multiple_peers() {
    let (manager1, network1, _dir1) = create_networked_manager("alice", vec![1, 2, 3, 4]).await;
    let (_manager2, _network2, _dir2) = create_networked_manager("bob", vec![5, 6, 7, 8]).await;
    let (_manager3, _network3, _dir3) = create_networked_manager("charlie", vec![9, 10, 11, 12]).await;

    // Create channel
    let channel_id = manager1.create_channel("multi-peer-test".to_string(), false).await.unwrap();

    // Register three users
    network1
        .register_channel_member(
            &channel_id,
            UserId("alice".to_string()),
            PeerId(vec![1, 2, 3, 4]),
        )
        .await;
    network1
        .register_channel_member(
            &channel_id,
            UserId("bob".to_string()),
            PeerId(vec![5, 6, 7, 8]),
        )
        .await;
    network1
        .register_channel_member(
            &channel_id,
            UserId("charlie".to_string()),
            PeerId(vec![9, 10, 11, 12]),
        )
        .await;

    // Verify all peers registered
    let peers = network1.get_channel_peers(&channel_id).await;
    assert_eq!(peers.len(), 3);

    // Send message should broadcast to all 3 peers (though router is mock)
    let result = manager1
        .send_message(&channel_id, "Broadcast message".as_bytes())
        .await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_network_layer_isolation() {
    // Two separate network layers should not share state
    let (router1, _handle1) = RouterHandle::new();
    let (router2, _handle2) = RouterHandle::new();

    let (network1, _rx1) = NetworkLayer::new(router1, PeerId(vec![1]));
    let (network2, _rx2) = NetworkLayer::new(router2, PeerId(vec![2]));

    let channel_id = ChannelId("test-channel".to_string());

    // Register member on network1
    network1
        .register_channel_member(&channel_id, UserId("user1".to_string()), PeerId(vec![10]))
        .await;

    // network2 should not see it
    let peers1 = network1.get_channel_peers(&channel_id).await;
    let peers2 = network2.get_channel_peers(&channel_id).await;

    assert_eq!(peers1.len(), 1);
    assert_eq!(peers2.len(), 0);
}

#[tokio::test]
async fn test_manager_without_network_still_works() {
    // Manager without network layer should work normally (local only)
    let config = Arc::new(Config::default());
    let shutdown = Arc::new(ShutdownCoordinator::new(std::time::Duration::from_secs(30)));
    let mls_service = Arc::new(MlsService::new(&config, shutdown.clone()));

    let temp_dir = tempfile::tempdir().unwrap();
    let store_config = LocalStoreConfig {
        data_dir: temp_dir.path().to_path_buf(),
        enable_encryption: false,
        snapshot_interval: 1000,
        max_log_size: 10_000_000,
        enable_compaction: false,
        require_signatures: false,
        authorized_keys: Vec::new(),
    };
    let store = Arc::new(LocalStore::new(store_config).unwrap());

    let identity = Arc::new(Identity::new(
        UserId(uuid::Uuid::new_v4().to_string()),
        "local_alice".to_string(),
        uuid::Uuid::new_v4().to_string(),
    ));

    let manager = ChannelManager::new(mls_service, store, identity, config.clone());

    // Should not have network enabled
    assert!(!manager.is_network_enabled());

    // Should still be able to create channels and send messages
    let channel_id = manager.create_channel("local-channel".to_string(), false).await.unwrap();
    let result = manager
        .send_message(&channel_id, "Local message".as_bytes())
        .await;

    assert!(result.is_ok());
}
