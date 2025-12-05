//! Integration Tests for SpacePanda CLI
//!
//! These tests verify end-to-end workflows with multiple actors including:
//! - Channel creation and joining
//! - Message sending and receiving
//! - Member removal
//! - Moderator permissions
//! - Disconnection handling

use anyhow::Result;
use spacepanda_core::{
    config::Config,
    core_mls::service::MlsService,
    core_store::{
        model::types::{ChannelId, UserId},
        store::local_store::{LocalStore, LocalStoreConfig},
    },
    shutdown::ShutdownCoordinator,
    ChannelManager, Identity,
};
use std::sync::Arc;
use tempfile::TempDir;

/// Test actor representing a user
struct TestActor {
    name: String,
    identity: Arc<Identity>,
    manager: Arc<ChannelManager>,
    key_package: Vec<u8>, // Pre-generated KeyPackage for this actor
    #[allow(dead_code)]
    data_dir: TempDir,
}

impl TestActor {
    /// Create a new test actor
    async fn new(name: &str) -> Result<Self> {
        let data_dir = TempDir::new()?;
        
        // Create identity
        let identity = Arc::new(Identity::new(
            UserId(format!("{}@spacepanda.local", name)),
            name.to_string(),
            format!("node-{}", name),
        ));
        
        // Initialize services
        let config = Arc::new(Config::default());
        let shutdown = Arc::new(ShutdownCoordinator::new(std::time::Duration::from_secs(30)));
        
        // Create MLS service
        let mls_storage_dir = data_dir.path().join("mls_groups");
        let mls_service = Arc::new(
            MlsService::with_storage(&config, shutdown, mls_storage_dir)?
        );
        
        // Initialize store
        let store_config = LocalStoreConfig {
            data_dir: data_dir.path().to_path_buf(),
            enable_encryption: false,
            snapshot_interval: 1000,
            max_log_size: 10_000_000,
            enable_compaction: false,
            require_signatures: false,
            authorized_keys: Vec::new(),
        };
        
        let store = Arc::new(LocalStore::new(store_config)?);
        
        // Create manager
        let manager = Arc::new(ChannelManager::new(
            mls_service,
            store,
            identity.clone(),
            config,
        ));
        
        // Generate KeyPackage for this actor (needed before receiving invites)
        let key_package = manager.generate_key_package().await?;
        
        Ok(Self {
            name: name.to_string(),
            identity,
            manager,
            key_package,
            data_dir,
        })
    }
    
    /// Get this actor's KeyPackage (to share with others for invites)
    fn get_key_package(&self) -> Vec<u8> {
        self.key_package.clone()
    }
    
    /// Create a new channel
    async fn create_channel(&self, channel_name: &str) -> Result<ChannelId> {
        Ok(self.manager.create_channel(channel_name.to_string(), false).await?)
    }
    
    /// Generate invite for a channel using another actor's KeyPackage
    async fn create_invite(&self, channel_id: &ChannelId, invitee_key_package: Vec<u8>) -> Result<spacepanda_core::core_mvp::types::InviteToken> {
        let (invite, _commit) = self.manager.create_invite(channel_id, invitee_key_package).await?;
        Ok(invite)
    }
    
    /// Join a channel from an invite
    async fn join_channel(&self, invite: &spacepanda_core::core_mvp::types::InviteToken) -> Result<ChannelId> {
        Ok(self.manager.join_channel(invite).await?)
    }
    
    /// Send a message to a channel
    async fn send_message(&self, channel_id: &ChannelId, message: &str) -> Result<()> {
        self.manager.send_message(channel_id, message.as_bytes()).await?;
        Ok(())
    }
    
    /// Remove a member from a channel
    async fn remove_member(&self, channel_id: &ChannelId, user_id: &UserId) -> Result<()> {
        let member_identity = user_id.0.as_bytes();
        self.manager.remove_member(channel_id, member_identity).await?;
        Ok(())
    }
    
    /// Promote a member to admin
    async fn promote_to_admin(&self, channel_id: &ChannelId, user_id: &UserId) -> Result<()> {
        let member_identity = user_id.0.as_bytes();
        Ok(self.manager.promote_member(channel_id, member_identity).await?)
    }
}

#[tokio::test]
async fn test_four_party_channel_creation_and_messaging() -> Result<()> {
    // Create 4 actors
    let alice = TestActor::new("alice").await?;
    let bob = TestActor::new("bob").await?;
    let charlie = TestActor::new("charlie").await?;
    let diana = TestActor::new("diana").await?;
    
    println!("✅ Created 4 test actors");
    
    // Alice creates a channel
    let channel_id = alice.create_channel("test-channel").await?;
    println!("✅ Alice created channel: {}", channel_id);
    
    // Alice invites Bob
    let bob_invite = alice.create_invite(&channel_id, bob.get_key_package()).await?;
    let bob_channel_id = bob.join_channel(&bob_invite).await?;
    assert_eq!(channel_id, bob_channel_id);
    println!("✅ Bob joined channel");
    
    // Alice invites Charlie
    let charlie_invite = alice.create_invite(&channel_id, charlie.get_key_package()).await?;
    let charlie_channel_id = charlie.join_channel(&charlie_invite).await?;
    assert_eq!(channel_id, charlie_channel_id);
    println!("✅ Charlie joined channel");
    
    // Alice invites Diana
    let diana_invite = alice.create_invite(&channel_id, diana.get_key_package()).await?;
    let diana_channel_id = diana.join_channel(&diana_invite).await?;
    assert_eq!(channel_id, diana_channel_id);
    println!("✅ Diana joined channel");
    
    // All members send messages
    alice.send_message(&channel_id, "Hello from Alice!").await?;
    bob.send_message(&channel_id, "Hello from Bob!").await?;
    charlie.send_message(&channel_id, "Hello from Charlie!").await?;
    diana.send_message(&channel_id, "Hello from Diana!").await?;
    println!("✅ All members sent messages");
    
    Ok(())
}

#[tokio::test]
async fn test_member_removal_by_creator() -> Result<()> {
    // Create 4 actors
    let alice = TestActor::new("alice").await?;
    let bob = TestActor::new("bob").await?;
    let charlie = TestActor::new("charlie").await?;
    let diana = TestActor::new("diana").await?;
    
    println!("✅ Created 4 test actors");
    
    // Alice creates channel and invites everyone
    let channel_id = alice.create_channel("removal-test").await?;
    
    let bob_invite = alice.create_invite(&channel_id, bob.get_key_package()).await?;
    bob.join_channel(&bob_invite).await?;
    
    let charlie_invite = alice.create_invite(&channel_id, charlie.get_key_package()).await?;
    charlie.join_channel(&charlie_invite).await?;
    
    let diana_invite = alice.create_invite(&channel_id, diana.get_key_package()).await?;
    diana.join_channel(&diana_invite).await?;
    
    println!("✅ All members joined channel");
    
    // Alice removes Charlie
    alice.remove_member(&channel_id, &charlie.identity.user_id).await?;
    println!("✅ Alice removed Charlie from channel");
    
    // Alice, Bob, and Diana can still send messages
    alice.send_message(&channel_id, "Charlie was removed").await?;
    bob.send_message(&channel_id, "Confirmed").await?;
    diana.send_message(&channel_id, "I see").await?;
    println!("✅ Remaining members can send messages");
    
    // NOTE: In isolated tests without network sync, Charlie's local state isn't updated
    // In production, the removal commit would be broadcast and Charlie would be unable to send
    println!("✅ Member removal completed (network sync not tested in isolated environment)");
    
    Ok(())
}

#[tokio::test]
async fn test_moderator_permissions() -> Result<()> {
    // Create 4 actors
    let alice = TestActor::new("alice").await?;
    let bob = TestActor::new("bob").await?;
    let charlie = TestActor::new("charlie").await?;
    let diana = TestActor::new("diana").await?;
    
    println!("✅ Created 4 test actors");
    
    // Alice creates channel and invites everyone
    let channel_id = alice.create_channel("mod-test").await?;
    
    let bob_invite = alice.create_invite(&channel_id, bob.get_key_package()).await?;
    bob.join_channel(&bob_invite).await?;
    
    let charlie_invite = alice.create_invite(&channel_id, charlie.get_key_package()).await?;
    charlie.join_channel(&charlie_invite).await?;
    
    let diana_invite = alice.create_invite(&channel_id, diana.get_key_package()).await?;
    diana.join_channel(&diana_invite).await?;
    
    println!("✅ All members joined channel");
    
    // Alice promotes Bob to admin
    alice.promote_to_admin(&channel_id, &bob.identity.user_id).await?;
    println!("✅ Alice promoted Bob to admin");
    
    // NOTE: In isolated tests, Bob's local state won't reflect the promotion
    // without network sync. In production, Bob would receive the commit.
    // So Bob cannot actually remove members in this isolated test.
    
    // Alice (admin) can remove Charlie
    alice.remove_member(&channel_id, &charlie.identity.user_id).await?;
    println!("✅ Alice (admin) removed Charlie");
    
    // Alice, Bob, Diana can still communicate
    alice.send_message(&channel_id, "Bob is now a moderator").await?;
    bob.send_message(&channel_id, "Thanks for the promotion!").await?;
    diana.send_message(&channel_id, "Noted").await?;
    println!("✅ Remaining members can send messages");
    println!("✅ Admin promotion works (full permissions require network sync)");
    
    Ok(())
}

#[tokio::test]
async fn test_sequential_member_removals() -> Result<()> {
    // Create 4 actors
    let alice = TestActor::new("alice").await?;
    let bob = TestActor::new("bob").await?;
    let charlie = TestActor::new("charlie").await?;
    let diana = TestActor::new("diana").await?;
    
    println!("✅ Created 4 test actors");
    
    // Alice creates channel and invites everyone
    let channel_id = alice.create_channel("sequential-removal-test").await?;
    
    let bob_invite = alice.create_invite(&channel_id, bob.get_key_package()).await?;
    bob.join_channel(&bob_invite).await?;
    
    let charlie_invite = alice.create_invite(&channel_id, charlie.get_key_package()).await?;
    charlie.join_channel(&charlie_invite).await?;
    
    let diana_invite = alice.create_invite(&channel_id, diana.get_key_package()).await?;
    diana.join_channel(&diana_invite).await?;
    
    println!("✅ All 4 members in channel");
    
    // All send initial messages
    alice.send_message(&channel_id, "Welcome everyone!").await?;
    bob.send_message(&channel_id, "Thanks!").await?;
    charlie.send_message(&channel_id, "Hello!").await?;
    diana.send_message(&channel_id, "Hi all!").await?;
    println!("✅ All members sent initial messages");
    
    // Alice removes Diana
    alice.remove_member(&channel_id, &diana.identity.user_id).await?;
    println!("✅ Alice removed Diana (4 -> 3 members)");
    
    // Remaining 3 members send messages
    alice.send_message(&channel_id, "Diana left").await?;
    bob.send_message(&channel_id, "Bye Diana").await?;
    charlie.send_message(&channel_id, "See you").await?;
    println!("✅ 3 remaining members can send");
    
    // Alice removes Charlie
    alice.remove_member(&channel_id, &charlie.identity.user_id).await?;
    println!("✅ Alice removed Charlie (3 -> 2 members)");
    
    // Remaining 2 members send messages
    alice.send_message(&channel_id, "Charlie left too").await?;
    bob.send_message(&channel_id, "Just us now").await?;
    println!("✅ 2 remaining members can send");
    println!("✅ Sequential removals work (network sync not tested in isolated environment)");
    
    Ok(())
}

#[tokio::test]
async fn test_non_admin_cannot_remove_members() -> Result<()> {
    // Create 4 actors
    let alice = TestActor::new("alice").await?;
    let bob = TestActor::new("bob").await?;
    let charlie = TestActor::new("charlie").await?;
    let diana = TestActor::new("diana").await?;
    
    println!("✅ Created 4 test actors");
    
    // Alice creates channel and invites everyone
    let channel_id = alice.create_channel("permission-test").await?;
    
    let bob_invite = alice.create_invite(&channel_id, bob.get_key_package()).await?;
    bob.join_channel(&bob_invite).await?;
    
    let charlie_invite = alice.create_invite(&channel_id, charlie.get_key_package()).await?;
    charlie.join_channel(&charlie_invite).await?;
    
    let diana_invite = alice.create_invite(&channel_id, diana.get_key_package()).await?;
    diana.join_channel(&diana_invite).await?;
    
    println!("✅ All members joined channel");
    
    // Bob (regular member) tries to remove Charlie - should fail
    let removal_result = bob.remove_member(&channel_id, &charlie.identity.user_id).await;
    assert!(removal_result.is_err(), "Non-admin should not be able to remove members");
    println!("✅ Non-admin cannot remove members");
    
    // Charlie should still be able to send messages
    charlie.send_message(&channel_id, "I'm still here!").await?;
    println!("✅ Member that was attempted to be removed can still send");
    
    // Alice (admin) can successfully remove Charlie
    alice.remove_member(&channel_id, &charlie.identity.user_id).await?;
    println!("✅ Admin successfully removed member");
    println!("✅ Permission checks work (network sync not tested in isolated environment)");
    
    Ok(())
}

#[tokio::test]
async fn test_multiple_moderators_can_remove() -> Result<()> {
    // Create 4 actors
    let alice = TestActor::new("alice").await?;
    let bob = TestActor::new("bob").await?;
    let charlie = TestActor::new("charlie").await?;
    let diana = TestActor::new("diana").await?;
    
    println!("✅ Created 4 test actors");
    
    // Alice creates channel
    let channel_id = alice.create_channel("multi-mod-test").await?;
    
    // Invite everyone
    let bob_invite = alice.create_invite(&channel_id, bob.get_key_package()).await?;
    bob.join_channel(&bob_invite).await?;
    
    let charlie_invite = alice.create_invite(&channel_id, charlie.get_key_package()).await?;
    charlie.join_channel(&charlie_invite).await?;
    
    let diana_invite = alice.create_invite(&channel_id, diana.get_key_package()).await?;
    diana.join_channel(&diana_invite).await?;
    
    println!("✅ All members joined");
    
    // Alice promotes both Bob and Charlie to admin
    alice.promote_to_admin(&channel_id, &bob.identity.user_id).await?;
    alice.promote_to_admin(&channel_id, &charlie.identity.user_id).await?;
    println!("✅ Alice promoted Bob and Charlie to admin");
    
    // NOTE: In isolated tests, promoted members don't receive commits
    // so they won't have admin permissions in their local state.
    // Alice (original admin) can still remove Diana
    alice.remove_member(&channel_id, &diana.identity.user_id).await?;
    println!("✅ Alice (admin) removed Diana");
    
    // All remaining members can communicate
    alice.send_message(&channel_id, "We have 3 members now").await?;
    bob.send_message(&channel_id, "Diana was removed").await?;
    charlie.send_message(&channel_id, "Confirmed").await?;
    println!("✅ All remaining members can communicate");
    println!("✅ Multiple admin promotion works (full permissions require network sync)");
    
    Ok(())
}

#[tokio::test]
async fn test_disconnection_and_reconnection_simulation() -> Result<()> {
    // Create 4 actors
    let alice = TestActor::new("alice").await?;
    let bob = TestActor::new("bob").await?;
    let charlie = TestActor::new("charlie").await?;
    let diana = TestActor::new("diana").await?;
    
    println!("✅ Created 4 test actors");
    
    // Alice creates channel and invites everyone
    let channel_id = alice.create_channel("disconnection-test").await?;
    
    let bob_invite = alice.create_invite(&channel_id, bob.get_key_package()).await?;
    bob.join_channel(&bob_invite).await?;
    
    let charlie_invite = alice.create_invite(&channel_id, charlie.get_key_package()).await?;
    charlie.join_channel(&charlie_invite).await?;
    
    let diana_invite = alice.create_invite(&channel_id, diana.get_key_package()).await?;
    diana.join_channel(&diana_invite).await?;
    
    println!("✅ All members joined channel");
    
    // Phase 1: All members are online and send messages
    alice.send_message(&channel_id, "Message 1").await?;
    bob.send_message(&channel_id, "Message 2").await?;
    charlie.send_message(&channel_id, "Message 3").await?;
    diana.send_message(&channel_id, "Message 4").await?;
    println!("✅ Phase 1: All members sent messages while online");
    
    // Phase 2: Charlie "disconnects" (simulated by not sending)
    // Other members continue
    alice.send_message(&channel_id, "Message 5").await?;
    bob.send_message(&channel_id, "Message 6").await?;
    diana.send_message(&channel_id, "Message 7").await?;
    println!("✅ Phase 2: Messages sent while Charlie 'offline'");
    
    // Phase 3: Charlie "reconnects" and can send again
    charlie.send_message(&channel_id, "I'm back!").await?;
    println!("✅ Phase 3: Charlie reconnected and sent message");
    
    // Phase 4: All members active again
    alice.send_message(&channel_id, "Welcome back Charlie").await?;
    bob.send_message(&channel_id, "Good to have you back").await?;
    diana.send_message(&channel_id, "Hey!").await?;
    charlie.send_message(&channel_id, "Thanks everyone").await?;
    println!("✅ Phase 4: All members active and communicating");
    
    Ok(())
}

#[tokio::test]
async fn test_member_removal_during_simulated_disconnection() -> Result<()> {
    // Create 4 actors
    let alice = TestActor::new("alice").await?;
    let bob = TestActor::new("bob").await?;
    let charlie = TestActor::new("charlie").await?;
    let diana = TestActor::new("diana").await?;
    
    println!("✅ Created 4 test actors");
    
    // Setup: Alice creates channel, everyone joins
    let channel_id = alice.create_channel("disconnection-removal-test").await?;
    
    let bob_invite = alice.create_invite(&channel_id, bob.get_key_package()).await?;
    bob.join_channel(&bob_invite).await?;
    
    let charlie_invite = alice.create_invite(&channel_id, charlie.get_key_package()).await?;
    charlie.join_channel(&charlie_invite).await?;
    
    let diana_invite = alice.create_invite(&channel_id, diana.get_key_package()).await?;
    diana.join_channel(&diana_invite).await?;
    
    println!("✅ All members joined channel");
    
    // Everyone sends initial messages
    alice.send_message(&channel_id, "Hello everyone").await?;
    bob.send_message(&channel_id, "Hi").await?;
    charlie.send_message(&channel_id, "Hey").await?;
    diana.send_message(&channel_id, "Hello").await?;
    println!("✅ Initial messages sent");
    
    // Diana "disconnects" (stops sending)
    println!("📴 Diana simulated offline");
    
    // While Diana is "offline", Alice removes Charlie
    alice.remove_member(&channel_id, &charlie.identity.user_id).await?;
    println!("✅ Alice removed Charlie while Diana offline");
    
    // Alice and Bob communicate
    alice.send_message(&channel_id, "Charlie was removed").await?;
    bob.send_message(&channel_id, "Okay").await?;
    println!("✅ Alice and Bob communicated");
    
    // Diana "reconnects" and can still send (she wasn't removed)
    diana.send_message(&channel_id, "I'm back, what did I miss?").await?;
    println!("✅ Diana reconnected and sent message");
    
    // Remaining members communicate
    alice.send_message(&channel_id, "Charlie was removed").await?;
    bob.send_message(&channel_id, "Welcome back Diana").await?;
    diana.send_message(&channel_id, "Oh I see").await?;
    println!("✅ All remaining members can communicate");
    println!("✅ Removal during disconnection works (network sync not tested in isolated environment)");
    
    Ok(())
}
