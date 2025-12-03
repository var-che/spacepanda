/*
    Advanced OR-Set Tests

    Tests covering:
    1. Removing one element shouldn't affect others
    2. Merge must preserve tombstoned add_ids
    3. Multiple adds then remove (all add_ids tombstoned)
*/

use crate::core_store::crdt::{AddId, Crdt, ORSet, VectorClock};

#[test]
fn test_orset_remove_one_element_preserves_others() {
    // Goal: Tombstones target only element-specific add_ids
    let mut set = ORSet::new();

    let mut vc1 = VectorClock::new();
    vc1.increment("n1");
    let alice_id = AddId::new("alice".to_string(), 1);
    set.add("alice".to_string(), alice_id, vc1.clone());

    vc1.increment("n1");
    let bob_id = AddId::new("bob".to_string(), 2);
    set.add("bob".to_string(), bob_id, vc1.clone());

    // Remove alice
    vc1.increment("n1");
    set.remove(&"alice".to_string(), vc1);

    // Alice should be gone, Bob should remain
    assert!(!set.contains(&"alice".to_string()));
    assert!(set.contains(&"bob".to_string()));
}

#[test]
fn test_orset_merge_preserves_tombstones() {
    // Goal: If one replica tombstones an add, another must respect it
    let mut s1 = ORSet::new();
    let mut s2 = ORSet::new();

    // Both replicas start with the same add
    let mut vc1 = VectorClock::new();
    vc1.increment("n1");
    let add_id1 = AddId::new("x".to_string(), 1);
    s1.add("x".to_string(), add_id1.clone(), vc1.clone());

    let mut vc2 = VectorClock::new();
    vc2.increment("n2");
    s2.add("x".to_string(), add_id1, vc2.clone());

    // s1 removes "x" (tombstones add_id1)
    vc1.increment("n1");
    s1.remove(&"x".to_string(), vc1);

    // After remove, s1 should not contain "x"
    assert!(!s1.contains(&"x".to_string()));

    // Merge s2 into s1 - s1's tombstone should prevent re-adding
    s1.merge(&s2).unwrap();

    // s1 should still not contain "x" (its tombstone prevents it)
    assert!(!s1.contains(&"x".to_string()));

    // Merge s1 into s2 - s2 should learn about the tombstone
    s2.merge(&s1).unwrap();

    // Both should NOT contain "x" (tombstone should propagate)
    assert!(!s1.contains(&"x".to_string()));
    assert!(!s2.contains(&"x".to_string()));
}

#[test]
fn test_orset_multiple_adds_then_remove_tombstones_all() {
    // Goal: All add_ids for the same element must be tombstoned
    let mut set = ORSet::new();

    let mut vc = VectorClock::new();

    // Add "x" three times with different add_ids
    vc.increment("n1");
    let add_id1 = AddId::new("x".to_string(), 1);
    set.add("x".to_string(), add_id1, vc.clone());

    vc.increment("n1");
    let add_id2 = AddId::new("x".to_string(), 2);
    set.add("x".to_string(), add_id2, vc.clone());

    vc.increment("n1");
    let add_id3 = AddId::new("x".to_string(), 3);
    set.add("x".to_string(), add_id3, vc.clone());

    // Verify all three adds are tracked
    assert!(set.contains(&"x".to_string()));

    // Remove "x" (should tombstone all three add_ids)
    vc.increment("n1");
    let tombstoned = set.remove(&"x".to_string(), vc);

    // Should have tombstoned all 3 add_ids
    assert_eq!(tombstoned.len(), 3);
    assert!(!set.contains(&"x".to_string()));
}
