/*
    Message - defines DHT message types.

    Responsibilities:
    `message.rs` defines the DHT message types used in the Kademlia protocol.
    It is aware of the following message types:

    Request messages:
    - FIND_NODE(target_id)
    - FIND_VALUE(key)
    - STORE_VALUE(key, value)
    - PING

    Response messages:
    - NODES(list of closest nodes)
    - VALUE(value)
    - PONG

    Serialization is done with CBOR or bincode.

    Inputs:
    - outbound/inbound network traffic

    outputs:
    - structured message enums
*/

use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use super::{DhtKey, DhtValue};

/// DHT RPC message types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DhtMessage {
    /// Ping request - check if peer is alive
    Ping {
        sender_id: DhtKey,
        timestamp: u64,
    },
    
    /// Pong response - acknowledge ping
    Pong {
        sender_id: DhtKey,
        timestamp: u64,
    },
    
    /// Find node request - lookup k closest nodes to target
    FindNode {
        sender_id: DhtKey,
        target: DhtKey,
        request_id: u64,
    },
    
    /// Find node response - return closest known nodes
    FindNodeResponse {
        sender_id: DhtKey,
        nodes: Vec<PeerInfo>,
        request_id: u64,
    },
    
    /// Find value request - lookup value by key
    FindValue {
        sender_id: DhtKey,
        key: DhtKey,
        request_id: u64,
    },
    
    /// Find value response - return value or closest nodes
    FindValueResponse {
        sender_id: DhtKey,
        request_id: u64,
        result: FindValueResult,
    },
    
    /// Store value request - store key-value pair
    Store {
        sender_id: DhtKey,
        key: DhtKey,
        value: DhtValue,
        request_id: u64,
    },
    
    /// Store acknowledgment - confirm storage
    StoreAck {
        sender_id: DhtKey,
        success: bool,
        request_id: u64,
        error: Option<String>,
    },
}

/// Result of FindValue RPC
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FindValueResult {
    /// Value found
    Found(DhtValue),
    /// Value not found, returning closest nodes
    NotFound { closest_nodes: Vec<PeerInfo> },
}

/// Peer information for routing
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PeerInfo {
    /// Peer's DHT ID
    pub id: DhtKey,
    /// Network address (e.g., "127.0.0.1:8080")
    pub address: String,
}

impl PeerInfo {
    pub fn new(id: DhtKey, address: String) -> Self {
        PeerInfo { id, address }
    }
}

impl DhtMessage {
    /// Get sender ID from any message
    pub fn sender_id(&self) -> DhtKey {
        match self {
            DhtMessage::Ping { sender_id, .. } => *sender_id,
            DhtMessage::Pong { sender_id, .. } => *sender_id,
            DhtMessage::FindNode { sender_id, .. } => *sender_id,
            DhtMessage::FindNodeResponse { sender_id, .. } => *sender_id,
            DhtMessage::FindValue { sender_id, .. } => *sender_id,
            DhtMessage::FindValueResponse { sender_id, .. } => *sender_id,
            DhtMessage::Store { sender_id, .. } => *sender_id,
            DhtMessage::StoreAck { sender_id, .. } => *sender_id,
        }
    }
    
    /// Get request ID if applicable
    pub fn request_id(&self) -> Option<u64> {
        match self {
            DhtMessage::FindNode { request_id, .. } => Some(*request_id),
            DhtMessage::FindNodeResponse { request_id, .. } => Some(*request_id),
            DhtMessage::FindValue { request_id, .. } => Some(*request_id),
            DhtMessage::FindValueResponse { request_id, .. } => Some(*request_id),
            DhtMessage::Store { request_id, .. } => Some(*request_id),
            DhtMessage::StoreAck { request_id, .. } => Some(*request_id),
            _ => None,
        }
    }
    
    /// Check if message is a request
    pub fn is_request(&self) -> bool {
        matches!(
            self,
            DhtMessage::Ping { .. } |
            DhtMessage::FindNode { .. } |
            DhtMessage::FindValue { .. } |
            DhtMessage::Store { .. }
        )
    }
    
    /// Check if message is a response
    pub fn is_response(&self) -> bool {
        matches!(
            self,
            DhtMessage::Pong { .. } |
            DhtMessage::FindNodeResponse { .. } |
            DhtMessage::FindValueResponse { .. } |
            DhtMessage::StoreAck { .. }
        )
    }
    
    /// Get message type name
    pub fn message_type(&self) -> &str {
        match self {
            DhtMessage::Ping { .. } => "Ping",
            DhtMessage::Pong { .. } => "Pong",
            DhtMessage::FindNode { .. } => "FindNode",
            DhtMessage::FindNodeResponse { .. } => "FindNodeResponse",
            DhtMessage::FindValue { .. } => "FindValue",
            DhtMessage::FindValueResponse { .. } => "FindValueResponse",
            DhtMessage::Store { .. } => "Store",
            DhtMessage::StoreAck { .. } => "StoreAck",
        }
    }
    
    /// Create a ping request
    pub fn new_ping(sender_id: DhtKey) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        DhtMessage::Ping { sender_id, timestamp }
    }
    
    /// Create a pong response
    pub fn new_pong(sender_id: DhtKey) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        DhtMessage::Pong { sender_id, timestamp }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ping_pong() {
        let sender = DhtKey::hash(b"sender");
        let ping = DhtMessage::new_ping(sender);
        let pong = DhtMessage::new_pong(sender);
        
        assert!(ping.is_request());
        assert!(!ping.is_response());
        assert!(!pong.is_request());
        assert!(pong.is_response());
        
        assert_eq!(ping.sender_id(), sender);
        assert_eq!(pong.sender_id(), sender);
    }

    #[test]
    fn test_find_node() {
        let sender = DhtKey::hash(b"sender");
        let target = DhtKey::hash(b"target");
        
        let msg = DhtMessage::FindNode {
            sender_id: sender,
            target,
            request_id: 123,
        };
        
        assert!(msg.is_request());
        assert_eq!(msg.sender_id(), sender);
        assert_eq!(msg.request_id(), Some(123));
        assert_eq!(msg.message_type(), "FindNode");
    }

    #[test]
    fn test_find_node_response() {
        let sender = DhtKey::hash(b"sender");
        let peer1 = PeerInfo::new(DhtKey::hash(b"peer1"), "127.0.0.1:8001".to_string());
        let peer2 = PeerInfo::new(DhtKey::hash(b"peer2"), "127.0.0.1:8002".to_string());
        
        let msg = DhtMessage::FindNodeResponse {
            sender_id: sender,
            nodes: vec![peer1, peer2],
            request_id: 123,
        };
        
        assert!(msg.is_response());
        assert_eq!(msg.request_id(), Some(123));
    }

    #[test]
    fn test_find_value() {
        let sender = DhtKey::hash(b"sender");
        let key = DhtKey::hash(b"key");
        
        let msg = DhtMessage::FindValue {
            sender_id: sender,
            key,
            request_id: 456,
        };
        
        assert!(msg.is_request());
        assert_eq!(msg.request_id(), Some(456));
    }

    #[test]
    fn test_find_value_response_found() {
        let sender = DhtKey::hash(b"sender");
        let value = DhtValue::new(b"data".to_vec()).with_ttl(3600);
        
        let msg = DhtMessage::FindValueResponse {
            sender_id: sender,
            request_id: 456,
            result: FindValueResult::Found(value.clone()),
        };
        
        assert!(msg.is_response());
        
        if let DhtMessage::FindValueResponse { result: FindValueResult::Found(v), .. } = msg {
            assert_eq!(v.data, b"data");
        } else {
            panic!("Expected FindValueResponse with Found");
        }
    }

    #[test]
    fn test_find_value_response_not_found() {
        let sender = DhtKey::hash(b"sender");
        let peer1 = PeerInfo::new(DhtKey::hash(b"peer1"), "127.0.0.1:8001".to_string());
        
        let msg = DhtMessage::FindValueResponse {
            sender_id: sender,
            request_id: 456,
            result: FindValueResult::NotFound {
                closest_nodes: vec![peer1],
            },
        };
        
        assert!(msg.is_response());
    }

    #[test]
    fn test_store() {
        let sender = DhtKey::hash(b"sender");
        let key = DhtKey::hash(b"key");
        let value = DhtValue::new(b"data".to_vec()).with_ttl(3600);
        
        let msg = DhtMessage::Store {
            sender_id: sender,
            key,
            value,
            request_id: 789,
        };
        
        assert!(msg.is_request());
        assert_eq!(msg.request_id(), Some(789));
    }

    #[test]
    fn test_store_ack_success() {
        let sender = DhtKey::hash(b"sender");
        
        let msg = DhtMessage::StoreAck {
            sender_id: sender,
            success: true,
            request_id: 789,
            error: None,
        };
        
        assert!(msg.is_response());
        
        if let DhtMessage::StoreAck { success, error, .. } = msg {
            assert!(success);
            assert!(error.is_none());
        }
    }

    #[test]
    fn test_store_ack_failure() {
        let sender = DhtKey::hash(b"sender");
        
        let msg = DhtMessage::StoreAck {
            sender_id: sender,
            success: false,
            request_id: 789,
            error: Some("storage full".to_string()),
        };
        
        if let DhtMessage::StoreAck { success, error, .. } = msg {
            assert!(!success);
            assert_eq!(error.unwrap(), "storage full");
        }
    }

    #[test]
    fn test_message_type_consistency() {
        let sender = DhtKey::hash(b"sender");
        let msg = DhtMessage::new_ping(sender);
        
        assert_eq!(msg.message_type(), "Ping");
        assert_eq!(msg.sender_id(), sender);
    }

    #[test]
    fn test_peer_info() {
        let id = DhtKey::hash(b"peer");
        let addr = "192.168.1.1:8080".to_string();
        
        let peer = PeerInfo::new(id, addr.clone());
        
        assert_eq!(peer.id, id);
        assert_eq!(peer.address, addr);
    }

    #[test]
    fn test_peer_info_equality() {
        let id = DhtKey::hash(b"peer");
        let peer1 = PeerInfo::new(id, "127.0.0.1:8080".to_string());
        let peer2 = PeerInfo::new(id, "127.0.0.1:8080".to_string());
        
        assert_eq!(peer1, peer2);
    }
}
