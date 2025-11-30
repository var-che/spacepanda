/*
    Advanced Vector Clock Tests
    
    Tests covering:
    1. Merge purity (shouldn't modify right-hand operand)
    2. Equality with different HashMap ordering
    3. Three-way concurrency chain
    4. happened_before must fail if any element is greater
*/

use crate::core_store::crdt::VectorClock;

#[test]
fn test_vector_clock_merge_does_not_modify_operand() {
    // Goal: Ensure merge is pure on the right-hand operand
    let mut vc1 = VectorClock::new();
    vc1.increment("n1");
    vc1.increment("n1");
    vc1.increment("n1");
    vc1.increment("n1");
    vc1.increment("n1");
    
    let mut vc2 = VectorClock::new();
    vc2.increment("n1");
    vc2.increment("n2");
    vc2.increment("n2");
    vc2.increment("n2");
    vc2.increment("n2");
    vc2.increment("n2");
    vc2.increment("n2");
    vc2.increment("n2");
    vc2.increment("n2");
    vc2.increment("n2");
    
    let vc2_before = vc2.clone();
    vc1.merge(&vc2);
    
    // vc2 should remain unchanged
    assert_eq!(vc2, vc2_before);
}

#[test]
fn test_vector_clock_equality_across_different_orderings() {
    // Goal: HashMap iteration order should not affect equality
    let mut vc1 = VectorClock::new();
    for _ in 0..5 { vc1.increment("a"); }
    for _ in 0..7 { vc1.increment("b"); }
    for _ in 0..9 { vc1.increment("c"); }
    
    let mut vc2 = VectorClock::new();
    for _ in 0..9 { vc2.increment("c"); }
    for _ in 0..5 { vc2.increment("a"); }
    for _ in 0..7 { vc2.increment("b"); }
    
    // Should be equal regardless of insertion order
    assert_eq!(vc1, vc2);
    assert!(!vc1.is_concurrent(&vc2));
}

#[test]
fn test_vector_clock_three_way_concurrency() {
    // Goal: All three clocks should be pairwise concurrent
    let mut vc_a = VectorClock::new();
    for _ in 0..5 { vc_a.increment("n1"); }
    
    let mut vc_b = VectorClock::new();
    for _ in 0..3 { vc_b.increment("n2"); }
    
    let mut vc_c = VectorClock::new();
    for _ in 0..7 { vc_c.increment("n3"); }
    
    // All should be pairwise concurrent
    assert!(vc_a.is_concurrent(&vc_b));
    assert!(vc_b.is_concurrent(&vc_c));
    assert!(vc_a.is_concurrent(&vc_c));
}

#[test]
fn test_vector_clock_happened_before_fails_if_any_greater() {
    // Goal: HB must require <= everywhere, and < at least somewhere
    let mut vc_a = VectorClock::new();
    for _ in 0..3 { vc_a.increment("x"); }
    for _ in 0..10 { vc_a.increment("y"); }
    
    let mut vc_b = VectorClock::new();
    for _ in 0..3 { vc_b.increment("x"); }
    for _ in 0..9 { vc_b.increment("y"); }
    
    // A has y:10, B has y:9, so A did NOT happen before B
    assert!(!vc_a.happened_before(&vc_b));
    
    // They're also not concurrent (x is equal, y differs)
    assert!(!vc_a.is_concurrent(&vc_b));
    
    // But B happened before A
    assert!(vc_b.happened_before(&vc_a));
}
