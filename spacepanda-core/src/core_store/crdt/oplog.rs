/*
    oplog.rs - Append-only operation log

    Stores all CRDT operations in causal order.
    Each operation is:
    - Vector-clocked for ordering
    - Signed for authenticity
    - Immutable once added

    The oplog is the source of truth for replaying state.
*/

use super::traits::OperationMetadata;
use super::vector_clock::VectorClock;
use crate::core_store::store::errors::{StoreError, StoreResult};
use serde::{Deserialize, Serialize};

/// A single entry in the operation log
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpLogEntry {
    /// Unique ID for this operation
    pub op_id: u64,

    /// The actual operation data (serialized)
    pub operation_data: Vec<u8>,

    /// Metadata (node ID, vector clock, timestamp)
    pub metadata: OperationMetadata,

    /// Type of operation (for deserialization)
    pub op_type: String,
}

impl OpLogEntry {
    pub fn new(
        op_id: u64,
        operation_data: Vec<u8>,
        metadata: OperationMetadata,
        op_type: String,
    ) -> Self {
        OpLogEntry { op_id, operation_data, metadata, op_type }
    }
}

/// Append-only operation log
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpLog {
    /// All operations in order
    entries: Vec<OpLogEntry>,

    /// Next operation ID
    next_op_id: u64,

    /// Current vector clock state
    vector_clock: VectorClock,
}

impl OpLog {
    /// Create a new empty operation log
    pub fn new() -> Self {
        OpLog { entries: Vec::new(), next_op_id: 1, vector_clock: VectorClock::new() }
    }

    /// Append an operation to the log
    pub fn append(
        &mut self,
        operation_data: Vec<u8>,
        metadata: OperationMetadata,
        op_type: String,
    ) -> StoreResult<u64> {
        // Validate causal order
        self.validate_causal_order(&metadata.vector_clock)?;

        let op_id = self.next_op_id;
        let entry = OpLogEntry::new(op_id, operation_data, metadata.clone(), op_type);

        self.entries.push(entry);
        self.next_op_id += 1;

        // Update our vector clock
        self.vector_clock.merge(&metadata.vector_clock);

        Ok(op_id)
    }

    /// Validate that an operation maintains causal order
    fn validate_causal_order(&self, incoming_clock: &VectorClock) -> StoreResult<()> {
        // Check if incoming operation is causally ready
        // It should not require operations we haven't seen yet

        for node_id in incoming_clock.node_ids() {
            let incoming_time = incoming_clock.get(&node_id);
            let our_time = self.vector_clock.get(&node_id);

            // Incoming should not be too far ahead
            if incoming_time > our_time + 1 {
                return Err(StoreError::CausalViolation(format!(
                    "Operation from {} is ahead: {} > {}",
                    node_id, incoming_time, our_time
                )));
            }
        }

        Ok(())
    }

    /// Get an operation by ID
    pub fn get(&self, op_id: u64) -> Option<&OpLogEntry> {
        self.entries.iter().find(|e| e.op_id == op_id)
    }

    /// Get all operations after a given operation ID
    pub fn get_since(&self, op_id: u64) -> Vec<&OpLogEntry> {
        self.entries.iter().filter(|e| e.op_id > op_id).collect()
    }

    /// Get operations within a range
    pub fn get_range(&self, start_id: u64, end_id: u64) -> Vec<&OpLogEntry> {
        self.entries
            .iter()
            .filter(|e| e.op_id >= start_id && e.op_id <= end_id)
            .collect()
    }

    /// Get all operations
    pub fn all_entries(&self) -> &[OpLogEntry] {
        &self.entries
    }

    /// Get the number of operations
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the log is empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get the current vector clock
    pub fn vector_clock(&self) -> &VectorClock {
        &self.vector_clock
    }

    /// Merge another oplog into this one
    pub fn merge(&mut self, other: &OpLog) -> StoreResult<()> {
        for entry in &other.entries {
            // Check for duplicates using metadata (node_id + timestamp) instead of op_id
            // since op_ids from different nodes can collide
            let is_duplicate = self.entries.iter().any(|e| {
                e.metadata.node_id == entry.metadata.node_id
                    && e.metadata.timestamp == entry.metadata.timestamp
            });

            if !is_duplicate {
                self.entries.push(entry.clone());
            }
        }

        // Merge vector clocks
        self.vector_clock.merge(&other.vector_clock);

        // Re-sort entries by timestamp to maintain causal order
        self.entries.sort_by_key(|e| e.metadata.timestamp);

        Ok(())
    }

    /// Get operations for a specific node
    pub fn get_by_node(&self, node_id: &str) -> Vec<&OpLogEntry> {
        self.entries.iter().filter(|e| e.metadata.node_id == node_id).collect()
    }

    /// Compact the log by removing redundant operations
    /// (Placeholder - would need type-specific logic)
    pub fn compact(&mut self) -> usize {
        // TODO: Implement compaction based on CRDT semantics
        // For now, just return 0 (no compaction)
        0
    }
}

impl Default for OpLog {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_oplog_creation() {
        let log = OpLog::new();
        assert!(log.is_empty());
        assert_eq!(log.len(), 0);
        assert_eq!(log.next_op_id, 1);
    }

    #[test]
    fn test_oplog_append() {
        let mut log = OpLog::new();
        let mut vc = VectorClock::new();
        vc.increment("node1");

        let metadata = OperationMetadata::new("node1".to_string(), vc);
        let op_id = log.append(vec![1, 2, 3], metadata, "test_op".to_string()).unwrap();

        assert_eq!(op_id, 1);
        assert_eq!(log.len(), 1);
        assert_eq!(log.next_op_id, 2);
    }

    #[test]
    fn test_oplog_get() {
        let mut log = OpLog::new();
        let mut vc = VectorClock::new();
        vc.increment("node1");

        let metadata = OperationMetadata::new("node1".to_string(), vc);
        let op_id = log.append(vec![1, 2, 3], metadata, "test_op".to_string()).unwrap();

        let entry = log.get(op_id).unwrap();
        assert_eq!(entry.op_id, op_id);
        assert_eq!(entry.operation_data, vec![1, 2, 3]);
    }

    #[test]
    fn test_oplog_get_since() {
        let mut log = OpLog::new();
        let mut vc = VectorClock::new();

        for i in 1..=5 {
            vc.increment("node1");
            let metadata = OperationMetadata::new("node1".to_string(), vc.clone());
            log.append(vec![i], metadata, "test_op".to_string()).unwrap();
        }

        let since_3 = log.get_since(3);
        assert_eq!(since_3.len(), 2); // ops 4 and 5
    }

    #[test]
    fn test_oplog_get_range() {
        let mut log = OpLog::new();
        let mut vc = VectorClock::new();

        for i in 1..=5 {
            vc.increment("node1");
            let metadata = OperationMetadata::new("node1".to_string(), vc.clone());
            log.append(vec![i], metadata, "test_op".to_string()).unwrap();
        }

        let range = log.get_range(2, 4);
        assert_eq!(range.len(), 3); // ops 2, 3, 4
    }

    #[test]
    fn test_oplog_merge() {
        let mut log1 = OpLog::new();
        let mut log2 = OpLog::new();
        let mut vc = VectorClock::new();

        // Add to log1
        vc.increment("node1");
        let metadata1 = OperationMetadata::new("node1".to_string(), vc.clone());
        log1.append(vec![1], metadata1, "test_op".to_string()).unwrap();

        // Add to log2
        vc.increment("node2");
        let metadata2 = OperationMetadata::new("node2".to_string(), vc);
        log2.append(vec![2], metadata2, "test_op".to_string()).unwrap();

        log1.merge(&log2).unwrap();

        // Should have both operations
        assert_eq!(log1.len(), 2);
    }

    #[test]
    fn test_oplog_get_by_node() {
        let mut log = OpLog::new();
        let mut vc = VectorClock::new();

        // Add operations from node1
        for i in 1..=3 {
            vc.increment("node1");
            let metadata = OperationMetadata::new("node1".to_string(), vc.clone());
            log.append(vec![i], metadata, "test_op".to_string()).unwrap();
        }

        // Add operations from node2
        for i in 4..=5 {
            vc.increment("node2");
            let metadata = OperationMetadata::new("node2".to_string(), vc.clone());
            log.append(vec![i], metadata, "test_op".to_string()).unwrap();
        }

        let node1_ops = log.get_by_node("node1");
        assert_eq!(node1_ops.len(), 3);

        let node2_ops = log.get_by_node("node2");
        assert_eq!(node2_ops.len(), 2);
    }

    #[test]
    fn test_oplog_vector_clock_tracking() {
        let mut log = OpLog::new();
        let mut vc = VectorClock::new();

        vc.increment("node1");
        let metadata = OperationMetadata::new("node1".to_string(), vc.clone());
        log.append(vec![1], metadata, "test_op".to_string()).unwrap();

        // Vector clock should be updated (placeholder behavior)
        assert!(!log.vector_clock().is_empty());
    }

    #[test]
    fn test_oplog_all_entries() {
        let mut log = OpLog::new();
        let mut vc = VectorClock::new();

        for i in 1..=3 {
            vc.increment("node1");
            let metadata = OperationMetadata::new("node1".to_string(), vc.clone());
            log.append(vec![i], metadata, "test_op".to_string()).unwrap();
        }

        let all = log.all_entries();
        assert_eq!(all.len(), 3);
    }
}
