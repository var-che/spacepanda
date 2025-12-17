//! Stress Tests for SQL Storage
//!
//! Tests performance and correctness with large datasets, high load,
//! and concurrent operations.

#[cfg(test)]
mod tests {
    use crate::core_mls::storage::sql_store::SqlStorageProvider;
    use crate::core_mls::traits::storage::{PersistedGroupSnapshot, StorageProvider};
    use std::time::Instant;
    use tempfile::tempdir;
    use tokio::task::JoinSet;

    #[tokio::test]
    async fn test_large_message_history() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("large_history.db");
        let storage = SqlStorageProvider::new(&db_path).unwrap();

        let group_id = b"large_history_group";

        // Create channel
        storage
            .save_channel_metadata(group_id, b"name", None, b"members", 1)
            .await
            .unwrap();

        // Save 10,000 messages
        let start = Instant::now();
        for i in 0..10_000 {
            let msg_id = format!("msg_{}", i).into_bytes();
            let content = format!("encrypted_content_{}", i).into_bytes();
            let sealed_sender_bytes = format!("sender_hash_{}", i % 10).into_bytes();

            storage
                .save_message(&msg_id, group_id, &content, &sealed_sender_bytes, i as i64)
                .await
                .unwrap();
        }
        let write_duration = start.elapsed();
        println!("Wrote 10,000 messages in {:?}", write_duration);

        // Test pagination performance
        let start = Instant::now();
        let messages = storage.load_messages(group_id, 100, 0).await.unwrap();
        let read_duration = start.elapsed();

        assert_eq!(messages.len(), 100);
        assert_eq!(messages[0].3, 9999); // Most recent
        println!("Read first page (100) in {:?}", read_duration);

        // Test middle page
        let start = Instant::now();
        let messages = storage.load_messages(group_id, 100, 5000).await.unwrap();
        let mid_page_duration = start.elapsed();

        assert_eq!(messages.len(), 100);
        assert_eq!(messages[0].3, 4999);
        println!("Read middle page (100) in {:?}", mid_page_duration);

        // Test unprocessed count performance
        let start = Instant::now();
        let count = storage.get_unprocessed_count(group_id).await.unwrap();
        let count_duration = start.elapsed();

        assert_eq!(count, 10_000);
        println!("Counted 10,000 unprocessed in {:?}", count_duration);

        // Test pruning performance
        let start = Instant::now();
        let deleted = storage.prune_old_messages(group_id, 1000).await.unwrap();
        let prune_duration = start.elapsed();

        assert_eq!(deleted, 9_000);
        println!("Pruned 9,000 messages in {:?}", prune_duration);

        // Verify only 1000 remain
        let messages = storage.load_messages(group_id, 10_000, 0).await.unwrap();
        assert_eq!(messages.len(), 1000);
        assert_eq!(messages[0].3, 9999); // Most recent still there
        assert_eq!(messages[999].3, 9000); // Oldest kept
    }

    #[tokio::test]
    async fn test_many_channels() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("many_channels.db");
        let storage = SqlStorageProvider::new(&db_path).unwrap();

        // Create 1,000 channels
        let start = Instant::now();
        for i in 0..1_000 {
            let group_id = format!("channel_{}", i).into_bytes();
            storage
                .save_channel_metadata(&group_id, b"name", None, b"members", 1)
                .await
                .unwrap();
        }
        let create_duration = start.elapsed();
        println!("Created 1,000 channels in {:?}", create_duration);

        // List all channels
        let start = Instant::now();
        let channels = storage.list_channels(false).await.unwrap();
        let list_duration = start.elapsed();

        assert_eq!(channels.len(), 1000);
        println!("Listed 1,000 channels in {:?}", list_duration);

        // Add messages to each channel
        let start = Instant::now();
        for i in 0..1_000 {
            let group_id = format!("channel_{}", i).into_bytes();
            for j in 0..10 {
                let msg_id = format!("msg_{}_{}", i, j).into_bytes();
                storage
                    .save_message(&msg_id, &group_id, b"content", b"sender", j)
                    .await
                    .unwrap();
            }
        }
        let msg_duration = start.elapsed();
        println!("Added 10 messages to each of 1,000 channels in {:?}", msg_duration);

        // Archive half the channels
        let start = Instant::now();
        for i in 0..500 {
            let group_id = format!("channel_{}", i).into_bytes();
            storage.archive_channel(&group_id).await.unwrap();
        }
        let archive_duration = start.elapsed();
        println!("Archived 500 channels in {:?}", archive_duration);

        // Verify only 500 active
        let channels = storage.list_channels(false).await.unwrap();
        assert_eq!(channels.len(), 500);
    }

    #[tokio::test]
    async fn test_large_group_snapshots() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("large_snapshots.db");
        let storage = SqlStorageProvider::new(&db_path).unwrap();

        // Create 1,000 groups with large serialized data
        let start = Instant::now();
        for i in 0..1_000 {
            let group_id = format!("group_{}", i).into_bytes();
            let large_data = vec![0u8; 10_000]; // 10KB per group

            let snapshot =
                PersistedGroupSnapshot { group_id, epoch: i as u64, serialized_group: large_data };

            storage.save_group_snapshot(snapshot).await.unwrap();
        }
        let save_duration = start.elapsed();
        println!("Saved 1,000 groups (10KB each) in {:?}", save_duration);

        // List all groups
        let start = Instant::now();
        let groups = storage.list_groups().await.unwrap();
        let list_duration = start.elapsed();

        assert_eq!(groups.len(), 1000);
        println!("Listed 1,000 groups in {:?}", list_duration);

        // Load a specific group
        let start = Instant::now();
        let loaded = storage.load_group_snapshot(&b"group_500".to_vec()).await.unwrap();
        let load_duration = start.elapsed();

        assert_eq!(loaded.serialized_group.len(), 10_000);
        println!("Loaded 10KB group snapshot in {:?}", load_duration);
    }

    #[tokio::test]
    async fn test_bulk_message_processing() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("bulk_processing.db");
        let storage = SqlStorageProvider::new(&db_path).unwrap();

        let group_id = b"bulk_test_group";

        storage
            .save_channel_metadata(group_id, b"name", None, b"members", 1)
            .await
            .unwrap();

        // Save 5,000 messages
        for i in 0..5_000 {
            let msg_id = format!("msg_{}", i).into_bytes();
            storage.save_message(&msg_id, group_id, b"content", b"sender", i).await.unwrap();
        }

        // Mark all as processed
        let start = Instant::now();
        for i in 0..5_000 {
            let msg_id = format!("msg_{}", i).into_bytes();
            storage.mark_message_processed(&msg_id).await.unwrap();
        }
        let process_duration = start.elapsed();
        println!("Marked 5,000 messages as processed in {:?}", process_duration);

        // Verify all processed
        let count = storage.get_unprocessed_count(group_id).await.unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_atomic_batch_operations() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("batch_ops.db");
        let storage = SqlStorageProvider::new(&db_path).unwrap();

        // Create 100 snapshots
        let mut snapshots = Vec::new();
        for i in 0..100 {
            snapshots.push(PersistedGroupSnapshot {
                group_id: format!("batch_group_{}", i).into_bytes(),
                epoch: i as u64,
                serialized_group: vec![i as u8; 1000],
            });
        }

        // Save all atomically
        let start = Instant::now();
        storage.save_group_snapshots_atomic(&snapshots).await.unwrap();
        let batch_save_duration = start.elapsed();
        println!("Saved 100 snapshots atomically in {:?}", batch_save_duration);

        // Verify all saved
        let groups = storage.list_groups().await.unwrap();
        assert_eq!(groups.len(), 100);

        // Delete all atomically
        let group_ids: Vec<_> = snapshots.iter().map(|s| s.group_id.clone()).collect();
        let start = Instant::now();
        storage.delete_groups_atomic(&group_ids).await.unwrap();
        let batch_delete_duration = start.elapsed();
        println!("Deleted 100 snapshots atomically in {:?}", batch_delete_duration);

        // Verify all deleted
        let groups = storage.list_groups().await.unwrap();
        assert_eq!(groups.len(), 0);
    }

    #[tokio::test]
    async fn test_pagination_correctness_large_dataset() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("pagination_test.db");
        let storage = SqlStorageProvider::new(&db_path).unwrap();

        let group_id = b"pagination_group";

        storage
            .save_channel_metadata(group_id, b"name", None, b"members", 1)
            .await
            .unwrap();

        // Save 1,000 messages with known sequence
        for i in 0..1_000 {
            let msg_id = format!("msg_{:04}", i).into_bytes();
            storage.save_message(&msg_id, group_id, b"content", b"sender", i).await.unwrap();
        }

        // Paginate through all messages and verify order
        let page_size = 50;
        let mut all_sequences = Vec::new();

        for page in 0..20 {
            let messages =
                storage.load_messages(group_id, page_size, page * page_size).await.unwrap();

            for msg in messages {
                all_sequences.push(msg.3); // sequence number
            }
        }

        // Verify we got all 1000 messages in reverse chronological order
        assert_eq!(all_sequences.len(), 1000);
        assert_eq!(all_sequences[0], 999);
        assert_eq!(all_sequences[999], 0);

        // Verify strictly decreasing
        for i in 0..all_sequences.len() - 1 {
            assert!(all_sequences[i] > all_sequences[i + 1]);
        }
    }

    #[tokio::test]
    async fn test_database_size_estimation() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("size_test.db");
        let storage = SqlStorageProvider::new(&db_path).unwrap();

        // Add known amount of data
        let group_id = b"size_test_group";
        storage
            .save_channel_metadata(group_id, b"name", None, b"members", 1)
            .await
            .unwrap();

        // Add 1,000 messages with 1KB content each
        for i in 0..1_000 {
            let msg_id = format!("msg_{}", i).into_bytes();
            let content = vec![0u8; 1024]; // 1KB
            storage.save_message(&msg_id, group_id, &content, b"sender", i).await.unwrap();
        }

        // Check database file size
        let metadata = std::fs::metadata(&db_path).unwrap();
        let size_mb = metadata.len() as f64 / 1_048_576.0;
        println!("Database size with 1,000 1KB messages: {:.2} MB", size_mb);

        // Size should be reasonable (< 5MB for 1MB of data plus overhead)
        assert!(size_mb < 5.0);
    }

    #[tokio::test]
    async fn test_concurrent_channel_operations() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("concurrent_channels.db");
        let storage = std::sync::Arc::new(SqlStorageProvider::new(&db_path).unwrap());

        let mut tasks = JoinSet::new();

        // Spawn 10 tasks, each creating 10 channels
        for task_id in 0..10 {
            let storage = storage.clone();
            tasks.spawn(async move {
                for i in 0..10 {
                    let group_id = format!("task_{}_channel_{}", task_id, i).into_bytes();
                    storage
                        .save_channel_metadata(&group_id, b"name", None, b"members", 1)
                        .await
                        .unwrap();
                }
            });
        }

        // Wait for all tasks
        while tasks.join_next().await.is_some() {}

        // Verify all 100 channels were created
        let channels = storage.list_channels(false).await.unwrap();
        assert_eq!(channels.len(), 100);
    }

    #[tokio::test]
    async fn test_concurrent_message_writes() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("concurrent_messages.db");
        let storage = std::sync::Arc::new(SqlStorageProvider::new(&db_path).unwrap());

        let group_id = b"concurrent_msg_group";

        // Create channel first
        storage
            .save_channel_metadata(group_id, b"name", None, b"members", 1)
            .await
            .unwrap();

        let mut tasks = JoinSet::new();

        // Spawn 10 tasks, each writing 100 messages
        for task_id in 0..10 {
            let storage = storage.clone();
            let group_id = group_id.to_vec();

            tasks.spawn(async move {
                for i in 0..100 {
                    let msg_id = format!("task_{}_msg_{}", task_id, i).into_bytes();
                    let sequence = (task_id * 100 + i) as i64;

                    storage
                        .save_message(&msg_id, &group_id, b"content", b"sender", sequence)
                        .await
                        .unwrap();
                }
            });
        }

        // Wait for all tasks
        while tasks.join_next().await.is_some() {}

        // Verify all 1000 messages were written
        let messages = storage.load_messages(group_id, 1000, 0).await.unwrap();
        assert_eq!(messages.len(), 1000);
    }
}
