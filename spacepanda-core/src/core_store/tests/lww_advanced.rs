/*
    Advanced LWW-Register Tests

    Tests covering:
    1. Node-id tie-break (lexicographic comparison)
    2. Merge commutativity
*/

use crate::core_store::crdt::{LWWRegister, VectorClock};

#[test]
fn test_lww_node_id_tiebreak() {
    // Goal: Two writes with same timestamp → lexicographically greater actor wins
    let mut vc1 = VectorClock::new();
    vc1.increment("node1");
    let mut r1 = LWWRegister::new();
    r1.set("A".to_string(), 5, "node1".to_string(), vc1);

    let mut vc2 = VectorClock::new();
    vc2.increment("node9");
    let mut r2 = LWWRegister::new();
    r2.set("B".to_string(), 5, "node9".to_string(), vc2);

    // Merge r2 into r1
    r1.merge(&r2);

    // "node9" > "node1" lexicographically, so "B" should win
    assert_eq!(r1.get(), Some(&"B".to_string()));
}

#[test]
fn test_lww_merge_commutativity() {
    // Goal: r1.merge(r2) == r2.merge(r1)
    let mut vc1 = VectorClock::new();
    vc1.increment("n1");
    let mut r1 = LWWRegister::new();
    r1.set("Alpha".to_string(), 11, "n1".to_string(), vc1);

    let mut vc2 = VectorClock::new();
    vc2.increment("n2");
    let mut r2 = LWWRegister::new();
    r2.set("Beta".to_string(), 8, "n2".to_string(), vc2);

    // Merge in both directions
    let mut merged1 = r1.clone();
    merged1.merge(&r2);

    let mut merged2 = r2.clone();
    merged2.merge(&r1);

    // Both should converge to "Alpha" (higher timestamp)
    assert_eq!(merged1.get(), Some(&"Alpha".to_string()));
    assert_eq!(merged2.get(), Some(&"Alpha".to_string()));

    // Should be identical
    assert_eq!(merged1.get(), merged2.get());
    assert_eq!(merged1.timestamp(), merged2.timestamp());
}
