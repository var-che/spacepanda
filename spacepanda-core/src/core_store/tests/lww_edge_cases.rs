/*
    LWW-Register Edge Case Tests

    Tests covering:
    1. Equal timestamps & node IDs (no-op merge)
    2. Reverse order merges
    3. Deep merge chains (5-hop replication)
*/

use crate::core_store::crdt::{Crdt, LWWRegister, VectorClock};

#[test]
fn test_lww_merge_with_equal_timestamps_and_node_ids() {
    // Goal: Ensure the merge is a no-op when everything is identical
    let mut vc = VectorClock::new();
    vc.increment("n1");

    let mut reg1 = LWWRegister::new();
    reg1.set("A".to_string(), 100, "n1".to_string(), vc.clone());

    let mut reg2 = LWWRegister::new();
    reg2.set("A".to_string(), 100, "n1".to_string(), vc);

    let mut merged = reg1.clone();
    merged.merge(&reg2);

    assert_eq!(merged.get(), Some(&"A".to_string()));
    assert_eq!(merged.timestamp(), 100);
}

#[test]
fn test_lww_merge_older_into_newer_reverse_order() {
    // Goal: Ensure that merge works even when done in reverse
    let mut vc_old = VectorClock::new();
    vc_old.increment("n1");
    let mut reg_old = LWWRegister::with_value("old".to_string(), "n1".to_string());
    reg_old.set("old".to_string(), 10, "n1".to_string(), vc_old);

    let mut vc_new = VectorClock::new();
    vc_new.increment("n2");
    let mut reg_new = LWWRegister::with_value("new".to_string(), "n2".to_string());
    reg_new.set("new".to_string(), 30, "n2".to_string(), vc_new);

    // Merge in reverse order (newer into older)
    let mut merged = reg_new.clone();
    merged.merge(&reg_old);

    assert_eq!(merged.get(), Some(&"new".to_string()));
}

#[test]
fn test_lww_deep_merge_chain() {
    // Simulates 5-hop replication
    let mut vc = VectorClock::new();
    vc.increment("n0");
    let mut reg = LWWRegister::new();
    reg.set("init".to_string(), 1, "n0".to_string(), vc);

    // Update through 5 hops
    for i in 1..=5 {
        let mut vc_i = VectorClock::new();
        vc_i.increment(&format!("n{}", i));
        reg.set(format!("v{}", i), i as u64, format!("n{}", i), vc_i);
    }

    // Create replicas from different hops
    let mut vc2 = VectorClock::new();
    vc2.increment("n2");
    let mut replica_a = LWWRegister::new();
    replica_a.set("v2".to_string(), 2, "n2".to_string(), vc2);

    let mut vc4 = VectorClock::new();
    vc4.increment("n4");
    let mut replica_b = LWWRegister::new();
    replica_b.set("v4".to_string(), 4, "n4".to_string(), vc4);

    // Merge replicas
    let mut merged = replica_a;
    merged.merge(&replica_b);
    merged.merge(&reg);

    assert_eq!(merged.get(), Some(&"v5".to_string()));
}
