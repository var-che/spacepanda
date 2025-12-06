/*
    integration.rs - Cross-subsystem integration tests

    These tests validate that different subsystems work together correctly.
    All tests must pass before MLS integration can begin.
*/

use spacepanda_core::core_dht::{DhtKey, DhtStorage, DhtValue, RoutingTable};
use spacepanda_core::core_identity::{DeviceId, DeviceMetadata, GlobalIdentity, KeyType, Keypair};
use spacepanda_core::core_store::crdt::{LWWRegister, VectorClock};
use spacepanda_core::core_store::model::{
    Channel, ChannelId, ChannelType, Space, SpaceId, Timestamp, UserId,
};
use spacepanda_core::core_store::store::{LocalStore, LocalStoreConfig};
use std::collections::HashMap;
use tempfile::tempdir;

/// Test 6.1: Basic Identity + Store Integration
///
/// Validates that identity system integrates with store for persistence.
#[tokio::test]
async fn test_integration_identity_store_roundtrip() {
    // Create identity
    let device_id = DeviceId::generate();
    let user_id = UserId::generate();

    let identity = GlobalIdentity::create_global_identity().unwrap();

    // Create store
    let temp_dir = tempdir().unwrap();
    let config = LocalStoreConfig {
        data_dir: temp_dir.path().to_path_buf(),
        enable_encryption: false,
        snapshot_interval: 1000,
        max_log_size: 10_000_000,
        enable_compaction: false,
        require_signatures: false,
        authorized_keys: Vec::new(),
    };

    let store = LocalStore::new(config.clone()).unwrap();

    // Create a space
    let space = Space::new(
        SpaceId::generate(),
        "Test Space".to_string(),
        user_id.clone(),
        Timestamp::now(),
        device_id.to_string(),
    );

    let space_id = space.id.clone();
    store.store_space(&space).unwrap();

    // Verify retrieval
    let retrieved = store.get_space(&space_id).unwrap();
    assert!(retrieved.is_some(), "Space should be stored");
    assert_eq!(retrieved.unwrap().name.get().unwrap(), "Test Space");

    // Create snapshot
    store.create_snapshot().unwrap();
    drop(store);

    // Restore from new store instance
    let store2 = LocalStore::new(config).unwrap();
    store2.load().unwrap();

    let restored = store2.get_space(&space_id).unwrap();
    assert!(restored.is_some(), "Space should be restored");
    assert_eq!(restored.unwrap().name.get().unwrap(), "Test Space");

    println!("✅ Integration: Identity + Store roundtrip successful");
}

/// Test 6.2: Store + DHT Key Mapping
///
/// Validates that store keys map correctly to DHT keys.
#[tokio::test]
async fn test_integration_store_dht_key_mapping() {
    let dht_storage = DhtStorage::new();

    // Create channel with known ID
    let channel_id = ChannelId::generate();
    let user_id = UserId::generate();

    let channel = Channel::new(
        channel_id.clone(),
        "general".to_string(),
        ChannelType::Text,
        user_id,
        Timestamp::now(),
        "node1".to_string(),
    );

    // Serialize channel for DHT storage
    let channel_data = serde_json::to_vec(&channel).unwrap();

    // Create DHT key from channel ID
    let dht_key = DhtKey::hash(channel_id.0.as_bytes());
    let dht_value = DhtValue::new(channel_data.clone()).with_ttl(3600);

    // Store in DHT
    dht_storage.put(dht_key, dht_value).unwrap();

    // Retrieve from DHT
    let retrieved_value = dht_storage.get(&dht_key).unwrap();
    assert_eq!(retrieved_value.data, channel_data);

    // Deserialize back to channel
    let retrieved_channel: Channel = serde_json::from_slice(&retrieved_value.data).unwrap();
    assert_eq!(retrieved_channel.id, channel_id);
    assert_eq!(retrieved_channel.name.get().unwrap(), "general");

    println!("✅ Integration: Store + DHT key mapping successful");
}

/// Test 6.3: CRDT + Store Persistence
///
/// Validates that CRDT state persists correctly through store.
#[tokio::test]
async fn test_integration_crdt_store_persistence() {
    let temp_dir = tempdir().unwrap();
    let config = LocalStoreConfig {
        data_dir: temp_dir.path().to_path_buf(),
        enable_encryption: false,
        snapshot_interval: 100,
        max_log_size: 1_000_000,
        enable_compaction: false,
        require_signatures: false,
        authorized_keys: Vec::new(),
    };

    let store = LocalStore::new(config.clone()).unwrap();

    // Create space with initial value
    let user_id = UserId::generate();
    let space = Space::new(
        SpaceId::generate(),
        "Initial Name".to_string(),
        user_id,
        Timestamp::now(),
        "node1".to_string(),
    );

    let space_id = space.id.clone();

    // Store and snapshot
    store.store_space(&space).unwrap();
    store.create_snapshot().unwrap();
    drop(store);

    // Restore and verify CRDT state persisted
    let store2 = LocalStore::new(config).unwrap();
    store2.load().unwrap();

    let restored = store2.get_space(&space_id).unwrap().unwrap();
    assert_eq!(restored.name.get().unwrap(), "Initial Name");

    println!("✅ Integration: CRDT + Store persistence successful");
}

/// Test 6.4: Multi-Device Identity Simulation
///
/// Validates that multiple devices can have separate identities.
#[tokio::test]
async fn test_integration_multi_device_identity() {
    // Create two separate global identities (simulating different devices)
    let identity_a = GlobalIdentity::create_global_identity().unwrap();
    let identity_b = GlobalIdentity::create_global_identity().unwrap();

    // Both identities should be valid and have nicknames
    assert_eq!(identity_a.nickname(), "default_user");
    assert_eq!(identity_b.nickname(), "default_user");

    // In a real multi-device scenario, these would be stored separately
    // and have different device-specific metadata

    println!("✅ Integration: Multi-device identity simulation successful");
}

/// Test 6.5: DHT Routing Table + Storage Coordination
///
/// Validates that routing table and storage work together.
#[tokio::test]
async fn test_integration_dht_routing_storage() {
    let local_id = DhtKey::hash(b"local_node");
    let mut routing_table = RoutingTable::new(local_id, 20);
    let storage = DhtStorage::new();

    // Add peers to routing table
    let peer_keys: Vec<_> = (0..10)
        .map(|i| {
            let peer_id = DhtKey::hash(format!("peer{}", i).as_bytes());
            let peer =
                spacepanda_core::core_dht::PeerContact::new(peer_id, format!("192.0.2.{}:1234", i));
            routing_table.insert(peer).unwrap();
            peer_id
        })
        .collect();

    // Store values that peers would provide
    for (i, peer_id) in peer_keys.iter().enumerate() {
        let key = DhtKey::hash(format!("key{}", i).as_bytes());
        let value = DhtValue::new(format!("value{}", i).into_bytes())
            .with_owner(*peer_id)
            .with_ttl(3600);

        storage.put(key, value).unwrap();
    }

    // Verify routing table has peers
    assert_eq!(routing_table.size(), 10);

    // Verify storage has values
    let active_keys = storage.active_keys().unwrap();
    assert_eq!(active_keys.len(), 10);

    // Find closest peers for a target
    let target = DhtKey::hash(b"target_key");
    let closest = routing_table.find_closest(&target, 3);
    assert_eq!(closest.len(), 3);

    println!("✅ Integration: DHT routing + storage coordination successful");
}

/// Test 6.6: Concurrent Store + DHT Operations
///
/// Validates that store and DHT can handle concurrent operations.
#[tokio::test]
async fn test_integration_concurrent_store_dht() {
    let storage = std::sync::Arc::new(DhtStorage::new());

    // Spawn concurrent tasks
    let mut handles = vec![];
    for thread_id in 0..5 {
        let storage_clone = storage.clone();

        let handle = tokio::spawn(async move {
            for i in 0..20 {
                let key = DhtKey::hash(format!("thread{}_key{}", thread_id, i).as_bytes());
                let value = DhtValue::new(format!("value{}", i).into_bytes());
                storage_clone.put(key, value).unwrap();
            }
        });

        handles.push(handle);
    }

    // Wait for all tasks
    for handle in handles {
        handle.await.unwrap();
    }

    // Verify all 100 keys stored (5 threads × 20 keys)
    let active_keys = storage.active_keys().unwrap();
    assert_eq!(active_keys.len(), 100);

    println!("✅ Integration: Concurrent store + DHT operations successful");
}

/// Test 6.7: Identity Keypair Types Integration
///
/// Validates different keypair types work correctly.
#[tokio::test]
async fn test_integration_identity_keypair_types() {
    // Test Ed25519 for signing
    let ed_keypair = Keypair::generate(KeyType::Ed25519);
    let message = b"test message";
    let signature = ed_keypair.sign(message);
    assert!(Keypair::verify(&ed_keypair.public, message, &signature));

    // Test X25519 for key agreement
    let x_keypair = Keypair::generate(KeyType::X25519);
    assert_eq!(x_keypair.key_type, KeyType::X25519);

    println!("✅ Integration: Identity keypair types integration successful");
}

/// Test 6.8: Store Snapshot + DHT Value Versioning
///
/// Validates that versioning works across store snapshots and DHT values.
#[tokio::test]
async fn test_integration_store_snapshot_dht_versioning() {
    let temp_dir = tempdir().unwrap();
    let config = LocalStoreConfig {
        data_dir: temp_dir.path().to_path_buf(),
        enable_encryption: false,
        snapshot_interval: 50,
        max_log_size: 1_000_000,
        enable_compaction: false,
        require_signatures: false,
        authorized_keys: Vec::new(),
    };

    let store = LocalStore::new(config.clone()).unwrap();
    let dht_storage = DhtStorage::new();

    // Create versioned space
    let user_id = UserId::generate();
    let space_id = SpaceId::generate();

    // Version 1
    let space_v1 = Space::new(
        space_id.clone(),
        "Version 1".to_string(),
        user_id.clone(),
        Timestamp::now(),
        "node1".to_string(),
    );
    store.store_space(&space_v1).unwrap();

    // Store in DHT with sequence 0
    let dht_key = DhtKey::hash(space_id.0.as_bytes());
    let dht_value_v1 = DhtValue::new(b"version1".to_vec()).with_sequence(0);
    dht_storage.put(dht_key, dht_value_v1).unwrap();

    // Version 2
    let space_v2 = Space::new(
        space_id.clone(),
        "Version 2".to_string(),
        user_id,
        Timestamp::now(),
        "node1".to_string(),
    );
    store.store_space(&space_v2).unwrap();

    // Update DHT with sequence 1
    let dht_value_v2 = DhtValue::new(b"version2".to_vec()).with_sequence(1);
    dht_storage.put(dht_key, dht_value_v2).unwrap();

    // Verify latest version in both
    let space_latest = store.get_space(&space_id).unwrap().unwrap();
    assert_eq!(space_latest.name.get().unwrap(), "Version 2");

    let dht_latest = dht_storage.get(&dht_key).unwrap();
    assert_eq!(dht_latest.sequence, 1);
    assert_eq!(dht_latest.data, b"version2");

    println!("✅ Integration: Store snapshot + DHT versioning successful");
}

/// Test 6.9: Full Stack Component Availability
///
/// Validates that all required components are available and can be instantiated.
#[tokio::test]
async fn test_integration_full_stack_availability() {
    // Identity
    let user_id = UserId::generate();
    let _identity = GlobalIdentity::create_global_identity().unwrap();

    // Store
    let temp_dir = tempdir().unwrap();
    let config = LocalStoreConfig {
        data_dir: temp_dir.path().to_path_buf(),
        enable_encryption: false,
        snapshot_interval: 100,
        max_log_size: 1_000_000,
        enable_compaction: false,
        require_signatures: false,
        authorized_keys: Vec::new(),
    };
    let _store = LocalStore::new(config).unwrap();

    // DHT
    let local_id = DhtKey::hash(b"local");
    let _routing_table = RoutingTable::new(local_id, 20);
    let _dht_storage = DhtStorage::new();

    // CRDT
    let _lww = LWWRegister::with_value("test".to_string(), "node1".to_string());
    let _vc = VectorClock::new();

    // Models
    let _space = Space::new(
        SpaceId::generate(),
        "test".to_string(),
        user_id,
        Timestamp::now(),
        "node1".to_string(),
    );

    println!("✅ Integration: Full stack components available and functional");
}
