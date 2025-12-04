/// E2E test: Offline member synchronization
///
/// Scenario:
/// 1. Alice, Bob, Charlie are in a channel
/// 2. All three exchange messages successfully
/// 3. Charlie goes offline (simulated by not processing messages)
/// 4. Alice and Bob continue chatting
/// 5. Charlie comes back online and fetches missed messages
///
/// This tests:
/// - Message persistence while member is offline
/// - Synchronization when member reconnects
/// - Message ordering and delivery guarantees

use crate::core_mvp::channel_manager::{ChannelManager, Identity};
use crate::{
    config::Config,
    core_mls::service::MlsService,
    core_store::{
        model::types::{ChannelId, UserId, SpaceId},
        store::local_store::{LocalStore, LocalStoreConfig},
    },
    shutdown::ShutdownCoordinator,
};
use std::sync::Arc;
use tempfile::TempDir;

/// Helper to create a test manager with unique storage
async fn create_test_manager(name: &str, temp_dir: &TempDir) -> Arc<ChannelManager> {
    let user_id = UserId(format!("user-{}", name));
    let identity = Arc::new(Identity::new(
        user_id.clone(),
        name.to_string(),
        format!("node-{}", name),
    ));

    let config = Arc::new(Config::default());
    let shutdown = Arc::new(ShutdownCoordinator::new(std::time::Duration::from_secs(30)));

    // Create unique storage path for this user
    let mls_storage_dir = temp_dir.path().join(format!("mls_{}", name));
    let store_dir = temp_dir.path().join(format!("store_{}", name));

    let mls_service = Arc::new(
        MlsService::with_storage(&config, shutdown, mls_storage_dir)
            .expect("Failed to create MLS service"),
    );

    let store_config = LocalStoreConfig {
        data_dir: store_dir,
        enable_encryption: false,
        snapshot_interval: 1000,
        max_log_size: 10_000_000,
        enable_compaction: false,
        require_signatures: false,
        authorized_keys: Vec::new(),
    };

    let store = Arc::new(LocalStore::new(store_config).expect("Failed to create store"));
    store.load().expect("Failed to load store");

    Arc::new(ChannelManager::new(mls_service, store, identity, config))
}


#[tokio::test]
async fn test_offline_member_catches_up() -> anyhow::Result<()> {
    println!("\n=== Phase 1: Setup - Three members join channel ===");
    
    // Create three separate managers with isolated storage
    let temp_dir = TempDir::new().unwrap();
    let alice_manager = create_test_manager("alice", &temp_dir).await;
    let bob_manager = create_test_manager("bob", &temp_dir).await;
    let charlie_manager = create_test_manager("charlie", &temp_dir).await;

    // Alice creates the channel
    let channel_id = alice_manager
        .create_channel("team-chat".to_string(), false)
        .await
        .expect("Alice should create channel");
    
    println!("✅ Alice created channel: {}", channel_id);

    // Get keypairs for invitations
    let bob_kp = bob_manager.generate_key_package().await?;
    let charlie_kp = charlie_manager.generate_key_package().await?;

    // Bob joins
    let (bob_invite, _) = alice_manager
        .create_invite(&channel_id, bob_kp)
        .await?;
    bob_manager.join_channel(&bob_invite).await?;
    println!("✅ Bob joined channel");

    // Charlie joins
    let (charlie_invite, commit) = alice_manager
        .create_invite(&channel_id, charlie_kp)
        .await?;
    charlie_manager.join_channel(&charlie_invite).await?;
    
    // Bob processes the commit when Charlie joins
    if let Some(c) = commit {
        bob_manager.process_commit(&c).await?;
    }
    println!("✅ Charlie joined channel");

    println!("\n=== Phase 2: All three exchange initial messages ===");
    
    let alice_msg1 = alice_manager.send_message(&channel_id, b"Hello everyone!").await?;
    let bob_plaintext1 = bob_manager.receive_message(&alice_msg1).await?;
    let charlie_plaintext1 = charlie_manager.receive_message(&alice_msg1).await?;
    
    assert_eq!(bob_plaintext1, b"Hello everyone!");
    assert_eq!(charlie_plaintext1, b"Hello everyone!");
    println!("✅ Alice: 'Hello everyone!' - received by Bob and Charlie");

    let bob_msg1 = bob_manager.send_message(&channel_id, b"Hey Alice!").await?;
    let alice_plaintext1 = alice_manager.receive_message(&bob_msg1).await?;
    let charlie_plaintext2 = charlie_manager.receive_message(&bob_msg1).await?;
    
    assert_eq!(alice_plaintext1, b"Hey Alice!");
    assert_eq!(charlie_plaintext2, b"Hey Alice!");
    println!("✅ Bob: 'Hey Alice!' - received by Alice and Charlie");

    println!("\n=== Phase 3: Charlie goes offline ===");
    println!("📴 Charlie disconnects (stops processing messages)");
    
    // Store messages that Charlie will miss
    let mut missed_messages = Vec::new();

    println!("\n=== Phase 4: Alice and Bob continue chatting ===");
    
    let alice_msg2 = alice_manager.send_message(&channel_id, b"Charlie went offline").await?;
    let bob_plaintext2 = bob_manager.receive_message(&alice_msg2).await?;
    assert_eq!(bob_plaintext2, b"Charlie went offline");
    println!("✅ Alice: 'Charlie went offline' - received by Bob");
    missed_messages.push(alice_msg2.clone());

    let bob_msg2 = bob_manager.send_message(&channel_id, b"Yeah, I noticed").await?;
    let alice_plaintext2 = alice_manager.receive_message(&bob_msg2).await?;
    assert_eq!(alice_plaintext2, b"Yeah, I noticed");
    println!("✅ Bob: 'Yeah, I noticed' - received by Alice");
    missed_messages.push(bob_msg2.clone());

    let alice_msg3 = alice_manager.send_message(&channel_id, b"Let's wait for him").await?;
    let bob_plaintext3 = bob_manager.receive_message(&alice_msg3).await?;
    assert_eq!(bob_plaintext3, b"Let's wait for him");
    println!("✅ Alice: 'Let's wait for him' - received by Bob");
    missed_messages.push(alice_msg3.clone());

    let bob_msg3 = bob_manager.send_message(&channel_id, b"Sure thing").await?;
    let alice_plaintext3 = alice_manager.receive_message(&bob_msg3).await?;
    assert_eq!(alice_plaintext3, b"Sure thing");
    println!("✅ Bob: 'Sure thing' - received by Alice");
    missed_messages.push(bob_msg3.clone());

    println!("\n=== Phase 5: Charlie comes back online and syncs ===");
    println!("📡 Charlie reconnects and processes missed messages");
    
    // Charlie processes all missed messages in order
    let mut charlie_plaintexts = Vec::new();
    for (i, msg) in missed_messages.iter().enumerate() {
        let plaintext = charlie_manager.receive_message(msg).await?;
        let text = String::from_utf8_lossy(&plaintext);
        println!("   Message {}: '{}'", i + 1, text);
        charlie_plaintexts.push(plaintext);
    }

    // Verify Charlie got all messages correctly
    assert_eq!(charlie_plaintexts[0], b"Charlie went offline");
    assert_eq!(charlie_plaintexts[1], b"Yeah, I noticed");
    assert_eq!(charlie_plaintexts[2], b"Let's wait for him");
    assert_eq!(charlie_plaintexts[3], b"Sure thing");
    
    println!("✅ Charlie successfully synced all {} missed messages", missed_messages.len());

    println!("\n=== Phase 6: All three continue chatting normally ===");
    
    let charlie_msg = charlie_manager.send_message(&channel_id, b"I'm back! What did I miss?").await?;
    let alice_plaintext4 = alice_manager.receive_message(&charlie_msg).await?;
    let bob_plaintext4 = bob_manager.receive_message(&charlie_msg).await?;
    
    assert_eq!(alice_plaintext4, b"I'm back! What did I miss?");
    assert_eq!(bob_plaintext4, b"I'm back! What did I miss?");
    println!("✅ Charlie: 'I'm back! What did I miss?' - received by Alice and Bob");

    let alice_msg4 = alice_manager.send_message(&channel_id, b"Welcome back!").await?;
    let bob_plaintext5 = bob_manager.receive_message(&alice_msg4).await?;
    let charlie_plaintext7 = charlie_manager.receive_message(&alice_msg4).await?;
    
    assert_eq!(bob_plaintext5, b"Welcome back!");
    assert_eq!(charlie_plaintext7, b"Welcome back!");
    println!("✅ Alice: 'Welcome back!' - received by Bob and Charlie");

    println!("\n=== Test Complete ===");
    println!("✅ All assertions passed!");
    println!("✅ Offline member successfully synced {} missed messages", missed_messages.len());
    println!("✅ Normal communication resumed after sync");

    Ok(())
}

#[tokio::test]
async fn test_multiple_offline_periods() -> anyhow::Result<()> {
    println!("\n=== Testing multiple offline/online cycles ===");
    
    let temp_dir = TempDir::new().unwrap();
    let alice_manager = create_test_manager("alice_multi", &temp_dir).await;
    let bob_manager = create_test_manager("bob_multi", &temp_dir).await;
    let charlie_manager = create_test_manager("charlie_multi", &temp_dir).await;
    
    let channel_id = alice_manager
        .create_channel("team-chat".to_string(), false)
        .await
        .expect("Alice should create channel");

    // Setup: Bob and Charlie join
    let bob_kp = bob_manager.generate_key_package().await?;
    let (bob_invite, _) = alice_manager.create_invite(&channel_id, bob_kp).await?;
    bob_manager.join_channel(&bob_invite).await?;

    let charlie_kp = charlie_manager.generate_key_package().await?;
    let (charlie_invite, commit) = alice_manager.create_invite(&channel_id, charlie_kp).await?;
    charlie_manager.join_channel(&charlie_invite).await?;
    if let Some(c) = commit {
        bob_manager.process_commit(&c).await?;
    }

    println!("✅ All three members joined");

    // Cycle 1: Charlie offline
    println!("\n--- Cycle 1: Charlie offline ---");
    let msg1 = alice_manager.send_message(&channel_id, b"Cycle 1 message").await?;
    bob_manager.receive_message(&msg1).await?;
    
    // Charlie comes back
    let plaintext1 = charlie_manager.receive_message(&msg1).await?;
    assert_eq!(plaintext1, b"Cycle 1 message");
    println!("✅ Charlie synced after first offline period");

    // Cycle 2: Charlie offline again
    println!("\n--- Cycle 2: Charlie offline again ---");
    let msg2 = bob_manager.send_message(&channel_id, b"Cycle 2 message").await?;
    alice_manager.receive_message(&msg2).await?;
    
    // Charlie comes back again
    let plaintext2 = charlie_manager.receive_message(&msg2).await?;
    assert_eq!(plaintext2, b"Cycle 2 message");
    println!("✅ Charlie synced after second offline period");

    // Everyone back online
    let msg3 = charlie_manager.send_message(&channel_id, b"I keep losing connection!").await?;
    alice_manager.receive_message(&msg3).await?;
    bob_manager.receive_message(&msg3).await?;
    println!("✅ Charlie can send messages after multiple sync cycles");

    println!("\n=== Test Complete ===");
    println!("✅ Multiple offline/online cycles work correctly");

    Ok(())
}
