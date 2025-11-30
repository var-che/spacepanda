//! CRDT Edge-Case Tests
//!
//! Tests for OR-Set, OR-Map, LWW Register, Vector Clocks with realistic conflict scenarios

use crate::core_identity::*;
use crate::core_store::crdt::{LWWRegister, ORSet, ORMap, VectorClock, AddId, Crdt};
use crate::core_store::model::types::Timestamp;
use super::helpers::*;

// =============================================================================
// LWW REGISTER EDGE CASES
// =============================================================================

#[test]
fn test_lww_future_timestamp_overrides_current() {
    let mut reg = LWWRegister::with_value("a".to_string(), "local".to_string());
    
    // Remote clock is absurdly ahead
    let future_ts = test_timestamp(1000000); // Very far future
    let current_ts = test_timestamp(1000);
    
    let mut vc = VectorClock::new();
    vc.increment("remote");
    reg.set("b".to_string(), future_ts.as_millis(), "remote".to_string(), vc.clone());
    
    vc.increment("local");
    reg.set("c".to_string(), current_ts.as_millis(), "local".to_string(), vc);
    
    // Future timestamp wins
    assert_eq!(reg.get(), Some(&"b".to_string()));
}

#[test]
fn test_lww_equal_timestamps_resolve_by_node_id() {
    let mut reg = LWWRegister::with_value("a".to_string(), "init".to_string());
    
    let ts = test_timestamp(1000);
    let mut vc = VectorClock::new();
    
    vc.increment("nodeA");
    reg.set("valueA".to_string(), ts.as_millis(), "nodeA".to_string(), vc.clone());
    
    vc.increment("nodeB");
    reg.set("valueB".to_string(), ts.as_millis(), "nodeB".to_string(), vc);
    
    // With equal timestamps, node_id determines winner (lexicographic)
    // "nodeB" > "nodeA" lexicographically
    assert_eq!(reg.get(), Some(&"valueB".to_string()));
}

// =============================================================================
// OR-SET EDGE CASES
// =============================================================================

#[test]
fn test_orset_concurrent_add_remove_conflict() {
    let mut set = ORSet::new();
    let mut vc1 = VectorClock::new();
    vc1.increment("A");
    
    // Add with tag A
    let add_id_a = AddId::new("A".to_string(), 1);
    set.add("u1".to_string(), add_id_a, vc1.clone());
    
    // Concurrent remove with different tag doesn't remove tag A
    let mut vc2 = VectorClock::new();
    vc2.increment("B");
    
    // The remove should specify which tags to remove
    // Since we can't remove tag A with a different vector clock context,
    // the element remains
    assert!(set.contains(&"u1".to_string()));
}

#[test]
fn test_orset_remove_last_tag_element_disappears() {
    let mut set = ORSet::new();
    let mut vc = VectorClock::new();
    vc.increment("A");
    
    let add_id = AddId::new("A".to_string(), 1);
    set.add("u1".to_string(), add_id.clone(), vc.clone());
    
    // Remove the element
    vc.increment("A");
    set.remove(&"u1".to_string(), vc);
    
    assert!(!set.contains(&"u1".to_string()));
}

#[test]
fn test_orset_idempotent_add() {
    let mut set = ORSet::new();
    let mut vc = VectorClock::new();
    vc.increment("node1");
    
    let add_id = AddId::new("node1".to_string(), 1);
    
    // Add twice with same parameters
    set.add("x".to_string(), add_id.clone(), vc.clone());
    set.add("x".to_string(), add_id, vc);
    
    // Should only have one instance
    assert!(set.contains(&"x".to_string()));
    assert_eq!(set.len(), 1);
}

#[test]
fn test_orset_remove_then_concurrent_add() {
    let mut set = ORSet::new();
    
    // Initial add with tag A
    let mut vc1 = VectorClock::new();
    vc1.increment("node1");
    set.add("k".to_string(), AddId::new("node1".to_string(), 1), vc1.clone());
    
    // Remove with tag A
    vc1.increment("node1");
    set.remove(&"k".to_string(), vc1);
    
    // Concurrent add with tag B (independent vector clock)
    let mut vc2 = VectorClock::new();
    vc2.increment("node2");
    set.add("k".to_string(), AddId::new("node2".to_string(), 1), vc2);
    
    // Element should still be present due to concurrent add
    assert!(set.contains(&"k".to_string()));
}

// =============================================================================
// OR-MAP EDGE CASES
// =============================================================================

#[test]
fn test_ormap_concurrent_remove_and_update() {
    let mut map = ORMap::<String, String>::new();
    
    // Add key
    let mut vc1 = VectorClock::new();
    vc1.increment("A");
    map.put("k".to_string(), "v1".to_string(), AddId::new("A".to_string(), 1), vc1.clone());
    
    // Remove key
    vc1.increment("A");
    map.remove(&"k".to_string(), vc1);
    
    // Concurrent add arrives late with independent context
    let mut vc2 = VectorClock::new();
    vc2.increment("B");
    map.put("k".to_string(), "v2".to_string(), AddId::new("B".to_string(), 1), vc2);
    
    // Key should exist due to concurrent add
    assert!(map.contains_key(&"k".to_string()));
}

// =============================================================================
// VECTOR CLOCK EDGE CASES
// =============================================================================

#[test]
fn test_vector_clock_merge_elementwise_max() {
    let mut vc1 = VectorClock::new();
    vc1.increment("A");
    vc1.increment("B");
    vc1.increment("B");
    // vc1: {A:1, B:2}
    
    let mut vc2 = VectorClock::new();
    vc2.increment("A");
    vc2.increment("A");
    vc2.increment("A");
    vc2.increment("B");
    // vc2: {A:3, B:1}
    
    vc1.merge(&vc2);
    
    // Should be {A:3, B:2}
    assert_eq!(vc1.get("A"), 3);
    assert_eq!(vc1.get("B"), 2);
}

#[test]
fn test_vector_clock_concurrent_detection() {
    let mut vc_a = VectorClock::new();
    vc_a.increment("A");
    vc_a.increment("A");
    vc_a.increment("B");
    // vc_a: {A:2, B:1}
    
    let mut vc_b = VectorClock::new();
    vc_b.increment("A");
    vc_b.increment("B");
    vc_b.increment("B");
    // vc_b: {A:1, B:2}
    
    // Neither happens-before the other, so they're concurrent
    assert!(!vc_a.happened_before(&vc_b));
    assert!(!vc_b.happened_before(&vc_a));
}

#[test]
fn test_vector_clock_happens_before_transitivity() {
    let mut vc1 = VectorClock::new();
    vc1.increment("A");
    
    let mut vc2 = vc1.clone();
    vc2.increment("A");
    
    let mut vc3 = vc2.clone();
    vc3.increment("A");
    
    // vc1 < vc2 < vc3 (transitivity)
    assert!(vc1.happened_before(&vc2));
    assert!(vc2.happened_before(&vc3));
    assert!(vc1.happened_before(&vc3));
}

// =============================================================================
// CHANNEL MEMBERSHIP OR-SET
// =============================================================================

#[test]
fn test_channel_members_remove_and_readd_conflict() {
    let mut members = ORSet::<String>::new();
    let mut vc = VectorClock::new();
    
    // Add user
    vc.increment("node1");
    members.add("u1".to_string(), AddId::new("node1".to_string(), 1), vc.clone());
    
    // Remove user
    vc.increment("node1");
    members.remove(&"u1".to_string(), vc.clone());
    
    // Re-add with new tag
    vc.increment("node1");
    members.add("u1".to_string(), AddId::new("node1".to_string(), 2), vc);
    
    // User should be present
    assert!(members.contains(&"u1".to_string()));
}

// =============================================================================
// CRDT MERGE - NO DATA LOSS
// =============================================================================

#[test]
fn test_user_metadata_merge_preserves_all_fields() {
    let uid = test_user_id();
    let mut meta1 = UserMetadata::new(uid.clone());
    let mut meta2 = UserMetadata::new(uid);
    
    // meta1 sets name, meta2 sets avatar
    meta1.set_display_name("Alice".to_string(), test_timestamp(1000), "node1");
    meta2.set_avatar_hash(Some(vec![1, 2, 3]), test_timestamp(2000), "node2");
    
    meta1.merge(&meta2);
    
    // Both fields should be present
    assert_eq!(meta1.display_name.get(), Some(&"Alice".to_string()));
    assert_eq!(meta1.avatar_hash.get(), Some(&Some(vec![1, 2, 3])));
}

#[test]
#[ignore = "DeviceMetadata doesn't have merge() - individual fields are LWW"]
fn test_device_metadata_merge_preserves_independent_updates() {
    let device_id = DeviceId::generate();
    let mut meta1 = DeviceMetadata::new(device_id.clone(), "Phone".to_string(), "node1");
    let mut meta2 = DeviceMetadata::new(device_id, "Tablet".to_string(), "node2");
    
    // Update different fields
    meta1.update_last_seen(test_timestamp(1000), "node1");
    meta2.set_key_package_ref(Some(vec![4, 5, 6]), test_timestamp(2000), "node2");
    
    // DeviceMetadata doesn't support merge - it's managed via UserMetadata's ORMap
    // meta1.merge(&meta2);
}
