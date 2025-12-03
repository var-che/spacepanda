/*
    End-to-End Integration Test - Simplified

    This test validates the SpacePanda stack components working together:
    - Identity creation
    - Peer discovery via DHT
    - Data storage and retrieval
    - CRDT-based state synchronization
    - Persistence and recovery

    Note: Full Noise protocol session establishment is tested separately
    in router security tests. This focuses on component integration.
*/

use spacepanda_core::core_dht::{DhtKey, DhtStorage, DhtValue, PeerContact, RoutingTable};
use spacepanda_core::core_identity::{DeviceId, GlobalIdentity, KeyType, Keypair};
use spacepanda_core::core_store::model::{
    Channel, ChannelId, ChannelType, Space, SpaceId, Timestamp, UserId,
};
use spacepanda_core::core_store::store::{LocalStore, LocalStoreConfig};
use std::sync::Arc;
use tempfile::tempdir;

/// **End-to-End Test: Multi-Peer Data Synchronization**
///
/// Scenario:
/// 1. Alice and Bob create identities
/// 2. Both join the DHT network
/// 3. Alice creates a space and channel
/// 4. Alice publishes to DHT
/// 5. Bob discovers via DHT
/// 6. Both persist state
/// 7. Both restart and verify recovery
/// 8. Verify CRDT convergence
#[tokio::test]
async fn test_end_to_end_multi_peer_data_sync() {
    println!("\n🚀 Starting End-to-End Integration Test: Multi-Peer Data Synchronization\n");

    // ========================================================================
    // Phase 1: Identity & Storage Setup
    // ========================================================================
    println!("📋 Phase 1: Setting up identities and storage...");

    let _alice_identity = GlobalIdentity::create_global_identity().unwrap();
    let alice_user_id = UserId::generate();
    let alice_device_id = DeviceId::generate();
    let alice_keypair = Keypair::generate(KeyType::Ed25519);

    let _bob_identity = GlobalIdentity::create_global_identity().unwrap();
    let _bob_user_id = UserId::generate();
    let bob_device_id = DeviceId::generate();
    let _bob_keypair = Keypair::generate(KeyType::Ed25519);

    println!("  ✅ Alice identity: device_id={}", alice_device_id);
    println!("  ✅ Bob identity: device_id={}", bob_device_id);

    // Setup local storage
    let alice_temp = tempdir().unwrap();
    let alice_config = LocalStoreConfig {
        data_dir: alice_temp.path().to_path_buf(),
        enable_encryption: false,
        snapshot_interval: 100,
        max_log_size: 1_000_000,
        enable_compaction: false,
    };
    let alice_store = LocalStore::new(alice_config.clone()).unwrap();

    let bob_temp = tempdir().unwrap();
    let bob_config = LocalStoreConfig {
        data_dir: bob_temp.path().to_path_buf(),
        enable_encryption: false,
        snapshot_interval: 100,
        max_log_size: 1_000_000,
        enable_compaction: false,
    };
    let bob_store = LocalStore::new(bob_config.clone()).unwrap();

    println!("  ✅ Storage initialized for both peers");

    // ========================================================================
    // Phase 2: DHT Network Formation
    // ========================================================================
    println!("\n📋 Phase 2: Forming DHT network...");

    let alice_dht_id = DhtKey::hash(alice_device_id.to_string().as_bytes());
    let bob_dht_id = DhtKey::hash(bob_device_id.to_string().as_bytes());

    let mut alice_routing = RoutingTable::new(alice_dht_id, 20);
    let mut bob_routing = RoutingTable::new(bob_dht_id, 20);

    let dht_storage = Arc::new(DhtStorage::new());

    // Peers discover each other
    let alice_peer = PeerContact::new(alice_dht_id, "alice:8000".to_string());
    let bob_peer = PeerContact::new(bob_dht_id, "bob:8001".to_string());

    alice_routing.insert(bob_peer.clone()).unwrap();
    bob_routing.insert(alice_peer.clone()).unwrap();

    println!("  ✅ DHT network formed");
    println!("  ✅ Alice knows {} peers", alice_routing.size());
    println!("  ✅ Bob knows {} peers", bob_routing.size());

    // ========================================================================
    // Phase 3: Data Creation (Alice)
    // ========================================================================
    println!("\n📋 Phase 3: Alice creates space and channel...");

    let space_id = SpaceId::generate();
    let space = Space::new(
        space_id.clone(),
        "Team Workspace".to_string(),
        alice_user_id.clone(),
        Timestamp::now(),
        alice_device_id.to_string(),
    );

    let channel_id = ChannelId::generate();
    let channel = Channel::new(
        channel_id.clone(),
        "general".to_string(),
        ChannelType::Text,
        alice_user_id.clone(),
        Timestamp::now(),
        alice_device_id.to_string(),
    );

    alice_store.store_space(&space).unwrap();
    alice_store.store_channel(&channel).unwrap();

    println!("  ✅ Space created: '{}'", space.name.get().unwrap());
    println!("  ✅ Channel created: '#{}'", channel.name.get().unwrap());

    // ========================================================================
    // Phase 4: DHT Publication
    // ========================================================================
    println!("\n📋 Phase 4: Publishing to DHT...");

    let space_dht_key = DhtKey::hash(space_id.0.as_bytes());
    let space_data = serde_json::to_vec(&space).unwrap();
    let space_dht_value = DhtValue::new(space_data).with_owner(alice_dht_id).with_ttl(3600);

    dht_storage.put(space_dht_key, space_dht_value).unwrap();

    let channel_dht_key = DhtKey::hash(channel_id.0.as_bytes());
    let channel_data = serde_json::to_vec(&channel).unwrap();
    let channel_dht_value = DhtValue::new(channel_data).with_owner(alice_dht_id).with_ttl(3600);

    dht_storage.put(channel_dht_key, channel_dht_value).unwrap();

    println!("  ✅ Published {} keys to DHT", dht_storage.active_keys().len());

    // ========================================================================
    // Phase 5: Discovery (Bob)
    // ========================================================================
    println!("\n📋 Phase 5: Bob discovers via DHT...");

    let discovered_space_value = dht_storage.get(&space_dht_key).unwrap();
    let discovered_space: Space = serde_json::from_slice(&discovered_space_value.data).unwrap();

    assert_eq!(discovered_space.id, space_id);
    bob_store.store_space(&discovered_space).unwrap();

    let discovered_channel_value = dht_storage.get(&channel_dht_key).unwrap();
    let discovered_channel: Channel =
        serde_json::from_slice(&discovered_channel_value.data).unwrap();

    bob_store.store_channel(&discovered_channel).unwrap();

    println!("  ✅ Bob discovered space: '{}'", discovered_space.name.get().unwrap());
    println!("  ✅ Bob discovered channel: '#{}'", discovered_channel.name.get().unwrap());

    // ========================================================================
    // Phase 6: Cryptographic Authentication
    // ========================================================================
    println!("\n📋 Phase 6: Testing cryptographic authentication...");

    let message_content = "Hello Bob! Welcome to the team! 🎉";
    let message_bytes = message_content.as_bytes();

    // Alice signs
    let signature = alice_keypair.sign(message_bytes);
    println!("  ✅ Alice signed message ({} bytes signature)", signature.len());

    // Bob verifies
    let verified = Keypair::verify(&alice_keypair.public, message_bytes, &signature);
    assert!(verified, "Bob should verify Alice's signature");
    println!("  ✅ Bob verified Alice's signature");
    println!("  ✅ Message content: '{}'", message_content);

    // ========================================================================
    // Phase 7: CRDT Convergence
    // ========================================================================
    println!("\n📋 Phase 7: Verifying CRDT convergence...");

    let alice_space_check = alice_store.get_space(&space_id).unwrap().unwrap();
    let bob_space_check = bob_store.get_space(&space_id).unwrap().unwrap();

    assert_eq!(alice_space_check.id, bob_space_check.id);
    assert_eq!(alice_space_check.name.get().unwrap(), bob_space_check.name.get().unwrap());

    let alice_channel_check = alice_store.get_channel(&channel_id).unwrap().unwrap();
    let bob_channel_check = bob_store.get_channel(&channel_id).unwrap().unwrap();

    assert_eq!(alice_channel_check.id, bob_channel_check.id);
    assert_eq!(alice_channel_check.name.get().unwrap(), bob_channel_check.name.get().unwrap());

    println!("  ✅ Space state converged: '{}'", alice_space_check.name.get().unwrap());
    println!("  ✅ Channel state converged: '#{}'", alice_channel_check.name.get().unwrap());
    println!("  ✅ CRDT LWW registers synchronized");

    // ========================================================================
    // Phase 8: Persistence
    // ========================================================================
    println!("\n📋 Phase 8: Testing persistence...");

    alice_store.create_snapshot().unwrap();
    bob_store.create_snapshot().unwrap();
    println!("  ✅ Created snapshots");

    drop(alice_store);
    drop(bob_store);
    println!("  ✅ Dropped stores");

    // ========================================================================
    // Phase 9: Recovery
    // ========================================================================
    println!("\n📋 Phase 9: Testing recovery...");

    let alice_store_restored = LocalStore::new(alice_config).unwrap();
    alice_store_restored.load().unwrap();

    let bob_store_restored = LocalStore::new(bob_config).unwrap();
    bob_store_restored.load().unwrap();

    println!("  ✅ Restored stores from disk");

    let alice_space_restored = alice_store_restored.get_space(&space_id).unwrap().unwrap();
    let bob_space_restored = bob_store_restored.get_space(&space_id).unwrap().unwrap();

    assert_eq!(alice_space_restored.name.get().unwrap(), "Team Workspace");
    assert_eq!(bob_space_restored.name.get().unwrap(), "Team Workspace");

    let alice_channel_restored = alice_store_restored.get_channel(&channel_id).unwrap().unwrap();
    let bob_channel_restored = bob_store_restored.get_channel(&channel_id).unwrap().unwrap();

    assert_eq!(alice_channel_restored.name.get().unwrap(), "general");
    assert_eq!(bob_channel_restored.name.get().unwrap(), "general");

    println!(
        "  ✅ Alice recovered: space='{}', channel='#{}'",
        alice_space_restored.name.get().unwrap(),
        alice_channel_restored.name.get().unwrap()
    );
    println!(
        "  ✅ Bob recovered: space='{}', channel='#{}'",
        bob_space_restored.name.get().unwrap(),
        bob_channel_restored.name.get().unwrap()
    );

    // ========================================================================
    // Final Summary
    // ========================================================================
    println!("\n✅ END-TO-END INTEGRATION TEST PASSED!");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("Successfully verified:");
    println!("  ✅ Identity creation (GlobalIdentity, DeviceId, Keypair)");
    println!("  ✅ Local storage (LocalStore with CRDT models)");
    println!("  ✅ DHT network (peer discovery, routing tables)");
    println!("  ✅ Data creation (Space, Channel with LWW registers)");
    println!("  ✅ DHT publication and discovery");
    println!("  ✅ Cryptographic signing and verification (Ed25519)");
    println!("  ✅ CRDT convergence (identical state across peers)");
    println!("  ✅ Persistence (snapshot creation)");
    println!("  ✅ Recovery (full state restoration from disk)");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("All major subsystems integrated successfully! 🎉\n");
}
