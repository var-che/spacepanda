//! Full Join Flow Test - Complete E2E Encrypted Messaging
//!
//! This test implements the complete golden path from DOC 2:
//! 1. Alice creates channel
//! 2. Alice creates invite (Welcome) for Bob
//! 3. Bob joins using Welcome
//! 4. Alice sends encrypted message
//! 5. Bob receives and decrypts

use crate::core_mvp::channel_manager::{ChannelManager, Identity};
use crate::core_mvp::errors::MvpResult;
use crate::core_store::model::types::UserId;
use crate::core_mls::service::MlsService;
use crate::core_store::store::local_store::{LocalStore, LocalStoreConfig};
use crate::config::Config;
use crate::shutdown::ShutdownCoordinator;
use std::sync::Arc;
use std::time::Duration;
use tempfile::tempdir;

/// Helper to create a test manager instance
async fn create_test_manager(name: &str) -> (Arc<ChannelManager>, tempfile::TempDir) {
    let temp_dir = tempdir().unwrap();
    let config = Arc::new(Config::default());
    let shutdown = Arc::new(ShutdownCoordinator::new(Duration::from_secs(5)));

    let mls_service = Arc::new(MlsService::new(&config, shutdown));

    let store_config = LocalStoreConfig {
        data_dir: temp_dir.path().to_path_buf(),
        enable_encryption: false,
        require_signatures: false,
        authorized_keys: Vec::new(),
        ..Default::default()
    };
    let store = Arc::new(LocalStore::new(store_config).unwrap());

    let user_id = UserId(format!("{}@spacepanda.local", name));
    let identity = Arc::new(Identity::new(
        user_id,
        name.to_string(),
        format!("{}-node", name),
    ));

    let manager = Arc::new(ChannelManager::new(
        mls_service,
        store,
        identity,
        config,
    ));

    (manager, temp_dir)
}

/// Test the complete join flow: invite → join → encrypt → decrypt
///
/// This test validates the full end-to-end encrypted messaging flow:
/// 1. Alice creates a channel
/// 2. Bob generates a key package using ChannelManager
/// 3. Alice creates an invite with Bob's key package
/// 4. Bob joins the channel from the Welcome message
/// 5. Alice sends an encrypted message
/// 6. Bob decrypts the message
/// 7. Bob sends a reply
/// 8. Alice decrypts Bob's reply
///
/// This demonstrates complete MLS integration with proper key package management.
#[tokio::test]
async fn test_full_join_flow() -> MvpResult<()> {
    println!("\n=== FULL JOIN FLOW TEST ===\n");

    // Setup: Create Alice and Bob
    let (alice_manager, _alice_dir) = create_test_manager("alice").await;
    let (bob_manager, _bob_dir) = create_test_manager("bob").await;

    // Step 1: Alice creates a channel
    println!("Step 1: Alice creates channel");
    let channel_name = "spacepanda-test".to_string();
    let channel_id = alice_manager
        .create_channel(channel_name.clone(), false)
        .await?;
    println!("  ✓ Channel created: {:?}", channel_id);

    // Verify channel exists for Alice
    let alice_channel = alice_manager.get_channel(&channel_id).await?;
    assert_eq!(alice_channel.name, channel_name);
    println!("  ✓ Alice can access channel metadata");

    // Step 2: Bob generates a key package
    // Bob's ChannelManager generates and stores his KeyPackageBundle
    println!("\nStep 2: Bob generates key package");
    let bob_key_package = bob_manager.generate_key_package().await?;
    println!("  ✓ Bob's key package generated ({} bytes)", bob_key_package.len());

    // Step 3: Alice creates an invite for Bob
    println!("\nStep 3: Alice creates invite for Bob");
    let (invite, _commit) = alice_manager
        .create_invite(&channel_id, bob_key_package)
        .await?;
    println!("  ✓ Invite created");
    println!("    - Welcome blob: {} bytes", invite.welcome_blob.len());
    println!("    - Channel ID: {:?}", invite.channel_id);
    
    assert_eq!(invite.channel_id, channel_id);
    assert!(!invite.welcome_blob.is_empty());

    // Step 4: Bob joins the channel using the Welcome
    println!("\nStep 4: Bob joins channel using Welcome");
    let bob_channel_id = bob_manager.join_channel(&invite).await?;
    println!("  ✓ Bob joined channel: {:?}", bob_channel_id);
    
    assert_eq!(bob_channel_id, channel_id);

    // Verify Bob can access channel metadata
    let bob_channel = bob_manager.get_channel(&channel_id).await?;
    assert_eq!(bob_channel.name, channel_name);
    println!("  ✓ Bob can access channel metadata");

    // Step 5: Alice sends an encrypted message
    println!("\nStep 5: Alice sends encrypted message");
    let message = b"Hello Bob! Welcome to SpacePanda!";
    let ciphertext = alice_manager
        .send_message(&channel_id, message)
        .await?;
    println!("  ✓ Message encrypted ({} bytes ciphertext)", ciphertext.len());
    
    assert!(!ciphertext.is_empty());

    // Step 6: Bob receives and decrypts the message
    println!("\nStep 6: Bob decrypts message");
    let plaintext = bob_manager.receive_message(&ciphertext).await?;
    println!("  ✓ Message decrypted ({} bytes plaintext)", plaintext.len());
    println!("    Content: {:?}", String::from_utf8_lossy(&plaintext));
    
    assert_eq!(plaintext, message);

    // Step 7: Bob sends a reply
    println!("\nStep 7: Bob sends reply");
    let reply = b"Thanks Alice! Glad to be here!";
    let reply_ciphertext = bob_manager
        .send_message(&channel_id, reply)
        .await?;
    println!("  ✓ Reply encrypted ({} bytes)", reply_ciphertext.len());

    // Step 8: Alice receives Bob's reply
    println!("\nStep 8: Alice decrypts reply");
    let reply_plaintext = alice_manager.receive_message(&reply_ciphertext).await?;
    println!("  ✓ Reply decrypted");
    println!("    Content: {:?}", String::from_utf8_lossy(&reply_plaintext));
    
    assert_eq!(reply_plaintext, reply);

    println!("\n=== ✅ FULL JOIN FLOW TEST PASSED! ===\n");
    Ok(())
}

/// Test multiple message exchange between two users
#[tokio::test]
async fn test_multiple_message_exchange() -> MvpResult<()> {
    println!("\n=== MULTIPLE MESSAGE EXCHANGE TEST ===\n");

    // Setup
    let (alice_manager, _alice_dir) = create_test_manager("alice").await;
    let (bob_manager, _bob_dir) = create_test_manager("bob").await;

    // Create channel and join
    let channel_id = alice_manager
        .create_channel("test-chat".to_string(), false)
        .await?;

    let bob_kp = bob_manager.generate_key_package().await?;
    let (invite, _commit) = alice_manager
        .create_invite(&channel_id, bob_kp)
        .await?;
    bob_manager.join_channel(&invite).await?;

    println!("Channel setup complete. Testing message exchange...\n");

    // Exchange 5 message pairs
    for i in 1..=5 {
        // Alice sends
        let alice_msg = format!("Message {} from Alice", i);
        let ct = alice_manager
            .send_message(&channel_id, alice_msg.as_bytes())
            .await?;
        let pt = bob_manager.receive_message(&ct).await?;
        assert_eq!(pt, alice_msg.as_bytes());
        println!("  ✓ Alice -> Bob: {}", alice_msg);

        // Bob replies
        let bob_msg = format!("Reply {} from Bob", i);
        let ct = bob_manager
            .send_message(&channel_id, bob_msg.as_bytes())
            .await?;
        let pt = alice_manager.receive_message(&ct).await?;
        assert_eq!(pt, bob_msg.as_bytes());
        println!("  ✓ Bob -> Alice: {}", bob_msg);
    }

    println!("\n=== ✅ MULTIPLE MESSAGE EXCHANGE PASSED! ===\n");
    Ok(())
}

/// Test three-party group communication
#[tokio::test]
async fn test_three_party_group() -> MvpResult<()> {
    println!("\n=== THREE PARTY GROUP TEST ===\n");

    // Setup
    let (alice_manager, _alice_dir) = create_test_manager("alice").await;
    let (bob_manager, _bob_dir) = create_test_manager("bob").await;
    let (charlie_manager, _charlie_dir) = create_test_manager("charlie").await;

    // Alice creates channel
    let channel_id = alice_manager
        .create_channel("three-way".to_string(), false)
        .await?;
    println!("✓ Alice created channel");

    // Alice invites Bob
    let bob_kp = bob_manager.generate_key_package().await?;
    let (bob_invite, _commit) = alice_manager
        .create_invite(&channel_id, bob_kp)
        .await?;
    bob_manager.join_channel(&bob_invite).await?;
    println!("✓ Bob joined");

    // Alice invites Charlie - Bob needs to process the commit to stay in sync
    let charlie_kp = charlie_manager.generate_key_package().await?;
    let (charlie_invite, commit) = alice_manager
        .create_invite(&channel_id, charlie_kp)
        .await?;
    charlie_manager.join_channel(&charlie_invite).await?;
    println!("✓ Charlie joined");

    // Bob processes the commit from adding Charlie to advance his epoch
    if let Some(commit_bytes) = commit {
        bob_manager.process_commit(&commit_bytes).await?;
        println!("✓ Bob processed commit (epoch synced)");
    }

    // Alice broadcasts a message
    println!("\nAlice broadcasts to group...");
    let msg = b"Hello everyone!";
    let ct = alice_manager.send_message(&channel_id, msg).await?;

    // Both Bob and Charlie can decrypt
    let bob_pt = bob_manager.receive_message(&ct).await?;
    assert_eq!(bob_pt, msg);
    println!("  ✓ Bob received: {:?}", String::from_utf8_lossy(&bob_pt));

    let charlie_pt = charlie_manager.receive_message(&ct).await?;
    assert_eq!(charlie_pt, msg);
    println!("  ✓ Charlie received: {:?}", String::from_utf8_lossy(&charlie_pt));

    println!("\n=== ✅ THREE-PARTY GROUP TEST PASSED! ===\n");
    Ok(())
}

/// Test invite creation with real OpenMLS key packages
///
/// This test validates that we can:
/// 1. Create a channel
/// 2. Generate a real OpenMLS key package  
/// 3. Create an invite with the key package
/// 4. Verify the invite contains a proper Welcome message
///
/// This confirms the invite creation flow works correctly, even though
/// the join flow requires additional key package management (see ignored tests above).
#[tokio::test]
async fn test_invite_creation_with_real_key_package() -> MvpResult<()> {
    println!("\n=== INVITE CREATION TEST ===\n");

    // Setup: Create Alice and Bob
    let (alice_manager, _alice_dir) = create_test_manager("alice").await;
    let (bob_manager, _bob_dir) = create_test_manager("bob").await;
    println!("✓ Alice and Bob created");

    // Step 1: Alice creates a channel
    let channel_name = "test-invites".to_string();
    let channel_id = alice_manager
        .create_channel(channel_name.clone(), false)
        .await?;
    println!("✓ Channel created: {:?}", channel_id);

    // Step 2: Bob generates a key package
    let bob_key_package = bob_manager.generate_key_package().await?;
    println!("✓ Bob's key package generated ({} bytes)", bob_key_package.len());
    
    // Verify the key package was created
    assert!(!bob_key_package.is_empty());
    println!("  - Public key package: {} bytes", bob_key_package.len());

    // Step 3: Alice creates an invite using Bob's key package
    let (invite, _commit) = alice_manager
        .create_invite(&channel_id, bob_key_package)
        .await?;
    println!("✓ Invite created successfully");
    println!("  - Welcome blob: {} bytes", invite.welcome_blob.len());
    println!("  - Channel ID: {:?}", invite.channel_id);
    
    // Verify invite contents
    assert_eq!(invite.channel_id, channel_id);
    assert!(!invite.welcome_blob.is_empty());
    assert!(!invite.is_expired());

    // Step 4: Verify we can retrieve channel metadata
    let channel = alice_manager.get_channel(&channel_id).await?;
    assert_eq!(channel.name, channel_name);
    println!("✓ Channel metadata verified");

    println!("\n=== ✅ INVITE CREATION TEST PASSED! ===\n");
    Ok(())
}

/// Test a four-person MLS group with all members able to communicate
///
/// Tests:
/// 1. Alice creates a channel
/// 2. Alice adds Bob, Charlie, and Dave sequentially
/// 3. All members sync epochs via commits
/// 4. Each member sends a message that all others can decrypt
#[tokio::test]
async fn test_four_party_group() -> MvpResult<()> {
    println!("\n=== FOUR-PARTY GROUP TEST ===\n");

    // Setup: Create all four members
    let (alice_manager, _alice_dir) = create_test_manager("alice").await;
    let (bob_manager, _bob_dir) = create_test_manager("bob").await;
    let (charlie_manager, _charlie_dir) = create_test_manager("charlie").await;
    let (dave_manager, _dave_dir) = create_test_manager("dave").await;
    println!("✓ All four members created");

    // Step 1: Alice creates a channel
    let channel_id = alice_manager
        .create_channel("four-party-chat".to_string(), false)
        .await?;
    println!("✓ Alice created channel");

    // Step 2: Build the group - add each member with epoch sync
    println!("\nBuilding four-party group...");
    
    // Add Bob
    let bob_kp = bob_manager.generate_key_package().await?;
    let (bob_invite, _) = alice_manager.create_invite(&channel_id, bob_kp).await?;
    bob_manager.join_channel(&bob_invite).await?;
    println!("  ✓ Bob joined (2 members)");
    
    // Add Charlie - Bob must process commit
    let charlie_kp = charlie_manager.generate_key_package().await?;
    let (charlie_invite, commit) = alice_manager.create_invite(&channel_id, charlie_kp).await?;
    if let Some(c) = commit {
        bob_manager.process_commit(&c).await?;
    }
    charlie_manager.join_channel(&charlie_invite).await?;
    println!("  ✓ Charlie joined (3 members)");
    
    // Add Dave - Bob and Charlie must process commit
    let dave_kp = dave_manager.generate_key_package().await?;
    let (dave_invite, commit) = alice_manager.create_invite(&channel_id, dave_kp).await?;
    if let Some(c) = commit {
        bob_manager.process_commit(&c).await?;
        charlie_manager.process_commit(&c).await?;
    }
    dave_manager.join_channel(&dave_invite).await?;
    println!("  ✓ Dave joined (4 members)");

    // Step 3: Test all-to-all messaging
    println!("\nTesting all-to-all messaging...");
    
    // Alice sends, all others receive
    let alice_msg = b"Hello from Alice!";
    let ct = alice_manager.send_message(&channel_id, alice_msg).await?;
    assert_eq!(bob_manager.receive_message(&ct).await?, alice_msg);
    assert_eq!(charlie_manager.receive_message(&ct).await?, alice_msg);
    assert_eq!(dave_manager.receive_message(&ct).await?, alice_msg);
    println!("  ✓ Alice's message received by all");

    // Bob sends, all others receive
    let bob_msg = b"Bob here!";
    let ct = bob_manager.send_message(&channel_id, bob_msg).await?;
    assert_eq!(alice_manager.receive_message(&ct).await?, bob_msg);
    assert_eq!(charlie_manager.receive_message(&ct).await?, bob_msg);
    assert_eq!(dave_manager.receive_message(&ct).await?, bob_msg);
    println!("  ✓ Bob's message received by all");

    // Charlie sends, all others receive
    let charlie_msg = b"Charlie checking in";
    let ct = charlie_manager.send_message(&channel_id, charlie_msg).await?;
    assert_eq!(alice_manager.receive_message(&ct).await?, charlie_msg);
    assert_eq!(bob_manager.receive_message(&ct).await?, charlie_msg);
    assert_eq!(dave_manager.receive_message(&ct).await?, charlie_msg);
    println!("  ✓ Charlie's message received by all");

    // Dave sends, all others receive
    let dave_msg = b"Dave says hi!";
    let ct = dave_manager.send_message(&channel_id, dave_msg).await?;
    assert_eq!(alice_manager.receive_message(&ct).await?, dave_msg);
    assert_eq!(bob_manager.receive_message(&ct).await?, dave_msg);
    assert_eq!(charlie_manager.receive_message(&ct).await?, dave_msg);
    println!("  ✓ Dave's message received by all");

    println!("\n=== ✅ FOUR-PARTY GROUP TEST PASSED! ===\n");
    Ok(())
}

/// Test removing a member from a four-party group
///
/// Tests:
/// 1. Create four-party group (Alice, Bob, Charlie, Dave)
/// 2. Verify all can communicate
/// 3. Alice removes Bob
/// 4. Remaining members (Charlie, Dave) process removal commit
/// 5. Verify only remaining members can decrypt new messages
/// 6. Verify removed member (Bob) cannot decrypt or send
#[tokio::test]
async fn test_four_party_member_removal() -> MvpResult<()> {
    println!("\n=== FOUR-PARTY MEMBER REMOVAL TEST ===\n");

    // Setup: Create all four members
    let (alice_manager, _alice_dir) = create_test_manager("alice").await;
    let (bob_manager, _bob_dir) = create_test_manager("bob").await;
    let (charlie_manager, _charlie_dir) = create_test_manager("charlie").await;
    let (dave_manager, _dave_dir) = create_test_manager("dave").await;
    println!("✓ All four members created");

    // Build the group
    let channel_id = alice_manager
        .create_channel("removal-test".to_string(), false)
        .await?;
    println!("✓ Alice created channel");

    // Add members with epoch sync
    let bob_kp = bob_manager.generate_key_package().await?;
    let (bob_invite, _) = alice_manager.create_invite(&channel_id, bob_kp).await?;
    bob_manager.join_channel(&bob_invite).await?;
    println!("  ✓ Bob joined (2 members)");
    
    let charlie_kp = charlie_manager.generate_key_package().await?;
    let (charlie_invite, c1) = alice_manager.create_invite(&channel_id, charlie_kp).await?;
    if let Some(c) = c1 { bob_manager.process_commit(&c).await?; }
    charlie_manager.join_channel(&charlie_invite).await?;
    println!("  ✓ Charlie joined (3 members)");
    
    let dave_kp = dave_manager.generate_key_package().await?;
    let (dave_invite, c2) = alice_manager.create_invite(&channel_id, dave_kp).await?;
    if let Some(c) = c2 {
        bob_manager.process_commit(&c).await?;
        charlie_manager.process_commit(&c).await?;
    }
    dave_manager.join_channel(&dave_invite).await?;
    println!("  ✓ Dave joined (4 members)");

    // Verify all can communicate before removal
    println!("\nVerifying pre-removal communication...");
    let pre_msg = b"Before removal";
    let ct = alice_manager.send_message(&channel_id, pre_msg).await?;
    assert_eq!(bob_manager.receive_message(&ct).await?, pre_msg);
    assert_eq!(charlie_manager.receive_message(&ct).await?, pre_msg);
    assert_eq!(dave_manager.receive_message(&ct).await?, pre_msg);
    println!("  ✓ All members can communicate");

    // Remove Bob
    println!("\nRemoving Bob from the group...");
    let bob_identity = bob_manager.identity().user_id.0.as_bytes();
    let removal_commit = alice_manager
        .remove_member(&channel_id, bob_identity)
        .await?;
    println!("  ✓ Removal commit generated ({} bytes)", removal_commit.len());

    // Remaining members process removal
    println!("Processing removal commit for remaining members...");
    charlie_manager.process_commit(&removal_commit).await?;
    dave_manager.process_commit(&removal_commit).await?;
    println!("  ✓ Charlie and Dave processed removal");

    // Test post-removal messaging
    println!("\nTesting post-removal messaging...");
    let post_msg = b"After Bob was removed";
    let ct_after = alice_manager.send_message(&channel_id, post_msg).await?;
    
    // Charlie and Dave can decrypt
    assert_eq!(charlie_manager.receive_message(&ct_after).await?, post_msg);
    println!("  ✓ Charlie can decrypt post-removal message");
    
    assert_eq!(dave_manager.receive_message(&ct_after).await?, post_msg);
    println!("  ✓ Dave can decrypt post-removal message");

    // Bob CANNOT decrypt (should fail)
    println!("\nVerifying Bob cannot decrypt post-removal...");
    let bob_result = bob_manager.receive_message(&ct_after).await;
    assert!(
        bob_result.is_err(),
        "Bob should not be able to decrypt after removal"
    );
    println!("  ✓ Bob correctly cannot decrypt (removed from group)");

    // Bob CANNOT send (should fail)
    println!("Verifying Bob cannot send post-removal...");
    let bob_send_result = bob_manager
        .send_message(&channel_id, b"Bob tries to send")
        .await;
    assert!(
        bob_send_result.is_err(),
        "Bob should not be able to send after removal"
    );
    println!("  ✓ Bob correctly cannot send (removed from group)");

    // Remaining members can still communicate with each other
    println!("\nVerifying remaining members can still communicate...");
    let charlie_msg = b"Charlie confirms functionality";
    let ct_charlie = charlie_manager.send_message(&channel_id, charlie_msg).await?;
    assert_eq!(alice_manager.receive_message(&ct_charlie).await?, charlie_msg);
    assert_eq!(dave_manager.receive_message(&ct_charlie).await?, charlie_msg);
    println!("  ✓ Remaining members can communicate");

    println!("\n=== ✅ MEMBER REMOVAL TEST PASSED! ===\n");
    Ok(())
}

/// Test permission system - only admins can remove members
#[tokio::test]
async fn test_admin_permissions_for_removal() -> MvpResult<()> {
    println!("\n=== TESTING ADMIN PERMISSIONS ===\n");

    // Create Alice (admin) and Bob (member)
    let (alice_manager, _alice_dir) = create_test_manager("alice").await;
    let (bob_manager, _bob_dir) = create_test_manager("bob").await;

    // Alice creates channel (becomes admin)
    let channel_id = alice_manager
        .create_channel("test-channel".to_string(), false)
        .await?;
    println!("✓ Alice created channel as admin");

    // Generate invite for Bob
    let bob_key_package = bob_manager.generate_key_package().await?;
    let invite = alice_manager
        .create_invite(&channel_id, bob_key_package)
        .await?;

    // Bob joins
    let _bob_channel_id = bob_manager.join_channel(&invite).await?;
    println!("✓ Bob joined as regular member");

    // Create Charlie to attempt removal
    let (charlie_manager, _charlie_dir) = create_test_manager("charlie").await;
    let charlie_key_package = charlie_manager.generate_key_package().await?;
    let charlie_invite = alice_manager
        .create_invite(&channel_id, charlie_key_package)
        .await?;
    let _charlie_channel_id = charlie_manager.join_channel(&charlie_invite).await?;
    println!("✓ Charlie joined as regular member");

    // Test 1: Alice (admin) CAN remove Bob
    println!("\nTest 1: Admin removing member...");
    let bob_identity = bob_manager.identity().user_id.0.as_bytes();
    let result = alice_manager.remove_member(&channel_id, bob_identity).await;
    assert!(result.is_ok(), "Admin should be able to remove members");
    println!("  ✓ Alice (admin) successfully removed Bob");

    // Test 2: Charlie (member) CANNOT remove anyone
    println!("\nTest 2: Non-admin attempting removal...");
    // Create new test since we need all 3 members
    let (alice_manager2, _alice_dir2) = create_test_manager("alice2").await;
    let (bob_manager2, _bob_dir2) = create_test_manager("bob2").await;
    let (charlie_manager2, _charlie_dir2) = create_test_manager("charlie2").await;

    let channel_id2 = alice_manager2
        .create_channel("test-channel-2".to_string(), false)
        .await?;

    let bob_kp2 = bob_manager2.generate_key_package().await?;
    let invite2 = alice_manager2.create_invite(&channel_id2, bob_kp2).await?;
    bob_manager2.join_channel(&invite2).await?;

    let charlie_kp2 = charlie_manager2.generate_key_package().await?;
    let charlie_inv2 = alice_manager2.create_invite(&channel_id2, charlie_kp2).await?;
    charlie_manager2.join_channel(&charlie_inv2).await?;

    let bob2_identity = bob_manager2.identity().user_id.0.as_bytes();
    let result = charlie_manager2.remove_member(&channel_id2, bob2_identity).await;
    assert!(result.is_err(), "Non-admin should NOT be able to remove members");
    println!("  ✓ Charlie (member) was correctly denied permission");

    println!("\n=== ✅ PERMISSION TESTS PASSED! ===\n");
    Ok(())
}

/// Test role queries
#[tokio::test]
async fn test_role_queries() -> MvpResult<()> {
    println!("\n=== TESTING ROLE QUERIES ===\n");

    let (alice_manager, _alice_dir) = create_test_manager("alice").await;
    let (bob_manager, _bob_dir) = create_test_manager("bob").await;

    // Alice creates channel
    let channel_id = alice_manager
        .create_channel("test-channel".to_string(), false)
        .await?;

    // Bob joins
    let bob_key_package = bob_manager.generate_key_package().await?;
    let invite = alice_manager.create_invite(&channel_id, bob_key_package).await?;
    bob_manager.join_channel(&invite).await?;

    // Test get_member_role
    let alice_identity = alice_manager.identity().user_id.0.as_bytes();
    let alice_role = alice_manager.get_member_role(&channel_id, alice_identity).await?;
    assert_eq!(alice_role, crate::core_mls::types::MemberRole::Admin);
    println!("✓ Alice is Admin (creator)");

    let bob_identity = bob_manager.identity().user_id.0.as_bytes();
    let bob_role = alice_manager.get_member_role(&channel_id, bob_identity).await?;
    assert_eq!(bob_role, crate::core_mls::types::MemberRole::Member);
    println!("✓ Bob is Member (joined)");

    // Test is_admin
    let alice_is_admin = alice_manager.is_admin(&channel_id, alice_identity).await?;
    assert!(alice_is_admin);
    println!("✓ is_admin returns true for Alice");

    let bob_is_admin = alice_manager.is_admin(&channel_id, bob_identity).await?;
    assert!(!bob_is_admin);
    println!("✓ is_admin returns false for Bob");

    println!("\n=== ✅ ROLE QUERY TESTS PASSED! ===\n");
    Ok(())
}

/// Test promote/demote functionality (stub test since persistence not implemented)
#[tokio::test]
async fn test_promote_demote_operations() -> MvpResult<()> {
    println!("\n=== TESTING PROMOTE/DEMOTE OPERATIONS ===\n");

    let (alice_manager, _alice_dir) = create_test_manager("alice").await;
    let (bob_manager, _bob_dir) = create_test_manager("bob").await;

    // Alice creates channel
    let channel_id = alice_manager
        .create_channel("test-channel".to_string(), false)
        .await?;

    // Bob joins
    let bob_key_package = bob_manager.generate_key_package().await?;
    let invite = alice_manager.create_invite(&channel_id, bob_key_package).await?;
    bob_manager.join_channel(&invite).await?;

    let bob_identity = bob_manager.identity().user_id.0.as_bytes();

    // Test: Admin can call promote_member (even if persistence not implemented)
    let result = alice_manager.promote_member(&channel_id, bob_identity).await;
    assert!(result.is_ok(), "Admin should be able to call promote_member");
    println!("✓ Alice (admin) can call promote_member");

    // Test: Admin can call demote_member
    let result = alice_manager.demote_member(&channel_id, bob_identity).await;
    assert!(result.is_ok(), "Admin should be able to call demote_member");
    println!("✓ Alice (admin) can call demote_member");

    // Test: Non-admin CANNOT promote
    let alice_identity = alice_manager.identity().user_id.0.as_bytes();
    let result = bob_manager.promote_member(&channel_id, alice_identity).await;
    assert!(result.is_err(), "Non-admin should NOT be able to promote");
    println!("✓ Bob (member) correctly denied permission to promote");

    // Test: Non-admin CANNOT demote
    let result = bob_manager.demote_member(&channel_id, alice_identity).await;
    assert!(result.is_err(), "Non-admin should NOT be able to demote");
    println!("✓ Bob (member) correctly denied permission to demote");

    println!("\n=== ✅ PROMOTE/DEMOTE TESTS PASSED! ===\n");
    Ok(())
}

/// Test adding and removing reactions
#[tokio::test]
async fn test_message_reactions() -> MvpResult<()> {
    use crate::core_store::model::types::MessageId;

    println!("\n=== TESTING MESSAGE REACTIONS ===\n");

    let (alice_manager, _alice_dir) = create_test_manager("alice").await;
    let (bob_manager, _bob_dir) = create_test_manager("bob").await;

    // Create channel
    let channel_id = alice_manager
        .create_channel("test-channel".to_string(), false)
        .await?;
    
    // Bob joins
    let bob_kp = bob_manager.generate_key_package().await?;
    let invite = alice_manager.create_invite(&channel_id, bob_kp).await?;
    bob_manager.join_channel(&invite).await?;

    // Create a fake message ID for testing
    let message_id = MessageId::generate();
    println!("✓ Test message ID: {}", message_id);

    // Test 1: Alice adds a reaction
    println!("\nTest 1: Adding reactions...");
    alice_manager.add_reaction(&message_id, "👍".to_string()).await?;
    println!("  ✓ Alice reacted with 👍");

    bob_manager.add_reaction(&message_id, "👍".to_string()).await?;
    println!("  ✓ Bob reacted with 👍");

    bob_manager.add_reaction(&message_id, "❤️".to_string()).await?;
    println!("  ✓ Bob reacted with ❤️");

    // Test 2: Get reactions and verify counts
    println!("\nTest 2: Verifying reaction counts...");
    let reactions = alice_manager.get_reactions(&message_id).await?;
    assert_eq!(reactions.len(), 2, "Should have 2 unique emoji reactions");
    
    // Find the thumbs up reaction
    let thumbs_up = reactions.iter().find(|r| r.emoji == "👍").unwrap();
    assert_eq!(thumbs_up.count, 2, "Thumbs up should have 2 reactions");
    println!("  ✓ 👍 has {} reactions", thumbs_up.count);

    let heart = reactions.iter().find(|r| r.emoji == "❤️").unwrap();
    assert_eq!(heart.count, 1, "Heart should have 1 reaction");
    println!("  ✓ ❤️ has {} reaction", heart.count);

    // Test 3: Verify user_reacted flag
    println!("\nTest 3: Verifying user_reacted flag...");
    assert!(thumbs_up.user_reacted, "Alice should have reacted with 👍");
    assert!(!heart.user_reacted, "Alice should NOT have reacted with ❤️");
    println!("  ✓ user_reacted flags are correct for Alice");

    // Test 4: Duplicate reaction should fail
    println!("\nTest 4: Testing duplicate reaction prevention...");
    let result = alice_manager.add_reaction(&message_id, "👍".to_string()).await;
    assert!(result.is_err(), "Duplicate reaction should fail");
    println!("  ✓ Duplicate reaction correctly prevented");

    // Test 5: Remove reaction
    println!("\nTest 5: Removing reactions...");
    alice_manager.remove_reaction(&message_id, "👍".to_string()).await?;
    println!("  ✓ Alice removed 👍");

    let reactions = alice_manager.get_reactions(&message_id).await?;
    let thumbs_up = reactions.iter().find(|r| r.emoji == "👍").unwrap();
    assert_eq!(thumbs_up.count, 1, "Thumbs up should now have 1 reaction");
    assert!(!thumbs_up.user_reacted, "Alice should no longer have 👍 reaction");
    println!("  ✓ Reaction count updated correctly");

    // Test 6: Remove non-existent reaction should fail
    println!("\nTest 6: Testing invalid removal...");
    let result = alice_manager.remove_reaction(&message_id, "🎉".to_string()).await;
    assert!(result.is_err(), "Removing non-existent reaction should fail");
    println!("  ✓ Invalid removal correctly prevented");

    println!("\n=== ✅ MESSAGE REACTIONS TESTS PASSED! ===\n");
    Ok(())
}

/// Test reaction aggregation and sorting
#[tokio::test]
async fn test_reaction_aggregation() -> MvpResult<()> {
    use crate::core_store::model::types::MessageId;

    println!("\n=== TESTING REACTION AGGREGATION ===\n");

    let (alice_manager, _alice_dir) = create_test_manager("alice").await;
    let (bob_manager, _bob_dir) = create_test_manager("bob").await;
    let (charlie_manager, _charlie_dir) = create_test_manager("charlie").await;

    // Create channel and add members
    let channel_id = alice_manager
        .create_channel("test-channel".to_string(), false)
        .await?;

    let bob_kp = bob_manager.generate_key_package().await?;
    let invite = alice_manager.create_invite(&channel_id, bob_kp).await?;
    bob_manager.join_channel(&invite).await?;

    let charlie_kp = charlie_manager.generate_key_package().await?;
    let invite = alice_manager.create_invite(&channel_id, charlie_kp).await?;
    charlie_manager.join_channel(&invite).await?;

    let message_id = MessageId::generate();

    // Add reactions: 3 people react with 👍, 2 with ❤️, 1 with 🎉
    println!("Adding reactions from multiple users...");
    alice_manager.add_reaction(&message_id, "👍".to_string()).await?;
    bob_manager.add_reaction(&message_id, "👍".to_string()).await?;
    charlie_manager.add_reaction(&message_id, "👍".to_string()).await?;
    
    alice_manager.add_reaction(&message_id, "❤️".to_string()).await?;
    bob_manager.add_reaction(&message_id, "❤️".to_string()).await?;
    
    charlie_manager.add_reaction(&message_id, "🎉".to_string()).await?;
    println!("✓ Reactions added");

    // Get reactions and verify sorting (most popular first)
    println!("\nVerifying aggregation and sorting...");
    let reactions = alice_manager.get_reactions(&message_id).await?;
    
    assert_eq!(reactions.len(), 3, "Should have 3 unique emojis");
    assert_eq!(reactions[0].emoji, "👍", "Most popular should be first");
    assert_eq!(reactions[0].count, 3, "Thumbs up should have 3");
    assert_eq!(reactions[1].emoji, "❤️", "Second most popular");
    assert_eq!(reactions[1].count, 2, "Heart should have 2");
    assert_eq!(reactions[2].emoji, "🎉", "Least popular");
    assert_eq!(reactions[2].count, 1, "Party should have 1");
    
    println!("  ✓ Sorted by count: {} ({}), {} ({}), {} ({})",
        reactions[0].emoji, reactions[0].count,
        reactions[1].emoji, reactions[1].count,
        reactions[2].emoji, reactions[2].count
    );

    // Verify user lists
    println!("\nVerifying user lists...");
    assert_eq!(reactions[0].users.len(), 3, "Thumbs up should have 3 users");
    assert_eq!(reactions[1].users.len(), 2, "Heart should have 2 users");
    assert_eq!(reactions[2].users.len(), 1, "Party should have 1 user");
    println!("  ✓ User lists are correct");

    println!("\n=== ✅ REACTION AGGREGATION TESTS PASSED! ===\n");
    Ok(())
}

/// Test message threading functionality
#[tokio::test]
async fn test_message_threading() -> MvpResult<()> {
    use crate::core_store::model::types::MessageId;
    use crate::core_mvp::types::ChatMessage;

    println!("\n=== TESTING MESSAGE THREADING ===\n");

    let (alice_manager, _alice_dir) = create_test_manager("alice").await;
    let (bob_manager, _bob_dir) = create_test_manager("bob").await;

    // Create channel
    let channel_id = alice_manager
        .create_channel("test-channel".to_string(), false)
        .await?;
    
    // Bob joins
    let bob_kp = bob_manager.generate_key_package().await?;
    let invite = alice_manager.create_invite(&channel_id, bob_kp).await?;
    bob_manager.join_channel(&invite).await?;

    println!("Test 1: Creating a thread with replies...");
    
    // Alice posts root message
    let root_msg = ChatMessage::new(
        channel_id.clone(),
        alice_manager.identity().user_id.clone(),
        b"What's everyone's favorite color?".to_vec(),
    );
    let root_id = root_msg.message_id.clone();
    alice_manager.store_message(root_msg).await?;
    println!("  ✓ Root message created: {}", root_id);

    // Bob replies
    let bob_reply = ChatMessage::new(
        channel_id.clone(),
        bob_manager.identity().user_id.clone(),
        b"I like blue!".to_vec(),
    ).reply_to(root_id.clone());
    bob_manager.store_message(bob_reply).await?;
    println!("  ✓ Bob replied");

    // Alice replies
    let alice_reply = ChatMessage::new(
        channel_id.clone(),
        alice_manager.identity().user_id.clone(),
        b"Mine is green".to_vec(),
    ).reply_to(root_id.clone());
    alice_manager.store_message(alice_reply).await?;
    println!("  ✓ Alice replied");

    // Test 2: Get thread info
    println!("\nTest 2: Getting thread info...");
    let thread_info = alice_manager.get_thread_info(&root_id).await?;
    assert!(thread_info.is_some(), "Thread info should exist");
    
    let info = thread_info.unwrap();
    assert_eq!(info.reply_count, 2, "Should have 2 replies");
    assert_eq!(info.participant_count, 2, "Should have 2 participants");
    assert!(info.last_reply_preview.is_some(), "Should have last reply preview");
    println!("  ✓ Thread has {} replies from {} participants", 
        info.reply_count, info.participant_count);

    // Test 3: Get thread replies
    println!("\nTest 3: Getting thread replies...");
    let replies = alice_manager.get_thread_replies(&root_id).await?;
    assert_eq!(replies.len(), 2, "Should get 2 replies");
    assert_eq!(
        replies[0].body_as_string().unwrap(),
        "I like blue!",
        "First reply should be from Bob"
    );
    assert_eq!(
        replies[1].body_as_string().unwrap(),
        "Mine is green",
        "Second reply should be from Alice"
    );
    println!("  ✓ Retrieved {} replies in correct order", replies.len());

    // Test 4: Get message with thread context
    println!("\nTest 4: Getting message with thread context...");
    let msg_with_thread = alice_manager.get_message_with_thread(&root_id).await?;
    assert!(msg_with_thread.is_some(), "Should find message");
    
    let mwt = msg_with_thread.unwrap();
    assert_eq!(mwt.message.message_id, root_id, "Should be the root message");
    assert!(mwt.thread_info.is_some(), "Should have thread info");
    assert!(mwt.parent_message.is_none(), "Root message has no parent");
    println!("  ✓ Message has thread context");

    // Test 5: Get reply with parent context
    println!("\nTest 5: Getting reply with parent context...");
    let reply_id = replies[0].message_id.clone();
    let reply_with_context = alice_manager.get_message_with_thread(&reply_id).await?;
    assert!(reply_with_context.is_some(), "Should find reply");
    
    let rwc = reply_with_context.unwrap();
    assert!(rwc.parent_message.is_some(), "Reply should have parent");
    assert_eq!(
        rwc.parent_message.unwrap().message_id,
        root_id,
        "Parent should be the root message"
    );
    println!("  ✓ Reply has parent message context");

    println!("\n=== ✅ MESSAGE THREADING TESTS PASSED! ===\n");
    Ok(())
}

/// Test channel threads listing
#[tokio::test]
async fn test_channel_threads_listing() -> MvpResult<()> {
    use crate::core_mvp::types::ChatMessage;

    println!("\n=== TESTING CHANNEL THREADS LISTING ===\n");

    let (alice_manager, _alice_dir) = create_test_manager("alice").await;

    // Create channel
    let channel_id = alice_manager
        .create_channel("general".to_string(), false)
        .await?;

    println!("Creating multiple threads in channel...");

    // Thread 1: "Project updates"
    let thread1_root = ChatMessage::new(
        channel_id.clone(),
        alice_manager.identity().user_id.clone(),
        b"Project updates thread".to_vec(),
    );
    let thread1_id = thread1_root.message_id.clone();
    alice_manager.store_message(thread1_root).await?;
    
    // Add 2 replies to thread 1
    for i in 1..=2 {
        let reply = ChatMessage::new(
            channel_id.clone(),
            alice_manager.identity().user_id.clone(),
            format!("Update {}", i).into_bytes(),
        ).reply_to(thread1_id.clone());
        alice_manager.store_message(reply).await?;
    }
    println!("  ✓ Thread 1: Project updates (2 replies)");

    // Thread 2: "Random chat"
    let thread2_root = ChatMessage::new(
        channel_id.clone(),
        alice_manager.identity().user_id.clone(),
        b"Random chat thread".to_vec(),
    );
    let thread2_id = thread2_root.message_id.clone();
    alice_manager.store_message(thread2_root).await?;
    
    // Add 3 replies to thread 2
    for i in 1..=3 {
        let reply = ChatMessage::new(
            channel_id.clone(),
            alice_manager.identity().user_id.clone(),
            format!("Chat {}", i).into_bytes(),
        ).reply_to(thread2_id.clone());
        alice_manager.store_message(reply).await?;
    }
    println!("  ✓ Thread 2: Random chat (3 replies)");

    // Thread 3: No replies
    let thread3_root = ChatMessage::new(
        channel_id.clone(),
        alice_manager.identity().user_id.clone(),
        b"Silent thread".to_vec(),
    );
    alice_manager.store_message(thread3_root).await?;
    println!("  ✓ Thread 3: Silent (0 replies)");

    // Test: Get all channel threads
    println!("\nGetting all threads from channel...");
    let threads = alice_manager.get_channel_threads(&channel_id).await?;
    
    assert_eq!(threads.len(), 3, "Should have 3 root threads");
    println!("  ✓ Found {} threads", threads.len());

    // Verify thread 1
    let t1 = threads.iter().find(|t| t.message.message_id == thread1_id).unwrap();
    assert!(t1.thread_info.is_some(), "Thread 1 should have info");
    assert_eq!(t1.thread_info.as_ref().unwrap().reply_count, 2, "Thread 1 should have 2 replies");
    println!("  ✓ Thread 1 has {} replies", t1.thread_info.as_ref().unwrap().reply_count);

    // Verify thread 2
    let t2 = threads.iter().find(|t| t.message.message_id == thread2_id).unwrap();
    assert!(t2.thread_info.is_some(), "Thread 2 should have info");
    assert_eq!(t2.thread_info.as_ref().unwrap().reply_count, 3, "Thread 2 should have 3 replies");
    println!("  ✓ Thread 2 has {} replies", t2.thread_info.as_ref().unwrap().reply_count);

    // Verify thread 3
    let t3 = threads.iter().find(|t| t.message.body_as_string().unwrap() == "Silent thread").unwrap();
    assert!(t3.thread_info.is_none(), "Thread 3 should have no thread info (no replies)");
    println!("  ✓ Thread 3 has no replies");

    println!("\n=== ✅ CHANNEL THREADS LISTING TESTS PASSED! ===\n");
    Ok(())
}

/// Test message persistence
#[tokio::test]
async fn test_message_persistence() -> MvpResult<()> {
    use crate::core_mvp::types::ChatMessage;

    println!("\n=== TESTING MESSAGE PERSISTENCE ===\n");

    let (alice_manager, _alice_dir) = create_test_manager("alice").await;

    // Create channel
    let channel_id = alice_manager
        .create_channel("persistent-channel".to_string(), false)
        .await?;

    println!("Test 1: Storing messages...");
    
    // Create and store 3 messages
    let msg1 = ChatMessage::new(
        channel_id.clone(),
        alice_manager.identity().user_id.clone(),
        b"First message".to_vec(),
    );
    let msg1_id = msg1.message_id.clone();
    alice_manager.store_message(msg1).await?;
    println!("  ✓ Stored message 1");

    let msg2 = ChatMessage::new(
        channel_id.clone(),
        alice_manager.identity().user_id.clone(),
        b"Second message".to_vec(),
    );
    let msg2_id = msg2.message_id.clone();
    alice_manager.store_message(msg2).await?;
    println!("  ✓ Stored message 2");

    let msg3 = ChatMessage::new(
        channel_id.clone(),
        alice_manager.identity().user_id.clone(),
        b"Third message".to_vec(),
    ).reply_to(msg1_id.clone());
    alice_manager.store_message(msg3).await?;
    println!("  ✓ Stored message 3 (reply to msg1)");

    // Test 2: Query messages from in-memory cache
    println!("\nTest 2: Querying from in-memory cache...");
    let thread_replies = alice_manager.get_thread_replies(&msg1_id).await?;
    assert_eq!(thread_replies.len(), 1, "Should have 1 reply");
    assert_eq!(
        thread_replies[0].body_as_string().unwrap(),
        "Third message",
        "Reply should be msg3"
    );
    println!("  ✓ Thread query works from cache");

    // Test 3: Create a new manager instance to test persistence
    println!("\nTest 3: Testing persistence across restarts...");
    let (alice_manager2, _alice_dir2) = create_test_manager("alice2").await;
    
    // Load messages from persistent store
    alice_manager2.load_channel_messages(&channel_id).await?;
    println!("  ✓ Loaded messages from store");

    // Verify messages were loaded
    let loaded_thread_replies = alice_manager2.get_thread_replies(&msg1_id).await?;
    assert_eq!(loaded_thread_replies.len(), 1, "Should have 1 reply after reload");
    assert_eq!(
        loaded_thread_replies[0].body_as_string().unwrap(),
        "Third message",
        "Reply should match after reload"
    );
    println!("  ✓ Messages persisted and loaded correctly");

    // Test 4: Query messages using store directly
    println!("\nTest 4: Querying store directly...");
    use crate::core_store::model::types::MessageId;
    
    let store_messages = alice_manager
        .get_stored_messages(&channel_id)
        .await?;
    
    assert_eq!(store_messages.len(), 3, "Store should have 3 messages");
    println!("  ✓ Store contains {} messages", store_messages.len());

    // Verify thread replies in store
    let store_replies = alice_manager
        .get_stored_thread_replies(&msg1_id)
        .await?;
    
    assert_eq!(store_replies.len(), 1, "Store should have 1 reply");
    assert_eq!(
        store_replies[0].reply_to,
        Some(msg1_id.clone()),
        "Reply should reference parent"
    );
    println!("  ✓ Store thread query works correctly");

    println!("\n=== ✅ MESSAGE PERSISTENCE TESTS PASSED! ===\n");
    Ok(())
}

/// Test message pagination
#[tokio::test]
async fn test_message_pagination() -> MvpResult<()> {
    use crate::core_mvp::types::ChatMessage;

    println!("\n=== TESTING MESSAGE PAGINATION ===\n");

    let (alice_manager, _alice_dir) = create_test_manager("alice").await;

    // Create channel
    let channel_id = alice_manager
        .create_channel("busy-channel".to_string(), false)
        .await?;

    println!("Creating 10 messages...");
    
    // Create 10 messages
    for i in 1..=10 {
        let msg = ChatMessage::new(
            channel_id.clone(),
            alice_manager.identity().user_id.clone(),
            format!("Message {}", i).into_bytes(),
        );
        alice_manager.store_message(msg).await?;
    }
    println!("  ✓ Created 10 messages");

    // Test pagination
    println!("\nTest: Paginated retrieval...");
    
    // Get first page (5 messages, offset 0)
    let page1 = alice_manager
        .get_stored_messages_paginated(&channel_id, 5, 0)
        .await?;
    
    assert_eq!(page1.len(), 5, "First page should have 5 messages");
    println!("  ✓ Page 1: {} messages", page1.len());

    // Get second page (5 messages, offset 5)
    let page2 = alice_manager
        .get_stored_messages_paginated(&channel_id, 5, 5)
        .await?;
    
    assert_eq!(page2.len(), 5, "Second page should have 5 messages");
    println!("  ✓ Page 2: {} messages", page2.len());

    // Verify no overlap (messages are sorted newest first)
    let page1_ids: Vec<_> = page1.iter().map(|m| &m.id).collect();
    let page2_ids: Vec<_> = page2.iter().map(|m| &m.id).collect();
    
    for id in &page2_ids {
        assert!(!page1_ids.contains(&id), "Pages should not overlap");
    }
    println!("  ✓ Pages don't overlap");

    // Get partial page (3 messages, offset 8)
    let page3 = alice_manager
        .get_stored_messages_paginated(&channel_id, 5, 8)
        .await?;
    
    assert_eq!(page3.len(), 2, "Third page should have remaining 2 messages");
    println!("  ✓ Partial page: {} messages", page3.len());

    println!("\n=== ✅ MESSAGE PAGINATION TESTS PASSED! ===\n");
    Ok(())
}

