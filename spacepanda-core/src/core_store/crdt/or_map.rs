/*
    or_map.rs - Observed-Remove Map CRDT
    
    A map that supports put and remove operations.
    Each key-value pair is tracked with unique IDs similar to OR-Set.
    Values for a key can be CRDTs themselves (e.g., LWW registers).
    
    Use cases:
    - User roles (user_id -> role_level)
    - Channel properties (property_name -> value)
    - Metadata maps
*/

use super::traits::{Crdt, OperationMetadata};
use super::vector_clock::VectorClock;
use super::or_set::{AddId, ORSet};
use crate::core_store::store::errors::StoreResult;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Observed-Remove Map CRDT
/// Each key maps to a value tracked by OR-Set semantics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ORMap<K: Clone + Eq + std::hash::Hash, V: Clone> {
    /// Map from key to (value, add_ids)
    map: HashMap<K, (V, ORSet<K>)>,
    
    /// Vector clock for causal ordering
    vector_clock: VectorClock,
}

/// Operations for OR-Map
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ORMapOperation<K: Clone, V: Clone> {
    /// Put a key-value pair
    Put {
        key: K,
        value: V,
        add_id: AddId,
        metadata: OperationMetadata,
    },
    /// Remove a key
    Remove {
        key: K,
        metadata: OperationMetadata,
    },
}

impl<K: Clone + Eq + std::hash::Hash, V: Clone> ORMap<K, V> {
    /// Create a new empty OR-Map
    pub fn new() -> Self {
        ORMap {
            map: HashMap::new(),
            vector_clock: VectorClock::new(),
        }
    }
    
    /// Put a key-value pair
    pub fn put(&mut self, key: K, value: V, add_id: AddId, vector_clock: VectorClock) {
        // Get existing OR-Set or create new one
        if let Some((existing_value, existing_set)) = self.map.get_mut(&key) {
            // Key exists - add to existing OR-Set and update value
            existing_set.add(key.clone(), add_id, vector_clock.clone());
            *existing_value = value;
        } else {
            // New key - create new OR-Set
            let mut key_set = ORSet::new();
            key_set.add(key.clone(), add_id, vector_clock.clone());
            self.map.insert(key, (value, key_set));
        }
        self.vector_clock.merge(&vector_clock);
    }
    
    /// Get a value by key
    pub fn get(&self, key: &K) -> Option<&V> {
        self.map.get(key).map(|(v, _)| v)
    }
    
    /// Remove a key
    pub fn remove(&mut self, key: &K, vector_clock: VectorClock) {
        self.map.remove(key);
        self.vector_clock.merge(&vector_clock);
    }
    
    /// Check if a key exists
    pub fn contains_key(&self, key: &K) -> bool {
        self.map.contains_key(key)
    }
    
    /// Get all keys
    pub fn keys(&self) -> Vec<K> {
        self.map.keys().cloned().collect()
    }
    
    /// Get all entries as (key, value) pairs
    pub fn entries(&self) -> Vec<(K, V)> {
        self.map.iter().map(|(k, (v, _))| (k.clone(), v.clone())).collect()
    }
    
    /// Get the number of entries
    pub fn len(&self) -> usize {
        self.map.len()
    }
    
    /// Check if the map is empty
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

impl<K: Clone + Eq + std::hash::Hash + Send + Sync, V: Clone + Send + Sync> Crdt for ORMap<K, V> {
    type Operation = ORMapOperation<K, V>;
    type Value = HashMap<K, V>;
    
    fn apply(&mut self, op: Self::Operation) -> StoreResult<()> {
        match op {
            ORMapOperation::Put {
                key,
                value,
                add_id,
                metadata,
            } => {
                self.put(key, value, add_id, metadata.vector_clock);
            }
            ORMapOperation::Remove { key, metadata } => {
                self.remove(&key, metadata.vector_clock);
            }
        }
        Ok(())
    }
    
    fn merge(&mut self, other: &Self) -> StoreResult<()> {
        for (key, (value, other_set)) in &other.map {
            if let Some((self_value, self_set)) = self.map.get_mut(key) {
                // Key exists in both maps - merge the OR-Sets
                self_set.merge(other_set)?;
                // Replace value with other's (simple last-write-wins)
                // Note: For CRDT values, use merge_nested() instead
                *self_value = value.clone();
            } else {
                // Key only in other - insert it
                self.map.insert(key.clone(), (value.clone(), other_set.clone()));
            }
        }
        
        self.vector_clock.merge(&other.vector_clock);
        Ok(())
    }
    
    fn value(&self) -> Self::Value {
        self.map.iter().map(|(k, (v, _))| (k.clone(), v.clone())).collect()
    }
    
    fn vector_clock(&self) -> &VectorClock {
        &self.vector_clock
    }
}

impl<K: Clone + Eq + std::hash::Hash, V: Clone> Default for ORMap<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

/// Specialized implementation for ORMap with CRDT values
/// This allows proper merging of nested CRDTs
impl<K: Clone + Eq + std::hash::Hash + Send + Sync, V: Clone + Crdt> ORMap<K, V> 
where
    V: Send + Sync,
{
    /// Merge with proper CRDT semantics for nested values
    pub fn merge_nested(&mut self, other: &Self) -> StoreResult<()> {
        for (key, (other_value, other_set)) in &other.map {
            if let Some((self_value, self_set)) = self.map.get_mut(key) {
                // Key exists in both - merge both the OR-Set and the CRDT value
                self_set.merge(other_set)?;
                self_value.merge(other_value)?;
            } else {
                // Key only in other - insert it
                self.map.insert(key.clone(), (other_value.clone(), other_set.clone()));
            }
        }
        
        self.vector_clock.merge(&other.vector_clock);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_or_map_creation() {
        let map: ORMap<String, i32> = ORMap::new();
        assert!(map.is_empty());
        assert_eq!(map.len(), 0);
    }
    
    #[test]
    fn test_or_map_put() {
        let mut map: ORMap<String, i32> = ORMap::new();
        let add_id = AddId::new("node1".to_string(), 1);
        let vc = VectorClock::new();
        
        map.put("key1".to_string(), 42, add_id, vc);
        
        assert!(map.contains_key(&"key1".to_string()));
        assert_eq!(map.get(&"key1".to_string()), Some(&42));
        assert_eq!(map.len(), 1);
    }
    
    #[test]
    fn test_or_map_put_update() {
        let mut map: ORMap<String, i32> = ORMap::new();
        let vc = VectorClock::new();
        
        map.put("key1".to_string(), 42, AddId::new("node1".to_string(), 1), vc.clone());
        map.put("key1".to_string(), 99, AddId::new("node1".to_string(), 2), vc);
        
        assert_eq!(map.get(&"key1".to_string()), Some(&99));
    }
    
    #[test]
    fn test_or_map_remove() {
        let mut map: ORMap<String, i32> = ORMap::new();
        let add_id = AddId::new("node1".to_string(), 1);
        let vc = VectorClock::new();
        
        map.put("key1".to_string(), 42, add_id, vc.clone());
        assert!(map.contains_key(&"key1".to_string()));
        
        map.remove(&"key1".to_string(), vc);
        assert!(!map.contains_key(&"key1".to_string()));
        assert_eq!(map.len(), 0);
    }
    
    #[test]
    fn test_or_map_keys() {
        let mut map: ORMap<String, i32> = ORMap::new();
        let vc = VectorClock::new();
        
        map.put("key1".to_string(), 1, AddId::new("node1".to_string(), 1), vc.clone());
        map.put("key2".to_string(), 2, AddId::new("node1".to_string(), 2), vc.clone());
        map.put("key3".to_string(), 3, AddId::new("node1".to_string(), 3), vc);
        
        let keys = map.keys();
        assert_eq!(keys.len(), 3);
        assert!(keys.contains(&"key1".to_string()));
        assert!(keys.contains(&"key2".to_string()));
        assert!(keys.contains(&"key3".to_string()));
    }
    
    #[test]
    fn test_or_map_entries() {
        let mut map: ORMap<String, i32> = ORMap::new();
        let vc = VectorClock::new();
        
        map.put("a".to_string(), 1, AddId::new("node1".to_string(), 1), vc.clone());
        map.put("b".to_string(), 2, AddId::new("node1".to_string(), 2), vc);
        
        let entries = map.entries();
        assert_eq!(entries.len(), 2);
    }
    
    #[test]
    fn test_or_map_merge() {
        let mut map1: ORMap<String, i32> = ORMap::new();
        let mut map2: ORMap<String, i32> = ORMap::new();
        let vc = VectorClock::new();
        
        map1.put("a".to_string(), 1, AddId::new("node1".to_string(), 1), vc.clone());
        map2.put("b".to_string(), 2, AddId::new("node2".to_string(), 1), vc);
        
        map1.merge(&map2).unwrap();
        
        assert!(map1.contains_key(&"a".to_string()));
        assert!(map1.contains_key(&"b".to_string()));
        assert_eq!(map1.len(), 2);
    }
    
    #[test]
    fn test_or_map_apply_put() {
        let mut map: ORMap<String, i32> = ORMap::new();
        let mut vc = VectorClock::new();
        vc.increment("node1");
        
        let metadata = OperationMetadata::new("node1".to_string(), vc);
        let op = ORMapOperation::Put {
            key: "test".to_string(),
            value: 42,
            add_id: AddId::new("node1".to_string(), 1),
            metadata,
        };
        
        map.apply(op).unwrap();
        assert_eq!(map.get(&"test".to_string()), Some(&42));
    }
    
    #[test]
    fn test_or_map_apply_remove() {
        let mut map: ORMap<String, i32> = ORMap::new();
        let mut vc = VectorClock::new();
        vc.increment("node1");
        
        map.put("test".to_string(), 42, AddId::new("node1".to_string(), 1), vc.clone());
        
        let metadata = OperationMetadata::new("node1".to_string(), vc);
        let op = ORMapOperation::Remove {
            key: "test".to_string(),
            metadata,
        };
        
        map.apply(op).unwrap();
        assert!(!map.contains_key(&"test".to_string()));
    }
    
    #[test]
    fn test_or_map_value() {
        let mut map: ORMap<String, i32> = ORMap::new();
        let vc = VectorClock::new();
        
        map.put("a".to_string(), 1, AddId::new("node1".to_string(), 1), vc.clone());
        map.put("b".to_string(), 2, AddId::new("node1".to_string(), 2), vc);
        
        let value = map.value();
        assert_eq!(value.len(), 2);
        assert_eq!(value.get(&"a".to_string()), Some(&1));
        assert_eq!(value.get(&"b".to_string()), Some(&2));
    }
}
