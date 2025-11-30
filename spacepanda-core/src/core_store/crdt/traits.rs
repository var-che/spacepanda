/*
    traits.rs - Core CRDT trait definitions
    
    Defines the unified interface that all CRDT types must implement:
    - Apply operations
    - Merge with other replicas
    - Query current state
*/

use super::vector_clock::VectorClock;
use crate::core_store::store::errors::StoreResult;
use serde::{Deserialize, Serialize};

/// Core trait that all CRDTs must implement
pub trait Crdt: Clone + Send + Sync {
    /// The type of operations this CRDT accepts
    type Operation: Clone + Send + Sync;
    
    /// The type of value this CRDT represents
    type Value: Clone;
    
    /// Apply a local operation to this CRDT
    /// This is called when the local node performs an action
    fn apply(&mut self, op: Self::Operation) -> StoreResult<()>;
    
    /// Merge another CRDT state into this one
    /// This is called when receiving state from a remote peer
    fn merge(&mut self, other: &Self) -> StoreResult<()>;
    
    /// Get the current value/state
    fn value(&self) -> Self::Value;
    
    /// Get the vector clock for causal ordering
    fn vector_clock(&self) -> &VectorClock;
}

/// Trait for CRDTs that can be validated before applying
pub trait ValidatedCrdt: Crdt {
    /// Validate an operation before applying it
    fn validate(&self, op: &Self::Operation) -> StoreResult<()>;
}

/// Trait for CRDTs that support tombstones (deletion markers)
pub trait TombstoneCrdt: Crdt {
    /// Check if an element is tombstoned (deleted but retained for sync)
    fn is_tombstoned(&self, key: &str) -> bool;
    
    /// Garbage collect tombstones older than the given threshold
    fn gc_tombstones(&mut self, threshold_ms: u64) -> usize;
}

/// Metadata attached to every CRDT operation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OperationMetadata {
    /// Node ID that created this operation
    pub node_id: String,
    
    /// Vector clock at the time of creation
    pub vector_clock: VectorClock,
    
    /// Timestamp in milliseconds since epoch
    pub timestamp: u64,
    
    /// Optional signature over the operation
    pub signature: Option<Vec<u8>>,
}

impl OperationMetadata {
    pub fn new(node_id: String, vector_clock: VectorClock) -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        
        OperationMetadata {
            node_id,
            vector_clock,
            timestamp,
            signature: None,
        }
    }
    
    pub fn with_signature(mut self, signature: Vec<u8>) -> Self {
        self.signature = Some(signature);
        self
    }
}

/// Generic CRDT operation wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrdtOperation<T> {
    /// The actual operation data
    pub data: T,
    
    /// Metadata for causal ordering and validation
    pub metadata: OperationMetadata,
}

impl<T> CrdtOperation<T> {
    pub fn new(data: T, metadata: OperationMetadata) -> Self {
        CrdtOperation { data, metadata }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_operation_metadata_creation() {
        let vc = VectorClock::new();
        let metadata = OperationMetadata::new("node1".to_string(), vc.clone());
        
        assert_eq!(metadata.node_id, "node1");
        assert_eq!(metadata.vector_clock, vc);
        assert!(metadata.timestamp > 0);
        assert!(metadata.signature.is_none());
    }
    
    #[test]
    fn test_operation_metadata_with_signature() {
        let vc = VectorClock::new();
        let sig = vec![1, 2, 3, 4];
        let metadata = OperationMetadata::new("node1".to_string(), vc)
            .with_signature(sig.clone());
        
        assert_eq!(metadata.signature, Some(sig));
    }
    
    #[test]
    fn test_crdt_operation_creation() {
        let vc = VectorClock::new();
        let metadata = OperationMetadata::new("node1".to_string(), vc);
        let op = CrdtOperation::new("test_data".to_string(), metadata.clone());
        
        assert_eq!(op.data, "test_data");
        assert_eq!(op.metadata, metadata);
    }
}
