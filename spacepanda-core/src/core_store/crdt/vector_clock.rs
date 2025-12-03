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
use std::cmp::Ordering;
use std::collections::HashMap;

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
        VectorClock { clock: HashMap::new() }
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

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    // Property: Increment increases clock value
    proptest! {
        #[test]
        fn prop_increment_increases(node_id in "[a-z]+", iterations in 1usize..100) {
            let mut vc = VectorClock::new();
            
            for i in 1..=iterations {
                vc.increment(&node_id);
                prop_assert_eq!(vc.get(&node_id), i as u64);
            }
        }
    }

    // Property: Merge is commutative (A ∪ B = B ∪ A)
    proptest! {
        #[test]
        fn prop_merge_commutative(
            ops_a in prop::collection::vec(("[a-z]+", 1u64..100), 1..10),
            ops_b in prop::collection::vec(("[a-z]+", 1u64..100), 1..10),
        ) {
            let mut vc_a1 = VectorClock::new();
            let mut vc_a2 = VectorClock::new();
            let mut vc_b1 = VectorClock::new();
            let mut vc_b2 = VectorClock::new();

            for (node, val) in &ops_a {
                vc_a1.set(node, *val);
                vc_a2.set(node, *val);
            }
            for (node, val) in &ops_b {
                vc_b1.set(node, *val);
                vc_b2.set(node, *val);
            }

            // A ∪ B
            vc_a1.merge(&vc_b1);
            // B ∪ A
            vc_b2.merge(&vc_a2);

            // Should be equal
            for node in vc_a1.node_ids() {
                prop_assert_eq!(vc_a1.get(&node), vc_b2.get(&node));
            }
        }
    }

    // Property: Merge is associative ((A ∪ B) ∪ C = A ∪ (B ∪ C))
    proptest! {
        #[test]
        fn prop_merge_associative(
            ops_a in prop::collection::vec(("[a-z]+", 1u64..50), 0..5),
            ops_b in prop::collection::vec(("[a-z]+", 1u64..50), 0..5),
            ops_c in prop::collection::vec(("[a-z]+", 1u64..50), 0..5),
        ) {
            let mut vc_a1 = VectorClock::new();
            let mut vc_a2 = VectorClock::new();
            let mut vc_b1 = VectorClock::new();
            let mut vc_b2 = VectorClock::new();
            let mut vc_c1 = VectorClock::new();
            let mut vc_c2 = VectorClock::new();

            for (node, val) in &ops_a {
                vc_a1.set(node, *val);
                vc_a2.set(node, *val);
            }
            for (node, val) in &ops_b {
                vc_b1.set(node, *val);
                vc_b2.set(node, *val);
            }
            for (node, val) in &ops_c {
                vc_c1.set(node, *val);
                vc_c2.set(node, *val);
            }

            // (A ∪ B) ∪ C
            vc_a1.merge(&vc_b1);
            vc_a1.merge(&vc_c1);

            // A ∪ (B ∪ C)
            vc_b2.merge(&vc_c2);
            vc_a2.merge(&vc_b2);

            // Should be equal
            let all_nodes: std::collections::HashSet<_> = 
                vc_a1.node_ids().into_iter().chain(vc_a2.node_ids()).collect();
            
            for node in all_nodes {
                prop_assert_eq!(vc_a1.get(&node), vc_a2.get(&node));
            }
        }
    }

    // Property: Idempotent merge (A ∪ A = A)
    proptest! {
        #[test]
        fn prop_merge_idempotent(
            ops in prop::collection::vec(("[a-z]+", 1u64..100), 1..10),
        ) {
            let mut vc1 = VectorClock::new();
            let mut vc2 = VectorClock::new();

            for (node, val) in &ops {
                vc1.set(node, *val);
                vc2.set(node, *val);
            }

            let original: std::collections::HashMap<_, _> = 
                vc1.node_ids().iter().map(|n| (n.clone(), vc1.get(n))).collect();

            vc1.merge(&vc2);

            // Should be unchanged
            for (node, val) in original {
                prop_assert_eq!(vc1.get(&node), val);
            }
        }
    }

    // Property: Merge takes maximum of each entry
    proptest! {
        #[test]
        fn prop_merge_takes_max(
            node_id in "[a-z]+",
            val_a in 1u64..100,
            val_b in 1u64..100,
        ) {
            let mut vc_a = VectorClock::new();
            let mut vc_b = VectorClock::new();

            vc_a.set(&node_id, val_a);
            vc_b.set(&node_id, val_b);

            vc_a.merge(&vc_b);

            prop_assert_eq!(vc_a.get(&node_id), val_a.max(val_b));
        }
    }

    // Property: Happened-before is transitive
    proptest! {
        #[test]
        fn prop_happened_before_transitive(
            node in "[a-z]+",
            t1 in 1u64..10,
            t2 in 10u64..20,
            t3 in 20u64..30,
        ) {
            let mut vc1 = VectorClock::new();
            let mut vc2 = VectorClock::new();
            let mut vc3 = VectorClock::new();

            vc1.set(&node, t1);
            vc2.set(&node, t2);
            vc3.set(&node, t3);

            // If vc1 < vc2 and vc2 < vc3, then vc1 < vc3
            if vc1.happened_before(&vc2) && vc2.happened_before(&vc3) {
                prop_assert!(vc1.happened_before(&vc3));
            }
        }
    }

    // Property: Concurrent events don't have happened-before relationship
    proptest! {
        #[test]
        fn prop_concurrent_no_happened_before(
            node_a in "[a-z]+",
            node_b in "[a-z]+",
            val_a in 1u64..100,
            val_b in 1u64..100,
        ) {
            if node_a == node_b {
                return Ok(());
            }

            let mut vc1 = VectorClock::new();
            let mut vc2 = VectorClock::new();

            vc1.set(&node_a, val_a);
            vc2.set(&node_b, val_b);

            // Concurrent events - neither happened before the other
            let hb_1_2 = vc1.happened_before(&vc2);
            let hb_2_1 = vc2.happened_before(&vc1);

            // At least one should be false (could both be false if independent)
            prop_assert!(!(hb_1_2 && hb_2_1));
        }
    }

    // Property: Self is not happened-before self
    proptest! {
        #[test]
        fn prop_not_happened_before_self(
            ops in prop::collection::vec(("[a-z]+", 1u64..100), 1..10),
        ) {
            let mut vc = VectorClock::new();
            for (node, val) in &ops {
                vc.set(node, *val);
            }

            prop_assert!(!vc.happened_before(&vc));
        }
    }
}
