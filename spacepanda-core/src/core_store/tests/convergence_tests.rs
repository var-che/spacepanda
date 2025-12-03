/*
    Convergence tests - Testing full replica convergence scenarios

    Tests:
    1. Full channel replica convergence
    2. Full space replica convergence
    3. Commutative and associative merge properties
    4. Idempotent merge operations
*/

use crate::core_store::crdt::{AddId, Crdt, LWWRegister, VectorClock};
use crate::core_store::model::types::{
    ChannelId, ChannelType, PermissionLevel, SpaceId, Timestamp, UserId,
};
use crate::core_store::model::{Channel, Role, Space};

#[test]
fn test_full_channel_replica_convergence() {
    let channel_id = ChannelId::generate();
    let creator = UserId::generate();
    let now = Timestamp::now();

    // Create two replicas
    let mut replica1 = Channel::new(
        channel_id.clone(),
        "General".to_string(),
        ChannelType::Text,
        creator.clone(),
        now,
        "node1".to_string(),
    );

    let mut replica2 = Channel::new(
        channel_id,
        "General".to_string(),
        ChannelType::Text,
        creator,
        now,
        "node2".to_string(),
    );

    // Make divergent edits
    let alice = UserId::generate();
    let bob = UserId::generate();

    let mut vc1 = VectorClock::new();
    vc1.increment("node1");
    let mut vc2 = VectorClock::new();
    vc2.increment("node2");

    let ts_base = Timestamp::now().as_millis();
    replica1.name.set(
        "Updated Name 1".to_string(),
        ts_base + 100,
        "node1".to_string(),
        vc1.clone(),
    );
    replica2.name.set(
        "Updated Name 2".to_string(),
        ts_base + 200,
        "node2".to_string(),
        vc2.clone(),
    );

    let add_id_alice = AddId::new(alice.0.clone(), Timestamp::now().as_millis());
    let add_id_bob = AddId::new(bob.0.clone(), Timestamp::now().as_millis());

    replica1.members.add(alice.clone(), add_id_alice, vc1);
    replica2.members.add(bob.clone(), add_id_bob, vc2);

    // Sync: merge in both directions
    let mut synced1 = replica1.clone();
    synced1.name.merge(&replica2.name);
    synced1.topic.merge(&replica2.topic);
    synced1.members.merge(&replica2.members).unwrap();
    synced1.pinned_messages.merge(&replica2.pinned_messages).unwrap();
    synced1.permissions.merge_nested(&replica2.permissions).unwrap();
    synced1.mls_identity.merge(&replica2.mls_identity).unwrap();

    let mut synced2 = replica2.clone();
    synced2.name.merge(&replica1.name);
    synced2.topic.merge(&replica1.topic);
    synced2.members.merge(&replica1.members).unwrap();
    synced2.pinned_messages.merge(&replica1.pinned_messages).unwrap();
    synced2.permissions.merge_nested(&replica1.permissions).unwrap();
    synced2.mls_identity.merge(&replica1.mls_identity).unwrap();

    // Both should converge to same state
    assert_eq!(synced1.get_name(), synced2.get_name());
    assert_eq!(synced1.get_members().len(), synced2.get_members().len());

    // Both members should be present
    assert!(synced1.has_member(&alice));
    assert!(synced1.has_member(&bob));
    assert!(synced2.has_member(&alice));
    assert!(synced2.has_member(&bob));

    // Name should be "Updated Name 2" (higher timestamp)
    assert_eq!(synced1.get_name(), Some(&"Updated Name 2".to_string()));
    assert_eq!(synced2.get_name(), Some(&"Updated Name 2".to_string()));
}

#[test]
fn test_full_space_replica_convergence() {
    let space_id = SpaceId::generate();
    let owner = UserId::generate();
    let now = Timestamp::now();

    let mut replica1 =
        Space::new(space_id.clone(), "Server".to_string(), owner.clone(), now, "node1".to_string());

    let mut replica2 = Space::new(space_id, "Server".to_string(), owner, now, "node2".to_string());

    // Divergent updates
    let alice = UserId::generate();
    let bob = UserId::generate();

    let mut vc1 = VectorClock::new();
    vc1.increment("node1");
    let mut vc2 = VectorClock::new();
    vc2.increment("node2");

    let add_id_alice = AddId::new(alice.0.clone(), Timestamp::now().as_millis());
    let add_id_bob = AddId::new(bob.0.clone(), Timestamp::now().as_millis());

    replica1.members.add(alice.clone(), add_id_alice, vc1.clone());
    replica2.members.add(bob.clone(), add_id_bob, vc2.clone());

    // Add different roles
    let admin_role = Role::new("Admin".to_string(), PermissionLevel::admin(), "node1".to_string());
    let mod_role = Role::new("Mod".to_string(), PermissionLevel::moderator(), "node2".to_string());

    let add_id_admin = AddId::new("admin".to_string(), Timestamp::now().as_millis());
    let add_id_mod = AddId::new("mod".to_string(), Timestamp::now().as_millis());

    replica1.roles.put("admin".to_string(), admin_role, add_id_admin, vc1);
    replica2.roles.put("mod".to_string(), mod_role, add_id_mod, vc2);

    // Sync both ways
    let mut synced1 = replica1.clone();
    synced1.name.merge(&replica2.name);
    synced1.description.merge(&replica2.description);
    synced1.channels.merge(&replica2.channels).unwrap();
    synced1.members.merge(&replica2.members).unwrap();
    synced1.roles.merge_nested(&replica2.roles).unwrap();
    synced1.member_roles.merge_nested(&replica2.member_roles).unwrap();
    synced1.mls_identity.merge(&replica2.mls_identity).unwrap();

    let mut synced2 = replica2.clone();
    synced2.name.merge(&replica1.name);
    synced2.description.merge(&replica1.description);
    synced2.channels.merge(&replica1.channels).unwrap();
    synced2.members.merge(&replica1.members).unwrap();
    synced2.roles.merge_nested(&replica1.roles).unwrap();
    synced2.member_roles.merge_nested(&replica1.member_roles).unwrap();
    synced2.mls_identity.merge(&replica1.mls_identity).unwrap();

    // Should converge
    assert_eq!(synced1.get_members().len(), synced2.get_members().len());
    assert_eq!(synced1.get_all_roles().len(), synced2.get_all_roles().len());

    // Both members present
    assert!(synced1.has_member(&alice));
    assert!(synced1.has_member(&bob));
    assert!(synced2.has_member(&alice));
    assert!(synced2.has_member(&bob));

    // Both roles present
    assert!(synced1.get_role("admin").is_some());
    assert!(synced1.get_role("mod").is_some());
    assert!(synced2.get_role("admin").is_some());
    assert!(synced2.get_role("mod").is_some());
}

#[test]
fn test_merge_is_commutative() {
    let channel_id = ChannelId::generate();
    let creator = UserId::generate();
    let now = Timestamp::now();

    let mut chan_a = Channel::new(
        channel_id.clone(),
        "A".to_string(),
        ChannelType::Text,
        creator.clone(),
        now,
        "node_a".to_string(),
    );

    let mut chan_b = Channel::new(
        channel_id,
        "B".to_string(),
        ChannelType::Text,
        creator,
        now,
        "node_b".to_string(),
    );

    let alice = UserId::generate();
    let bob = UserId::generate();

    let mut vc_a = VectorClock::new();
    vc_a.increment("node_a");
    let mut vc_b = VectorClock::new();
    vc_b.increment("node_b");

    let add_id_alice = AddId::new(alice.0.clone(), Timestamp::now().as_millis());
    let add_id_bob = AddId::new(bob.0.clone(), Timestamp::now().as_millis());

    chan_a.members.add(alice.clone(), add_id_alice, vc_a);
    chan_b.members.add(bob.clone(), add_id_bob, vc_b);

    // Merge A into B
    let mut merged_ab = chan_a.clone();
    merged_ab.members.merge(&chan_b.members).unwrap();

    // Merge B into A
    let mut merged_ba = chan_b.clone();
    merged_ba.members.merge(&chan_a.members).unwrap();

    // Should be commutative: A ⊔ B = B ⊔ A
    assert_eq!(merged_ab.get_members().len(), merged_ba.get_members().len());
    assert!(merged_ab.has_member(&alice));
    assert!(merged_ab.has_member(&bob));
    assert!(merged_ba.has_member(&alice));
    assert!(merged_ba.has_member(&bob));
}

#[test]
fn test_merge_is_idempotent() {
    let channel_id = ChannelId::generate();
    let creator = UserId::generate();
    let now = Timestamp::now();

    let mut chan = Channel::new(
        channel_id,
        "Channel".to_string(),
        ChannelType::Text,
        creator,
        now,
        "node1".to_string(),
    );

    let alice = UserId::generate();

    let mut vc = VectorClock::new();
    vc.increment("node1");
    let add_id = AddId::new(alice.0.clone(), Timestamp::now().as_millis());

    chan.members.add(alice.clone(), add_id, vc);

    let original_count = chan.get_members().len();

    // Merge with itself multiple times
    chan.members.merge(&chan.members.clone()).unwrap();
    chan.members.merge(&chan.members.clone()).unwrap();
    chan.members.merge(&chan.members.clone()).unwrap();

    // Should be idempotent: A ⊔ A = A
    assert_eq!(chan.get_members().len(), original_count);
    assert!(chan.has_member(&alice));
}

#[test]
fn test_three_way_merge_convergence() {
    let space_id = SpaceId::generate();
    let owner = UserId::generate();
    let now = Timestamp::now();

    // Three replicas
    let mut replica1 =
        Space::new(space_id.clone(), "Space".to_string(), owner.clone(), now, "node1".to_string());

    let mut replica2 =
        Space::new(space_id.clone(), "Space".to_string(), owner.clone(), now, "node2".to_string());

    let mut replica3 = Space::new(space_id, "Space".to_string(), owner, now, "node3".to_string());

    // Each adds a different member
    let alice = UserId::generate();
    let bob = UserId::generate();
    let charlie = UserId::generate();

    let mut vc1 = VectorClock::new();
    vc1.increment("node1");
    let mut vc2 = VectorClock::new();
    vc2.increment("node2");
    let mut vc3 = VectorClock::new();
    vc3.increment("node3");

    let add_id_alice = AddId::new(alice.0.clone(), Timestamp::now().as_millis());
    let add_id_bob = AddId::new(bob.0.clone(), Timestamp::now().as_millis());
    let add_id_charlie = AddId::new(charlie.0.clone(), Timestamp::now().as_millis());

    replica1.members.add(alice.clone(), add_id_alice, vc1);
    replica2.members.add(bob.clone(), add_id_bob, vc2);
    replica3.members.add(charlie.clone(), add_id_charlie, vc3);

    // Sync: 1 merges with 2, then with 3
    replica1.members.merge(&replica2.members).unwrap();
    replica1.members.merge(&replica3.members).unwrap();

    // All three members should be present
    assert!(replica1.has_member(&alice));
    assert!(replica1.has_member(&bob));
    assert!(replica1.has_member(&charlie));
    assert_eq!(replica1.get_members().len(), 3);
}
