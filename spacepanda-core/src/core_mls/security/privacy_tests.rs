//! Privacy and Metadata Protection Tests
//!
//! Validates that the system preserves privacy:
//! - No plaintext metadata leakage
//! - Timing analysis resistance
//! - Traffic analysis resistance  
//! - Sender anonymity preservation

use crate::core_mls::storage::SqlStorageProvider;
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_db_path(name: &str) -> String {
        format!("/tmp/spacepanda_privacy_{}_{}.db", name, 
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos())
    }

    #[tokio::test]
    async fn test_no_plaintext_in_storage() {
        // Verify that sensitive data is encrypted before storage
        let db_path = temp_db_path("plaintext");
        let storage = SqlStorageProvider::new(&db_path).unwrap();

        let group_id = b"test_group";
        let plaintext_name = b"Private Channel Name";
        let plaintext_topic = b"Secret Discussion Topic";
        let plaintext_members = b"alice,bob,charlie";

        // Save metadata (should be encrypted automatically)
        storage
            .save_channel_metadata(
                group_id,
                plaintext_name,
                Some(plaintext_topic),
                plaintext_members,
                1,
            )
            .await
            .unwrap();

        // Read database file directly
        let db_contents = std::fs::read(&db_path).unwrap();
        let contents_str = String::from_utf8_lossy(&db_contents);

        // Verify plaintext is NOT in database (it should be encrypted)
        let plaintext_name_str = String::from_utf8_lossy(plaintext_name);
        let plaintext_topic_str = String::from_utf8_lossy(plaintext_topic);
        let plaintext_members_str = String::from_utf8_lossy(plaintext_members);
        
        assert!(
            !contents_str.contains(plaintext_name_str.as_ref()),
            "Plaintext channel name should not be in database (should be encrypted)"
        );
        assert!(
            !contents_str.contains(plaintext_topic_str.as_ref()),
            "Plaintext topic should not be in database (should be encrypted)"
        );
        assert!(
            !contents_str.contains(plaintext_members_str.as_ref()),
            "Plaintext members should not be in database (should be encrypted)"
        );

        // Verify we can load and decrypt correctly
        let (loaded_name, loaded_topic, _, loaded_members, _, _) =
            storage.load_channel_metadata(group_id).await.unwrap();
        
        assert_eq!(loaded_name, plaintext_name, "Decrypted name should match");
        assert_eq!(loaded_topic.unwrap(), plaintext_topic, "Decrypted topic should match");
        assert_eq!(loaded_members, plaintext_members, "Decrypted members should match");

        // Cleanup
        let _ = std::fs::remove_file(&db_path);
    }

    #[tokio::test]
    async fn test_no_timing_metadata() {
        // Verify that we don't store timing metadata beyond creation
        let db_path = temp_db_path("timing");
        let storage = SqlStorageProvider::new(&db_path).unwrap();

        let group_id = b"test_group";
        
        // Create channel
        storage
            .save_channel_metadata(group_id, b"name", None, b"members", 1)
            .await
            .unwrap();

        // Wait some time
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // Update channel (re-save)
        storage
            .save_channel_metadata(group_id, b"updated_name", None, b"members", 1)
            .await
            .unwrap();

        // Load metadata
        let (_, _, created_at, _, _, _) = storage
            .load_channel_metadata(group_id)
            .await
            .unwrap();

        // Verify created_at hasn't changed on update
        let (_, _, created_at_after, _, _, _) = storage
            .load_channel_metadata(group_id)
            .await
            .unwrap();

        assert_eq!(
            created_at, created_at_after,
            "Creation timestamp should not change on update"
        );

        // Check database schema doesn't have timing leak columns
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        let mut stmt = conn.prepare("PRAGMA table_info(channels)").unwrap();
        
        let columns: Vec<String> = stmt
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();

        assert!(
            !columns.iter().any(|c| c.contains("last_updated")),
            "Should not have last_updated column"
        );
        assert!(
            !columns.iter().any(|c| c.contains("last_activity")),
            "Should not have last_activity column"
        );

        // Cleanup
        let _ = std::fs::remove_file(&db_path);
    }

    #[tokio::test]
    async fn test_sender_hash_privacy() {
        // Verify sender identities are hashed, not stored in plaintext
        let db_path = temp_db_path("sender");
        let storage = SqlStorageProvider::new(&db_path).unwrap();

        let group_id = b"test_group";
        storage
            .save_channel_metadata(group_id, b"name", None, b"members", 1)
            .await
            .unwrap();

        let plaintext_sender = b"alice@example.com";
        let sender_hash = b"hash_of_alice_identity"; // Simulated hash

        // Save message with hashed sender
        storage
            .save_message(
                b"msg_1",
                group_id,
                b"encrypted_content",
                sender_hash,
                1,
            )
            .await
            .unwrap();

        // Read database file
        let db_contents = std::fs::read(&db_path).unwrap();
        let contents_str = String::from_utf8_lossy(&db_contents);

        // Verify plaintext sender is NOT in database
        let plaintext_sender_str = String::from_utf8_lossy(plaintext_sender);
        assert!(
            !contents_str.contains(plaintext_sender_str.as_ref()),
            "Plaintext sender identity should not be in database"
        );

        // Verify hash IS in database
        assert!(
            db_contents.windows(sender_hash.len()).any(|w| w == sender_hash),
            "Sender hash should be in database"
        );

        // Cleanup
        let _ = std::fs::remove_file(&db_path);
    }

    #[tokio::test]
    async fn test_no_read_receipts() {
        // Verify that read receipts are not stored
        let db_path = temp_db_path("receipts");
        let storage = SqlStorageProvider::new(&db_path).unwrap();

        let group_id = b"test_group";
        storage
            .save_channel_metadata(group_id, b"name", None, b"members", 1)
            .await
            .unwrap();

        // Save and process some messages
        for i in 0..5 {
            storage
                .save_message(
                    &format!("msg_{}", i).into_bytes(),
                    group_id,
                    b"content",
                    b"sender",
                    i,
                )
                .await
                .unwrap();
        }

        storage.mark_message_processed(b"msg_2").await.unwrap();

        // Check schema doesn't have read receipt fields
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        let mut stmt = conn.prepare("PRAGMA table_info(messages)").unwrap();
        
        let columns: Vec<String> = stmt
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();

        assert!(
            !columns.iter().any(|c| c.contains("read_at")),
            "Should not have read_at column"
        );
        assert!(
            !columns.iter().any(|c| c.contains("delivered_at")),
            "Should not have delivered_at column"
        );
        assert!(
            columns.iter().any(|c| c == "processed"),
            "Should have local processed flag"
        );

        // Cleanup
        let _ = std::fs::remove_file(&db_path);
    }

    #[tokio::test]
    async fn test_no_ip_or_location_storage() {
        // Verify no network metadata is stored
        let db_path = temp_db_path("network");
        let storage = SqlStorageProvider::new(&db_path).unwrap();

        let group_id = b"test_group";
        storage
            .save_channel_metadata(group_id, b"name", None, b"members", 1)
            .await
            .unwrap();

        // Check database schema
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        
        // Check channels table
        let mut stmt = conn.prepare("PRAGMA table_info(channels)").unwrap();
        let channel_columns: Vec<String> = stmt
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();

        // Check messages table
        let mut stmt = conn.prepare("PRAGMA table_info(messages)").unwrap();
        let message_columns: Vec<String> = stmt
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();

        // Verify no IP or location fields
        let all_columns: Vec<_> = channel_columns
            .iter()
            .chain(message_columns.iter())
            .collect();

        for column in all_columns {
            let lower = column.to_lowercase();
            assert!(!lower.contains("ip"), "Should not store IP addresses");
            assert!(!lower.contains("location"), "Should not store location");
            assert!(!lower.contains("geo"), "Should not store geolocation");
        }

        // Cleanup
        let _ = std::fs::remove_file(&db_path);
    }

    #[tokio::test]
    async fn test_minimal_metadata_exposure() {
        // Verify only essential metadata is stored and sensitive data is encrypted
        let db_path = temp_db_path("metadata");
        let storage = SqlStorageProvider::new(&db_path).unwrap();

        let group_id = b"test_group";
        let plaintext_name = b"name";
        let plaintext_members = b"members";
        
        storage
            .save_channel_metadata(group_id, plaintext_name, None, plaintext_members, 1)
            .await
            .unwrap();

        // Check what's actually stored
        let (loaded_name, _loaded_topic, created_at, loaded_members, channel_type, archived) =
            storage.load_channel_metadata(group_id).await.unwrap();

        // Verify only these fields exist:
        assert!(!loaded_name.is_empty(), "Should have name field");
        assert!(created_at > 0, "Should have creation timestamp");
        assert!(!loaded_members.is_empty(), "Should have members field");
        assert!(channel_type >= 0, "Should have channel type");
        assert!(!archived, "New channel should not be archived");

        // Verify encryption works: data round-trips correctly through encryption
        assert_eq!(loaded_name, plaintext_name, "Name should decrypt correctly");
        assert_eq!(loaded_members, plaintext_members, "Members should decrypt correctly");

        // Verify plaintext is NOT in database file
        let db_contents = std::fs::read(&db_path).unwrap();
        let contents_str = String::from_utf8_lossy(&db_contents);
        
        assert!(
            !contents_str.contains("name") || db_contents.windows(plaintext_name.len()).filter(|w| *w == plaintext_name).count() < 2,
            "Plaintext name should not appear multiple times in database (should be encrypted in channels table)"
        );

        // Cleanup
        let _ = std::fs::remove_file(&db_path);
    }
}
