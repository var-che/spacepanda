/*
    Full Replica Merge Edge Case Tests
    
    Tests covering:
    1. Three-way divergent channel names
    2. Interleaving OR-Set and LWW updates
    3. Merge idempotency across all CRDT types
*/

use crate::core_store::model::{Channel, ChannelType, ChannelId, UserId, Timestamp};
use crate::core_store::crdt::{VectorClock, AddId, Crdt};

#[test]
fn test_three_way_divergent_channel_names() {
    // Goal: Three replicas update name at different times, converge to latest
    use crate::core_store::crdt::LWWRegister;
    
    let channel_id = ChannelId::generate();
    let creator = UserId::generate();
    let base_time = Timestamp::now();
    
    let mut rep1 = Channel::new(
        channel_id.clone(),
        "Base".to_string(),
        ChannelType::Text,
        creator.clone(),
        base_time,
        "rep1".to_string(),
    );
    
    let mut rep2 = Channel::new(
        channel_id.clone(),
        "Base".to_string(),
        ChannelType::Text,
        creator.clone(),
        base_time,
        "rep2".to_string(),
    );
    
    let mut rep3 = Channel::new(
        channel_id.clone(),
        "Base".to_string(),
        ChannelType::Text,
        creator,
        base_time,
        "rep3".to_string(),
    );
    
    // Reset name registers with timestamp 0 to start fresh
    rep1.name = LWWRegister::new();
    rep2.name = LWWRegister::new();
    rep3.name = LWWRegister::new();
    
    // rep1: "X" @ ts 10
    let mut vc1 = VectorClock::new();
    vc1.increment("rep1");
    rep1.name.set("X".to_string(), 10, "rep1".to_string(), vc1);
    
    // rep2: "Y" @ ts 20 (latest)
    let mut vc2 = VectorClock::new();
    vc2.increment("rep2");
    rep2.name.set("Y".to_string(), 20, "rep2".to_string(), vc2);
    
    // rep3: "Z" @ ts 15
    let mut vc3 = VectorClock::new();
    vc3.increment("rep3");
    rep3.name.set("Z".to_string(), 15, "rep3".to_string(), vc3);
    
    // Merge all
    rep1.name.merge(&rep2.name);
    rep1.name.merge(&rep3.name);
    
    // Should converge to "Y" (timestamp 20)
    assert_eq!(rep1.get_name(), Some(&"Y".to_string()));
}

#[test]
fn test_interleaving_orset_and_lww_updates() {
    // Goal: Mix LWW register updates with OR-Set concurrent adds
    use crate::core_store::crdt::LWWRegister;
    
    let channel_id = ChannelId::generate();
    let creator = UserId::generate();
    let base_time = Timestamp::now();
    
    let mut rep1 = Channel::new(
        channel_id.clone(),
        "Channel".to_string(),
        ChannelType::Text,
        creator.clone(),
        base_time,
        "rep1".to_string(),
    );
    
    let mut rep2 = Channel::new(
        channel_id,
        "Channel".to_string(),
        ChannelType::Text,
        creator,
        base_time,
        "rep2".to_string(),
    );
    
    // Reset name registers to start fresh
    rep1.name = LWWRegister::new();
    rep2.name = LWWRegister::new();
    
    // rep1: name = "Alpha" @ ts=10
    let mut vc1 = VectorClock::new();
    vc1.increment("rep1");
    rep1.name.set("Alpha".to_string(), 10, "rep1".to_string(), vc1.clone());
    
    // rep2: name = "Beta" @ ts=20
    let mut vc2 = VectorClock::new();
    vc2.increment("rep2");
    rep2.name.set("Beta".to_string(), 20, "rep2".to_string(), vc2.clone());
    
    // rep1: add alice to members
    let alice = UserId::generate();
    let add_id_alice = AddId::new(alice.0.clone(), Timestamp::now().as_millis());
    vc1.increment("rep1");
    rep1.members.add(alice.clone(), add_id_alice, vc1);
    
    // rep2: add bob to members
    let bob = UserId::generate();
    let add_id_bob = AddId::new(bob.0.clone(), Timestamp::now().as_millis());
    vc2.increment("rep2");
    rep2.members.add(bob.clone(), add_id_bob, vc2);
    
    // Merge
    rep1.name.merge(&rep2.name);
    rep1.members.merge(&rep2.members).unwrap();
    
    // Name should be "Beta" (later timestamp)
    assert_eq!(rep1.get_name(), Some(&"Beta".to_string()));
    
    // Both alice and bob should be in members
    assert!(rep1.has_member(&alice));
    assert!(rep1.has_member(&bob));
}

#[test]
fn test_merge_idempotency_across_all_crdt_types() {
    // Goal: Merging multiple times should be idempotent
    let channel_id = ChannelId::generate();
    let creator = UserId::generate();
    let base_time = Timestamp::now();
    
    let rep = Channel::new(
        channel_id.clone(),
        "Test".to_string(),
        ChannelType::Text,
        creator.clone(),
        base_time,
        "node1".to_string(),
    );
    
    let mut rep2 = rep.clone();
    
    // Make some updates to rep2
    let mut vc = VectorClock::new();
    vc.increment("node1");
    rep2.name.set("Updated".to_string(), Timestamp::now().as_millis(), "node1".to_string(), vc.clone());
    
    let alice = UserId::generate();
    let add_id = AddId::new(alice.0.clone(), Timestamp::now().as_millis());
    vc.increment("node1");
    rep2.members.add(alice.clone(), add_id, vc);
    
    // First merge
    let mut merged1 = rep.clone();
    merged1.name.merge(&rep2.name);
    merged1.topic.merge(&rep2.topic);
    merged1.members.merge(&rep2.members).unwrap();
    merged1.pinned_messages.merge(&rep2.pinned_messages).unwrap();
    
    // Second merge (should be idempotent)
    let mut merged2 = merged1.clone();
    merged2.name.merge(&rep2.name);
    merged2.topic.merge(&rep2.topic);
    merged2.members.merge(&rep2.members).unwrap();
    merged2.pinned_messages.merge(&rep2.pinned_messages).unwrap();
    
    // Third merge with original
    let mut merged3 = merged2.clone();
    merged3.name.merge(&rep.name);
    merged3.topic.merge(&rep.topic);
    merged3.members.merge(&rep.members).unwrap();
    merged3.pinned_messages.merge(&rep.pinned_messages).unwrap();
    
    // All merged results should be identical
    assert_eq!(merged1.get_name(), merged2.get_name());
    assert_eq!(merged2.get_name(), merged3.get_name());
    assert_eq!(merged1.has_member(&alice), merged2.has_member(&alice));
    assert_eq!(merged2.has_member(&alice), merged3.has_member(&alice));
}

#[test]
fn test_replica_divergence_and_convergence() {
    // Simulate network partition: two replicas diverge then re-sync
    use crate::core_store::crdt::LWWRegister;
    
    let channel_id = ChannelId::generate();
    let creator = UserId::generate();
    let base_time = Timestamp::now();
    
    let mut rep_a = Channel::new(
        channel_id.clone(),
        "Original".to_string(),
        ChannelType::Text,
        creator.clone(),
        base_time,
        "rep_a".to_string(),
    );
    
    let mut rep_b = rep_a.clone();
    
    // Reset name registers to start fresh
    rep_a.name = LWWRegister::new();
    rep_b.name = LWWRegister::new();
    
    // Partition: both replicas make independent changes
    
    // rep_a side
    let mut vc_a = VectorClock::new();
    vc_a.increment("rep_a");
    rep_a.name.set("Version A".to_string(), 100, "rep_a".to_string(), vc_a.clone());
    
    let user_a = UserId::generate();
    let add_id_a = AddId::new(user_a.0.clone(), 100);
    vc_a.increment("rep_a");
    rep_a.members.add(user_a.clone(), add_id_a, vc_a);
    
    // rep_b side
    let mut vc_b = VectorClock::new();
    vc_b.increment("rep_b");
    rep_b.name.set("Version B".to_string(), 200, "rep_b".to_string(), vc_b.clone());
    
    let user_b = UserId::generate();
    let add_id_b = AddId::new(user_b.0.clone(), 200);
    vc_b.increment("rep_b");
    rep_b.members.add(user_b.clone(), add_id_b, vc_b);
    
    // Network heals: merge both directions
    let mut converged_a = rep_a.clone();
    converged_a.name.merge(&rep_b.name);
    converged_a.members.merge(&rep_b.members).unwrap();
    
    let mut converged_b = rep_b.clone();
    converged_b.name.merge(&rep_a.name);
    converged_b.members.merge(&rep_a.members).unwrap();
    
    // Both should converge to same state
    assert_eq!(converged_a.get_name(), converged_b.get_name());
    assert_eq!(converged_a.get_name(), Some(&"Version B".to_string())); // Later timestamp
    
    // Both members should be present on both replicas
    assert!(converged_a.has_member(&user_a));
    assert!(converged_a.has_member(&user_b));
    assert!(converged_b.has_member(&user_a));
    assert!(converged_b.has_member(&user_b));
}
