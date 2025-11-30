/*
    vector_clock.rs - Vector clock implementation for causal ordering
    
    A vector clock tracks the logical time across distributed nodes.
    Used to determine causal relationships between events:
    - Happened-before
    - Concurrent
    - Happened-after
    
    Essential for CRDT conflict resolution.
*/

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::cmp::Ordering;

/// Node identifier for vector clock
pub type NodeId = String;

/// Vector clock for tracking causal order
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VectorClock {
    /// Map from node ID to logical timestamp
    clock: HashMap<NodeId, u64>,
}

impl VectorClock {
    /// Create a new empty vector clock
    pub fn new() -> Self {
        VectorClock {
            clock: HashMap::new(),
        }
    }
    
    /// Increment the clock for a given node
    pub fn increment(&mut self, node_id: &str) {
        let counter = self.clock.entry(node_id.to_string()).or_insert(0);
        *counter += 1;
    }
    
    /// Get the timestamp for a node
    pub fn get(&self, node_id: &str) -> u64 {
        self.clock.get(node_id).copied().unwrap_or(0)
    }
    
    /// Set the timestamp for a node
    pub fn set(&mut self, node_id: &str, timestamp: u64) {
        self.clock.insert(node_id.to_string(), timestamp);
    }
    
    /// Merge two vector clocks (take maximum of each entry)
    pub fn merge(&mut self, other: &VectorClock) {
        for (node_id, &timestamp) in &other.clock {
            let current = self.clock.entry(node_id.clone()).or_insert(0);
            *current = (*current).max(timestamp);
        }
    }
    
    /// Check if this clock happened before another
    /// Returns true if all entries in self <= other and at least one is strictly less
    pub fn happened_before(&self, other: &VectorClock) -> bool {
        let mut strictly_less = false;
        
        // Check all nodes in self
        for (node_id, &self_time) in &self.clock {
            let other_time = other.get(node_id);
            if self_time > other_time {
                return false; // Not happened-before if any entry is greater
            }
            if self_time < other_time {
                strictly_less = true;
            }
        }
        
        // Check if other has any nodes not in self with non-zero time
        for (node_id, &other_time) in &other.clock {
            if !self.clock.contains_key(node_id) && other_time > 0 {
                strictly_less = true;
            }
        }
        
        strictly_less
    }
    
    /// Check if two clocks are concurrent (neither happened before the other)
    pub fn is_concurrent(&self, other: &VectorClock) -> bool {
        !self.happened_before(other) && !other.happened_before(self) && self != other
    }
    
    /// Compare two vector clocks
    pub fn partial_cmp(&self, other: &VectorClock) -> Option<Ordering> {
        if self == other {
            Some(Ordering::Equal)
        } else if self.happened_before(other) {
            Some(Ordering::Less)
        } else if other.happened_before(self) {
            Some(Ordering::Greater)
        } else {
            None // Concurrent
        }
    }
    
    /// Get all node IDs in this clock
    pub fn node_ids(&self) -> Vec<String> {
        self.clock.keys().cloned().collect()
    }
    
    /// Check if clock is empty
    pub fn is_empty(&self) -> bool {
        self.clock.is_empty()
    }
    
    /// Get the number of nodes tracked
    pub fn len(&self) -> usize {
        self.clock.len()
    }
}

impl Default for VectorClock {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_vector_clock_creation() {
        let vc = VectorClock::new();
        assert!(vc.is_empty());
        assert_eq!(vc.len(), 0);
    }
    
    #[test]
    fn test_increment() {
        let mut vc = VectorClock::new();
        vc.increment("node1");
        assert_eq!(vc.get("node1"), 1);
        
        vc.increment("node1");
        assert_eq!(vc.get("node1"), 2);
        
        vc.increment("node2");
        assert_eq!(vc.get("node2"), 1);
    }
    
    #[test]
    fn test_get_nonexistent() {
        let vc = VectorClock::new();
        assert_eq!(vc.get("unknown"), 0);
    }
    
    #[test]
    fn test_set() {
        let mut vc = VectorClock::new();
        vc.set("node1", 5);
        assert_eq!(vc.get("node1"), 5);
    }
    
    #[test]
    fn test_merge() {
        let mut vc1 = VectorClock::new();
        vc1.set("node1", 3);
        vc1.set("node2", 1);
        
        let mut vc2 = VectorClock::new();
        vc2.set("node1", 2);
        vc2.set("node2", 4);
        vc2.set("node3", 1);
        
        vc1.merge(&vc2);
        
        assert_eq!(vc1.get("node1"), 3); // max(3, 2)
        assert_eq!(vc1.get("node2"), 4); // max(1, 4)
        assert_eq!(vc1.get("node3"), 1); // max(0, 1)
    }
    
    #[test]
    fn test_happened_before() {
        let mut vc1 = VectorClock::new();
        vc1.set("node1", 1);
        vc1.set("node2", 2);
        
        let mut vc2 = VectorClock::new();
        vc2.set("node1", 2);
        vc2.set("node2", 3);
        
        assert!(vc1.happened_before(&vc2));
        assert!(!vc2.happened_before(&vc1));
    }
    
    #[test]
    fn test_concurrent() {
        let mut vc1 = VectorClock::new();
        vc1.set("node1", 2);
        vc1.set("node2", 1);
        
        let mut vc2 = VectorClock::new();
        vc2.set("node1", 1);
        vc2.set("node2", 2);
        
        assert!(vc1.is_concurrent(&vc2));
        assert!(vc2.is_concurrent(&vc1));
    }
    
    #[test]
    fn test_partial_cmp() {
        let mut vc1 = VectorClock::new();
        vc1.set("node1", 1);
        
        let mut vc2 = VectorClock::new();
        vc2.set("node1", 2);
        
        assert_eq!(vc1.partial_cmp(&vc2), Some(Ordering::Less));
        assert_eq!(vc2.partial_cmp(&vc1), Some(Ordering::Greater));
        
        let vc3 = vc1.clone();
        assert_eq!(vc1.partial_cmp(&vc3), Some(Ordering::Equal));
    }
    
    #[test]
    fn test_partial_cmp_concurrent() {
        let mut vc1 = VectorClock::new();
        vc1.set("node1", 2);
        vc1.set("node2", 1);
        
        let mut vc2 = VectorClock::new();
        vc2.set("node1", 1);
        vc2.set("node2", 2);
        
        assert_eq!(vc1.partial_cmp(&vc2), None); // Concurrent
    }
    
    #[test]
    fn test_node_ids() {
        let mut vc = VectorClock::new();
        vc.set("node1", 1);
        vc.set("node2", 2);
        
        let ids = vc.node_ids();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&"node1".to_string()));
        assert!(ids.contains(&"node2".to_string()));
    }
    
    #[test]
    fn test_happened_before_with_missing_nodes() {
        let mut vc1 = VectorClock::new();
        vc1.set("node1", 1);
        
        let mut vc2 = VectorClock::new();
        vc2.set("node1", 1);
        vc2.set("node2", 1);
        
        assert!(vc1.happened_before(&vc2));
        assert!(!vc2.happened_before(&vc1));
    }
}
