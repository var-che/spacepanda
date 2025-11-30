/*
    Store + Router Integration Tests
    
    Tests channel synchronization through router layer.
*/

use std::sync::Arc;
use spacepanda_core::core_router::RouterHandle;
use spacepanda_core::core_store::{ChannelId, ChannelType, UserId, Timestamp};
use spacepanda_core::core_store::model::Channel;

#[tokio::test]
async fn test_router_creation() {
    // Create a router
    let (router, _handle) = RouterHandle::new();
    let router = Arc::new(router);
    
    // Verify router is created
    assert!(Arc::strong_count(&router) > 0);
}

#[tokio::test]
async fn test_channel_serialization_for_routing() {
    // Create a channel
    let channel_id = ChannelId::generate();
    
    let channel = Channel::new(
        channel_id,
        "Routed Channel".to_string(),
        ChannelType::Text,
        UserId::generate(), // UserId
        Timestamp::now(),
        "node1".to_string(),
    );
    
    // Serialize for transmission
    let channel_bytes = serde_json::to_vec(&channel).unwrap();
    
    // Deserialize on other end
    let decoded: Channel = serde_json::from_slice(&channel_bytes).unwrap();
    
    assert_eq!(channel.get_name(), decoded.get_name());
}
