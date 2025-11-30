/*
    Store + DHT Integration Tests
    
    Tests CRDT state synchronization through DHT storage.
*/

use std::time::Duration;
use spacepanda_core::core_dht::{DhtKey, DhtValue, DhtStorage};
use spacepanda_core::core_store::{ChannelId, ChannelType, UserId, Timestamp};
use spacepanda_core::core_store::model::Channel;

#[tokio::test]
async fn test_store_channel_in_dht() {
    // Create a channel
    let channel_id = ChannelId::generate();
    
    let channel = Channel::new(
        channel_id.clone(),
        "Test Channel".to_string(),
        ChannelType::Text,
        UserId::generate(), // UserId
        Timestamp::now(),
        "node1".to_string(),
    );
    
    // Serialize channel
    let channel_json = serde_json::to_vec(&channel).unwrap();
    
    // Store in DHT
    let dht = DhtStorage::new();
    let channel_key = DhtKey::hash(format!("channel/{}", channel_id).as_bytes());
    let channel_value = DhtValue::new(channel_json.clone());
    
    dht.put(channel_key.clone(), channel_value).unwrap();
    
    // Retrieve from DHT
    let retrieved = dht.get(&channel_key).unwrap();
    assert_eq!(retrieved.data, channel_json);
}

#[tokio::test]
async fn test_dht_value_expiration() {
    let dht = DhtStorage::new();
    
    // Create value with short TTL (1 second)
    let key = DhtKey::hash(b"test_key");
    let mut value = DhtValue::new(vec![1, 2, 3, 4]);
    value = value.with_ttl(1);
    
    dht.put(key.clone(), value).unwrap();
    
    // Should exist immediately
    assert!(dht.get(&key).is_ok());
    
    // Wait for expiration
    tokio::time::sleep(Duration::from_secs(2)).await;
    
    // Should be expired
    assert!(dht.get(&key).is_err());
}

#[tokio::test]
async fn test_multi_node_dht_replication() {
    // Simulate DHT replication between nodes
    let node1_dht = DhtStorage::new();
    let node2_dht = DhtStorage::new();
    
    // Node 1 stores a value
    let key = DhtKey::hash(b"shared_key");
    let value = DhtValue::new(vec![10, 20, 30]);
    node1_dht.put(key.clone(), value.clone()).unwrap();
    
    // Simulate DHT replication to node 2
    node2_dht.put(key.clone(), value).unwrap();
    
    // Both nodes should have the value
    let val1 = node1_dht.get(&key).unwrap();
    let val2 = node2_dht.get(&key).unwrap();
    
    assert_eq!(val1.data, val2.data);
}
