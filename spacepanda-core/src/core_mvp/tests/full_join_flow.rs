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

