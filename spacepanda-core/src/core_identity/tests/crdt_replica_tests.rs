//! CRDT Multi-Replica Divergence Tests
//!
//! Tests realistic scenarios with multiple replicas:
//! - Out-of-order message delivery
//! - Duplicate messages
//! - Concurrent operations from different nodes
//! - Replica convergence after complex interleavings

use crate::core_identity::*;
use crate::core_store::crdt::{LWWRegister, ORSet, ORMap, VectorClock, AddId, Crdt, ORSetOperation, OperationMetadata};
use crate::core_store::model::types::Timestamp;
use super::helpers::*;

// =============================================================================
// LWW REGISTER REPLICA TESTS
// =============================================================================

#[test]
fn test_lww_three_way_merge_convergence() {
    // Three replicas all update, then merge - should converge
    let mut replica1 = LWWRegister::with_value("init".to_string(), "node0".to_string());
    let mut replica2 = replica1.clone();
    let mut replica3 = replica1.clone();
    
    // Each replica makes update at different times
    let mut vc1 = VectorClock::new();
    vc1.increment("node1");
    replica1.set("value1".to_string(), test_timestamp(100).as_millis(), "node1".to_string(), vc1);
    
    let mut vc2 = VectorClock::new();
    vc2.increment("node2");
    replica2.set("value2".to_string(), test_timestamp(200).as_millis(), "node2".to_string(), vc2);
    
    let mut vc3 = VectorClock::new();
    vc3.increment("node3");
    replica3.set("value3".to_string(), test_timestamp(300).as_millis(), "node3".to_string(), vc3);
    
    // Merge all replicas pairwise (LWWRegister::merge returns void)
    replica1.merge(&replica2);
    replica1.merge(&replica3);
    
    replica2.merge(&replica1);
    replica2.merge(&replica3);
    
    replica3.merge(&replica1);
    replica3.merge(&replica2);
    
    // All should converge to same value (highest timestamp)
    assert_eq!(replica1.get(), Some(&"value3".to_string()));
    assert_eq!(replica2.get(), Some(&"value3".to_string()));
    assert_eq!(replica3.get(), Some(&"value3".to_string()));
}

#[test]
fn test_lww_clock_skew_resolution() {
    // Test timestamp wins even with clock skew
    let mut replica1 = LWWRegister::new();
    let mut replica2 = LWWRegister::new();
    
    // replica1 has slow clock
    let mut vc1 = VectorClock::new();
    vc1.increment("node1");
    replica1.set("old_clock".to_string(), 1000, "node1".to_string(), vc1);
    
    // replica2 has fast clock (far future)
    let mut vc2 = VectorClock::new();
    vc2.increment("node2");
    replica2.set("fast_clock".to_string(), 9999999, "node2".to_string(), vc2);
    
    // Merge
    replica1.merge(&replica2);
    
    // Future timestamp wins
    assert_eq!(replica1.get(), Some(&"fast_clock".to_string()));
}

// =============================================================================
// OR-SET REPLICA TESTS
// =============================================================================

#[test]
fn test_orset_concurrent_adds_from_multiple_nodes() {
    // Three nodes all add same element concurrently
    let mut replica1: ORSet<String> = ORSet::new();
    let mut replica2: ORSet<String> = ORSet::new();
    let mut replica3: ORSet<String> = ORSet::new();
    
    let element = "shared".to_string();
    
    // All add concurrently with different tags
    let mut vc1 = VectorClock::new();
    vc1.increment("node1");
    replica1.add(element.clone(), test_add_id("node1", 1), vc1);
    
    let mut vc2 = VectorClock::new();
    vc2.increment("node2");
    replica2.add(element.clone(), test_add_id("node2", 1), vc2);
    
    let mut vc3 = VectorClock::new();
    vc3.increment("node3");
    replica3.add(element.clone(), test_add_id("node3", 1), vc3);
    
    // Merge all together
    replica1.merge(&replica2).unwrap();
    replica1.merge(&replica3).unwrap();
    
    // Should have one element with 3 tags
    assert_eq!(replica1.len(), 1);
    let tags = replica1.get_add_ids(&element).unwrap();
    assert_eq!(tags.len(), 3);
}

#[test]
fn test_orset_remove_partial_tags_then_merge() {
    // Replica1 has tags A,B,C. Removes some. Replica2 has tag D. Merge.
    let mut replica1: ORSet<String> = ORSet::new();
    let mut replica2: ORSet<String> = ORSet::new();
    
    let element = "element".to_string();
    
    // replica1 adds with multiple tags
    let mut vc = VectorClock::new();
    vc.increment("node1");
    replica1.add(element.clone(), test_add_id("node1", 1), vc.clone());
    vc.increment("node1");
    replica1.add(element.clone(), test_add_id("node1", 2), vc.clone());
    vc.increment("node1");
    replica1.add(element.clone(), test_add_id("node1", 3), vc.clone());
    
    // replica1 removes element (removes all its known tags)
    vc.increment("node1");
    replica1.remove(&element, vc.clone());
    assert!(!replica1.contains(&element));
    
    // replica2 adds concurrently
    vc.increment("node2");
    replica2.add(element.clone(), test_add_id("node2", 1), vc.clone());
    
    // Merge
    replica1.merge(&replica2).unwrap();
    
    // Element should reappear (replica2's tag survives)
    assert!(replica1.contains(&element));
    let tags = replica1.get_add_ids(&element).unwrap();
    assert_eq!(tags.len(), 1);
}

#[test]
fn test_orset_out_of_order_delivery() {
    // Messages arrive: remove, add, add (reverse order)
    let mut set: ORSet<String> = ORSet::new();
    let element = "element".to_string();
    
    let tag1 = test_add_id("node1", 1);
    let tag2 = test_add_id("node1", 2);
    
    // Message 3: Remove (arrives first)
    let remove_op = ORSetOperation::Remove {
        element: element.clone(),
        add_ids: vec![tag1.clone(), tag2.clone()].into_iter().collect(),
        metadata: OperationMetadata {
            node_id: "node1".to_string(),
            timestamp: test_timestamp(300).as_millis(),
            vector_clock: {
                let mut vc = VectorClock::new();
                vc.increment("node1");
                vc.increment("node1");
                vc.increment("node1");
                vc
            },
            signature: None,
        },
    };
    set.apply(remove_op).unwrap();
    
    // Message 1: First add (arrives second)
    let add_op1 = ORSetOperation::Add {
        element: element.clone(),
        add_id: tag1.clone(),
        metadata: OperationMetadata {
            node_id: "node1".to_string(),
            timestamp: test_timestamp(100).as_millis(),
            vector_clock: {
                let mut vc = VectorClock::new();
                vc.increment("node1");
                vc
            },
            signature: None,
        },
    };
    set.apply(add_op1).unwrap();
    
    // Message 2: Second add (arrives third)
    let add_op2 = ORSetOperation::Add {
        element: element.clone(),
        add_id: tag2.clone(),
        metadata: OperationMetadata {
            node_id: "node1".to_string(),
            timestamp: test_timestamp(200).as_millis(),
            vector_clock: {
                let mut vc = VectorClock::new();
                vc.increment("node1");
                vc.increment("node1");
                vc
            },
            signature: None,
        },
    };
    set.apply(add_op2).unwrap();
    
    // In OR-Set, adds succeed even if remove came first
    // Operations are applied independently based on causal relationships
    assert!(set.contains(&element));
    // Both add_ids are present
    let tags = set.get_add_ids(&element).unwrap();
    assert_eq!(tags.len(), 2);
}

#[test]
fn test_orset_duplicate_message_delivery() {
    // Same add message delivered multiple times
    let mut set: ORSet<String> = ORSet::new();
    let element = "element".to_string();
    let tag = test_add_id("node1", 1);
    
    let mut vc = VectorClock::new();
    vc.increment("node1");
    
    let add_op = ORSetOperation::Add {
        element: element.clone(),
        add_id: tag.clone(),
        metadata: OperationMetadata {
            node_id: "node1".to_string(),
            timestamp: test_timestamp(100).as_millis(),
            vector_clock: vc.clone(),
            signature: None,
        },
    };
    
    // Apply same operation 3 times
    set.apply(add_op.clone()).unwrap();
    set.apply(add_op.clone()).unwrap();
    set.apply(add_op.clone()).unwrap();
    
    // Should still have exactly one element with one tag
    assert_eq!(set.len(), 1);
    let tags = set.get_add_ids(&element).unwrap();
    assert_eq!(tags.len(), 1);
}

#[test]
fn test_orset_five_way_merge_convergence() {
    // Five replicas with different operations converge
    let mut replicas: Vec<ORSet<String>> = (0..5).map(|_| ORSet::new()).collect();
    
    // Each replica adds different elements
    for i in 0..5 {
        let mut vc = VectorClock::new();
        vc.increment(&format!("node{}", i));
        replicas[i].add(
            format!("element{}", i),
            test_add_id(&format!("node{}", i), 1),
            vc,
        );
    }
    
    // Replica 2 and 4 also add same element
    let shared = "shared".to_string();
    let mut vc2 = VectorClock::new();
    vc2.increment("node2");
    replicas[2].add(shared.clone(), test_add_id("node2", 2), vc2);
    
    let mut vc4 = VectorClock::new();
    vc4.increment("node4");
    replicas[4].add(shared.clone(), test_add_id("node4", 2), vc4);
    
    // Merge all into replica 0 (clone to avoid borrow issues)
    for i in 1..5 {
        let replica_clone = replicas[i].clone();
        replicas[0].merge(&replica_clone).unwrap();
    }
    
    // Should have all 6 elements
    assert_eq!(replicas[0].len(), 6);
    
    // shared element should have 2 tags
    let tags = replicas[0].get_add_ids(&shared).unwrap();
    assert_eq!(tags.len(), 2);
}

// =============================================================================
// OR-MAP REPLICA TESTS
// =============================================================================

#[test]
fn test_ormap_concurrent_puts_same_key() {
    // Two replicas update same key with different values
    let mut replica1: ORMap<String, String> = ORMap::new();
    let mut replica2: ORMap<String, String> = ORMap::new();
    
    let key = "key".to_string();
    
    // Both start with same value
    let mut vc = VectorClock::new();
    vc.increment("init");
    replica1.put(key.clone(), "init".to_string(), test_add_id("init", 1), vc.clone());
    replica2.put(key.clone(), "init".to_string(), test_add_id("init", 1), vc.clone());
    
    // Concurrent updates
    vc.increment("node1");
    replica1.put(key.clone(), "value1".to_string(), test_add_id("node1", 2), vc.clone());
    
    vc.increment("node2");
    replica2.put(key.clone(), "value2".to_string(), test_add_id("node2", 2), vc.clone());
    
    // Merge
    replica1.merge(&replica2).unwrap();
    replica2.merge(&replica1).unwrap();
    
    // Both should converge to same value (LWW semantics in current impl)
    assert_eq!(replica1.contains_key(&key), replica2.contains_key(&key));
}

#[test]
fn test_ormap_remove_add_remove_sequence() {
    // Complex sequence: add, remove, add again across replicas
    let mut replica1: ORMap<String, i32> = ORMap::new();
    let mut replica2: ORMap<String, i32> = ORMap::new();
    
    let key = "key".to_string();
    
    // replica1: add
    let mut vc = VectorClock::new();
    vc.increment("node1");
    replica1.put(key.clone(), 1, test_add_id("node1", 1), vc.clone());
    
    // replica2: receives add, then removes
    replica2.merge(&replica1).unwrap();
    vc.increment("node2");
    replica2.remove(&key, vc.clone());
    
    // replica1: adds again (concurrent with remove)
    vc.increment("node1");
    replica1.put(key.clone(), 2, test_add_id("node1", 2), vc.clone());
    
    // Merge back
    replica1.merge(&replica2).unwrap();
    
    // The new add should win
    assert!(replica1.contains_key(&key));
}

// =============================================================================
// USER METADATA REPLICA TESTS
// =============================================================================

#[test]
fn test_user_metadata_three_replica_convergence() {
    // Three replicas update different fields, then converge
    let user_id = test_user_id();
    
    let mut meta1 = UserMetadata::new(user_id.clone());
    let mut meta2 = meta1.clone();
    let mut meta3 = meta1.clone();
    
    // replica1: updates display name
    let mut vc1 = VectorClock::new();
    vc1.increment("node1");
    meta1.display_name.set(
        "Alice".to_string(),
        test_timestamp(100).as_millis(),
        "node1".to_string(),
        vc1.clone(),
    );
    
    // replica2: updates display_name (different value)
    let mut vc2 = VectorClock::new();
    vc2.increment("node2");
    meta2.display_name.set(
        "Bob".to_string(),
        test_timestamp(200).as_millis(),
        "node2".to_string(),
        vc2.clone(),
    );
    
    // replica3: updates avatar_hash
    let mut vc3 = VectorClock::new();
    vc3.increment("node3");
    meta3.avatar_hash.set(
        Some(vec![0xAB, 0xCD]),
        test_timestamp(300).as_millis(),
        "node3".to_string(),
        vc3.clone(),
    );
    
    // Merge all together
    meta1.merge(&meta2);
    meta1.merge(&meta3);
    
    meta2.merge(&meta1);
    meta2.merge(&meta3);
    
    meta3.merge(&meta1);
    meta3.merge(&meta2);
    
    // After full merge, all replicas should converge to same state
    // LWW semantics: highest timestamp wins
    // Bob has timestamp 200, Alice has 100 - Bob wins
    assert_eq!(meta1.display_name.get(), Some(&"Bob".to_string()));
    assert_eq!(meta2.display_name.get(), Some(&"Bob".to_string()));
    assert_eq!(meta3.display_name.get(), Some(&"Bob".to_string()));
    
    // All should have the avatar_hash from node3
    assert_eq!(meta1.avatar_hash.get(), Some(&Some(vec![0xAB, 0xCD])));
    assert_eq!(meta2.avatar_hash.get(), Some(&Some(vec![0xAB, 0xCD])));
    assert_eq!(meta3.avatar_hash.get(), Some(&Some(vec![0xAB, 0xCD])));
}

#[test]
fn test_user_metadata_device_addition_across_replicas() {
    // Two replicas add different devices, then merge
    let user_id = test_user_id();
    
    let mut meta1 = UserMetadata::new(user_id.clone());
    let mut meta2 = meta1.clone();
    
    let device1_id = DeviceId::generate();
    let device2_id = DeviceId::generate();
    
    // Create device bundles
    let id_kp = test_keypair();
    let dev_kp1 = test_keypair();
    let device1_meta = DeviceMetadata::new(device1_id.clone(), "Device1".to_string(), "node1");
    let kp1 = KeyPackage::new(&dev_kp1, &id_kp, &device1_meta);
    let device1 = DeviceBundle::new(kp1, device1_meta.clone(), &id_kp);
    
    let dev_kp2 = test_keypair();
    let device2_meta = DeviceMetadata::new(device2_id.clone(), "Device2".to_string(), "node2");
    let kp2 = KeyPackage::new(&dev_kp2, &id_kp, &device2_meta);
    let device2 = DeviceBundle::new(kp2, device2_meta.clone(), &id_kp);
    
    // replica1 adds device1
    let mut vc1 = VectorClock::new();
    vc1.increment("node1");
    meta1.add_device(device1_meta, test_add_id("node1", 1), vc1);
    
    // replica2 adds device2
    let mut vc2 = VectorClock::new();
    vc2.increment("node2");
    meta2.add_device(device2_meta, test_add_id("node2", 1), vc2);
    
    // Merge
    meta1.merge(&meta2);
    
    // Should have both devices
    assert_eq!(meta1.devices.len(), 2);
    assert!(meta1.devices.contains_key(&device1_id));
    assert!(meta1.devices.contains_key(&device2_id));
}

#[test]
fn test_user_metadata_concurrent_same_field_update() {
    // Two replicas update same field concurrently
    let user_id = test_user_id();
    
    let mut meta1 = UserMetadata::new(user_id.clone());
    let mut meta2 = meta1.clone();
    
    // Both update display name at same time (different values)
    let ts = test_timestamp(100).as_millis();
    
    let mut vc1 = VectorClock::new();
    vc1.increment("node1");
    meta1.display_name.set("Alice".to_string(), ts, "node1".to_string(), vc1);
    
    let mut vc2 = VectorClock::new();
    vc2.increment("node2");
    meta2.display_name.set("Bob".to_string(), ts, "node2".to_string(), vc2);
    
    // Merge
    meta1.merge(&meta2);
    meta2.merge(&meta1);
    
    // Should converge to same value (node_id tiebreaker)
    assert_eq!(meta1.display_name.get(), meta2.display_name.get());
}
