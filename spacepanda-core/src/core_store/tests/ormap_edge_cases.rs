/*
    OR-Map / Nested CRDT Edge Case Tests
    
    Tests covering:
    1. Remove key should remove nested registers too
    2. Nested CRDT merge preservation
    3. Re-add key after remove creates fresh CRDT
*/

use crate::core_store::crdt::{ORMap, LWWRegister, VectorClock, AddId, Crdt};

#[test]
fn test_ormap_remove_key_removes_nested_registers() {
    // Goal: Remove tombstone only affects adds it has seen
    let mut map1 = ORMap::new();
    let mut map2 = ORMap::new();
    
    // map1: put then remove
    let mut vc1 = VectorClock::new();
    vc1.increment("n1");
    let mut reg1 = LWWRegister::new();
    reg1.set(10, 10, "n1".to_string(), vc1.clone());
    let add_id1 = AddId::new("k".to_string(), 1);
    map1.put("k".to_string(), reg1, add_id1, vc1.clone());
    
    // map2: put with different add_id
    let mut vc2 = VectorClock::new();
    vc2.increment("n2");
    let mut reg2 = LWWRegister::new();
    reg2.set(20, 20, "n2".to_string(), vc2.clone());
    let add_id2 = AddId::new("k".to_string(), 2);
    map2.put("k".to_string(), reg2, add_id2, vc2);
    
    // Merge first so map1 sees map2's add
    map1.merge_nested(&map2).unwrap();
    
    // Now remove (will tombstone both add_ids)
    vc1.increment("n1");
    map1.remove(&"k".to_string(), vc1);
    
    // map should NOT contain "k" (all adds tombstoned)
    assert!(!map1.contains_key(&"k".to_string()));
}

#[test]
fn test_ormap_nested_crdt_merge_preservation() {
    // Goal: Multi-hop nested merges converge properly
    let mut map1 = ORMap::new();
    let mut map2 = ORMap::new();
    let mut map3 = ORMap::new();
    
    let mut vc1 = VectorClock::new();
    vc1.increment("n1");
    let mut reg1 = LWWRegister::new();
    reg1.set(10, 10, "n1".to_string(), vc1.clone());
    let add_id1 = AddId::new("roleX".to_string(), 1);
    map1.put("roleX".to_string(), reg1, add_id1, vc1);
    
    let mut vc2 = VectorClock::new();
    vc2.increment("n2");
    let mut reg2 = LWWRegister::new();
    reg2.set(20, 20, "n2".to_string(), vc2.clone());
    let add_id2 = AddId::new("roleX".to_string(), 2);
    map2.put("roleX".to_string(), reg2, add_id2, vc2);
    
    let mut vc3 = VectorClock::new();
    vc3.increment("n3");
    let mut reg3 = LWWRegister::new();
    reg3.set(15, 15, "n3".to_string(), vc3.clone());
    let add_id3 = AddId::new("roleX".to_string(), 3);
    map3.put("roleX".to_string(), reg3, add_id3, vc3);
    
    // Multi-hop merge
    map1.merge_nested(&map2).unwrap();
    map1.merge_nested(&map3).unwrap();
    
    // Should converge to value with highest timestamp (20)
    let role_reg = map1.get(&"roleX".to_string()).unwrap();
    assert_eq!(role_reg.get(), Some(&20));
}

#[test]
fn test_ormap_readd_key_after_remove_creates_fresh_crdt() {
    // Goal: Re-adding after remove should create a fresh CRDT instance
    let mut map1 = ORMap::new();
    let mut map2 = ORMap::new();
    
    // map1: put then remove
    let mut vc1 = VectorClock::new();
    vc1.increment("n1");
    let mut reg1 = LWWRegister::new();
    reg1.set(1, 1, "n1".to_string(), vc1.clone());
    let add_id1 = AddId::new("a".to_string(), 1);
    map1.put("a".to_string(), reg1, add_id1, vc1.clone());
    
    vc1.increment("n1");
    vc1.increment("n1");
    map1.remove(&"a".to_string(), vc1);
    
    // map2: fresh add with new value
    let mut vc2 = VectorClock::new();
    vc2.increment("n2");
    let mut reg2 = LWWRegister::new();
    reg2.set(99, 99, "n2".to_string(), vc2.clone());
    let add_id2 = AddId::new("a".to_string(), 99);
    map2.put("a".to_string(), reg2, add_id2, vc2);
    
    // Merge
    map1.merge_nested(&map2).unwrap();
    
    // Should contain the new value
    let reg = map1.get(&"a".to_string()).unwrap();
    assert_eq!(reg.get(), Some(&99));
}

#[test]
fn test_ormap_concurrent_puts_with_different_values() {
    // Two nodes put different values for same key concurrently
    let mut map1 = ORMap::new();
    let mut map2 = ORMap::new();
    
    let mut vc1 = VectorClock::new();
    vc1.increment("n1");
    let mut reg1 = LWWRegister::new();
    reg1.set("value1".to_string(), 10, "n1".to_string(), vc1.clone());
    let add_id1 = AddId::new("key".to_string(), 10);
    map1.put("key".to_string(), reg1, add_id1, vc1);
    
    let mut vc2 = VectorClock::new();
    vc2.increment("n2");
    let mut reg2 = LWWRegister::new();
    reg2.set("value2".to_string(), 20, "n2".to_string(), vc2.clone());
    let add_id2 = AddId::new("key".to_string(), 20);
    map2.put("key".to_string(), reg2, add_id2, vc2);
    
    // Merge
    map1.merge_nested(&map2).unwrap();
    
    // Should converge to later timestamp
    let reg = map1.get(&"key".to_string()).unwrap();
    assert_eq!(reg.get(), Some(&"value2".to_string()));
}
