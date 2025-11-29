/*
    Events - event definitions for DHT internal and external notifications

    Events:
    - ValueFound
    - SearchCompleted
    - SearchFailed
    - BucketUpdated
    - PeerExpired
    - KeyReplicated
    - ValueStored

    This is also used for logging, metrics and high level logic.

    Inputs:
    - triggered across subsystems

    Outputs:
    - passed to listeners / subscribers
*/

use serde::{Deserialize, Serialize};
use super::{DhtKey, DhtValue};

/// Events emitted by the DHT subsystem
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DhtEvent {
    /// Value successfully stored locally
    ValueStored {
        key: DhtKey,
    },
    
    /// Value found during search
    ValueFound {
        key: DhtKey,
        value: DhtValue,
    },
    
    /// Search completed successfully
    SearchCompleted {
        key: DhtKey,
        nodes_queried: usize,
    },
    
    /// Search failed to find value
    SearchFailed {
        key: DhtKey,
        reason: String,
    },
    
    /// Routing table bucket updated
    BucketUpdated {
        bucket_index: usize,
        peer_count: usize,
    },
    
    /// Peer expired from routing table
    PeerExpired {
        peer_id: DhtKey,
        reason: String,
    },
    
    /// Key successfully replicated to peer
    KeyReplicated {
        key: DhtKey,
        peer_id: DhtKey,
    },
    
    /// New peer discovered
    PeerDiscovered {
        peer_id: DhtKey,
    },
    
    /// Peer removed from routing table
    PeerRemoved {
        peer_id: DhtKey,
    },
    
    /// Replication round completed
    ReplicationCompleted {
        keys_replicated: usize,
        peers_contacted: usize,
    },
    
    /// Garbage collection completed
    GarbageCollectionCompleted {
        entries_removed: usize,
    },
    
    /// Value validation failed
    ValidationFailed {
        key: DhtKey,
        reason: String,
    },
    
    /// Storage capacity warning
    StorageWarning {
        current_size: usize,
        capacity: usize,
    },
}

impl DhtEvent {
    /// Get event type as string for logging
    pub fn event_type(&self) -> &str {
        match self {
            DhtEvent::ValueStored { .. } => "ValueStored",
            DhtEvent::ValueFound { .. } => "ValueFound",
            DhtEvent::SearchCompleted { .. } => "SearchCompleted",
            DhtEvent::SearchFailed { .. } => "SearchFailed",
            DhtEvent::BucketUpdated { .. } => "BucketUpdated",
            DhtEvent::PeerExpired { .. } => "PeerExpired",
            DhtEvent::KeyReplicated { .. } => "KeyReplicated",
            DhtEvent::PeerDiscovered { .. } => "PeerDiscovered",
            DhtEvent::PeerRemoved { .. } => "PeerRemoved",
            DhtEvent::ReplicationCompleted { .. } => "ReplicationCompleted",
            DhtEvent::GarbageCollectionCompleted { .. } => "GarbageCollectionCompleted",
            DhtEvent::ValidationFailed { .. } => "ValidationFailed",
            DhtEvent::StorageWarning { .. } => "StorageWarning",
        }
    }
    
    /// Check if event is critical (requires immediate attention)
    pub fn is_critical(&self) -> bool {
        matches!(
            self,
            DhtEvent::SearchFailed { .. } | 
            DhtEvent::ValidationFailed { .. } | 
            DhtEvent::StorageWarning { .. }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_type() {
        let key = DhtKey::hash(b"test");
        let event = DhtEvent::ValueStored { key };
        assert_eq!(event.event_type(), "ValueStored");
    }

    #[test]
    fn test_is_critical() {
        let key = DhtKey::hash(b"test");
        
        let critical = DhtEvent::SearchFailed {
            key,
            reason: "timeout".to_string(),
        };
        assert!(critical.is_critical());
        
        let normal = DhtEvent::ValueStored { key };
        assert!(!normal.is_critical());
    }

    #[test]
    fn test_event_type_matching() {
        let key = DhtKey::hash(b"test");
        let event = DhtEvent::PeerDiscovered { peer_id: key };
        
        assert_eq!(event.event_type(), "PeerDiscovered");
    }

    #[test]
    fn test_value_found_event() {
        let key = DhtKey::hash(b"test");
        let value = DhtValue::new(b"data".to_vec()).with_ttl(3600);
        
        let event = DhtEvent::ValueFound {
            key,
            value: value.clone(),
        };
        
        if let DhtEvent::ValueFound { value: v, .. } = event {
            assert_eq!(v.data, b"data");
        } else {
            panic!("Expected ValueFound event");
        }
    }

    #[test]
    fn test_search_completed_event() {
        let key = DhtKey::hash(b"test");
        let event = DhtEvent::SearchCompleted {
            key,
            nodes_queried: 10,
        };
        
        assert_eq!(event.event_type(), "SearchCompleted");
        assert!(!event.is_critical());
    }

    #[test]
    fn test_bucket_updated_event() {
        let event = DhtEvent::BucketUpdated {
            bucket_index: 5,
            peer_count: 20,
        };
        
        assert_eq!(event.event_type(), "BucketUpdated");
    }

    #[test]
    fn test_replication_completed_event() {
        let event = DhtEvent::ReplicationCompleted {
            keys_replicated: 100,
            peers_contacted: 10,
        };
        
        assert_eq!(event.event_type(), "ReplicationCompleted");
    }

    #[test]
    fn test_validation_failed_event() {
        let key = DhtKey::hash(b"test");
        let event = DhtEvent::ValidationFailed {
            key,
            reason: "invalid signature".to_string(),
        };
        
        assert!(event.is_critical());
    }

    #[test]
    fn test_storage_warning_event() {
        let event = DhtEvent::StorageWarning {
            current_size: 900,
            capacity: 1000,
        };
        
        assert!(event.is_critical());
    }

    #[test]
    fn test_garbage_collection_event() {
        let event = DhtEvent::GarbageCollectionCompleted {
            entries_removed: 50,
        };
        
        assert_eq!(event.event_type(), "GarbageCollectionCompleted");
        assert!(!event.is_critical());
    }
}