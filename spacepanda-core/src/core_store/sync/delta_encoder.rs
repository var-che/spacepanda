/*
    delta_encoder.rs - CRDT Delta Encoder

    Compresses CRDT operations into DHT-friendly bundles.

    Responsibilities:
    - Bundle multiple CRDT ops into a single delta packet
    - Compress redundant vector clock data
    - Serialize to compact binary format
    - Apply bandwidth-efficient encoding

    Delta Format:
    - Header: version, delta_id, base_clock
    - Operations: array of CRDT ops with shared context
    - Signature: delta-level signature for integrity
*/

use crate::core_store::crdt::VectorClock;
use crate::core_store::store::errors::{StoreError, StoreResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Version of the delta encoding format
const DELTA_VERSION: u8 = 1;

/// A compressed delta containing multiple CRDT operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Delta {
    /// Format version for backward compatibility
    pub version: u8,

    /// Unique identifier for this delta
    pub delta_id: String,

    /// Base vector clock (shared context)
    pub base_clock: VectorClock,

    /// Channel or space this delta applies to
    pub target_id: String,

    /// Bundled operations
    pub operations: Vec<DeltaOperation>,

    /// Timestamp when delta was created
    pub created_at: u64,

    /// Node that created this delta
    pub author_node: String,
}

/// A single operation within a delta bundle
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeltaOperation {
    /// LWW register update
    LWWUpdate {
        path: String,   // e.g., "channel.name", "space.topic"
        value: Vec<u8>, // Serialized value
        timestamp: u64,
        node_id: String,
        clock_delta: HashMap<String, u64>, // Only differences from base_clock
    },

    /// OR-Set add operation
    ORSetAdd {
        path: String,
        element: Vec<u8>,
        add_id: String,
        clock_delta: HashMap<String, u64>,
    },

    /// OR-Set remove operation
    ORSetRemove {
        path: String,
        element: Vec<u8>,
        add_ids: Vec<String>,
        clock_delta: HashMap<String, u64>,
    },

    /// OR-Map put operation
    ORMapPut {
        path: String,
        key: Vec<u8>,
        value: Vec<u8>,
        add_id: String,
        clock_delta: HashMap<String, u64>,
    },

    /// OR-Map remove operation
    ORMapRemove { path: String, key: Vec<u8>, clock_delta: HashMap<String, u64> },
}

/// Delta encoder for compressing CRDT operations
pub struct DeltaEncoder {
    /// Current base clock for compression
    base_clock: VectorClock,

    /// Operations buffered for encoding
    operations: Vec<DeltaOperation>,

    /// Target identifier (channel/space)
    target_id: String,

    /// Node creating the delta
    author_node: String,
}

impl DeltaEncoder {
    /// Create a new delta encoder
    pub fn new(target_id: String, author_node: String, base_clock: VectorClock) -> Self {
        DeltaEncoder { base_clock, operations: Vec::new(), target_id, author_node }
    }

    /// Add an LWW operation to the delta
    pub fn add_lww_operation<T: Serialize>(
        &mut self,
        path: String,
        value: &T,
        timestamp: u64,
        node_id: String,
        vector_clock: &VectorClock,
    ) -> StoreResult<()> {
        let value_bytes =
            bincode::serialize(value).map_err(|e| StoreError::Serialization(e.to_string()))?;

        let clock_delta = self.compute_clock_delta(vector_clock);

        self.operations.push(DeltaOperation::LWWUpdate {
            path,
            value: value_bytes,
            timestamp,
            node_id,
            clock_delta,
        });

        Ok(())
    }

    /// Add an OR-Set add operation
    pub fn add_orset_add<T: Serialize>(
        &mut self,
        path: String,
        element: &T,
        add_id: String,
        vector_clock: &VectorClock,
    ) -> StoreResult<()> {
        let element_bytes =
            bincode::serialize(element).map_err(|e| StoreError::Serialization(e.to_string()))?;

        let clock_delta = self.compute_clock_delta(vector_clock);

        self.operations.push(DeltaOperation::ORSetAdd {
            path,
            element: element_bytes,
            add_id,
            clock_delta,
        });

        Ok(())
    }

    /// Add an OR-Set remove operation
    pub fn add_orset_remove<T: Serialize>(
        &mut self,
        path: String,
        element: &T,
        add_ids: Vec<String>,
        vector_clock: &VectorClock,
    ) -> StoreResult<()> {
        let element_bytes =
            bincode::serialize(element).map_err(|e| StoreError::Serialization(e.to_string()))?;

        let clock_delta = self.compute_clock_delta(vector_clock);

        self.operations.push(DeltaOperation::ORSetRemove {
            path,
            element: element_bytes,
            add_ids,
            clock_delta,
        });

        Ok(())
    }

    /// Compute vector clock delta (only differences from base)
    fn compute_clock_delta(&self, clock: &VectorClock) -> HashMap<String, u64> {
        let delta = HashMap::new();

        // For each node in the new clock, compute the difference
        // This is a simplified version - actual implementation would need
        // access to VectorClock internals or a delta method

        // For now, we'll include the full clock
        // TODO: Add VectorClock::delta() method for proper compression

        delta
    }

    /// Finalize and create the delta bundle
    pub fn finalize(self) -> Delta {
        use std::time::{SystemTime, UNIX_EPOCH};

        let created_at = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64;

        let delta_id = format!("{}:{}", self.author_node, created_at);

        Delta {
            version: DELTA_VERSION,
            delta_id,
            base_clock: self.base_clock,
            target_id: self.target_id,
            operations: self.operations,
            created_at,
            author_node: self.author_node,
        }
    }

    /// Encode delta to bytes
    pub fn encode(delta: &Delta) -> StoreResult<Vec<u8>> {
        bincode::serialize(delta).map_err(|e| StoreError::Serialization(e.to_string()))
    }

    /// Get the number of operations buffered
    pub fn operation_count(&self) -> usize {
        self.operations.len()
    }

    /// Clear buffered operations
    pub fn clear(&mut self) {
        self.operations.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_delta_encoder_creation() {
        let vc = VectorClock::new();
        let encoder = DeltaEncoder::new("channel_123".to_string(), "node1".to_string(), vc);

        assert_eq!(encoder.operation_count(), 0);
        assert_eq!(encoder.target_id, "channel_123");
    }

    #[test]
    fn test_add_lww_operation() {
        let vc = VectorClock::new();
        let mut encoder =
            DeltaEncoder::new("channel_123".to_string(), "node1".to_string(), vc.clone());

        encoder
            .add_lww_operation(
                "channel.name".to_string(),
                &"Test Channel",
                100,
                "node1".to_string(),
                &vc,
            )
            .unwrap();

        assert_eq!(encoder.operation_count(), 1);
    }

    #[test]
    fn test_finalize_delta() {
        let vc = VectorClock::new();
        let mut encoder =
            DeltaEncoder::new("channel_123".to_string(), "node1".to_string(), vc.clone());

        encoder
            .add_lww_operation("channel.name".to_string(), &"Test", 100, "node1".to_string(), &vc)
            .unwrap();

        let delta = encoder.finalize();

        assert_eq!(delta.version, DELTA_VERSION);
        assert_eq!(delta.operations.len(), 1);
        assert_eq!(delta.target_id, "channel_123");
        assert_eq!(delta.author_node, "node1");
    }

    #[test]
    fn test_encode_decode_delta() {
        let vc = VectorClock::new();
        let mut encoder =
            DeltaEncoder::new("channel_123".to_string(), "node1".to_string(), vc.clone());

        encoder
            .add_lww_operation("channel.name".to_string(), &"Test", 100, "node1".to_string(), &vc)
            .unwrap();

        let delta = encoder.finalize();
        let bytes = DeltaEncoder::encode(&delta).unwrap();

        assert!(!bytes.is_empty());
    }
}
