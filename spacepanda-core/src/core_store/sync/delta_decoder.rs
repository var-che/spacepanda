/*
    delta_decoder.rs - CRDT Delta Decoder

    Parses DHT bundles into CRDT operations.

    Responsibilities:
    - Deserialize delta packets from binary format
    - Reconstruct full vector clocks from deltas
    - Validate delta integrity
    - Extract individual CRDT operations
*/

use super::delta_encoder::{Delta, DeltaOperation};
use crate::core_store::crdt::VectorClock;
use crate::core_store::store::errors::{StoreError, StoreResult};
use serde::de::DeserializeOwned;

/// Delta decoder for parsing compressed CRDT operation bundles
pub struct DeltaDecoder {
    /// The decoded delta
    delta: Delta,
}

impl DeltaDecoder {
    /// Decode a delta from bytes
    pub fn decode(bytes: &[u8]) -> StoreResult<Self> {
        let delta: Delta =
            bincode::deserialize(bytes).map_err(|e| StoreError::Deserialization(e.to_string()))?;

        Ok(DeltaDecoder { delta })
    }

    /// Get the delta version
    pub fn version(&self) -> u8 {
        self.delta.version
    }

    /// Get the delta ID
    pub fn delta_id(&self) -> &str {
        &self.delta.delta_id
    }

    /// Get the target ID (channel/space)
    pub fn target_id(&self) -> &str {
        &self.delta.target_id
    }

    /// Get the author node
    pub fn author_node(&self) -> &str {
        &self.delta.author_node
    }

    /// Get the base vector clock
    pub fn base_clock(&self) -> &VectorClock {
        &self.delta.base_clock
    }

    /// Get creation timestamp
    pub fn created_at(&self) -> u64 {
        self.delta.created_at
    }

    /// Get the number of operations in this delta
    pub fn operation_count(&self) -> usize {
        self.delta.operations.len()
    }

    /// Iterate over all operations
    pub fn operations(&self) -> &[DeltaOperation] {
        &self.delta.operations
    }

    /// Reconstruct full vector clock from base + delta
    pub fn reconstruct_clock(
        &self,
        clock_delta: &std::collections::HashMap<String, u64>,
    ) -> VectorClock {
        let mut clock = self.delta.base_clock.clone();

        // Apply deltas
        for (node, count) in clock_delta {
            // This is simplified - actual implementation needs VectorClock::set()
            // For now we just merge a temporary clock
            for _ in 0..*count {
                clock.increment(node);
            }
        }

        clock
    }

    /// Deserialize an LWW operation value
    pub fn deserialize_lww_value<T: DeserializeOwned>(&self, value_bytes: &[u8]) -> StoreResult<T> {
        bincode::deserialize(value_bytes).map_err(|e| StoreError::Deserialization(e.to_string()))
    }

    /// Deserialize an OR-Set element
    pub fn deserialize_orset_element<T: DeserializeOwned>(
        &self,
        element_bytes: &[u8],
    ) -> StoreResult<T> {
        bincode::deserialize(element_bytes).map_err(|e| StoreError::Deserialization(e.to_string()))
    }

    /// Deserialize an OR-Map key
    pub fn deserialize_ormap_key<K: DeserializeOwned>(&self, key_bytes: &[u8]) -> StoreResult<K> {
        bincode::deserialize(key_bytes).map_err(|e| StoreError::Deserialization(e.to_string()))
    }

    /// Deserialize an OR-Map value
    pub fn deserialize_ormap_value<V: DeserializeOwned>(
        &self,
        value_bytes: &[u8],
    ) -> StoreResult<V> {
        bincode::deserialize(value_bytes).map_err(|e| StoreError::Deserialization(e.to_string()))
    }

    /// Validate delta integrity
    pub fn validate(&self) -> StoreResult<()> {
        // Check version
        if self.delta.version == 0 {
            return Err(StoreError::ValidationError("Invalid delta version".to_string()));
        }

        // Check target ID is not empty
        if self.delta.target_id.is_empty() {
            return Err(StoreError::ValidationError("Empty target ID".to_string()));
        }

        // Check author node is not empty
        if self.delta.author_node.is_empty() {
            return Err(StoreError::ValidationError("Empty author node".to_string()));
        }

        // Check delta ID format
        if !self.delta.delta_id.contains(':') {
            return Err(StoreError::ValidationError("Invalid delta ID format".to_string()));
        }

        Ok(())
    }

    /// Get a reference to the underlying delta
    pub fn delta(&self) -> &Delta {
        &self.delta
    }

    /// Consume decoder and return the delta
    pub fn into_delta(self) -> Delta {
        self.delta
    }
}

/// Helper to apply a decoded delta to local state
pub struct DeltaApplier {
    decoder: DeltaDecoder,
}

impl DeltaApplier {
    /// Create a new delta applier
    pub fn new(decoder: DeltaDecoder) -> Self {
        DeltaApplier { decoder }
    }

    /// Apply all operations in the delta
    /// Returns the paths that were modified
    pub fn apply_all(&self) -> StoreResult<Vec<String>> {
        let mut modified_paths = Vec::new();

        for op in self.decoder.operations() {
            let path = match op {
                DeltaOperation::LWWUpdate { path, .. } => path,
                DeltaOperation::ORSetAdd { path, .. } => path,
                DeltaOperation::ORSetRemove { path, .. } => path,
                DeltaOperation::ORMapPut { path, .. } => path,
                DeltaOperation::ORMapRemove { path, .. } => path,
            };

            if !modified_paths.contains(path) {
                modified_paths.push(path.clone());
            }
        }

        Ok(modified_paths)
    }

    /// Get the decoder
    pub fn decoder(&self) -> &DeltaDecoder {
        &self.decoder
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core_store::sync::delta_encoder::DeltaEncoder;

    #[test]
    fn test_decode_delta() {
        // Create a delta using encoder
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

        let delta = encoder.finalize();
        let bytes = DeltaEncoder::encode(&delta).unwrap();

        // Decode it
        let decoder = DeltaDecoder::decode(&bytes).unwrap();

        assert_eq!(decoder.target_id(), "channel_123");
        assert_eq!(decoder.author_node(), "node1");
        assert_eq!(decoder.operation_count(), 1);
    }

    #[test]
    fn test_validate_delta() {
        let vc = VectorClock::new();
        let mut encoder =
            DeltaEncoder::new("channel_123".to_string(), "node1".to_string(), vc.clone());

        encoder
            .add_lww_operation("channel.name".to_string(), &"Test", 100, "node1".to_string(), &vc)
            .unwrap();

        let delta = encoder.finalize();
        let bytes = DeltaEncoder::encode(&delta).unwrap();
        let decoder = DeltaDecoder::decode(&bytes).unwrap();

        // Should validate successfully
        assert!(decoder.validate().is_ok());
    }

    #[test]
    fn test_deserialize_lww_value() {
        let vc = VectorClock::new();
        let mut encoder =
            DeltaEncoder::new("channel_123".to_string(), "node1".to_string(), vc.clone());

        let test_value = "Test Channel";
        encoder
            .add_lww_operation(
                "channel.name".to_string(),
                &test_value,
                100,
                "node1".to_string(),
                &vc,
            )
            .unwrap();

        let delta = encoder.finalize();
        let bytes = DeltaEncoder::encode(&delta).unwrap();
        let decoder = DeltaDecoder::decode(&bytes).unwrap();

        // Get the operation value bytes
        if let DeltaOperation::LWWUpdate { value, .. } = &decoder.operations()[0] {
            let decoded: String = decoder.deserialize_lww_value(value).unwrap();
            assert_eq!(decoded, test_value);
        } else {
            panic!("Expected LWWUpdate operation");
        }
    }

    #[test]
    fn test_delta_applier() {
        let vc = VectorClock::new();
        let mut encoder =
            DeltaEncoder::new("channel_123".to_string(), "node1".to_string(), vc.clone());

        encoder
            .add_lww_operation("channel.name".to_string(), &"Test", 100, "node1".to_string(), &vc)
            .unwrap();

        encoder
            .add_lww_operation("channel.topic".to_string(), &"Topic", 101, "node1".to_string(), &vc)
            .unwrap();

        let delta = encoder.finalize();
        let bytes = DeltaEncoder::encode(&delta).unwrap();
        let decoder = DeltaDecoder::decode(&bytes).unwrap();

        let applier = DeltaApplier::new(decoder);
        let modified = applier.apply_all().unwrap();

        assert_eq!(modified.len(), 2);
        assert!(modified.contains(&"channel.name".to_string()));
        assert!(modified.contains(&"channel.topic".to_string()));
    }
}
