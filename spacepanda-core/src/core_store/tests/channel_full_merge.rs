/*
    Full Channel Replica Merge Test

    Tests covering:
    1. Topic, name, members, pinned must all converge correctly
*/

use crate::core_store::crdt::{AddId, Crdt, LWWRegister, VectorClock};
use crate::core_store::model::{Channel, ChannelId, ChannelType, MessageId, Timestamp, UserId};

#[test]
fn test_channel_full_replica_merge_convergence() {
    // Goal: Full multi-field CRDT merge consistency
    let channel_id = ChannelId::generate();
    let creator = UserId::generate();
    let base_time = Timestamp::now();

    // Create two replicas
    let mut rep1 = Channel::new(
        channel_id.clone(),
        "Initial".to_string(),
        ChannelType::Text,
        creator.clone(),
        base_time,
        "rep1".to_string(),
    );

    let mut rep2 = Channel::new(
        channel_id,
        "Initial".to_string(),
        ChannelType::Text,
        creator,
        base_time,
        "rep2".to_string(),
    );

    // Reset name and topic to start fresh
    rep1.name = LWWRegister::new();
    rep1.topic = LWWRegister::new();
    rep2.name = LWWRegister::new();
    rep2.topic = LWWRegister::new();

    // rep1 state:
    //   name = "A" @ t10
    //   topic = "T1" @ t5
    //   members = { alice }
    //   pinned = { msg1 }
    let mut vc1 = VectorClock::new();
    vc1.increment("rep1");
    rep1.name.set("A".to_string(), 10, "rep1".to_string(), vc1.clone());

    vc1.increment("rep1");
    rep1.topic.set("T1".to_string(), 5, "rep1".to_string(), vc1.clone());

    let alice = UserId::generate();
    let alice_add_id = AddId::new(alice.0.clone(), 100);
    vc1.increment("rep1");
    rep1.members.add(alice.clone(), alice_add_id, vc1.clone());

    let msg1 = MessageId::generate();
    let msg1_add_id = AddId::new(msg1.0.clone(), 101);
    vc1.increment("rep1");
    rep1.pinned_messages.add(msg1.clone(), msg1_add_id, vc1);

    // rep2 state:
    //   name = "B" @ t20 (higher - should win)
    //   topic = "T2" @ t50 (higher - should win)
    //   members = { bob }
    //   pinned = { msg2 }
    let mut vc2 = VectorClock::new();
    vc2.increment("rep2");
    rep2.name.set("B".to_string(), 20, "rep2".to_string(), vc2.clone());

    vc2.increment("rep2");
    rep2.topic.set("T2".to_string(), 50, "rep2".to_string(), vc2.clone());

    let bob = UserId::generate();
    let bob_add_id = AddId::new(bob.0.clone(), 200);
    vc2.increment("rep2");
    rep2.members.add(bob.clone(), bob_add_id, vc2.clone());

    let msg2 = MessageId::generate();
    let msg2_add_id = AddId::new(msg2.0.clone(), 201);
    vc2.increment("rep2");
    rep2.pinned_messages.add(msg2.clone(), msg2_add_id, vc2);

    // Merge rep2 into rep1
    rep1.name.merge(&rep2.name);
    rep1.topic.merge(&rep2.topic);
    rep1.members.merge(&rep2.members).unwrap();
    rep1.pinned_messages.merge(&rep2.pinned_messages).unwrap();

    // Expected final state:
    // name → "B" (higher ts: 20 > 10)
    assert_eq!(rep1.get_name(), Some(&"B".to_string()));

    // topic → "T2" (higher ts: 50 > 5)
    assert_eq!(rep1.get_topic(), Some(&"T2".to_string()));

    // members → { alice, bob } (OR-Set union)
    assert!(rep1.has_member(&alice));
    assert!(rep1.has_member(&bob));

    // pinned → { msg1, msg2 } (OR-Set union)
    assert!(rep1.pinned_messages.contains(&msg1));
    assert!(rep1.pinned_messages.contains(&msg2));
}
