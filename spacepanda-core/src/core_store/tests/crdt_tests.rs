/*
    CRDT unit tests - Testing CRDT correctness and convergence

    Tests:
    1. LWW Register converges on latest timestamp
    2. OR-Set add/remove convergence
    3. OR-Map preserves nested CRDTs
    4. Vector clock causal ordering
*/

use crate::core_store::crdt::{AddId, Crdt, LWWRegister, ORMap, ORSet, VectorClock};
use crate::core_store::model::types::{PermissionLevel, Timestamp};

#[test]
fn test_lww_register_converges_on_latest_timestamp() {
    // Create two registers with different timestamps
    let mut reg_old = LWWRegister::new();
    let mut vc_old = VectorClock::new();
    vc_old.increment("node1");
    reg_old.set("old".to_string(), 10, "node1".to_string(), vc_old);

    let mut reg_new = LWWRegister::new();
    let mut vc_new = VectorClock::new();
    vc_new.increment("node2");
    reg_new.set("new".to_string(), 20, "node2".to_string(), vc_new);

    // Merge old into new using the new standalone merge method
    let mut merged = reg_old.clone();
    merged.merge(&reg_new);

    // Should converge to latest timestamp
    assert_eq!(merged.get(), Some(&"new".to_string()));
}

#[test]
fn test_lww_register_tiebreaker_uses_node_id() {
    // Same timestamp, different node IDs
    let mut reg_a = LWWRegister::new();
    let mut vc_a = VectorClock::new();
    vc_a.increment("node_a");
    reg_a.set("value_a".to_string(), 100, "node_a".to_string(), vc_a);

    let mut reg_b = LWWRegister::new();
    let mut vc_b = VectorClock::new();
    vc_b.increment("node_b");
    reg_b.set("value_b".to_string(), 100, "node_b".to_string(), vc_b);

    // Merge - should use node_id as tiebreaker (lexicographic order)
    let mut merged = reg_a.clone();
    merged.merge(&reg_b);

    // node_b > node_a lexicographically
    assert_eq!(merged.get(), Some(&"value_b".to_string()));
}

#[test]
fn test_orset_add_remove_converges() {
    use crate::core_store::crdt::AddId;

    let mut s1 = ORSet::new();
    let mut s2 = ORSet::new();

    let mut vc1 = VectorClock::new();
    vc1.increment("node1");
    let add_id1 = AddId::new("alice".to_string(), Timestamp::now().as_millis());

    let mut vc2 = VectorClock::new();
    vc2.increment("node2");
    let add_id2 = AddId::new("bob".to_string(), Timestamp::now().as_millis());

    let mut vc3 = VectorClock::new();
    vc3.increment("node1");

    // s1 adds alice, then removes alice
    s1.add("alice".to_string(), add_id1.clone(), vc1.clone());
    s1.remove(&"alice".to_string(), vc3);

    // s2 adds bob
    s2.add("bob".to_string(), add_id2, vc2);

    // Merge
    let mut merged = s1.clone();
    merged.merge(&s2).unwrap();

    // alice was removed, bob remains
    assert!(!merged.contains(&"alice".to_string()));
    assert!(merged.contains(&"bob".to_string()));
}

#[test]
fn test_ormap_nested_crdts_merge() {
    use crate::core_store::crdt::AddId;

    let mut m1 = ORMap::<String, LWWRegister<i32>>::new();
    let mut m2 = ORMap::<String, LWWRegister<i32>>::new();

    // Create LWW registers with different values
    let mut reg1 = LWWRegister::new();
    let mut vc1 = VectorClock::new();
    vc1.increment("node1");
    reg1.set(10, 10, "node1".to_string(), vc1.clone());

    let mut reg2 = LWWRegister::new();
    let mut vc2 = VectorClock::new();
    vc2.increment("node2");
    reg2.set(20, 20, "node2".to_string(), vc2.clone());

    let add_id1 = AddId::new("role1".to_string(), Timestamp::now().as_millis());
    let add_id2 = AddId::new("role1".to_string(), Timestamp::now().as_millis());

    // Put into maps
    m1.put("role1".to_string(), reg1, add_id1, vc1);
    m2.put("role1".to_string(), reg2, add_id2, vc2);

    // Merge
    let mut merged = m1.clone();
    merged.merge_nested(&m2).unwrap();

    // Should have the later timestamp value
    let role_reg = merged.get(&"role1".to_string()).unwrap();
    assert_eq!(role_reg.get(), Some(&20));
}

#[test]
fn test_vector_clock_detects_concurrency() {
    let mut vc1 = VectorClock::new();
    let mut vc2 = VectorClock::new();

    // Both increment independently
    vc1.increment("node1");
    vc2.increment("node2");

    // They should be concurrent
    assert!(vc1.is_concurrent(&vc2));
    assert!(vc2.is_concurrent(&vc1));
}

#[test]
fn test_vector_clock_detects_happened_before() {
    let mut vc1 = VectorClock::new();
    vc1.increment("node1");

    let mut vc2 = vc1.clone();
    vc2.increment("node1");

    // vc1 happened before vc2
    assert!(vc1.happened_before(&vc2));
    assert!(!vc2.happened_before(&vc1));
    assert!(!vc1.is_concurrent(&vc2));
}

#[test]
fn test_orset_concurrent_adds_preserved() {
    use crate::core_store::crdt::AddId;

    let mut s1 = ORSet::new();
    let mut s2 = ORSet::new();

    let mut vc1 = VectorClock::new();
    vc1.increment("node1");

    let mut vc2 = VectorClock::new();
    vc2.increment("node2");

    let add_id1 = AddId::new("alice".to_string(), Timestamp::now().as_millis());
    let add_id2 = AddId::new("bob".to_string(), Timestamp::now().as_millis());

    // Concurrent adds on different nodes
    s1.add("alice".to_string(), add_id1, vc1);
    s2.add("bob".to_string(), add_id2, vc2);

    // Merge both ways
    let mut merged1 = s1.clone();
    merged1.merge(&s2).unwrap();

    let mut merged2 = s2.clone();
    merged2.merge(&s1).unwrap();

    // Both should have both elements
    assert!(merged1.contains(&"alice".to_string()));
    assert!(merged1.contains(&"bob".to_string()));
    assert!(merged2.contains(&"alice".to_string()));
    assert!(merged2.contains(&"bob".to_string()));

    // Merges should be commutative (sort for order-independent comparison)
    let mut elems1 = merged1.elements();
    elems1.sort();
    let mut elems2 = merged2.elements();
    elems2.sort();
    assert_eq!(elems1, elems2);
}

#[test]
fn test_ormap_with_permission_levels() {
    use crate::core_store::crdt::AddId;

    let mut m1 = ORMap::<String, LWWRegister<PermissionLevel>>::new();
    let mut m2 = ORMap::<String, LWWRegister<PermissionLevel>>::new();

    let mut perm_mod = LWWRegister::new();
    let mut vc1 = VectorClock::new();
    vc1.increment("node1");
    perm_mod.set(PermissionLevel::moderator(), 10, "node1".to_string(), vc1.clone());

    let mut perm_admin = LWWRegister::new();
    let mut vc2 = VectorClock::new();
    vc2.increment("node2");
    perm_admin.set(PermissionLevel::admin(), 20, "node2".to_string(), vc2.clone());

    let add_id1 = AddId::new("alice".to_string(), Timestamp::now().as_millis());
    let add_id2 = AddId::new("alice".to_string(), Timestamp::now().as_millis());

    // m1 sets alice to moderator, m2 sets alice to admin (later timestamp)
    m1.put("alice".to_string(), perm_mod, add_id1, vc1);
    m2.put("alice".to_string(), perm_admin, add_id2, vc2);

    // Merge
    let mut merged = m1.clone();
    merged.merge_nested(&m2).unwrap();

    // Should converge to admin (later timestamp)
    let alice_perm = merged.get(&"alice".to_string()).unwrap();
    assert_eq!(alice_perm.get(), Some(&PermissionLevel::admin()));
}
