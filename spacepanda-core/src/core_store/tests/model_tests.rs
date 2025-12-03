/*
    Model integration tests - Testing Channel, Space, and Role convergence

    Tests:
    1. Channel metadata convergence
    2. Space role definitions merge correctly
    3. Member role assignments converge deterministically
    4. Identity metadata is preserved across merges
    5. Permission checks work correctly
*/

use crate::core_store::crdt::{AddId, Crdt, LWWRegister, VectorClock};
use crate::core_store::model::types::{
    ChannelId, ChannelType, IdentityMeta, MessageId, PermissionLevel, SpaceId, Timestamp, UserId,
};
use crate::core_store::model::{Channel, Role, Space};

#[test]
fn test_channel_metadata_converges() {
    let channel_id = ChannelId::generate();
    let creator = UserId::generate();
    let now = Timestamp::now();

    let mut chan_a = Channel::new(
        channel_id.clone(),
        "Hello".to_string(),
        ChannelType::Text,
        creator.clone(),
        now,
        "node_a".to_string(),
    );

    let mut chan_b = Channel::new(
        channel_id.clone(),
        "World".to_string(),
        ChannelType::Text,
        creator.clone(),
        now,
        "node_b".to_string(),
    );

    // Update name on different nodes with different timestamps
    let mut vc_a = VectorClock::new();
    vc_a.increment("node_a");
    let ts_a = Timestamp::now().as_millis();
    chan_a
        .name
        .set("Hello World".to_string(), ts_a, "node_a".to_string(), vc_a.clone());

    let mut vc_b = VectorClock::new();
    vc_b.increment("node_b");
    let ts_b = ts_a + 1000; // Later timestamp
    chan_b
        .name
        .set("Updated Topic".to_string(), ts_b, "node_b".to_string(), vc_b.clone());

    // Add different members concurrently
    let alice = UserId::generate();
    let bob = UserId::generate();

    let add_id_alice = AddId::new(alice.0.clone(), Timestamp::now().as_millis());
    let add_id_bob = AddId::new(bob.0.clone(), Timestamp::now().as_millis());

    chan_a.members.add(alice.clone(), add_id_alice, vc_a);
    chan_b.members.add(bob.clone(), add_id_bob, vc_b);

    // Merge channel names
    chan_a.name.merge(&chan_b.name);
    chan_a.topic.merge(&chan_b.topic);
    chan_a.members.merge(&chan_b.members).unwrap();

    // Name should converge to latest timestamp
    assert_eq!(chan_a.get_name(), Some(&"Updated Topic".to_string()));

    // Both members should be present
    assert!(chan_a.has_member(&alice));
    assert!(chan_a.has_member(&bob));
}

#[test]
fn test_space_roles_merge_correctly() {
    let space_id = SpaceId::generate();
    let owner = UserId::generate();
    let now = Timestamp::now();

    let mut space1 = Space::new(
        space_id.clone(),
        "My Space".to_string(),
        owner.clone(),
        now,
        "node1".to_string(),
    );

    let mut space2 = Space::new(space_id, "My Space".to_string(), owner, now, "node2".to_string());

    // Create different roles on each node
    let admin_role = Role::new("Admin".to_string(), PermissionLevel::admin(), "node1".to_string());
    let mod_role =
        Role::new("Moderator".to_string(), PermissionLevel::moderator(), "node2".to_string());

    let mut vc1 = VectorClock::new();
    vc1.increment("node1");
    let add_id1 = AddId::new("admin".to_string(), Timestamp::now().as_millis());

    let mut vc2 = VectorClock::new();
    vc2.increment("node2");
    let add_id2 = AddId::new("mod".to_string(), Timestamp::now().as_millis());

    space1.roles.put("admin".to_string(), admin_role, add_id1, vc1);
    space2.roles.put("mod".to_string(), mod_role, add_id2, vc2);

    // Merge
    space1.roles.merge_nested(&space2.roles).unwrap();

    // Both roles should be present
    assert!(space1.get_role("admin").is_some());
    assert!(space1.get_role("mod").is_some());

    assert_eq!(space1.get_role("admin").unwrap().get_name(), Some(&"Admin".to_string()));
    assert_eq!(space1.get_role("mod").unwrap().get_name(), Some(&"Moderator".to_string()));
}

#[test]
fn test_member_role_assignments_converge() {
    let space_id = SpaceId::generate();
    let owner = UserId::generate();
    let now = Timestamp::now();

    let mut space1 =
        Space::new(space_id.clone(), "Space".to_string(), owner.clone(), now, "node1".to_string());

    let mut space2 = Space::new(space_id, "Space".to_string(), owner, now, "node2".to_string());

    let alice = UserId::generate();

    // node1 assigns alice as moderator at time 10
    let mut role_mod = LWWRegister::new();
    let mut vc1 = VectorClock::new();
    vc1.increment("node1");
    role_mod.set("mod".to_string(), 10, "node1".to_string(), vc1.clone());

    // node2 assigns alice as admin at time 20 (later)
    let mut role_admin = LWWRegister::new();
    let mut vc2 = VectorClock::new();
    vc2.increment("node2");
    role_admin.set("admin".to_string(), 20, "node2".to_string(), vc2.clone());

    let add_id1 = AddId::new(alice.0.clone(), Timestamp::from_millis(10).as_millis());
    let add_id2 = AddId::new(alice.0.clone(), Timestamp::from_millis(20).as_millis());

    space1.member_roles.put(alice.clone(), role_mod, add_id1, vc1);
    space2.member_roles.put(alice.clone(), role_admin, add_id2, vc2);

    // Merge
    space1.member_roles.merge_nested(&space2.member_roles).unwrap();

    // Should converge to admin (later timestamp)
    assert_eq!(space1.get_user_role_id(&alice), Some(&"admin".to_string()));
}

#[test]
fn test_identity_metadata_converges() {
    let channel_id = ChannelId::generate();
    let creator = UserId::generate();
    let now = Timestamp::now();

    let mut chan1 = Channel::new(
        channel_id.clone(),
        "Channel".to_string(),
        ChannelType::Text,
        creator.clone(),
        now,
        "node1".to_string(),
    );

    let mut chan2 = Channel::new(
        channel_id,
        "Channel".to_string(),
        ChannelType::Text,
        creator,
        now,
        "node2".to_string(),
    );

    let alice = UserId::generate();
    let identity = IdentityMeta::new(0, vec![1, 2, 3], vec![4, 5, 6]);

    let mut vc1 = VectorClock::new();
    vc1.increment("node1");
    let add_id = AddId::new(alice.0.clone(), Timestamp::now().as_millis());

    chan1.mls_identity.put(alice.clone(), identity.clone(), add_id, vc1);

    // Merge
    chan1.mls_identity.merge(&chan2.mls_identity).unwrap();

    // Identity should be preserved
    assert_eq!(chan1.get_mls_identity(&alice), Some(&identity));
}

#[test]
fn test_owner_always_has_admin_permissions() {
    let space_id = SpaceId::generate();
    let owner = UserId::generate();
    let now = Timestamp::now();

    let space = Space::new(space_id, "Space".to_string(), owner.clone(), now, "node1".to_string());

    // Owner should have admin permissions even without explicit role
    assert_eq!(space.get_user_permission_level(&owner), Some(PermissionLevel::admin()));
}

#[test]
fn test_non_owner_uses_role_permissions() {
    let space_id = SpaceId::generate();
    let owner = UserId::generate();
    let now = Timestamp::now();

    let mut space =
        Space::new(space_id, "Space".to_string(), owner.clone(), now, "node1".to_string());

    // Create a member role
    let member_role =
        Role::new("Member".to_string(), PermissionLevel::member(), "node1".to_string());

    let mut vc = VectorClock::new();
    vc.increment("node1");
    let add_id = AddId::new("member".to_string(), Timestamp::now().as_millis());

    space.roles.put("member".to_string(), member_role, add_id.clone(), vc.clone());

    // Assign role to alice
    let alice = UserId::generate();
    let mut role_reg = LWWRegister::new();
    role_reg.set("member".to_string(), now.as_millis(), "node1".to_string(), vc.clone());

    let add_id_alice = AddId::new(alice.0.clone(), Timestamp::now().as_millis());
    space.member_roles.put(alice.clone(), role_reg, add_id_alice, vc);

    // Alice should have member permissions
    assert_eq!(space.get_user_permission_level(&alice), Some(PermissionLevel::member()));
}

#[test]
fn test_channel_pinned_messages_merge() {
    let channel_id = ChannelId::generate();
    let creator = UserId::generate();
    let now = Timestamp::now();

    let mut chan1 = Channel::new(
        channel_id.clone(),
        "Channel".to_string(),
        ChannelType::Text,
        creator.clone(),
        now,
        "node1".to_string(),
    );

    let mut chan2 = Channel::new(
        channel_id,
        "Channel".to_string(),
        ChannelType::Text,
        creator,
        now,
        "node2".to_string(),
    );

    let msg1 = MessageId::generate();
    let msg2 = MessageId::generate();

    let mut vc1 = VectorClock::new();
    vc1.increment("node1");
    let add_id1 = AddId::new(msg1.0.clone(), Timestamp::now().as_millis());

    let mut vc2 = VectorClock::new();
    vc2.increment("node2");
    let add_id2 = AddId::new(msg2.0.clone(), Timestamp::now().as_millis());

    // Pin different messages on each node
    chan1.pinned_messages.add(msg1.clone(), add_id1, vc1);
    chan2.pinned_messages.add(msg2.clone(), add_id2, vc2);

    // Merge
    chan1.pinned_messages.merge(&chan2.pinned_messages).unwrap();

    // Both messages should be pinned
    assert!(chan1.is_pinned(&msg1));
    assert!(chan1.is_pinned(&msg2));
}
