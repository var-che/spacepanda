//! HTTP Harness Integration Tests for Admin Roles and Member Management
//!
//! Tests the HTTP API endpoints for:
//! - Admin role assignment (creator gets admin)
//! - Member removal (admin-only)
//! - Promote/demote operations
//! - Role queries
//!
//! These tests verify the core functionality that the HTTP endpoints expose.

use spacepanda_core::config::Config;
use spacepanda_core::core_mls::service::MlsService;
use spacepanda_core::core_store::model::types::UserId;
use spacepanda_core::core_store::store::{LocalStore, LocalStoreConfig};
use spacepanda_core::shutdown::ShutdownCoordinator;
use spacepanda_core::{ChannelManager, Identity};
use std::sync::Arc;
use tempfile::TempDir;

/// Helper to create a test ChannelManager
async fn create_test_manager(name: &str) -> (Arc<ChannelManager>, TempDir) {
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
async fn test_creator_is_admin() {
    println!("\n=== Testing: Channel creator gets Admin role ===\n");

    let (alice_manager, _alice_dir) = create_test_manager("alice").await;

    // Alice creates a channel
    let channel_id = alice_manager
        .create_channel("admin-test".to_string(), false)
        .await
        .expect("Alice should create channel");
    println!("✓ Channel created");

    // Query Alice's role
    let alice_identity = alice_manager.identity().user_id.0.as_bytes();
    let role = alice_manager
        .get_member_role(&channel_id, alice_identity)
        .await
        .expect("Should get Alice's role");

    assert_eq!(
        role,
        spacepanda_core::core_mls::types::MemberRole::Admin,
        "Creator should be Admin"
    );
    println!("✓ Alice has Admin role");

    // Verify is_admin also works
    let is_admin = alice_manager
        .is_admin(&channel_id, alice_identity)
        .await
        .expect("Should check if admin");
    assert!(is_admin, "is_admin should return true for creator");
    println!("✓ is_admin returns true for creator");

    println!("\n=== ✅ Creator Admin Test PASSED ===\n");
}

#[tokio::test]
async fn test_member_removal_requires_admin() {
    println!("\n=== Testing: Member removal requires Admin ===\n");

    let (alice_manager, _alice_dir) = create_test_manager("alice").await;
    let (bob_manager, _bob_dir) = create_test_manager("bob").await;
    let (charlie_manager, _charlie_dir) = create_test_manager("charlie").await;

    // Alice creates channel
    let channel_id = alice_manager
        .create_channel("removal-test".to_string(), false)
        .await
        .expect("Alice should create channel");
    println!("✓ Channel created");

    // Bob joins
    let bob_kp = bob_manager.generate_key_package().await.unwrap();
    let (invite, _) = alice_manager
        .create_invite(&channel_id, bob_kp)
        .await
        .expect("Alice should invite Bob");
    bob_manager.join_channel(&invite).await.expect("Bob should join");
    println!("✓ Bob joined");

    // Charlie joins
    let charlie_kp = charlie_manager.generate_key_package().await.unwrap();
    let (invite, commit) = alice_manager
        .create_invite(&channel_id, charlie_kp)
        .await
        .expect("Alice should invite Charlie");

    // Bob processes Charlie's join
    if let Some(commit_bytes) = commit {
        bob_manager.process_commit(&commit_bytes).await.expect("Bob should process");
    }

    charlie_manager.join_channel(&invite).await.expect("Charlie should join");
    println!("✓ Charlie joined");

    let charlie_identity = charlie_manager.identity().user_id.0.as_bytes();

    // Test: Bob (member) CANNOT remove Charlie
    println!("\nTesting Bob (member) cannot remove Charlie...");
    let result = bob_manager.remove_member(&channel_id, charlie_identity).await;
    assert!(result.is_err(), "Bob should be denied (not admin)");
    println!("✓ Bob correctly denied (not admin)");

    // Test: Alice (admin) CAN remove Charlie
    println!("\nTesting Alice (admin) can remove Charlie...");
    let result = alice_manager.remove_member(&channel_id, charlie_identity).await;
    assert!(result.is_ok(), "Alice should remove Charlie (is admin)");
    println!("✓ Alice successfully removed Charlie");

    println!("\n=== ✅ Admin-Only Removal Test PASSED ===\n");
}

#[tokio::test]
async fn test_promote_demote_operations() {
    println!("\n=== Testing: Promote/Demote operations ===\n");

    let (alice_manager, _alice_dir) = create_test_manager("alice").await;
    let (bob_manager, _bob_dir) = create_test_manager("bob").await;

    // Alice creates channel
    let channel_id = alice_manager
        .create_channel("promote-test".to_string(), false)
        .await
        .expect("Alice should create channel");

    // Bob joins
    let bob_kp = bob_manager.generate_key_package().await.unwrap();
    let (invite, _) = alice_manager
        .create_invite(&channel_id, bob_kp)
        .await
        .expect("Alice should invite Bob");
    bob_manager.join_channel(&invite).await.expect("Bob should join");
    println!("✓ Setup complete: Alice (admin), Bob (member)");

    let bob_identity = bob_manager.identity().user_id.0.as_bytes();
    let alice_identity = alice_manager.identity().user_id.0.as_bytes();

    // Test: Bob (member) CANNOT promote
    println!("\nTesting Bob (member) cannot promote...");
    let result = bob_manager.promote_member(&channel_id, alice_identity).await;
    assert!(result.is_err(), "Bob should be denied");
    println!("✓ Bob correctly denied permission to promote");

    // Test: Bob (member) CANNOT demote
    println!("\nTesting Bob (member) cannot demote...");
    let result = bob_manager.demote_member(&channel_id, alice_identity).await;
    assert!(result.is_err(), "Bob should be denied");
    println!("✓ Bob correctly denied permission to demote");

    // Test: Alice (admin) CAN promote Bob
    println!("\nTesting Alice (admin) can promote Bob...");
    let result = alice_manager.promote_member(&channel_id, bob_identity).await;
    assert!(result.is_ok(), "Alice can promote");
    println!("✓ Alice promoted Bob");

    // Test: Alice (admin) CAN demote Bob
    println!("\nTesting Alice (admin) can demote Bob...");
    let result = alice_manager.demote_member(&channel_id, bob_identity).await;
    assert!(result.is_ok(), "Alice can demote");
    println!("✓ Alice demoted Bob");

    println!("\n=== ✅ Promote/Demote Test PASSED ===\n");
}

#[tokio::test]
async fn test_role_query_operations() {
    println!("\n=== Testing: Role query operations ===\n");

    let (alice_manager, _alice_dir) = create_test_manager("alice").await;
    let (bob_manager, _bob_dir) = create_test_manager("bob").await;

    // Alice creates channel
    let channel_id = alice_manager
        .create_channel("role-query-test".to_string(), false)
        .await
        .expect("Alice should create channel");

    // Bob joins
    let bob_kp = bob_manager.generate_key_package().await.unwrap();
    let (invite, _) = alice_manager
        .create_invite(&channel_id, bob_kp)
        .await
        .expect("Alice should invite Bob");
    bob_manager.join_channel(&invite).await.expect("Bob should join");

    let alice_identity = alice_manager.identity().user_id.0.as_bytes();
    let bob_identity = bob_manager.identity().user_id.0.as_bytes();

    // Test: Query Alice's role (should be Admin)
    println!("\nQuerying Alice's role...");
    let role = alice_manager
        .get_member_role(&channel_id, alice_identity)
        .await
        .expect("Should get role");
    assert_eq!(role, spacepanda_core::core_mls::types::MemberRole::Admin);
    println!("✓ Alice has Admin role");

    // Test: Query Bob's role (should be Member)
    println!("\nQuerying Bob's role...");
    let role = alice_manager
        .get_member_role(&channel_id, bob_identity)
        .await
        .expect("Should get role");
    assert_eq!(role, spacepanda_core::core_mls::types::MemberRole::Member);
    println!("✓ Bob has Member role");

    // Test: is_admin for Alice
    let is_admin = alice_manager
        .is_admin(&channel_id, alice_identity)
        .await
        .expect("Should check if admin");
    assert!(is_admin);
    println!("✓ is_admin returns true for Alice");

    // Test: is_admin for Bob
    let is_admin = alice_manager
        .is_admin(&channel_id, bob_identity)
        .await
        .expect("Should check if admin");
    assert!(!is_admin);
    println!("✓ is_admin returns false for Bob");

    println!("\n=== ✅ Role Query Test PASSED ===\n");
}

#[tokio::test]
async fn test_full_admin_workflow() {
    println!("\n=== Testing: Full admin workflow ===\n");

    let (alice_manager, _alice_dir) = create_test_manager("alice").await;
    let (bob_manager, _bob_dir) = create_test_manager("bob").await;
    let (charlie_manager, _charlie_dir) = create_test_manager("charlie").await;

    // 1. Alice creates channel (becomes admin)
    println!("Step 1: Alice creates channel...");
    let channel_id = alice_manager
        .create_channel("admin-workflow".to_string(), false)
        .await
        .expect("Alice should create channel");
    println!("✓ Channel created");

    // 2. Bob joins (becomes member)
    println!("\nStep 2: Bob joins channel...");
    let bob_kp = bob_manager.generate_key_package().await.unwrap();
    let (invite, _) = alice_manager
        .create_invite(&channel_id, bob_kp)
        .await
        .expect("Alice should invite Bob");
    bob_manager.join_channel(&invite).await.expect("Bob should join");
    println!("✓ Bob joined as Member");

    // 3. Charlie joins
    println!("\nStep 3: Charlie joins channel...");
    let charlie_kp = charlie_manager.generate_key_package().await.unwrap();
    let (invite, commit) = alice_manager
        .create_invite(&channel_id, charlie_kp)
        .await
        .expect("Alice should invite Charlie");

    // Bob processes commit
    if let Some(commit_bytes) = commit {
        bob_manager.process_commit(&commit_bytes).await.expect("Bob should process");
    }

    charlie_manager.join_channel(&invite).await.expect("Charlie should join");
    println!("✓ Charlie joined as Member");

    // 4. Alice promotes Bob to admin
    println!("\nStep 4: Alice promotes Bob to Admin...");
    let bob_identity = bob_manager.identity().user_id.0.as_bytes();
    alice_manager
        .promote_member(&channel_id, bob_identity)
        .await
        .expect("Alice should promote Bob");
    println!("✓ Bob promoted to Admin (permission granted, role change pending CRDT)");

    // Note: promote/demote currently don't persist role changes (pending CRDT integration)
    // So Bob won't actually have admin permissions yet
    println!("\nStep 5: Verifying promote/demote permissions work...");

    // Verify Alice can still perform admin actions
    let charlie_identity = charlie_manager.identity().user_id.0.as_bytes();
    let result = alice_manager.remove_member(&channel_id, charlie_identity).await;
    assert!(result.is_ok(), "Alice (admin) can remove Charlie");
    println!("✓ Alice successfully removed Charlie");

    // 6. Verify final state
    println!("\nStep 6: Verifying final state...");
    let alice_identity = alice_manager.identity().user_id.0.as_bytes();
    let alice_role = alice_manager
        .get_member_role(&channel_id, alice_identity)
        .await
        .expect("Should get Alice's role");
    assert_eq!(
        alice_role,
        spacepanda_core::core_mls::types::MemberRole::Admin,
        "Alice should still be Admin"
    );
    println!("✓ Alice is still Admin");

    println!("\n=== ✅ Full Admin Workflow PASSED ===\n");
    println!("Note: Promote/demote functionality verified for permissions,");
    println!("      but role persistence pending CRDT integration");
}
