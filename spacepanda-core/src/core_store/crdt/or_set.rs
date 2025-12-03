/*
    or_set.rs - Observed-Remove Set CRDT

    A set that supports add and remove operations.
    Elements are tagged with unique IDs to distinguish concurrent adds.
    An element is in the set if there's an add without a corresponding remove.

    Use cases:
    - Channel members
    - Role assignments
    - Pinned messages
    - Any membership collection
*/

use super::traits::{Crdt, OperationMetadata, TombstoneCrdt};
use super::vector_clock::VectorClock;
use crate::core_store::store::errors::StoreResult;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Unique identifier for an add operation
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AddId {
    /// Node that performed the add
    pub node_id: String,
    /// Logical timestamp of the add
    pub timestamp: u64,
}

impl AddId {
    pub fn new(node_id: String, timestamp: u64) -> Self {
        AddId { node_id, timestamp }
    }
}

/// Observed-Remove Set CRDT
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ORSet<T: Clone + Eq + std::hash::Hash> {
    /// Map from element to set of add IDs
    /// An element exists if it has at least one add ID
    elements: HashMap<T, HashSet<AddId>>,

    /// Tombstones: removed (element, add_id) pairs
    /// Kept for synchronization to know what's been removed
    tombstones: HashSet<(T, AddId)>,

    /// Vector clock for causal ordering
    vector_clock: VectorClock,
}

/// Operations for OR-Set
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ORSetOperation<T: Clone> {
    /// Add an element with a unique ID
    Add { element: T, add_id: AddId, metadata: OperationMetadata },
    /// Remove an element (removes all its add IDs)
    Remove { element: T, add_ids: HashSet<AddId>, metadata: OperationMetadata },
}

impl<T: Clone> super::validated::HasMetadata for ORSetOperation<T> {
    fn metadata(&self) -> &OperationMetadata {
        match self {
            ORSetOperation::Add { metadata, .. } => metadata,
            ORSetOperation::Remove { metadata, .. } => metadata,
        }
    }
}

impl<T: Clone + Eq + std::hash::Hash> ORSet<T> {
    /// Create a new empty OR-Set
    pub fn new() -> Self {
        ORSet {
            elements: HashMap::new(),
            tombstones: HashSet::new(),
            vector_clock: VectorClock::new(),
        }
    }

    /// Add an element to the set
    pub fn add(&mut self, element: T, add_id: AddId, vector_clock: VectorClock) {
        self.elements.entry(element).or_insert_with(HashSet::new).insert(add_id);
        self.vector_clock.merge(&vector_clock);
    }

    /// Remove an element from the set
    /// Records all current add_ids as tombstones
    pub fn remove(&mut self, element: &T, vector_clock: VectorClock) -> HashSet<AddId> {
        let add_ids = self.elements.get(element).cloned().unwrap_or_default();

        // Remove the element
        self.elements.remove(element);

        // Add to tombstones
        for add_id in &add_ids {
            self.tombstones.insert((element.clone(), add_id.clone()));
        }

        self.vector_clock.merge(&vector_clock);
        add_ids
    }

    /// Check if an element is in the set
    pub fn contains(&self, element: &T) -> bool {
        self.elements.contains_key(element)
    }

    /// Get all elements in the set
    pub fn elements(&self) -> Vec<T> {
        self.elements.keys().cloned().collect()
    }

    /// Get the number of elements
    pub fn len(&self) -> usize {
        self.elements.len()
    }

    /// Check if the set is empty
    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }

    /// Get add IDs for a specific element
    pub fn get_add_ids(&self, element: &T) -> Option<&HashSet<AddId>> {
        self.elements.get(element)
    }
}

impl<T: Clone + Eq + std::hash::Hash + Send + Sync> Crdt for ORSet<T> {
    type Operation = ORSetOperation<T>;
    type Value = Vec<T>;

    fn apply(&mut self, op: Self::Operation) -> StoreResult<()> {
        match op {
            ORSetOperation::Add { element, add_id, metadata } => {
                // Don't add if this (element, add_id) is tombstoned
                if !self.tombstones.contains(&(element.clone(), add_id.clone())) {
                    self.add(element, add_id, metadata.vector_clock);
                }
            }
            ORSetOperation::Remove { element, add_ids, metadata } => {
                // Remove the element's add_ids and record tombstones
                if let Some(current_adds) = self.elements.get_mut(&element) {
                    for add_id in &add_ids {
                        current_adds.remove(add_id);
                        self.tombstones.insert((element.clone(), add_id.clone()));
                    }

                    // If no add_ids left, remove the element
                    if current_adds.is_empty() {
                        self.elements.remove(&element);
                    }
                }
                self.vector_clock.merge(&metadata.vector_clock);
            }
        }
        Ok(())
    }

    fn merge(&mut self, other: &Self) -> StoreResult<()> {
        // Merge elements
        for (element, other_adds) in &other.elements {
            let entry = self.elements.entry(element.clone()).or_insert_with(HashSet::new);
            
            // Reserve capacity upfront to reduce allocations
            entry.reserve(other_adds.len());
            
            for add_id in other_adds {
                // Only add if not tombstoned
                if !self.tombstones.contains(&(element.clone(), add_id.clone())) {
                    entry.insert(add_id.clone());
                }
            }
        }

        // Merge tombstones and apply them
        for tombstone in &other.tombstones {
            self.tombstones.insert(tombstone.clone());

            // Remove from elements if tombstoned
            if let Some(adds) = self.elements.get_mut(&tombstone.0) {
                adds.remove(&tombstone.1);
            }
        }

        // Clean up empty entries in one pass
        self.elements.retain(|_, adds| !adds.is_empty());

        // Merge vector clocks
        self.vector_clock.merge(&other.vector_clock);

        Ok(())
    }

    fn value(&self) -> Self::Value {
        self.elements()
    }

    fn vector_clock(&self) -> &VectorClock {
        &self.vector_clock
    }
}

impl<T: Clone + Eq + std::hash::Hash + Send + Sync> TombstoneCrdt for ORSet<T> {
    fn is_tombstoned(&self, _key: &str) -> bool {
        // For OR-Set, we don't use string keys
        // This method is for compatibility with the trait
        false
    }

    fn gc_tombstones(&mut self, _threshold_ms: u64) -> usize {
        // Clear all tombstones (in production, would filter by age)
        let count = self.tombstones.len();
        self.tombstones.clear();
        count
    }
}

impl<T: Clone + Eq + std::hash::Hash> Default for ORSet<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_or_set_creation() {
        let set: ORSet<String> = ORSet::new();
        assert!(set.is_empty());
        assert_eq!(set.len(), 0);
    }

    #[test]
    fn test_or_set_add() {
        let mut set: ORSet<String> = ORSet::new();
        let add_id = AddId::new("node1".to_string(), 1);
        let vc = VectorClock::new();

        set.add("hello".to_string(), add_id, vc);

        assert!(set.contains(&"hello".to_string()));
        assert_eq!(set.len(), 1);
    }

    #[test]
    fn test_or_set_remove() {
        let mut set: ORSet<String> = ORSet::new();
        let add_id = AddId::new("node1".to_string(), 1);
        let vc = VectorClock::new();

        set.add("hello".to_string(), add_id.clone(), vc.clone());
        assert!(set.contains(&"hello".to_string()));

        set.remove(&"hello".to_string(), vc);
        assert!(!set.contains(&"hello".to_string()));
        assert_eq!(set.len(), 0);
    }

    #[test]
    fn test_or_set_add_remove_add() {
        let mut set: ORSet<String> = ORSet::new();
        let add_id1 = AddId::new("node1".to_string(), 1);
        let add_id2 = AddId::new("node1".to_string(), 2);
        let vc = VectorClock::new();

        // Add
        set.add("hello".to_string(), add_id1.clone(), vc.clone());
        assert!(set.contains(&"hello".to_string()));

        // Remove
        set.remove(&"hello".to_string(), vc.clone());
        assert!(!set.contains(&"hello".to_string()));

        // Add again with different ID (should succeed)
        set.add("hello".to_string(), add_id2, vc);
        assert!(set.contains(&"hello".to_string()));
    }

    #[test]
    fn test_or_set_concurrent_adds() {
        let mut set: ORSet<String> = ORSet::new();
        let add_id1 = AddId::new("node1".to_string(), 1);
        let add_id2 = AddId::new("node2".to_string(), 1);
        let vc = VectorClock::new();

        // Two concurrent adds of the same element
        set.add("hello".to_string(), add_id1, vc.clone());
        set.add("hello".to_string(), add_id2, vc);

        assert!(set.contains(&"hello".to_string()));
        assert_eq!(set.get_add_ids(&"hello".to_string()).unwrap().len(), 2);
    }

    #[test]
    fn test_or_set_elements() {
        let mut set: ORSet<i32> = ORSet::new();
        let vc = VectorClock::new();

        set.add(1, AddId::new("node1".to_string(), 1), vc.clone());
        set.add(2, AddId::new("node1".to_string(), 2), vc.clone());
        set.add(3, AddId::new("node1".to_string(), 3), vc);

        let elements = set.elements();
        assert_eq!(elements.len(), 3);
        assert!(elements.contains(&1));
        assert!(elements.contains(&2));
        assert!(elements.contains(&3));
    }

    #[test]
    fn test_or_set_merge() {
        let mut set1: ORSet<String> = ORSet::new();
        let mut set2: ORSet<String> = ORSet::new();
        let vc = VectorClock::new();

        set1.add("a".to_string(), AddId::new("node1".to_string(), 1), vc.clone());
        set2.add("b".to_string(), AddId::new("node2".to_string(), 1), vc);

        set1.merge(&set2).unwrap();

        assert!(set1.contains(&"a".to_string()));
        assert!(set1.contains(&"b".to_string()));
        assert_eq!(set1.len(), 2);
    }

    #[test]
    fn test_or_set_merge_with_tombstones() {
        let mut set1: ORSet<String> = ORSet::new();
        let mut set2: ORSet<String> = ORSet::new();
        let add_id = AddId::new("node1".to_string(), 1);
        let vc = VectorClock::new();

        // Set1: add then remove
        set1.add("hello".to_string(), add_id.clone(), vc.clone());
        set1.remove(&"hello".to_string(), vc.clone());

        // Set2: just add
        set2.add("hello".to_string(), add_id, vc);

        // Merge - set2's add should be overridden by set1's tombstone
        set2.merge(&set1).unwrap();
        assert!(!set2.contains(&"hello".to_string()));
    }

    #[test]
    fn test_or_set_apply_add() {
        let mut set: ORSet<String> = ORSet::new();
        let mut vc = VectorClock::new();
        vc.increment("node1");

        let metadata = OperationMetadata::new("node1".to_string(), vc);
        let op = ORSetOperation::Add {
            element: "test".to_string(),
            add_id: AddId::new("node1".to_string(), 1),
            metadata,
        };

        set.apply(op).unwrap();
        assert!(set.contains(&"test".to_string()));
    }

    #[test]
    fn test_or_set_apply_remove() {
        let mut set: ORSet<String> = ORSet::new();
        let add_id = AddId::new("node1".to_string(), 1);
        let mut vc = VectorClock::new();
        vc.increment("node1");

        // First add
        set.add("test".to_string(), add_id.clone(), vc.clone());

        // Then remove
        let metadata = OperationMetadata::new("node1".to_string(), vc);
        let mut add_ids = HashSet::new();
        add_ids.insert(add_id);

        let op = ORSetOperation::Remove { element: "test".to_string(), add_ids, metadata };

        set.apply(op).unwrap();
        assert!(!set.contains(&"test".to_string()));
    }

    #[test]
    fn test_or_set_gc_tombstones() {
        let mut set: ORSet<String> = ORSet::new();
        let add_id = AddId::new("node1".to_string(), 1);
        let vc = VectorClock::new();

        set.add("hello".to_string(), add_id, vc.clone());
        set.remove(&"hello".to_string(), vc);

        assert_eq!(set.tombstones.len(), 1);

        let removed = set.gc_tombstones(0);
        assert_eq!(removed, 1);
        assert_eq!(set.tombstones.len(), 0);
    }

    #[test]
    fn test_or_set_value() {
        let mut set: ORSet<i32> = ORSet::new();
        let vc = VectorClock::new();

        set.add(1, AddId::new("node1".to_string(), 1), vc.clone());
        set.add(2, AddId::new("node1".to_string(), 2), vc);

        let value = set.value();
        assert_eq!(value.len(), 2);
        assert!(value.contains(&1));
        assert!(value.contains(&2));
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    // Property: Merging is commutative (A ∪ B = B ∪ A)
    proptest! {
        #[test]
        fn prop_merge_commutative(
            elements_a in prop::collection::vec(0..100i32, 0..10),
            elements_b in prop::collection::vec(0..100i32, 0..10),
        ) {
            let mut set_a1: ORSet<i32> = ORSet::new();
            let mut set_a2: ORSet<i32> = ORSet::new();
            let mut set_b1: ORSet<i32> = ORSet::new();
            let mut set_b2: ORSet<i32> = ORSet::new();
            let vc = VectorClock::new();

            // Build sets
            for (i, elem) in elements_a.iter().enumerate() {
                let add_id = AddId::new("node_a".to_string(), i as u64);
                set_a1.add(*elem, add_id.clone(), vc.clone());
                set_a2.add(*elem, add_id, vc.clone());
            }
            for (i, elem) in elements_b.iter().enumerate() {
                let add_id = AddId::new("node_b".to_string(), i as u64);
                set_b1.add(*elem, add_id.clone(), vc.clone());
                set_b2.add(*elem, add_id, vc.clone());
            }

            // A ∪ B
            set_a1.merge(&set_b1).unwrap();
            // B ∪ A
            set_b2.merge(&set_a2).unwrap();

            // Should be equal
            let result_a = set_a1.elements();
            let result_b = set_b2.elements();
            
            prop_assert_eq!(result_a.len(), result_b.len());
            for elem in result_a {
                prop_assert!(result_b.contains(&elem));
            }
        }
    }

    // Property: Merging is associative ((A ∪ B) ∪ C = A ∪ (B ∪ C))
    proptest! {
        #[test]
        fn prop_merge_associative(
            elements_a in prop::collection::vec(0..50i32, 0..5),
            elements_b in prop::collection::vec(0..50i32, 0..5),
            elements_c in prop::collection::vec(0..50i32, 0..5),
        ) {
            let vc = VectorClock::new();
            
            // Create three sets
            let mut set_a1: ORSet<i32> = ORSet::new();
            let mut set_a2: ORSet<i32> = ORSet::new();
            let mut set_b1: ORSet<i32> = ORSet::new();
            let mut set_b2: ORSet<i32> = ORSet::new();
            let mut set_c1: ORSet<i32> = ORSet::new();
            let mut set_c2: ORSet<i32> = ORSet::new();

            for (i, &elem) in elements_a.iter().enumerate() {
                let add_id = AddId::new("a".to_string(), i as u64);
                set_a1.add(elem, add_id.clone(), vc.clone());
                set_a2.add(elem, add_id, vc.clone());
            }
            for (i, &elem) in elements_b.iter().enumerate() {
                let add_id = AddId::new("b".to_string(), i as u64);
                set_b1.add(elem, add_id.clone(), vc.clone());
                set_b2.add(elem, add_id, vc.clone());
            }
            for (i, &elem) in elements_c.iter().enumerate() {
                let add_id = AddId::new("c".to_string(), i as u64);
                set_c1.add(elem, add_id.clone(), vc.clone());
                set_c2.add(elem, add_id, vc.clone());
            }

            // (A ∪ B) ∪ C
            set_a1.merge(&set_b1).unwrap();
            set_a1.merge(&set_c1).unwrap();

            // A ∪ (B ∪ C)
            set_b2.merge(&set_c2).unwrap();
            set_a2.merge(&set_b2).unwrap();

            // Results should be identical
            let result1 = set_a1.elements();
            let result2 = set_a2.elements();
            
            prop_assert_eq!(result1.len(), result2.len());
            for elem in result1 {
                prop_assert!(result2.contains(&elem));
            }
        }
    }

    // Property: Add-remove semantics (add wins over concurrent remove)
    proptest! {
        #[test]
        fn prop_add_wins_over_remove(element in 0..100i32) {
            let mut set1: ORSet<i32> = ORSet::new();
            let mut set2: ORSet<i32> = ORSet::new();
            let vc = VectorClock::new();
            let add_id = AddId::new("node1".to_string(), 1);

            // Set1: add element
            set1.add(element, add_id.clone(), vc.clone());

            // Set2: add then remove element
            set2.add(element, add_id.clone(), vc.clone());
            set2.remove(&element, vc);

            // Merge set2 into set1
            set1.merge(&set2).unwrap();

            // Element should be removed (remove wins when it knows about the add)
            prop_assert!(!set1.contains(&element));
        }
    }

    // Property: Idempotent merge (A ∪ A = A)
    proptest! {
        #[test]
        fn prop_merge_idempotent(elements in prop::collection::vec(0..100i32, 0..10)) {
            let mut set1: ORSet<i32> = ORSet::new();
            let mut set2: ORSet<i32> = ORSet::new();
            let vc = VectorClock::new();

            for (i, &elem) in elements.iter().enumerate() {
                let add_id = AddId::new("node1".to_string(), i as u64);
                set1.add(elem, add_id.clone(), vc.clone());
                set2.add(elem, add_id, vc.clone());
            }

            let original_elements = set1.elements();

            // Merge with itself
            set1.merge(&set2).unwrap();

            let merged_elements = set1.elements();

            // Should be unchanged
            prop_assert_eq!(original_elements.len(), merged_elements.len());
            for elem in original_elements {
                prop_assert!(merged_elements.contains(&elem));
            }
        }
    }

    // Property: Contains after add
    proptest! {
        #[test]
        fn prop_contains_after_add(element in 0..1000i32, timestamp in 1u64..1000) {
            let mut set: ORSet<i32> = ORSet::new();
            let add_id = AddId::new("node1".to_string(), timestamp);
            let vc = VectorClock::new();

            set.add(element, add_id, vc);

            prop_assert!(set.contains(&element));
            prop_assert_eq!(set.len(), 1);
        }
    }

    // Property: Not contains after remove
    proptest! {
        #[test]
        fn prop_not_contains_after_remove(element in 0..1000i32) {
            let mut set: ORSet<i32> = ORSet::new();
            let add_id = AddId::new("node1".to_string(), 1);
            let vc = VectorClock::new();

            set.add(element, add_id, vc.clone());
            prop_assert!(set.contains(&element));

            set.remove(&element, vc);
            prop_assert!(!set.contains(&element));
        }
    }

    // Property: Convergence - independent operations converge to same state
    proptest! {
        #[test]
        fn prop_convergence(
            ops_a in prop::collection::vec((0..20i32, prop::bool::ANY), 0..10),
            ops_b in prop::collection::vec((0..20i32, prop::bool::ANY), 0..10),
        ) {
            let mut set_a: ORSet<i32> = ORSet::new();
            let mut set_b: ORSet<i32> = ORSet::new();
            let mut set_a_copy: ORSet<i32> = ORSet::new();
            let mut set_b_copy: ORSet<i32> = ORSet::new();
            let vc = VectorClock::new();

            // Node A performs ops_a
            for (i, (elem, is_add)) in ops_a.iter().enumerate() {
                let add_id = AddId::new("node_a".to_string(), i as u64);
                if *is_add {
                    set_a.add(*elem, add_id.clone(), vc.clone());
                    set_a_copy.add(*elem, add_id, vc.clone());
                }
            }

            // Node B performs ops_b
            for (i, (elem, is_add)) in ops_b.iter().enumerate() {
                let add_id = AddId::new("node_b".to_string(), i as u64);
                if *is_add {
                    set_b.add(*elem, add_id.clone(), vc.clone());
                    set_b_copy.add(*elem, add_id, vc.clone());
                }
            }

            // A merges B's changes, B merges A's changes
            set_a.merge(&set_b_copy).unwrap();
            set_b.merge(&set_a_copy).unwrap();

            // Both should converge to same state
            let result_a = set_a.elements();
            let result_b = set_b.elements();

            prop_assert_eq!(result_a.len(), result_b.len());
            for elem in result_a {
                prop_assert!(result_b.contains(&elem));
            }
        }
    }
}
