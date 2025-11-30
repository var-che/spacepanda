/*
    dht_adapter.rs - Bridge between local store and DHT layer
    
    Coordinates synchronization between local CRDT state and DHT storage.
    Handles:
    - Publishing local operations to DHT
    - Fetching remote operations from DHT
    - Delta encoding for efficient sync
    - Conflict resolution
*/

use crate::core_store::crdt::{Crdt, OperationMetadata, VectorClock};
use crate::core_store::model::{SpaceId, ChannelId};
use crate::core_store::store::errors::{StoreResult, StoreError};
use serde::{Serialize, Deserialize};
use std::collections::HashMap;

/// DHT key for a CRDT object
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DhtObjectKey {
    Space(SpaceId),
    Channel(ChannelId),
}

impl DhtObjectKey {
    /// Convert to DHT key bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        bincode::serialize(self).unwrap()
    }
    
    /// Parse from DHT key bytes
    pub fn from_bytes(bytes: &[u8]) -> StoreResult<Self> {
        Ok(bincode::deserialize(bytes)?)
    }
}

/// Delta update to be sent to DHT
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DhtDelta {
    /// Object being updated
    pub key: DhtObjectKey,
    
    /// Operation metadata
    pub metadata: OperationMetadata,
    
    /// Serialized operation data
    pub operation_data: Vec<u8>,
}

/// Adapter for DHT integration
pub struct DhtAdapter {
    /// Pending deltas to publish
    pending_deltas: Vec<DhtDelta>,
    
    /// Last sync vector clock per object
    last_sync: HashMap<DhtObjectKey, VectorClock>,
}

impl DhtAdapter {
    pub fn new() -> Self {
        DhtAdapter {
            pending_deltas: Vec::new(),
            last_sync: HashMap::new(),
        }
    }
    
    /// Queue an operation to be published to DHT
    pub fn queue_delta(
        &mut self,
        key: DhtObjectKey,
        metadata: OperationMetadata,
        operation_data: Vec<u8>,
    ) {
        let delta = DhtDelta {
            key,
            metadata,
            operation_data,
        };
        
        self.pending_deltas.push(delta);
    }
    
    /// Get all pending deltas and clear the queue
    pub fn take_pending_deltas(&mut self) -> Vec<DhtDelta> {
        std::mem::take(&mut self.pending_deltas)
    }
    
    /// Record that we synced an object at a specific vector clock
    pub fn record_sync(&mut self, key: DhtObjectKey, clock: VectorClock) {
        self.last_sync.insert(key, clock);
    }
    
    /// Get the last sync point for an object
    pub fn get_last_sync(&self, key: &DhtObjectKey) -> Option<&VectorClock> {
        self.last_sync.get(key)
    }
    
    /// Check if we need to sync an object
    pub fn needs_sync(&self, key: &DhtObjectKey, current_clock: &VectorClock) -> bool {
        match self.last_sync.get(key) {
            Some(last) => last != current_clock,
            None => true, // Never synced
        }
    }
    
    /// Generate delta between two vector clocks
    pub fn calculate_delta_operations(
        &self,
        _from_clock: &VectorClock,
        _to_clock: &VectorClock,
    ) -> Vec<String> {
        // Simplified: would need VectorClock to expose its internal state
        // or provide a diff method. For now, return empty vec.
        Vec::new()
    }
}

impl Default for DhtAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_dht_adapter_creation() {
        let adapter = DhtAdapter::new();
        assert_eq!(adapter.pending_deltas.len(), 0);
    }
    
    #[test]
    fn test_queue_delta() {
        let mut adapter = DhtAdapter::new();
        
        let key = DhtObjectKey::Space(SpaceId::generate());
        let metadata = OperationMetadata {
            timestamp: 1000,
            vector_clock: VectorClock::new(),
            signature: None,
            node_id: "node1".to_string(),
        };
        
        adapter.queue_delta(key, metadata, vec![1, 2, 3]);
        
        assert_eq!(adapter.pending_deltas.len(), 1);
    }
    
    #[test]
    fn test_take_pending_deltas() {
        let mut adapter = DhtAdapter::new();
        
        let key = DhtObjectKey::Channel(ChannelId::generate());
        let metadata = OperationMetadata {
            timestamp: 1000,
            vector_clock: VectorClock::new(),
            signature: None,
            node_id: "node1".to_string(),
        };
        
        adapter.queue_delta(key, metadata, vec![1, 2, 3]);
        
        let deltas = adapter.take_pending_deltas();
        assert_eq!(deltas.len(), 1);
        assert_eq!(adapter.pending_deltas.len(), 0);
    }
    
    #[test]
    fn test_sync_tracking() {
        let mut adapter = DhtAdapter::new();
        
        let key = DhtObjectKey::Space(SpaceId::generate());
        let clock = VectorClock::new();
        
        assert!(adapter.get_last_sync(&key).is_none());
        
        adapter.record_sync(key.clone(), clock.clone());
        
        assert!(adapter.get_last_sync(&key).is_some());
    }
    
    #[test]
    fn test_needs_sync() {
        let mut adapter = DhtAdapter::new();
        
        let key = DhtObjectKey::Space(SpaceId::generate());
        let mut clock1 = VectorClock::new();
        
        // Never synced
        assert!(adapter.needs_sync(&key, &clock1));
        
        adapter.record_sync(key.clone(), clock1.clone());
        
        // Same clock, no sync needed
        assert!(!adapter.needs_sync(&key, &clock1));
        
        // Different clock, sync needed
        clock1.increment("node1");
        assert!(adapter.needs_sync(&key, &clock1));
    }
    
    #[test]
    fn test_dht_key_serialization() {
        let key = DhtObjectKey::Space(SpaceId::generate());
        
        let bytes = key.to_bytes();
        let parsed = DhtObjectKey::from_bytes(&bytes).unwrap();
        
        assert_eq!(key, parsed);
    }
}
