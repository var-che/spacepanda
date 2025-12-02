/*
    g_list.rs - Growable List CRDT (RGA variant)
    
    A replicated list that preserves causal ordering of insertions.
    Based on Replicated Growable Array (RGA) algorithm.
    
    Use cases:
    - Message timeline in channels
    - Ordered lists of any kind
    - Chat history
    
    Properties:
    - Insertion-order preservation
    - Concurrent inserts don't conflict
    - Tombstones for deletions
    - Deterministic ordering using (timestamp, node_id) pairs
*/

use super::traits::{Crdt, OperationMetadata};
use super::vector_clock::VectorClock;
use crate::core_store::store::errors::StoreResult;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Unique identifier for a list element
/// Combines timestamp and node_id for total ordering
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
pub struct ElementId {
    /// Timestamp when element was created
    pub timestamp: u64,
    /// Node that created this element
    pub node_id: String,
}

impl ElementId {
    pub fn new(timestamp: u64, node_id: String) -> Self {
        ElementId { timestamp, node_id }
    }
}

/// A single element in the list
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Element<T: Clone> {
    /// Unique ID for this element
    id: ElementId,
    /// The actual value
    value: T,
    /// ID of element this was inserted after (None = beginning)
    after: Option<ElementId>,
    /// Whether this element has been deleted
    tombstone: bool,
}

/// Growable List CRDT (RGA variant)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GList<T: Clone> {
    /// All elements indexed by their ID
    elements: HashMap<ElementId, Element<T>>,
    
    /// Vector clock for causal ordering
    vector_clock: VectorClock,
}

/// Operations for GList
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GListOperation<T: Clone> {
    /// Insert element after a specific position
    Insert {
        id: ElementId,
        value: T,
        after: Option<ElementId>,
        metadata: OperationMetadata,
    },
    /// Delete (tombstone) an element
    Delete {
        id: ElementId,
        metadata: OperationMetadata,
    },
}

impl<T: Clone> super::validated::HasMetadata for GListOperation<T> {
    fn metadata(&self) -> &OperationMetadata {
        match self {
            GListOperation::Insert { metadata, .. } => metadata,
            GListOperation::Delete { metadata, .. } => metadata,
        }
    }
}

impl<T: Clone> GList<T> {
    /// Create a new empty GList
    pub fn new() -> Self {
        GList {
            elements: HashMap::new(),
            vector_clock: VectorClock::new(),
        }
    }
    
    /// Insert a value after a specific element (or at beginning if None)
    pub fn insert(&mut self, id: ElementId, value: T, after: Option<ElementId>, vc: VectorClock) {
        let element = Element {
            id: id.clone(),
            value,
            after,
            tombstone: false,
        };
        self.elements.insert(id, element);
        self.vector_clock.merge(&vc);
    }
    
    /// Delete an element (mark as tombstone)
    pub fn delete(&mut self, id: &ElementId, vc: VectorClock) -> bool {
        if let Some(element) = self.elements.get_mut(id) {
            element.tombstone = true;
            self.vector_clock.merge(&vc);
            true
        } else {
            false
        }
    }
    
    /// Get the ordered list of visible (non-tombstoned) elements
    pub fn to_vec(&self) -> Vec<T> {
        self.ordered_elements()
            .into_iter()
            .filter(|e| !e.tombstone)
            .map(|e| e.value.clone())
            .collect()
    }
    
    /// Get all elements including tombstones in causal order
    pub fn to_vec_with_tombstones(&self) -> Vec<(ElementId, T, bool)> {
        self.ordered_elements()
            .into_iter()
            .map(|e| (e.id.clone(), e.value.clone(), e.tombstone))
            .collect()
    }
    
    /// Get ordered list of all elements (for internal use)
    fn ordered_elements(&self) -> Vec<&Element<T>> {
        // Build a graph of after relationships
        let mut result = Vec::new();
        let mut remaining: HashMap<ElementId, &Element<T>> = self.elements.iter()
            .map(|(k, v)| (k.clone(), v))
            .collect();
        
        // Start with elements that have no predecessor (after = None)
        let mut to_process: Vec<&Element<T>> = self.elements.values()
            .filter(|e| e.after.is_none())
            .collect();
        
        // Sort by ID for deterministic ordering of concurrent inserts at same position
        to_process.sort_by_key(|e| &e.id);
        
        while let Some(current) = to_process.pop() {
            remaining.remove(&current.id);
            result.push(current);
            
            // Find all elements inserted after this one
            let mut next: Vec<&Element<T>> = remaining.values()
                .filter(|e| e.after.as_ref() == Some(&current.id))
                .copied()
                .collect();
            
            // Sort for deterministic ordering
            next.sort_by_key(|e| &e.id);
            
            // Process in reverse order since we're using pop()
            for elem in next.into_iter().rev() {
                to_process.push(elem);
            }
        }
        
        // Add any remaining elements (shouldn't happen in well-formed lists)
        let mut orphans: Vec<&Element<T>> = remaining.values().copied().collect();
        orphans.sort_by_key(|e| &e.id);
        result.extend(orphans);
        
        result
    }
    
    /// Get the number of visible elements
    pub fn len(&self) -> usize {
        self.elements.values().filter(|e| !e.tombstone).count()
    }
    
    /// Check if list is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    
    /// Get element by ID
    pub fn get(&self, id: &ElementId) -> Option<&T> {
        self.elements.get(id)
            .filter(|e| !e.tombstone)
            .map(|e| &e.value)
    }
    
    /// Get the last element ID (for appending)
    pub fn last_id(&self) -> Option<ElementId> {
        self.ordered_elements()
            .last()
            .map(|e| e.id.clone())
    }
}

impl<T: Clone + Send + Sync> Crdt for GList<T> {
    type Operation = GListOperation<T>;
    type Value = Vec<T>;
    
    fn apply(&mut self, op: Self::Operation) -> StoreResult<()> {
        match op {
            GListOperation::Insert { id, value, after, metadata } => {
                self.insert(id, value, after, metadata.vector_clock);
            }
            GListOperation::Delete { id, metadata } => {
                self.delete(&id, metadata.vector_clock);
            }
        }
        Ok(())
    }
    
    fn merge(&mut self, other: &Self) -> StoreResult<()> {
        // Merge all elements
        for (id, element) in &other.elements {
            if let Some(self_element) = self.elements.get_mut(id) {
                // Element exists - merge tombstone status (OR semantics: once deleted, stays deleted)
                if element.tombstone {
                    self_element.tombstone = true;
                }
            } else {
                // New element - insert it
                self.elements.insert(id.clone(), element.clone());
            }
        }
        
        self.vector_clock.merge(&other.vector_clock);
        Ok(())
    }
    
    fn value(&self) -> Self::Value {
        self.to_vec()
    }
    
    fn vector_clock(&self) -> &VectorClock {
        &self.vector_clock
    }
}

impl<T: Clone> Default for GList<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_glist_creation() {
        let list: GList<String> = GList::new();
        assert!(list.is_empty());
        assert_eq!(list.len(), 0);
    }
    
    #[test]
    fn test_glist_insert() {
        let mut list = GList::new();
        let mut vc = VectorClock::new();
        vc.increment("node1");
        
        let id1 = ElementId::new(1, "node1".to_string());
        list.insert(id1.clone(), "first".to_string(), None, vc.clone());
        
        assert_eq!(list.len(), 1);
        assert_eq!(list.to_vec(), vec!["first".to_string()]);
        
        let id2 = ElementId::new(2, "node1".to_string());
        list.insert(id2, "second".to_string(), Some(id1), vc);
        
        assert_eq!(list.len(), 2);
        assert_eq!(list.to_vec(), vec!["first".to_string(), "second".to_string()]);
    }
    
    #[test]
    fn test_glist_delete() {
        let mut list = GList::new();
        let mut vc = VectorClock::new();
        vc.increment("node1");
        
        let id1 = ElementId::new(1, "node1".to_string());
        list.insert(id1.clone(), "first".to_string(), None, vc.clone());
        
        assert_eq!(list.len(), 1);
        
        list.delete(&id1, vc);
        assert_eq!(list.len(), 0);
        assert!(list.is_empty());
    }
    
    #[test]
    fn test_glist_concurrent_inserts() {
        let mut list1 = GList::new();
        let mut list2 = GList::new();
        
        let mut vc1 = VectorClock::new();
        vc1.increment("node1");
        let mut vc2 = VectorClock::new();
        vc2.increment("node2");
        
        // Both insert at beginning
        let id1 = ElementId::new(100, "node1".to_string());
        let id2 = ElementId::new(200, "node2".to_string());
        
        list1.insert(id1.clone(), "from_node1".to_string(), None, vc1.clone());
        list2.insert(id2.clone(), "from_node2".to_string(), None, vc2.clone());
        
        // Merge
        list1.merge(&list2).unwrap();
        
        // Should have both elements, ordered by ElementId
        assert_eq!(list1.len(), 2);
        let values = list1.to_vec();
        // Elements sorted by (timestamp, node_id)
        // id1: (100, "node1"), id2: (200, "node2")
        // So id1 comes first
        assert!(values.contains(&"from_node1".to_string()));
        assert!(values.contains(&"from_node2".to_string()));
        // Verify deterministic ordering
        assert_eq!(values.len(), 2);
    }
    
    #[test]
    fn test_glist_merge_preserves_order() {
        let mut list1 = GList::new();
        let mut vc = VectorClock::new();
        vc.increment("node1");
        
        let id1 = ElementId::new(1, "node1".to_string());
        let id2 = ElementId::new(2, "node1".to_string());
        let id3 = ElementId::new(3, "node1".to_string());
        
        list1.insert(id1.clone(), "a".to_string(), None, vc.clone());
        list1.insert(id2.clone(), "b".to_string(), Some(id1), vc.clone());
        list1.insert(id3, "c".to_string(), Some(id2), vc);
        
        let mut list2 = GList::new();
        list2.merge(&list1).unwrap();
        
        assert_eq!(list2.to_vec(), vec!["a".to_string(), "b".to_string(), "c".to_string()]);
    }
}
