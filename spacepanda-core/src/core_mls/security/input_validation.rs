//! Input Validation and Security Hardening
//!
//! Tests for:
//! - SQL injection prevention
//! - Buffer overflow protection
//! - Malformed input handling
//! - DoS resistance

use crate::core_mls::storage::SqlStorageProvider;
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_db_path(name: &str) -> String {
        format!(
            "/tmp/spacepanda_validation_{}_{}.db",
            name,
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        )
    }

    #[tokio::test]
    async fn test_sql_injection_prevention() {
        // Verify parameterized queries prevent SQL injection
        let db_path = temp_db_path("injection");
        let storage = SqlStorageProvider::new(&db_path).unwrap();

        // Attempt SQL injection in group_id
        let malicious_group_id = b"'; DROP TABLE channels; --";

        let result = storage
            .save_channel_metadata(malicious_group_id, b"name", None, b"members", 1)
            .await;

        // Should succeed (safely escape the input)
        assert!(result.is_ok(), "Parameterized queries should handle special chars");

        // Verify table still exists
        let channels = storage.list_channels(false).await.unwrap();

        // Clean up
        let _ = storage.delete_channel(malicious_group_id).await;
        let _ = std::fs::remove_file(&db_path);
    }

    #[tokio::test]
    async fn test_large_input_handling() {
        // Test handling of very large inputs
        let db_path = temp_db_path("large");
        let storage = SqlStorageProvider::new(&db_path).unwrap();

        let group_id = b"test_group";

        // Large encrypted blob (1 MB)
        let large_blob = vec![0u8; 1024 * 1024];

        let result = storage
            .save_channel_metadata(group_id, &large_blob, Some(&large_blob), &large_blob, 1)
            .await;

        // Should handle large blobs gracefully
        assert!(result.is_ok(), "Should handle large encrypted blobs");

        // Verify we can load it back
        let (loaded_name, _, _, _, _, _) = storage.load_channel_metadata(group_id).await.unwrap();

        assert_eq!(loaded_name.len(), large_blob.len());

        // Cleanup
        let _ = std::fs::remove_file(&db_path);
    }

    #[tokio::test]
    async fn test_empty_input_handling() {
        // Test handling of empty/minimal inputs
        let db_path = temp_db_path("empty");
        let storage = SqlStorageProvider::new(&db_path).unwrap();

        let group_id = b"";

        // Empty group ID should be handled
        let result = storage.save_channel_metadata(group_id, b"name", None, b"members", 1).await;

        // Empty group_id is valid (zero-length blob)
        assert!(result.is_ok(), "Should handle empty group_id");

        // Cleanup
        let _ = std::fs::remove_file(&db_path);
    }

    #[tokio::test]
    async fn test_null_byte_handling() {
        // Test handling of null bytes in binary data
        let db_path = temp_db_path("nullbyte");
        let storage = SqlStorageProvider::new(&db_path).unwrap();

        let group_id = b"test\0group\0with\0nulls";
        let data_with_nulls = b"data\0with\0null\0bytes";

        let result = storage
            .save_channel_metadata(
                group_id,
                data_with_nulls,
                Some(data_with_nulls),
                data_with_nulls,
                1,
            )
            .await;

        assert!(result.is_ok(), "Should handle null bytes in binary data");

        // Verify data round-trips correctly
        let (loaded_name, loaded_topic, _, _, _, _) =
            storage.load_channel_metadata(group_id).await.unwrap();

        assert_eq!(loaded_name, data_with_nulls);
        assert_eq!(loaded_topic, Some(data_with_nulls.to_vec()));

        // Cleanup
        let _ = std::fs::remove_file(&db_path);
    }

    #[tokio::test]
    async fn test_invalid_group_id_handling() {
        // Test error handling for invalid group lookups
        let db_path = temp_db_path("invalid");
        let storage = SqlStorageProvider::new(&db_path).unwrap();

        // Try to load non-existent group
        let result = storage.load_channel_metadata(b"nonexistent").await;
        assert!(result.is_err(), "Should return error for non-existent group");

        // Try to delete non-existent group
        let result = storage.delete_channel(b"nonexistent").await;
        assert!(result.is_err(), "Should return error when deleting non-existent group");

        // Cleanup
        let _ = std::fs::remove_file(&db_path);
    }

    #[tokio::test]
    async fn test_concurrent_write_safety() {
        // Test that concurrent writes don't corrupt data
        let db_path = temp_db_path("concurrent");
        let storage = SqlStorageProvider::new(&db_path).unwrap();

        let group_id = b"concurrent_group";

        // Create channel first
        storage
            .save_channel_metadata(group_id, b"name", None, b"members", 1)
            .await
            .unwrap();

        // Write messages concurrently (from same storage instance)
        let mut handles = vec![];
        for i in 0..10 {
            let db_path = db_path.clone();
            let handle = tokio::spawn(async move {
                let storage = SqlStorageProvider::new(&db_path).unwrap();
                for j in 0..10 {
                    let msg_id = format!("msg_{}_{}", i, j).into_bytes();
                    storage
                        .save_message(
                            &msg_id,
                            b"concurrent_group",
                            b"content",
                            b"sender",
                            (i * 10 + j) as i64,
                        )
                        .await
                        .unwrap();
                }
            });
            handles.push(handle);
        }

        // Wait for all to complete
        for handle in handles {
            handle.await.unwrap();
        }

        // Verify all messages were saved
        let messages = storage.load_messages(group_id, 1000, 0).await.unwrap();
        assert_eq!(messages.len(), 100, "All concurrent writes should succeed");

        // Cleanup
        let _ = std::fs::remove_file(&db_path);
    }

    #[tokio::test]
    async fn test_pagination_bounds() {
        // Test pagination with extreme values
        let db_path = temp_db_path("pagination");
        let storage = SqlStorageProvider::new(&db_path).unwrap();

        let group_id = b"pagination_group";
        storage
            .save_channel_metadata(group_id, b"name", None, b"members", 1)
            .await
            .unwrap();

        // Add some messages
        for i in 0..10 {
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

        // Test with large limit
        let result = storage.load_messages(group_id, 1000000, 0).await;
        assert!(result.is_ok(), "Should handle large limit");

        // Test with large offset
        let messages = storage.load_messages(group_id, 10, 1000000).await.unwrap();
        assert_eq!(messages.len(), 0, "Large offset should return empty results");

        // Test with zero limit
        let messages = storage.load_messages(group_id, 0, 0).await.unwrap();
        assert_eq!(messages.len(), 0, "Zero limit should return empty results");

        // Cleanup
        let _ = std::fs::remove_file(&db_path);
    }

    #[tokio::test]
    async fn test_malformed_binary_data() {
        // Test with random binary data
        let db_path = temp_db_path("malformed");
        let storage = SqlStorageProvider::new(&db_path).unwrap();

        // Random bytes
        let random_data: Vec<u8> = (0..1000).map(|i| (i % 256) as u8).collect();

        let result = storage
            .save_channel_metadata(
                &random_data[0..32],
                &random_data,
                Some(&random_data),
                &random_data,
                1,
            )
            .await;

        assert!(result.is_ok(), "Should handle arbitrary binary data");

        // Verify round-trip
        let (loaded, _, _, _, _, _) =
            storage.load_channel_metadata(&random_data[0..32]).await.unwrap();

        assert_eq!(loaded, random_data);

        // Cleanup
        let _ = std::fs::remove_file(&db_path);
    }

    #[tokio::test]
    async fn test_unicode_and_special_characters() {
        // Test with various Unicode and special characters
        let db_path = temp_db_path("unicode");
        let storage = SqlStorageProvider::new(&db_path).unwrap();

        let unicode_data = "Test with emoji 🚀🔒🌍 and symbols ©®™".as_bytes();
        let special_chars = b"<>&\"'`|;$()[]{}*?~!@#%^&-+=";

        let result = storage
            .save_channel_metadata(
                b"unicode_group",
                unicode_data,
                Some(special_chars),
                unicode_data,
                1,
            )
            .await;

        assert!(result.is_ok(), "Should handle Unicode and special chars");

        // Verify round-trip
        let (loaded_name, loaded_topic, _, _, _, _) =
            storage.load_channel_metadata(b"unicode_group").await.unwrap();

        assert_eq!(loaded_name, unicode_data);
        assert_eq!(loaded_topic, Some(special_chars.to_vec()));

        // Cleanup
        let _ = std::fs::remove_file(&db_path);
    }
}
