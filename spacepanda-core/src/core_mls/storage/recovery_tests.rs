//! Restart Recovery Tests
//!
//! Tests that verify data persistence and recovery across process restarts.
//! These tests simulate real-world scenarios where the application crashes or
//! is restarted, and verify that all state is correctly restored.

#[cfg(test)]
mod tests {
    use crate::core_mls::storage::sql_store::SqlStorageProvider;
    use crate::core_mls::traits::storage::{PersistedGroupSnapshot, StorageProvider};
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_mls_group_recovery_after_restart() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("recovery_test.db");

        // Phase 1: Initial setup and save
        {
            let storage = SqlStorageProvider::new(&db_path).unwrap();

            let snapshot = PersistedGroupSnapshot {
                group_id: b"recovery_group_1".to_vec(),
                epoch: 5,
                serialized_group: vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
            };

            storage.save_group_snapshot(snapshot.clone()).await.unwrap();

            // Verify save worked
            let loaded = storage.load_group_snapshot(&snapshot.group_id).await.unwrap();
            assert_eq!(loaded.epoch, 5);
            assert_eq!(loaded.serialized_group.len(), 10);

            // Drop storage to simulate shutdown
        }

        // Phase 2: "Restart" - create new instance pointing to same DB
        {
            let storage = SqlStorageProvider::new(&db_path).unwrap();

            // Verify data persisted across restart
            let loaded = storage.load_group_snapshot(&b"recovery_group_1".to_vec()).await.unwrap();
            assert_eq!(loaded.group_id, b"recovery_group_1");
            assert_eq!(loaded.epoch, 5);
            assert_eq!(loaded.serialized_group, vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);

            // Verify we can update after restart
            let updated_snapshot = PersistedGroupSnapshot {
                group_id: b"recovery_group_1".to_vec(),
                epoch: 6,
                serialized_group: vec![11, 12, 13],
            };

            storage.save_group_snapshot(updated_snapshot).await.unwrap();

            let loaded = storage.load_group_snapshot(&b"recovery_group_1".to_vec()).await.unwrap();
            assert_eq!(loaded.epoch, 6);
        }

        // Phase 3: Another restart to verify the update persisted
        {
            let storage = SqlStorageProvider::new(&db_path).unwrap();
            let loaded = storage.load_group_snapshot(&b"recovery_group_1".to_vec()).await.unwrap();
            assert_eq!(loaded.epoch, 6);
            assert_eq!(loaded.serialized_group, vec![11, 12, 13]);
        }
    }

    #[tokio::test]
    async fn test_channel_metadata_recovery() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("channel_recovery.db");

        let group_id = b"channel_group_123";
        let encrypted_name = b"encrypted_name_data";
        let encrypted_topic = b"encrypted_topic_data";
        let encrypted_members = b"encrypted_members_blob";

        // Phase 1: Create channels
        {
            let storage = SqlStorageProvider::new(&db_path).unwrap();

            // Create 3 channels
            for i in 0..3 {
                let gid = format!("channel_group_{}", i).into_bytes();
                storage
                    .save_channel_metadata(&gid, encrypted_name, Some(encrypted_topic), encrypted_members, 1)
                    .await
                    .unwrap();
            }

            // Verify channels exist
            let channels = storage.list_channels(false).await.unwrap();
            assert_eq!(channels.len(), 3);
        }

        // Phase 2: Restart and verify recovery
        {
            let storage = SqlStorageProvider::new(&db_path).unwrap();

            // All channels should still exist
            let channels = storage.list_channels(false).await.unwrap();
            assert_eq!(channels.len(), 3);

            // Verify channel data integrity
            let (name, topic, _, members, ch_type, archived) = storage
                .load_channel_metadata(b"channel_group_1")
                .await
                .unwrap();

            assert_eq!(name, encrypted_name);
            assert_eq!(topic, Some(encrypted_topic.to_vec()));
            assert_eq!(members, encrypted_members);
            assert_eq!(ch_type, 1);
            assert!(!archived);
        }
    }

    #[tokio::test]
    async fn test_message_history_recovery() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("message_recovery.db");

        let group_id = b"msg_recovery_group";

        // Phase 1: Create channel and messages
        {
            let storage = SqlStorageProvider::new(&db_path).unwrap();

            storage
                .save_channel_metadata(group_id, b"name", None, b"members", 1)
                .await
                .unwrap();

            // Save 100 messages
            for i in 0..100 {
                let msg_id = format!("msg_{}", i).into_bytes();
                let content = format!("encrypted_content_{}", i).into_bytes();
                let sender = format!("sender_hash_{}", i % 5).into_bytes();

                storage
                    .save_message(&msg_id, group_id, &content, &sender, i as i64)
                    .await
                    .unwrap();
            }

            // Verify messages exist
            let messages = storage.load_messages(group_id, 100, 0).await.unwrap();
            assert_eq!(messages.len(), 100);
        }

        // Phase 2: Restart and verify all messages recovered
        {
            let storage = SqlStorageProvider::new(&db_path).unwrap();

            // All messages should still exist
            let messages = storage.load_messages(group_id, 100, 0).await.unwrap();
            assert_eq!(messages.len(), 100);

            // Verify they're in correct order (reverse chronological)
            assert_eq!(messages[0].3, 99); // Most recent
            assert_eq!(messages[99].3, 0); // Oldest

            // Verify we can still paginate correctly
            let page1 = storage.load_messages(group_id, 10, 0).await.unwrap();
            let page2 = storage.load_messages(group_id, 10, 10).await.unwrap();

            assert_eq!(page1.len(), 10);
            assert_eq!(page2.len(), 10);
            assert_eq!(page1[0].3, 99);
            assert_eq!(page2[0].3, 89);
        }
    }

    #[tokio::test]
    async fn test_partial_transaction_recovery() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("transaction_recovery.db");

        // Phase 1: Atomic save with multiple groups
        {
            let storage = SqlStorageProvider::new(&db_path).unwrap();

            let snapshots = vec![
                PersistedGroupSnapshot {
                    group_id: b"tx_group_1".to_vec(),
                    epoch: 1,
                    serialized_group: vec![1, 2, 3],
                },
                PersistedGroupSnapshot {
                    group_id: b"tx_group_2".to_vec(),
                    epoch: 2,
                    serialized_group: vec![4, 5, 6],
                },
                PersistedGroupSnapshot {
                    group_id: b"tx_group_3".to_vec(),
                    epoch: 3,
                    serialized_group: vec![7, 8, 9],
                },
            ];

            storage.save_group_snapshots_atomic(&snapshots).await.unwrap();
        }

        // Phase 2: Restart and verify atomicity (all or nothing)
        {
            let storage = SqlStorageProvider::new(&db_path).unwrap();

            // All 3 groups should be present (transaction succeeded)
            let groups = storage.list_groups().await.unwrap();
            assert_eq!(groups.len(), 3);

            // Verify each group's data
            for i in 1..=3 {
                let gid = format!("tx_group_{}", i).into_bytes();
                let loaded = storage.load_group_snapshot(&gid).await.unwrap();
                assert_eq!(loaded.epoch, i);
            }
        }
    }

    #[tokio::test]
    async fn test_key_package_recovery() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("keypackage_recovery.db");

        let kp_id = b"kp_recovery_1";
        let kp_data = b"key_package_data_blob";
        let cred_id = b"credential_123";

        // Phase 1: Store key package
        {
            let storage = SqlStorageProvider::new(&db_path).unwrap();

            storage
                .store_key_package(kp_id, kp_data, cred_id, None)
                .await
                .unwrap();
        }

        // Phase 2: Restart and load key package (marks as used)
        {
            let storage = SqlStorageProvider::new(&db_path).unwrap();

            // First load will succeed and mark as used
            let loaded = storage.load_key_package(kp_id).await.unwrap();
            assert_eq!(loaded, kp_data);
        }

        // Phase 3: Restart and verify used state persisted
        {
            let storage = SqlStorageProvider::new(&db_path).unwrap();

            // Used key packages should not be loadable
            let result = storage.load_key_package(kp_id).await;
            assert!(result.is_err()); // Should fail to load used package
        }
    }

    #[tokio::test]
    async fn test_archive_state_recovery() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("archive_recovery.db");

        let group_id = b"archive_test_group";

        // Phase 1: Create and archive channel
        {
            let storage = SqlStorageProvider::new(&db_path).unwrap();

            storage
                .save_channel_metadata(group_id, b"name", None, b"members", 1)
                .await
                .unwrap();

            storage.archive_channel(group_id).await.unwrap();

            // Verify archived state
            let (_, _, _, _, _, archived) = storage.load_channel_metadata(group_id).await.unwrap();
            assert!(archived);
        }

        // Phase 2: Restart and verify archive state persisted
        {
            let storage = SqlStorageProvider::new(&db_path).unwrap();

            let (_, _, _, _, _, archived) = storage.load_channel_metadata(group_id).await.unwrap();
            assert!(archived);

            // Archived channels should not appear in default list
            let channels = storage.list_channels(false).await.unwrap();
            assert_eq!(channels.len(), 0);

            // But should appear when including archived
            let channels = storage.list_channels(true).await.unwrap();
            assert_eq!(channels.len(), 1);
        }
    }

    #[tokio::test]
    async fn test_processed_state_recovery() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("processed_recovery.db");

        let group_id = b"processed_test_group";

        // Phase 1: Create messages and mark some as processed
        {
            let storage = SqlStorageProvider::new(&db_path).unwrap();

            storage
                .save_channel_metadata(group_id, b"name", None, b"members", 1)
                .await
                .unwrap();

            // Save 10 messages
            for i in 0..10 {
                let msg_id = format!("msg_{}", i).into_bytes();
                storage
                    .save_message(&msg_id, group_id, b"content", b"sender", i)
                    .await
                    .unwrap();
            }

            // Mark first 5 as processed
            for i in 0..5 {
                let msg_id = format!("msg_{}", i).into_bytes();
                storage.mark_message_processed(&msg_id).await.unwrap();
            }

            // Verify unprocessed count
            let count = storage.get_unprocessed_count(group_id).await.unwrap();
            assert_eq!(count, 5);
        }

        // Phase 2: Restart and verify processed state persisted
        {
            let storage = SqlStorageProvider::new(&db_path).unwrap();

            let count = storage.get_unprocessed_count(group_id).await.unwrap();
            assert_eq!(count, 5); // Still 5 unprocessed

            // Mark rest as processed
            for i in 5..10 {
                let msg_id = format!("msg_{}", i).into_bytes();
                storage.mark_message_processed(&msg_id).await.unwrap();
            }

            let count = storage.get_unprocessed_count(group_id).await.unwrap();
            assert_eq!(count, 0);
        }

        // Phase 3: Final restart to verify all processed
        {
            let storage = SqlStorageProvider::new(&db_path).unwrap();

            let count = storage.get_unprocessed_count(group_id).await.unwrap();
            assert_eq!(count, 0);
        }
    }

    #[tokio::test]
    async fn test_multiple_restarts_with_operations() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("multi_restart.db");

        // Simulate 5 restarts with operations between each
        for restart_num in 0..5 {
            let storage = SqlStorageProvider::new(&db_path).unwrap();

            // Add a new channel each restart
            let group_id = format!("restart_{}_group", restart_num).into_bytes();
            storage
                .save_channel_metadata(&group_id, b"name", None, b"members", 1)
                .await
                .unwrap();

            // Verify all previous channels still exist
            let channels = storage.list_channels(false).await.unwrap();
            assert_eq!(channels.len(), restart_num + 1);

            // Add messages to all existing channels
            for i in 0..=restart_num {
                let gid = format!("restart_{}_group", i).into_bytes();
                let msg_id = format!("restart_{}_msg_{}", i, restart_num).into_bytes();

                storage
                    .save_message(&msg_id, &gid, b"content", b"sender", restart_num as i64)
                    .await
                    .unwrap();
            }
        }

        // Final verification
        let storage = SqlStorageProvider::new(&db_path).unwrap();

        let channels = storage.list_channels(false).await.unwrap();
        assert_eq!(channels.len(), 5);

        // Each channel should have increasing number of messages
        for i in 0..5 {
            let gid = format!("restart_{}_group", i).into_bytes();
            let messages = storage.load_messages(&gid, 100, 0).await.unwrap();
            assert_eq!(messages.len(), 5 - i); // First channel has 5, last has 1
        }
    }
}
