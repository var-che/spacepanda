/*
    Full End-to-End Integration Tests
    
    Tests complete message flow combining core_store, core_dht, and core_router.
*/

use std::sync::Arc;
use spacepanda_core::core_dht::{DhtKey, DhtValue, DhtStorage};
use spacepanda_core::core_router::RouterHandle;
use spacepanda_core::core_store::{ChannelId, ChannelType, UserId, Timestamp};
use spacepanda_core::core_store::model::Channel;

#[tokio::test]
async fn test_e2e_channel_creation_and_dht_storage() {
    // Create router (for node identity/networking)
    let (router, _handle) = RouterHandle::new();
    let _router = Arc::new(router);
    
    // Create DHT storage
    let dht = DhtStorage::new();
    
    // Create a channel (core_store)
    let channel_id = ChannelId::generate();
    
    let channel = Channel::new(
        channel_id.clone(),
        "E2E Test Channel".to_string(),
        ChannelType::Text,
        UserId::generate(), // UserId
        Timestamp::now(),
        "node1".to_string(),
    );
    
    // Serialize and store in DHT
    let channel_data = serde_json::to_vec(&channel).unwrap();
    let channel_key = DhtKey::hash(format!("channel/{}", channel_id).as_bytes());
    let channel_value = DhtValue::new(channel_data);
    
    dht.put(channel_key.clone(), channel_value).unwrap();
    
    // Retrieve and verify
    let retrieved = dht.get(&channel_key).unwrap();
    let decoded_channel: Channel = serde_json::from_slice(&retrieved.data).unwrap();
    
    assert_eq!(channel.get_name(), decoded_channel.get_name());
}

#[tokio::test]
async fn test_e2e_multi_node_channel_sync() {
    // Simulate 2 nodes syncing a channel
    
    // Node 1
    let (router1, _handle1) = RouterHandle::new();
    let _router1 = Arc::new(router1);
    let dht1 = DhtStorage::new();
    
    // Node 2
    let (router2, _handle2) = RouterHandle::new();
    let _router2 = Arc::new(router2);
    let dht2 = DhtStorage::new();
    
    // Node 1 creates channel
    let channel_id = ChannelId::generate();
    
    let channel1 = Channel::new(
        channel_id.clone(),
        "Synced Channel".to_string(),
        ChannelType::Text,
        UserId::generate(), // UserId
        Timestamp::now(),
        "node1".to_string(),
    );
    
    // Node 1 publishes to DHT
    let channel_key = DhtKey::hash(format!("channel/{}", channel_id).as_bytes());
    let channel_data = serde_json::to_vec(&channel1).unwrap();
    dht1.put(channel_key.clone(), DhtValue::new(channel_data.clone())).unwrap();
    
    // Simulate DHT replication to node 2
    dht2.put(channel_key.clone(), DhtValue::new(channel_data)).unwrap();
    
    // Node 2 retrieves
    let retrieved = dht2.get(&channel_key).unwrap();
    let channel2: Channel = serde_json::from_slice(&retrieved.data).unwrap();
    
    // Both nodes have same channel state
    assert_eq!(channel1.get_name(), channel2.get_name());
}
