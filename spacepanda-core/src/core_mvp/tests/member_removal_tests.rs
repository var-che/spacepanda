//! Comprehensive tests for member removal feature
//!
//! These tests verify the complete member removal workflow including:
//! - Core ChannelManager.remove_member() functionality
//! - 4-party group scenarios
//! - Edge cases and error handling

use crate::config::Config;
use crate::core_mls::service::MlsService;
use crate::core_mvp::channel_manager::{ChannelManager, Identity};
use crate::core_store::model::types::UserId;
use crate::core_store::store::{LocalStore, LocalStoreConfig};
use crate::shutdown::ShutdownCoordinator;
use std::sync::Arc;
use tempfile::TempDir;

/// Helper to create a test ChannelManager
async fn create_manager(name: &str) -> (Arc<ChannelManager>, TempDir) {
    let config = Arc::new(Config::default());
    let shutdown = Arc::new(ShutdownCoordinator::new(std::time::Duration::from_secs(30)));
    let mls_service = Arc::new(MlsService::new(&config, shutdown.clone()));

    let temp_dir = tempfile::tempdir().unwrap();
    let store_config = LocalStoreConfig {
        data_dir: temp_dir.path().to_path_buf(),
        enable_encryption: false,
        snapshot_interval: 1000,
        max_log_size: 10_000_000,
        enable_compaction: false,
        require_signatures: false,
        authorized_keys: Vec::new(),
    };
    let store = Arc::new(LocalStore::new(store_config).unwrap());

    let identity = Arc::new(Identity::new(
        UserId(uuid::Uuid::new_v4().to_string()),
        name.to_string(),
        uuid::Uuid::new_v4().to_string(),
    ));

    let manager = Arc::new(ChannelManager::new(mls_service, store, identity, config.clone()));
    (manager, temp_dir)
}

#[tokio::test]
async fn test_remove_member_basic() {
    println!("\n=== TEST: Basic Member Removal ===");

    let (alice, _alice_dir) = create_manager("Alice").await;
    let (bob, _bob_dir) = create_manager("Bob").await;

    // Alice creates channel
    let channel_id = alice.create_channel("test".to_string(), false).await.unwrap();
    println!("✓ Alice created channel");

    // Alice invites Bob
    let bob_kp = bob.generate_key_package().await.unwrap();
    let (welcome, _) = alice.create_invite(&channel_id, bob_kp).await.unwrap();
    println!("✓ Alice invited Bob");

    // Bob joins
    bob.join_channel(&welcome).await.unwrap();
    println!("✓ Bob joined");

    // Alice removes Bob
    let bob_identity = bob.identity().user_id.0.as_bytes();
    let removal_commit = alice.remove_member(&channel_id, bob_identity).await.unwrap();
    println!("✓ Alice removed Bob");

    // Bob should not be able to process new messages
    let msg = alice.send_message(&channel_id, b"After removal").await.unwrap();
    let result = bob.process_commit(&msg).await;
    assert!(result.is_err(), "Bob should not decrypt after removal");
    println!("✓ Bob cannot decrypt after removal");

    println!("✅ Basic removal test passed\n");
}

#[tokio::test]
async fn test_remove_member_with_multiple_remaining() {
    println!("\n=== TEST: Remove Member with Multiple Remaining ===");

    let (alice, _a) = create_manager("Alice").await;
    let (bob, _b) = create_manager("Bob").await;
    let (charlie, _c) = create_manager("Charlie").await;
    let (dave, _d) = create_manager("Dave").await;

    // Setup 4-person group
    let channel_id = alice.create_channel("test".to_string(), false).await.unwrap();

    // Add Bob
    let bob_kp = bob.generate_key_package().await.unwrap();
    let (welcome, _) = alice.create_invite(&channel_id, bob_kp).await.unwrap();
    bob.join_channel(&welcome).await.unwrap();

    // Add Charlie
    let charlie_kp = charlie.generate_key_package().await.unwrap();
    let (welcome, commit) = alice.create_invite(&channel_id, charlie_kp).await.unwrap();
    if let Some(commit_bytes) = commit {
        bob.process_commit(&commit_bytes).await.unwrap();
    }
    charlie.join_channel(&welcome).await.unwrap();

    // Add Dave
    let dave_kp = dave.generate_key_package().await.unwrap();
    let (welcome, commit) = alice.create_invite(&channel_id, dave_kp).await.unwrap();
    if let Some(commit_bytes) = commit {
        bob.process_commit(&commit_bytes).await.unwrap();
        charlie.process_commit(&commit_bytes).await.unwrap();
    }
    dave.join_channel(&welcome).await.unwrap();

    println!("✓ 4-person group established");

    // Remove Bob
    let bob_identity = bob.identity().user_id.0.as_bytes();
    let removal_commit = alice.remove_member(&channel_id, bob_identity).await.unwrap();
    charlie.process_commit(&removal_commit).await.unwrap();
    dave.process_commit(&removal_commit).await.unwrap();
    println!("✓ Bob removed");

    // Alice, Charlie, Dave should still communicate
    let msg = alice.send_message(&channel_id, b"After Bob removed").await.unwrap();
    charlie.process_commit(&msg).await.unwrap();
    dave.process_commit(&msg).await.unwrap();
    println!("✓ Remaining members can communicate");

    // Bob should not decrypt
    let result = bob.process_commit(&msg).await;
    assert!(result.is_err());
    println!("✓ Removed member cannot decrypt");

    println!("✅ Multiple remaining members test passed\n");
}

#[tokio::test]
async fn test_remove_nonexistent_member() {
    println!("\n=== TEST: Remove Non-existent Member ===");

    let (alice, _a) = create_manager("Alice").await;
    let channel_id = alice.create_channel("test".to_string(), false).await.unwrap();

    // Try to remove a member that doesn't exist
    let fake_identity = b"nonexistent_user_id";
    let result = alice.remove_member(&channel_id, fake_identity).await;

    assert!(result.is_err(), "Should fail to remove non-existent member");
    println!("✓ Correctly rejects removing non-existent member");
    println!("✅ Error handling test passed\n");
}

#[tokio::test]
async fn test_remove_self() {
    println!("\n=== TEST: Remove Self ===");

    let (alice, _a) = create_manager("Alice").await;
    let (bob, _b) = create_manager("Bob").await;

    let channel_id = alice.create_channel("test".to_string(), false).await.unwrap();

    let bob_kp = bob.generate_key_package().await.unwrap();
    let (welcome, _) = alice.create_invite(&channel_id, bob_kp).await.unwrap();
    bob.join_channel(&welcome).await.unwrap();

    // Alice tries to remove herself
    let alice_identity = alice.identity().user_id.0.as_bytes();
    let result = alice.remove_member(&channel_id, alice_identity).await;

    // This might succeed or fail depending on MLS implementation
    // Document the behavior
    match result {
        Ok(_) => println!("✓ Self-removal is allowed"),
        Err(_) => println!("✓ Self-removal is rejected"),
    }

    println!("✅ Self-removal behavior documented\n");
}

#[tokio::test]
async fn test_sequential_removals() {
    println!("\n=== TEST: Sequential Member Removals ===");

    let (alice, _a) = create_manager("Alice").await;
    let (bob, _b) = create_manager("Bob").await;
    let (charlie, _c) = create_manager("Charlie").await;
    let (dave, _d) = create_manager("Dave").await;

    // Setup 4-person group
    let channel_id = alice.create_channel("test".to_string(), false).await.unwrap();

    let bob_kp = bob.generate_key_package().await.unwrap();
    let (welcome, _) = alice.create_invite(&channel_id, bob_kp).await.unwrap();
    bob.join_channel(&welcome).await.unwrap();

    let charlie_kp = charlie.generate_key_package().await.unwrap();
    let (welcome, commit) = alice.create_invite(&channel_id, charlie_kp).await.unwrap();
    if let Some(commit_bytes) = commit {
        bob.process_commit(&commit_bytes).await.unwrap();
    }
    charlie.join_channel(&welcome).await.unwrap();

    let dave_kp = dave.generate_key_package().await.unwrap();
    let (welcome, commit) = alice.create_invite(&channel_id, dave_kp).await.unwrap();
    if let Some(commit_bytes) = commit {
        bob.process_commit(&commit_bytes).await.unwrap();
        charlie.process_commit(&commit_bytes).await.unwrap();
    }
    dave.join_channel(&welcome).await.unwrap();

    println!("✓ 4-person group established");

    // Remove Bob
    let bob_identity = bob.identity().user_id.0.as_bytes();
    let commit1 = alice.remove_member(&channel_id, bob_identity).await.unwrap();
    charlie.process_commit(&commit1).await.unwrap();
    dave.process_commit(&commit1).await.unwrap();
    println!("✓ Bob removed");

    // Remove Charlie
    let charlie_identity = charlie.identity().user_id.0.as_bytes();
    let commit2 = alice.remove_member(&channel_id, charlie_identity).await.unwrap();
    dave.process_commit(&commit2).await.unwrap();
    println!("✓ Charlie removed");

    // Alice and Dave should still communicate
    let msg = alice.send_message(&channel_id, b"Just us now").await.unwrap();
    dave.process_commit(&msg).await.unwrap();
    println!("✓ Remaining 2 members can communicate");

    // Bob and Charlie should not decrypt
    assert!(bob.process_commit(&msg).await.is_err());
    assert!(charlie.process_commit(&msg).await.is_err());
    println!("✓ Both removed members cannot decrypt");

    println!("✅ Sequential removals test passed\n");
}
