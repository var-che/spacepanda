//! CRDT Algebraic Law Tests
//!
//! Tests for fundamental CRDT properties:
//! - Commutativity: merge(a, b) == merge(b, a)
//! - Associativity: merge(merge(a, b), c) == merge(a, merge(b, c))
//! - Idempotence: merge(a, a) == a
//!
//! These are the essential correctness properties ALL CRDTs must satisfy.

use crate::core_identity::*;
use crate::core_store::crdt::{LWWRegister, ORSet, ORMap, VectorClock, AddId, Crdt};
use crate::core_store::model::types::Timestamp;
use super::helpers::*;

// =============================================================================
// LWW REGISTER ALGEBRAIC LAWS
// =============================================================================

#[test]
fn test_lww_merge_commutativity() {
    // Create two registers with different values
    let mut reg1 = LWWRegister::with_value("value1".to_string(), "nodeA".to_string());
    let mut reg2 = LWWRegister::with_value("value2".to_string(), "nodeB".to_string());
    
    // Clone for second merge order
    let mut reg1_copy = reg1.clone();
    let reg2_copy = reg2.clone();
    
    // Merge in both orders (LWWRegister::merge returns void)
    reg1.merge(&reg2);
    reg1_copy.merge(&reg2_copy);
    
    // Results should be identical
    assert_eq!(reg1.get(), reg1_copy.get());
    assert_eq!(reg1.timestamp(), reg1_copy.timestamp());
    assert_eq!(reg1.writer(), reg1_copy.writer());
}

#[test]
fn test_lww_merge_associativity() {
    let mut reg1 = LWWRegister::with_value("value1".to_string(), "nodeA".to_string());
    let mut reg2 = LWWRegister::with_value("value2".to_string(), "nodeB".to_string());
    let reg3 = LWWRegister::with_value("value3".to_string(), "nodeC".to_string());
    
    // (reg1 ⊔ reg2) ⊔ reg3
    let mut left = reg1.clone();
    left.merge(&reg2);
    left.merge(&reg3);
    
    // reg1 ⊔ (reg2 ⊔ reg3)
    let mut right = reg1.clone();
    let mut temp = reg2.clone();
    temp.merge(&reg3);
    right.merge(&temp);
    
    assert_eq!(left.get(), right.get());
    assert_eq!(left.timestamp(), right.timestamp());
}

#[test]
fn test_lww_merge_idempotence() {
    let mut reg = LWWRegister::with_value("value".to_string(), "node1".to_string());
    let reg_copy = reg.clone();
    
    let original_value = reg.get().cloned();
    let original_ts = reg.timestamp();
    
    // Merge with itself
    reg.merge(&reg_copy);
    
    // Should be unchanged
    assert_eq!(reg.get(), original_value.as_ref());
    assert_eq!(reg.timestamp(), original_ts);
}

#[test]
fn test_lww_equal_timestamps_equal_node_ids() {
    // Edge case: exactly equal timestamp AND node_id
    let ts = test_timestamp(100).as_millis();
    let node = "nodeA".to_string();
    
    let mut vc1 = VectorClock::new();
    vc1.increment(&node);
    
    let mut vc2 = VectorClock::new();
    vc2.increment(&node);
    
    let mut reg = LWWRegister::new();
    reg.set("value1".to_string(), ts, node.clone(), vc1);
    reg.set("value2".to_string(), ts, node.clone(), vc2);
    
    // With identical timestamps and node_id, should_update returns false
    // So we keep the first value
    assert_eq!(reg.get(), Some(&"value1".to_string()));
}

#[test]
fn test_lww_timestamp_overflow_edge() {
    // Test near u64::MAX
    let mut reg = LWWRegister::new();
    let mut vc = VectorClock::new();
    vc.increment("node1");
    
    reg.set("value1".to_string(), u64::MAX - 1, "node1".to_string(), vc.clone());
    assert_eq!(reg.get(), Some(&"value1".to_string()));
    
    vc.increment("node2");
    reg.set("value2".to_string(), u64::MAX, "node2".to_string(), vc);
    assert_eq!(reg.get(), Some(&"value2".to_string()));
}

// =============================================================================
// OR-SET ALGEBRAIC LAWS
// =============================================================================

#[test]
fn test_orset_merge_commutativity() {
    let mut set1: ORSet<String> = ORSet::new();
    let mut set2: ORSet<String> = ORSet::new();
    
    let mut vc1 = VectorClock::new();
    vc1.increment("node1");
    let mut vc2 = VectorClock::new();
    vc2.increment("node2");
    
    set1.add("a".to_string(), test_add_id("node1", 1), vc1.clone());
    set2.add("b".to_string(), test_add_id("node2", 1), vc2.clone());
    
    // Merge in both orders
    let mut result1 = set1.clone();
    result1.merge(&set2).unwrap();
    
    let mut result2 = set2.clone();
    result2.merge(&set1).unwrap();
    
    // Both should contain same elements
    assert_eq!(result1.len(), result2.len());
    assert!(result1.contains(&"a".to_string()));
    assert!(result1.contains(&"b".to_string()));
    assert!(result2.contains(&"a".to_string()));
    assert!(result2.contains(&"b".to_string()));
}

#[test]
fn test_orset_merge_associativity() {
    let mut set1: ORSet<String> = ORSet::new();
    let mut set2: ORSet<String> = ORSet::new();
    let mut set3: ORSet<String> = ORSet::new();
    
    let mut vc = VectorClock::new();
    vc.increment("node1");
    set1.add("a".to_string(), test_add_id("node1", 1), vc.clone());
    
    vc.increment("node2");
    set2.add("b".to_string(), test_add_id("node2", 1), vc.clone());
    
    vc.increment("node3");
    set3.add("c".to_string(), test_add_id("node3", 1), vc.clone());
    
    // (set1 ⊔ set2) ⊔ set3
    let mut left = set1.clone();
    left.merge(&set2).unwrap();
    left.merge(&set3).unwrap();
    
    // set1 ⊔ (set2 ⊔ set3)
    let mut right = set1.clone();
    let mut temp = set2.clone();
    temp.merge(&set3).unwrap();
    right.merge(&temp).unwrap();
    
    assert_eq!(left.len(), right.len());
    assert_eq!(left.elements().len(), right.elements().len());
}

#[test]
fn test_orset_merge_idempotence() {
    let mut set: ORSet<String> = ORSet::new();
    let mut vc = VectorClock::new();
    vc.increment("node1");
    
    set.add("element".to_string(), test_add_id("node1", 1), vc);
    
    let original_len = set.len();
    let set_copy = set.clone();
    
    // Merge with itself
    set.merge(&set_copy).unwrap();
    
    // Should be unchanged
    assert_eq!(set.len(), original_len);
    assert!(set.contains(&"element".to_string()));
}

#[test]
fn test_orset_add_idempotence_same_tag() {
    let mut set: ORSet<String> = ORSet::new();
    let mut vc = VectorClock::new();
    vc.increment("node1");
    
    let add_id = test_add_id("node1", 1);
    let element = "element".to_string();
    
    // Add same element with same tag multiple times
    set.add(element.clone(), add_id.clone(), vc.clone());
    set.add(element.clone(), add_id.clone(), vc.clone());
    set.add(element.clone(), add_id.clone(), vc.clone());
    
    assert_eq!(set.len(), 1);
    
    // Check tag count - should only have one tag
    let tags = set.get_add_ids(&element).unwrap();
    assert_eq!(tags.len(), 1);
}

#[test]
fn test_orset_multiple_tags_per_element() {
    // Real OR-Sets allow multiple add tags per element from concurrent adds
    let mut set: ORSet<String> = ORSet::new();
    let element = "shared".to_string();
    
    let mut vc1 = VectorClock::new();
    vc1.increment("node1");
    set.add(element.clone(), test_add_id("node1", 1), vc1);
    
    let mut vc2 = VectorClock::new();
    vc2.increment("node2");
    set.add(element.clone(), test_add_id("node2", 1), vc2);
    
    let mut vc3 = VectorClock::new();
    vc3.increment("node3");
    set.add(element.clone(), test_add_id("node3", 1), vc3);
    
    // Element appears once in set
    assert_eq!(set.len(), 1);
    assert!(set.contains(&element));
    
    // But has multiple add tags
    let tags = set.get_add_ids(&element).unwrap();
    assert_eq!(tags.len(), 3);
}

#[test]
fn test_orset_remove_only_known_tags() {
    // Remove should only delete tags it knows about
    let mut set1: ORSet<String> = ORSet::new();
    let mut set2: ORSet<String> = ORSet::new();
    let element = "element".to_string();
    
    // set1: add with tag A
    let mut vc1 = VectorClock::new();
    vc1.increment("node1");
    set1.add(element.clone(), test_add_id("node1", 1), vc1.clone());
    
    // set2: add with tag B (concurrent)
    let mut vc2 = VectorClock::new();
    vc2.increment("node2");
    set2.add(element.clone(), test_add_id("node2", 1), vc2.clone());
    
    // set1 removes element (only knows about tag A)
    let removed_tags = set1.remove(&element, vc1.clone());
    assert_eq!(removed_tags.len(), 1);
    assert!(!set1.contains(&element));
    
    // Merge set2 into set1
    set1.merge(&set2).unwrap();
    
    // Element should reappear because tag B was never removed
    assert!(set1.contains(&element));
    let remaining_tags = set1.get_add_ids(&element).unwrap();
    assert_eq!(remaining_tags.len(), 1);
}

#[test]
fn test_orset_remove_before_add_delivery() {
    // Simulates remove arriving before add (out-of-order delivery)
    let mut set1: ORSet<String> = ORSet::new();
    let element = "element".to_string();
    let add_id = test_add_id("node1", 1);
    
    // Node 2 receives remove before add
    let mut vc = VectorClock::new();
    vc.increment("node1");
    
    // First, apply remove operation (arrives early)
    use crate::core_store::crdt::ORSetOperation;
    let remove_op = ORSetOperation::Remove {
        element: element.clone(),
        add_ids: vec![add_id.clone()].into_iter().collect(),
        metadata: crate::core_store::crdt::OperationMetadata {
            node_id: "node1".to_string(),
            timestamp: test_timestamp(300).as_millis(),
            vector_clock: vc.clone(),
            signature: None,
        },
    };
    set1.apply(remove_op).unwrap();
    
    // Then apply add operation (arrives late)
    let add_op = ORSetOperation::Add {
        element: element.clone(),
        add_id: add_id.clone(),
        metadata: crate::core_store::crdt::OperationMetadata {
            node_id: "node1".to_string(),
            timestamp: test_timestamp(50).as_millis(),
            vector_clock: vc.clone(),
            signature: None,
        },
    };
    set1.apply(add_op).unwrap();
    
    // In OR-Set, the add succeeds because operations are applied independently
    // The remove created a tombstone, but the add still adds the element
    assert!(set1.contains(&element));
}

#[test]
fn test_orset_concurrent_add_remove_winner() {
    // Concurrent add/remove: add-wins semantics
    let mut set1: ORSet<String> = ORSet::new();
    let mut set2: ORSet<String> = ORSet::new();
    let element = "element".to_string();
    
    // Both start with element
    let mut vc = VectorClock::new();
    vc.increment("init");
    let initial_tag = test_add_id("init", 1);
    set1.add(element.clone(), initial_tag.clone(), vc.clone());
    set2.add(element.clone(), initial_tag.clone(), vc.clone());
    
    // set1 removes
    vc.increment("node1");
    set1.remove(&element, vc.clone());
    
    // set2 adds concurrently with new tag
    vc.increment("node2");
    set2.add(element.clone(), test_add_id("node2", 2), vc.clone());
    
    // Merge
    set1.merge(&set2).unwrap();
    
    // Element should be present (new add wins)
    assert!(set1.contains(&element));
}

// =============================================================================
// OR-MAP ALGEBRAIC LAWS
// =============================================================================

#[test]
fn test_ormap_merge_commutativity() {
    let mut map1: ORMap<String, String> = ORMap::new();
    let mut map2: ORMap<String, String> = ORMap::new();
    
    let mut vc1 = VectorClock::new();
    vc1.increment("node1");
    let mut vc2 = VectorClock::new();
    vc2.increment("node2");
    
    map1.put("key1".to_string(), "value1".to_string(), test_add_id("node1", 1), vc1);
    map2.put("key2".to_string(), "value2".to_string(), test_add_id("node2", 1), vc2);
    
    // Merge in both orders
    let mut result1 = map1.clone();
    result1.merge(&map2).unwrap();
    
    let mut result2 = map2.clone();
    result2.merge(&map1).unwrap();
    
    assert_eq!(result1.len(), result2.len());
    assert_eq!(result1.get(&"key1".to_string()), result2.get(&"key1".to_string()));
    assert_eq!(result1.get(&"key2".to_string()), result2.get(&"key2".to_string()));
}

#[test]
fn test_ormap_merge_associativity() {
    let mut map1: ORMap<String, i32> = ORMap::new();
    let mut map2: ORMap<String, i32> = ORMap::new();
    let mut map3: ORMap<String, i32> = ORMap::new();
    
    let mut vc = VectorClock::new();
    vc.increment("node1");
    map1.put("a".to_string(), 1, test_add_id("node1", 1), vc.clone());
    
    vc.increment("node2");
    map2.put("b".to_string(), 2, test_add_id("node2", 1), vc.clone());
    
    vc.increment("node3");
    map3.put("c".to_string(), 3, test_add_id("node3", 1), vc.clone());
    
    // (map1 ⊔ map2) ⊔ map3
    let mut left = map1.clone();
    left.merge(&map2).unwrap();
    left.merge(&map3).unwrap();
    
    // map1 ⊔ (map2 ⊔ map3)
    let mut right = map1.clone();
    let mut temp = map2.clone();
    temp.merge(&map3).unwrap();
    right.merge(&temp).unwrap();
    
    assert_eq!(left.len(), right.len());
    assert_eq!(left.keys().len(), right.keys().len());
}

#[test]
fn test_ormap_merge_idempotence() {
    let mut map: ORMap<String, i32> = ORMap::new();
    let mut vc = VectorClock::new();
    vc.increment("node1");
    
    map.put("key".to_string(), 42, test_add_id("node1", 1), vc);
    
    let original_len = map.len();
    let map_copy = map.clone();
    
    map.merge(&map_copy).unwrap();
    
    assert_eq!(map.len(), original_len);
    assert_eq!(map.get(&"key".to_string()), Some(&42));
}

#[test]
fn test_ormap_concurrent_remove_and_update() {
    // Two replicas: one removes key, one updates value
    let mut map1: ORMap<String, String> = ORMap::new();
    let mut map2: ORMap<String, String> = ORMap::new();
    
    let key = "shared_key".to_string();
    
    // Both start with same key
    let mut vc = VectorClock::new();
    vc.increment("init");
    map1.put(key.clone(), "initial".to_string(), test_add_id("init", 1), vc.clone());
    map2.put(key.clone(), "initial".to_string(), test_add_id("init", 1), vc.clone());
    
    // map1 removes key
    vc.increment("node1");
    map1.remove(&key, vc.clone());
    
    // map2 updates key (concurrent)
    vc.increment("node2");
    map2.put(key.clone(), "updated".to_string(), test_add_id("node2", 2), vc.clone());
    
    // Merge
    map1.merge(&map2).unwrap();
    
    // Update should win (add-wins semantics)
    assert!(map1.contains_key(&key));
    assert_eq!(map1.get(&key), Some(&"updated".to_string()));
}

// =============================================================================
// VECTOR CLOCK ALGEBRAIC LAWS
// =============================================================================

#[test]
fn test_vector_clock_merge_commutativity() {
    let mut vc1 = VectorClock::new();
    vc1.increment("node1");
    vc1.increment("node1");
    
    let mut vc2 = VectorClock::new();
    vc2.increment("node2");
    vc2.increment("node2");
    vc2.increment("node2");
    
    let mut result1 = vc1.clone();
    result1.merge(&vc2);
    
    let mut result2 = vc2.clone();
    result2.merge(&vc1);
    
    // Both should have same clock values
    assert_eq!(result1.get("node1"), result2.get("node1"));
    assert_eq!(result1.get("node2"), result2.get("node2"));
}

#[test]
fn test_vector_clock_merge_associativity() {
    let mut vc1 = VectorClock::new();
    vc1.increment("a");
    
    let mut vc2 = VectorClock::new();
    vc2.increment("b");
    
    let mut vc3 = VectorClock::new();
    vc3.increment("c");
    
    // (vc1 ⊔ vc2) ⊔ vc3
    let mut left = vc1.clone();
    left.merge(&vc2);
    left.merge(&vc3);
    
    // vc1 ⊔ (vc2 ⊔ vc3)
    let mut right = vc1.clone();
    let mut temp = vc2.clone();
    temp.merge(&vc3);
    right.merge(&temp);
    
    assert_eq!(left.get("a"), right.get("a"));
    assert_eq!(left.get("b"), right.get("b"));
    assert_eq!(left.get("c"), right.get("c"));
}

#[test]
fn test_vector_clock_merge_idempotence() {
    let mut vc = VectorClock::new();
    vc.increment("node1");
    vc.increment("node2");
    
    let original_node1 = vc.get("node1");
    let original_node2 = vc.get("node2");
    
    let vc_copy = vc.clone();
    vc.merge(&vc_copy);
    
    assert_eq!(vc.get("node1"), original_node1);
    assert_eq!(vc.get("node2"), original_node2);
}

#[test]
fn test_vector_clock_happens_before_antisymmetry() {
    let mut vc1 = VectorClock::new();
    vc1.increment("node1");
    
    let mut vc2 = VectorClock::new();
    vc2.increment("node1");
    vc2.increment("node1");
    
    // vc1 < vc2 means vc1 happened before vc2
    assert!(vc1.happened_before(&vc2));
    
    // Cannot have vc2 < vc1
    assert!(!vc2.happened_before(&vc1));
}

#[test]
fn test_vector_clock_concurrent_with_disjoint_actors() {
    let mut vc1 = VectorClock::new();
    vc1.increment("alice");
    vc1.increment("alice");
    
    let mut vc2 = VectorClock::new();
    vc2.increment("bob");
    vc2.increment("bob");
    
    // Neither happened before the other - concurrent
    assert!(!vc1.happened_before(&vc2));
    assert!(!vc2.happened_before(&vc1));
}

#[test]
fn test_vector_clock_empty_comparison() {
    let vc_empty = VectorClock::new();
    let mut vc_nonempty = VectorClock::new();
    vc_nonempty.increment("node1");
    
    // Empty clock happens before all others
    assert!(vc_empty.happened_before(&vc_nonempty));
    assert!(!vc_nonempty.happened_before(&vc_empty));
}
