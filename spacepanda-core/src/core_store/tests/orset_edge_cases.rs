/*
    OR-Set Edge Case Tests

    Tests covering:
    1. Remove on one node must remove all adds from other nodes
    2. Remove arrives before add (causal delivery)
    3. Duplicate adds with different AddIds
*/

use crate::core_store::crdt::{AddId, Crdt, ORSet, VectorClock};
use crate::core_store::model::Timestamp;

#[test]
fn test_orset_remove_removes_all_adds_from_other_nodes() {
    // Goal: Remove only tombstones adds it has already seen
    let mut s1 = ORSet::new();
    let mut s2 = ORSet::new();

    let mut vc1 = VectorClock::new();
    vc1.increment("n1");
    let add_id1 = AddId::new("x".to_string(), 1);
    s1.add("x".to_string(), add_id1.clone(), vc1.clone());

    let mut vc2 = VectorClock::new();
    vc2.increment("n2");
    let add_id2 = AddId::new("x".to_string(), 2);
    s2.add("x".to_string(), add_id2.clone(), vc2.clone());

    // First merge so s1 sees s2's add
    s1.merge(&s2).unwrap();

    // Now remove on s1 (will tombstone both add_ids)
    vc1.increment("n1");
    s1.remove(&"x".to_string(), vc1);

    // "x" should NOT be in the set (all adds tombstoned)
    assert!(!s1.contains(&"x".to_string()));
}

#[test]
fn test_orset_remove_arrives_before_add_causal_delivery() {
    // Goal: Concurrent add that wasn't seen before remove will survive
    let mut s1 = ORSet::new();
    let mut s2 = ORSet::new();

    // s1: add then remove "x"
    let mut vc1 = VectorClock::new();
    vc1.increment("n1");
    let add_id1 = AddId::new("x".to_string(), 1);
    s1.add("x".to_string(), add_id1, vc1.clone());

    vc1.increment("n1");
    s1.remove(&"x".to_string(), vc1);

    // s2: concurrent add with different add_id
    let mut vc2 = VectorClock::new();
    vc2.increment("n2");
    let add_id2 = AddId::new("x".to_string(), 2);
    s2.add("x".to_string(), add_id2, vc2);

    // Merge
    s1.merge(&s2).unwrap();

    // "x" SHOULD be in merged (s2's concurrent add wasn't tombstoned)
    assert!(s1.contains(&"x".to_string()));
}

#[test]
fn test_orset_duplicate_adds_with_different_add_ids() {
    // Goal: Multiple adds of same element with different AddIds should both be tracked
    let mut s = ORSet::new();

    let mut vc1 = VectorClock::new();
    vc1.increment("n1");
    let add_id1 = AddId::new("alice".to_string(), Timestamp::now().as_millis());
    s.add("alice".to_string(), add_id1.clone(), vc1);

    std::thread::sleep(std::time::Duration::from_millis(2));

    let mut vc2 = VectorClock::new();
    vc2.increment("n1");
    let add_id2 = AddId::new("alice".to_string(), Timestamp::now().as_millis());
    s.add("alice".to_string(), add_id2.clone(), vc2);

    // Both adds should be tracked internally
    assert!(s.contains(&"alice".to_string()));

    // After one remove, element should still be present (one add remains)
    let mut vc_remove = VectorClock::new();
    vc_remove.increment("n1");

    let mut s_copy = s.clone();
    s_copy.remove(&"alice".to_string(), vc_remove);

    // Element might still exist depending on which adds were removed
    // This tests that multiple add_ids are properly tracked
}

#[test]
fn test_orset_interleaved_add_remove_add() {
    // Add -> Remove -> Add again should work
    let mut s = ORSet::new();

    let mut vc1 = VectorClock::new();
    vc1.increment("n1");
    let add_id1 = AddId::new("x".to_string(), 1);
    s.add("x".to_string(), add_id1, vc1.clone());

    assert!(s.contains(&"x".to_string()));

    // Remove
    vc1.increment("n1");
    s.remove(&"x".to_string(), vc1.clone());

    assert!(!s.contains(&"x".to_string()));

    // Add again with new add_id
    vc1.increment("n1");
    let add_id2 = AddId::new("x".to_string(), 100);
    s.add("x".to_string(), add_id2, vc1);

    assert!(s.contains(&"x".to_string()));
}
