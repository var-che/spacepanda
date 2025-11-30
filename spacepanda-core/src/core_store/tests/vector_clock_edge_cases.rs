/*
    Vector Clock Edge Case Tests
    
    Tests covering:
    1. Merging two clocks should max counters
    2. Reflexive causality test
    3. Complex partial ordering
*/

use crate::core_store::crdt::VectorClock;

#[test]
fn test_vector_clock_merge_max_counters() {
    // Goal: Merging should take the maximum of each counter
    let mut vc1 = VectorClock::new();
    vc1.increment("n1");
    vc1.increment("n1");
    vc1.increment("n2");
    vc1.increment("n2");
    vc1.increment("n2");
    vc1.increment("n2");
    vc1.increment("n2");
    
    let mut vc2 = VectorClock::new();
    for _ in 0..7 {
        vc2.increment("n1");
    }
    vc2.increment("n3");
    
    // vc1 = {n1:2, n2:5}
    // vc2 = {n1:7, n3:1}
    
    let mut vc_merged = vc1.clone();
    vc_merged.merge(&vc2);
    
    // Expected: {n1:7, n2:5, n3:1}
    assert_eq!(vc_merged.get("n1"), 7);
    assert_eq!(vc_merged.get("n2"), 5);
    assert_eq!(vc_merged.get("n3"), 1);
}

#[test]
fn test_vector_clock_reflexive_causality() {
    // Goal: A clock should not be concurrent with itself
    let mut vc = VectorClock::new();
    vc.increment("n1");
    vc.increment("n1");
    vc.increment("n1");
    vc.increment("n2");
    vc.increment("n2");
    vc.increment("n2");
    vc.increment("n2");
    
    // vc = {n1:3, n2:4}
    
    // Not concurrent with itself
    assert!(!vc.is_concurrent(&vc));
    
    // Equal to itself
    assert_eq!(vc, vc);
}

#[test]
fn test_vector_clock_complex_partial_ordering() {
    // Goal: Test complex happened-before and concurrent relationships
    
    // A = {n1:1, n2:0}
    let mut a = VectorClock::new();
    a.increment("n1");
    
    // B = {n1:1, n2:1}
    let mut b = VectorClock::new();
    b.increment("n1");
    b.increment("n2");
    
    // C = {n1:2, n2:0}
    let mut c = VectorClock::new();
    c.increment("n1");
    c.increment("n1");
    
    // A happened-before B (A < B)
    assert!(a.happened_before(&b));
    assert!(!b.happened_before(&a));
    
    // A happened-before C (A < C)
    assert!(a.happened_before(&c));
    assert!(!c.happened_before(&a));
    
    // B and C are concurrent
    assert!(b.is_concurrent(&c));
    assert!(c.is_concurrent(&b));
    assert!(!b.happened_before(&c));
    assert!(!c.happened_before(&b));
}

#[test]
fn test_vector_clock_incremental_causality() {
    // Test that incrementing maintains causal order
    let mut vc1 = VectorClock::new();
    vc1.increment("n1");
    
    let vc1_snapshot = vc1.clone();
    
    vc1.increment("n1");
    
    // vc1_snapshot happened-before vc1
    assert!(vc1_snapshot.happened_before(&vc1));
    assert!(!vc1.happened_before(&vc1_snapshot));
}

#[test]
fn test_vector_clock_multi_node_divergence() {
    // Three nodes diverge then converge
    let mut vc_a = VectorClock::new();
    let mut vc_b = VectorClock::new();
    let mut vc_c = VectorClock::new();
    
    // Node A does 3 ops
    vc_a.increment("A");
    vc_a.increment("A");
    vc_a.increment("A");
    
    // Node B does 2 ops
    vc_b.increment("B");
    vc_b.increment("B");
    
    // Node C does 1 op
    vc_c.increment("C");
    
    // All are concurrent with each other
    assert!(vc_a.is_concurrent(&vc_b));
    assert!(vc_b.is_concurrent(&vc_c));
    assert!(vc_a.is_concurrent(&vc_c));
    
    // Merge all
    let mut merged = vc_a.clone();
    merged.merge(&vc_b);
    merged.merge(&vc_c);
    
    // Merged should dominate all
    assert!(vc_a.happened_before(&merged));
    assert!(vc_b.happened_before(&merged));
    assert!(vc_c.happened_before(&merged));
    
    assert_eq!(merged.get("A"), 3);
    assert_eq!(merged.get("B"), 2);
    assert_eq!(merged.get("C"), 1);
}
