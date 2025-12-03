/*
    lww_register.rs - Last-Write-Wins Register CRDT

    A simple CRDT that holds a single value.
    Conflicts are resolved by taking the value with the latest timestamp.
    If timestamps are equal, use node ID as tiebreaker.

    Use cases:
    - Channel topic
    - User nickname
    - Space name
    - Any single-value field that can be overwritten
*/

use super::traits::{Crdt, OperationMetadata};
use super::vector_clock::VectorClock;
use crate::core_store::store::errors::StoreResult;
use serde::{Deserialize, Serialize};

/// Last-Write-Wins Register CRDT
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LWWRegister<T: Clone> {
    /// Current value
    value: Option<T>,

    /// Timestamp of last write
    timestamp: u64,

    /// Node ID of last writer (for tiebreaking)
    node_id: String,

    /// Vector clock for causal ordering
    vector_clock: VectorClock,
}

/// Operation for LWW Register
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LWWOperation<T: Clone> {
    /// New value to set
    pub value: T,

    /// Metadata for the operation
    pub metadata: OperationMetadata,
}

impl<T: Clone> super::validated::HasMetadata for LWWOperation<T> {
    fn metadata(&self) -> &OperationMetadata {
        &self.metadata
    }
}

impl<T: Clone> LWWRegister<T> {
    /// Create a new empty LWW register
    pub fn new() -> Self {
        LWWRegister {
            value: None,
            timestamp: 0,
            node_id: String::new(),
            vector_clock: VectorClock::new(),
        }
    }

    /// Create a new LWW register with an initial value
    pub fn with_value(value: T, node_id: String) -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64;

        let mut vc = VectorClock::new();
        vc.increment(&node_id);

        LWWRegister { value: Some(value), timestamp, node_id, vector_clock: vc }
    }

    /// Set a new value with given timestamp and node ID
    pub fn set(&mut self, value: T, timestamp: u64, node_id: String, vector_clock: VectorClock) {
        // Only update if new value wins
        if self.should_update(timestamp, &node_id) {
            self.value = Some(value);
            self.timestamp = timestamp;
            self.node_id = node_id;
            self.vector_clock.merge(&vector_clock);
        } else {
            // Still merge vector clocks even if we don't update the value
            self.vector_clock.merge(&vector_clock);
        }
    }

    /// Check if we should update based on timestamp and node ID
    fn should_update(&self, new_timestamp: u64, new_node_id: &str) -> bool {
        if new_timestamp > self.timestamp {
            true
        } else if new_timestamp == self.timestamp {
            // Tiebreaker: deterministic comparison
            // Use > for add-wins bias (if new value has greater node_id, it wins)
            new_node_id > self.node_id.as_str()
        } else {
            false
        }
    }

    /// Get the current value
    pub fn get(&self) -> Option<&T> {
        self.value.as_ref()
    }

    /// Get timestamp of last write
    pub fn timestamp(&self) -> u64 {
        self.timestamp
    }

    /// Get node ID of last writer
    pub fn writer(&self) -> &str {
        &self.node_id
    }

    /// Merge another LWW register into this one
    pub fn merge(&mut self, other: &LWWRegister<T>) {
        if let Some(ref other_value) = other.value {
            if self.should_update(other.timestamp, &other.node_id) {
                self.value = Some(other_value.clone());
                self.timestamp = other.timestamp;
                self.node_id = other.node_id.clone();
            }
        }
        // Always merge vector clocks
        self.vector_clock.merge(&other.vector_clock);
    }
}

impl<T: Clone + Send + Sync> Crdt for LWWRegister<T> {
    type Operation = LWWOperation<T>;
    type Value = Option<T>;

    fn apply(&mut self, op: Self::Operation) -> StoreResult<()> {
        self.set(op.value, op.metadata.timestamp, op.metadata.node_id, op.metadata.vector_clock);
        Ok(())
    }

    fn merge(&mut self, other: &Self) -> StoreResult<()> {
        // Use the non-Crdt merge method to avoid double vector clock merge
        self.merge(other);
        Ok(())
    }

    fn value(&self) -> Self::Value {
        self.value.clone()
    }

    fn vector_clock(&self) -> &VectorClock {
        &self.vector_clock
    }
}

impl<T: Clone> Default for LWWRegister<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lww_register_creation() {
        let reg: LWWRegister<String> = LWWRegister::new();
        assert!(reg.get().is_none());
        assert_eq!(reg.timestamp(), 0);
    }

    #[test]
    fn test_lww_register_with_value() {
        let reg = LWWRegister::with_value("hello".to_string(), "node1".to_string());
        assert_eq!(reg.get(), Some(&"hello".to_string()));
        assert!(reg.timestamp() > 0);
        assert_eq!(reg.writer(), "node1");
    }

    #[test]
    fn test_lww_set() {
        let mut reg: LWWRegister<i32> = LWWRegister::new();
        let vc = VectorClock::new();

        reg.set(42, 100, "node1".to_string(), vc.clone());
        assert_eq!(reg.get(), Some(&42));
        assert_eq!(reg.timestamp(), 100);
    }

    #[test]
    fn test_lww_later_write_wins() {
        let mut reg: LWWRegister<i32> = LWWRegister::new();
        let vc = VectorClock::new();

        reg.set(42, 100, "node1".to_string(), vc.clone());
        reg.set(99, 200, "node2".to_string(), vc.clone());

        assert_eq!(reg.get(), Some(&99));
        assert_eq!(reg.timestamp(), 200);
        assert_eq!(reg.writer(), "node2");
    }

    #[test]
    fn test_lww_earlier_write_ignored() {
        let mut reg: LWWRegister<i32> = LWWRegister::new();
        let vc = VectorClock::new();

        reg.set(42, 200, "node1".to_string(), vc.clone());
        reg.set(99, 100, "node2".to_string(), vc.clone());

        assert_eq!(reg.get(), Some(&42));
        assert_eq!(reg.timestamp(), 200);
        assert_eq!(reg.writer(), "node1");
    }

    #[test]
    fn test_lww_tiebreaker_by_node_id() {
        let mut reg: LWWRegister<i32> = LWWRegister::new();
        let vc = VectorClock::new();

        reg.set(42, 100, "node_a".to_string(), vc.clone());
        reg.set(99, 100, "node_b".to_string(), vc.clone());

        // "node_b" > "node_a" lexicographically
        assert_eq!(reg.get(), Some(&99));
        assert_eq!(reg.writer(), "node_b");
    }

    #[test]
    fn test_lww_merge() {
        let mut reg1 = LWWRegister::with_value(42, "node1".to_string());
        let reg2 = LWWRegister::with_value(99, "node2".to_string());

        // Ensure reg2 has a later timestamp
        std::thread::sleep(std::time::Duration::from_millis(10));
        let reg2 = LWWRegister::with_value(99, "node2".to_string());

        reg1.merge(&reg2);
        assert_eq!(reg1.get(), Some(&99));
    }

    #[test]
    fn test_lww_crdt_apply() {
        let mut reg: LWWRegister<String> = LWWRegister::new();
        let mut vc = VectorClock::new();
        vc.increment("node1");

        let metadata = OperationMetadata::new("node1".to_string(), vc);
        let op = LWWOperation { value: "test".to_string(), metadata };

        reg.apply(op).unwrap();
        assert_eq!(reg.get(), Some(&"test".to_string()));
    }

    #[test]
    fn test_lww_vector_clock_merges() {
        let mut reg: LWWRegister<i32> = LWWRegister::new();

        let mut vc1 = VectorClock::new();
        vc1.set("node1", 5);

        let mut vc2 = VectorClock::new();
        vc2.set("node2", 3);

        reg.set(42, 100, "node1".to_string(), vc1);
        reg.set(43, 50, "node2".to_string(), vc2); // Earlier timestamp, won't update value

        // Value should still be 42, but vector clock should have merged
        assert_eq!(reg.get(), Some(&42));
        assert_eq!(reg.vector_clock().get("node1"), 5);
        assert_eq!(reg.vector_clock().get("node2"), 3);
    }
}
