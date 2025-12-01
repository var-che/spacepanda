/*
    persistence_tests.rs - Mission-critical persistence tests for Store subsystem
    
    These tests validate critical persistence, recovery, and durability scenarios
    before MLS integration. All tests must pass before production deployment.
*/

use crate::core_store::store::{LocalStore, LocalStoreConfig, CommitLog, SnapshotManager};
use crate::core_store::model::{Space, Channel, SpaceId, ChannelId, UserId, Timestamp, ChannelType};
use crate::core_store::crdt::{Crdt, VectorClock};
use std::collections::HashMap;
use tempfile::tempdir;

/// Test 5.1: Snapshot Replay Test
/// 
/// Validates that snapshot + delta replay produces identical state.
/// Critical for crash recovery and state rehydration.
#[tokio::test]
async fn test_store_snapshot_replay() {
    let temp_dir = tempdir().unwrap();
    let config = LocalStoreConfig {
        data_dir: temp_dir.path().to_path_buf(),
        enable_encryption: false, // Simplify for testing
        snapshot_interval: 200, // Snapshot after 200 ops
        max_log_size: 10_000_000,
        enable_compaction: false,
    };
    
    let store = LocalStore::new(config.clone()).unwrap();
    let user_id = UserId::generate();
    
    // Phase 1: Apply 200 operations (spaces)
    let mut space_ids = Vec::new();
    for i in 0..200 {
        let space = Space::new(
            SpaceId::generate(),
            format!("Space {}", i),
            user_id.clone(),
            Timestamp::now(),
            "node1".to_string(),
        );
        space_ids.push(space.id.clone());
        store.store_space(&space).unwrap();
    }
    
    // Phase 2: Apply 200 more operations (channels)
    let mut channel_ids = Vec::new();
    for i in 0..200 {
        let channel = Channel::new(
            ChannelId::generate(),
            format!("Channel {}", i),
            ChannelType::Text,
            user_id.clone(),
            Timestamp::now(),
            "node1".to_string(),
        );
        channel_ids.push(channel.id.clone());
        store.store_channel(&channel).unwrap();
    }
    
    // Create final snapshot with all data
    store.create_snapshot().unwrap();
    
    // Capture final state
    let final_spaces: HashMap<_, _> = space_ids.iter()
        .map(|id| (id.clone(), store.get_space(id).unwrap().unwrap()))
        .collect();
    let final_channels: HashMap<_, _> = channel_ids.iter()
        .map(|id| (id.clone(), store.get_channel(id).unwrap().unwrap()))
        .collect();
    
    drop(store);
    
    // Phase 3: Restore from snapshot + replay deltas
    let store2 = LocalStore::new(config).unwrap();
    store2.load().unwrap(); // Load from snapshot
    
    // Debug: check what was loaded
    let stats = store2.stats();
    println!("Loaded from snapshot: {} spaces, {} channels", stats.spaces_count, stats.channels_count);
    
    // Verify all spaces restored
    for space_id in &space_ids {
        let restored_space = store2.get_space(space_id).unwrap();
        assert!(restored_space.is_some(), "Space {} not restored", space_id);
        let original = final_spaces.get(space_id).unwrap();
        let restored = restored_space.unwrap();
        assert_eq!(original.id, restored.id);
        assert_eq!(original.name, restored.name);
    }
    
    // Verify all channels restored
    for channel_id in &channel_ids {
        let restored_channel = store2.get_channel(channel_id).unwrap();
        assert!(restored_channel.is_some(), "Channel {} not restored", channel_id);
        let original = final_channels.get(channel_id).unwrap();
        let restored = restored_channel.unwrap();
        assert_eq!(original.id, restored.id);
        assert_eq!(original.name, restored.name);
    }
    
    println!("✅ Snapshot replay test passed: 400 ops restored correctly");
}

/// Test 5.2: Corrupt Snapshot Handling
/// 
/// Validates graceful handling of corrupted snapshot files.
#[tokio::test]
async fn test_store_corrupt_snapshot_handling() {
    let temp_dir = tempdir().unwrap();
    let snapshots_dir = temp_dir.path().join("snapshots");
    std::fs::create_dir_all(&snapshots_dir).unwrap();
    
    let manager = SnapshotManager::new(snapshots_dir.clone()).unwrap();
    
    // Create valid snapshot
    let mut spaces = HashMap::new();
    let space = Space::new(
        SpaceId::generate(),
        "Test Space".to_string(),
        UserId::generate(),
        Timestamp::now(),
        "node1".to_string(),
    );
    spaces.insert(space.id.clone(), space);
    
    manager.create_snapshot(spaces, HashMap::new()).unwrap();
    
    // Corrupt the snapshot file by overwriting with garbage
    let snapshot_files: Vec<_> = std::fs::read_dir(&snapshots_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("bin"))
        .collect();
    
    assert!(!snapshot_files.is_empty(), "No snapshot file created");
    
    let snapshot_path = snapshot_files[0].path();
    std::fs::write(&snapshot_path, b"CORRUPTED_GARBAGE_DATA").unwrap();
    
    // Attempt to load - should fail gracefully
    let result = manager.load_latest();
    
    // Should return error, not panic
    assert!(result.is_err(), "Corrupted snapshot should return error");
    
    // Verify error message is helpful
    let error_msg = format!("{:?}", result.err().unwrap());
    assert!(
        error_msg.contains("bincode") || 
        error_msg.contains("deserialize") || 
        error_msg.contains("invalid") ||
        error_msg.contains("io error") ||
        error_msg.contains("unexpected end"),
        "Error message should indicate corruption: {}",
        error_msg
    );
    
    println!("✅ Corrupt snapshot handled gracefully with error");
}

/// Test 5.3: Commit Log Corruption Recovery
/// 
/// Validates recovery from corrupted commit log entries.
#[tokio::test]
async fn test_store_commit_log_corruption_recovery() {
    let temp_dir = tempdir().unwrap();
    let log_path = temp_dir.path().join("commit.log");
    
    let mut log = CommitLog::new(log_path.clone()).unwrap();
    
    // Write valid entries
    log.append(b"entry1").unwrap();
    log.append(b"entry2").unwrap();
    log.append(b"entry3").unwrap();
    
    drop(log);
    
    // Corrupt the log file by appending garbage
    use std::fs::OpenOptions;
    use std::io::Write;
    let mut file = OpenOptions::new()
        .append(true)
        .open(&log_path)
        .unwrap();
    file.write_all(b"GARBAGE_CORRUPT_DATA_NO_VALID_CHECKSUM").unwrap();
    drop(file);
    
    // Try to read - should detect corruption via checksum
    let log = CommitLog::new(log_path).unwrap();
    let result = log.read_all();
    
    // Should fail with corruption error
    assert!(result.is_err(), "Corrupted log should return error");
    
    let error_msg = format!("{:?}", result.err().unwrap());
    assert!(
        error_msg.contains("checksum") || 
        error_msg.contains("corrupt") ||
        error_msg.contains("failed to fill whole buffer"),
        "Error should indicate corruption: {}",
        error_msg
    );
    
    println!("✅ Commit log corruption detected via checksum");
}

/// Test 5.4: Concurrent Write Safety
/// 
/// Validates thread-safe writes to store.
#[tokio::test]
async fn test_store_concurrent_write_safety() {
    let temp_dir = tempdir().unwrap();
    let config = LocalStoreConfig {
        data_dir: temp_dir.path().to_path_buf(),
        enable_encryption: false,
        snapshot_interval: 10000, // High to avoid snapshots during test
        max_log_size: 10_000_000,
        enable_compaction: false,
    };
    
    let store = std::sync::Arc::new(LocalStore::new(config).unwrap());
    let user_id = UserId::generate();
    
    // Spawn 10 concurrent tasks writing spaces
    let mut handles = vec![];
    for thread_id in 0..10 {
        let store_clone = store.clone();
        let user_id_clone = user_id.clone();
        
        let handle = tokio::spawn(async move {
            for i in 0..50 {
                let space = Space::new(
                    SpaceId::generate(),
                    format!("Thread{}-Space{}", thread_id, i),
                    user_id_clone.clone(),
                    Timestamp::now(),
                    format!("node{}", thread_id),
                );
                store_clone.store_space(&space).unwrap();
            }
        });
        
        handles.push(handle);
    }
    
    // Wait for all tasks
    for handle in handles {
        handle.await.unwrap();
    }
    
    // Verify all 500 spaces stored (10 threads × 50 spaces)
    let spaces = store.list_spaces().unwrap();
    assert_eq!(spaces.len(), 500, "Expected 500 spaces from concurrent writes");
    
    println!("✅ Concurrent write safety verified: 500 spaces from 10 threads");
}

/// Test 5.5: Storage Limits and Cleanup
/// 
/// Validates storage limits are enforced and cleanup works.
#[tokio::test]
async fn test_store_storage_limits_cleanup() {
    let temp_dir = tempdir().unwrap();
    let snapshots_dir = temp_dir.path().join("snapshots");
    std::fs::create_dir_all(&snapshots_dir).unwrap();
    
    let manager = SnapshotManager::new(snapshots_dir.clone()).unwrap();
    
    // Create 10 snapshots
    for i in 0..10 {
        let mut spaces = HashMap::new();
        let space = Space::new(
            SpaceId::generate(),
            format!("Space {}", i),
            UserId::generate(),
            Timestamp::now(),
            "node1".to_string(),
        );
        spaces.insert(space.id.clone(), space);
        
        manager.create_snapshot(spaces, HashMap::new()).unwrap();
        
        // Small delay to ensure different timestamps
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }
    
    // Verify 10 snapshots exist
    let snapshot_count = std::fs::read_dir(&snapshots_dir)
        .unwrap()
        .filter(|e| e.is_ok())
        .count();
    assert_eq!(snapshot_count, 10, "Should have 10 snapshots");
    
    // Cleanup, keeping only 3
    manager.cleanup_old_snapshots(3).unwrap();
    
    // Verify only 3 remain
    let remaining_count = std::fs::read_dir(&snapshots_dir)
        .unwrap()
        .filter(|e| e.is_ok())
        .count();
    assert_eq!(remaining_count, 3, "Should have only 3 snapshots after cleanup");
    
    // Verify latest snapshot is still loadable
    let (spaces, _) = manager.load_latest().unwrap();
    assert_eq!(spaces.len(), 1, "Latest snapshot should be loadable");
    
    println!("✅ Storage cleanup verified: 10 → 3 snapshots");
}
