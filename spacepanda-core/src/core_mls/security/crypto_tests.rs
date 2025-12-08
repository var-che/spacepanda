//! Cryptographic Security Tests
//!
//! Comprehensive tests for cryptographic primitives and key management:
//! - Key package expiration
//! - Key package lifecycle
//! - Storage isolation

use crate::core_mls::storage::SqlStorageProvider;
use crate::core_mls::traits::storage::StorageProvider;

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_db_path(name: &str) -> String {
        format!("/tmp/spacepanda_crypto_test_{}_{}.db", name, 
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos())
    }

    #[tokio::test]
    async fn test_key_package_expiration() {
        // Verify expired key packages are not used
        let db_path = temp_db_path("expiration");
        let storage = SqlStorageProvider::new(&db_path).unwrap();

        // Store key package with past expiration
        let kp_id = b"expired_key_package";
        let kp_data = b"key_package_data";
        let cred_id = b"credential";
        let expired_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
            - 3600; // 1 hour ago

        storage
            .store_key_package(kp_id, kp_data, cred_id, Some(expired_at))
            .await
            .unwrap();

        // Try to load expired key package (should fail)
        let result = storage.load_key_package(kp_id).await;
        assert!(
            result.is_err(),
            "Expired key packages should not be loadable"
        );

        // Cleanup should remove it
        let removed = storage.cleanup_expired_key_packages().unwrap();
        assert_eq!(removed, 1, "Should cleanup 1 expired key package");

        // Cleanup
        let _ = std::fs::remove_file(&db_path);
    }

    #[tokio::test]
    async fn test_key_package_uniqueness() {
        // Verify different key packages have different IDs
        let db_path = temp_db_path("uniqueness");
        let storage = SqlStorageProvider::new(&db_path).unwrap();

        // Store multiple key packages
        for i in 0..10 {
            let kp_id = format!("kp_{}", i).into_bytes();
            let kp_data = format!("data_{}", i).into_bytes();
            storage
                .store_key_package(&kp_id, &kp_data, b"cred", None)
                .await
                .unwrap();
        }

        // Verify all are stored
        for i in 0..10 {
            let kp_id = format!("kp_{}", i).into_bytes();
            let loaded = storage.load_key_package(&kp_id).await.unwrap();
            assert!(!loaded.is_empty());
        }

        // Cleanup
        let _ = std::fs::remove_file(&db_path);
    }

    #[tokio::test]
    async fn test_key_package_lifecycle() {
        // Test full lifecycle: store, load, mark used, cleanup
        let db_path = temp_db_path("lifecycle");
        let storage = SqlStorageProvider::new(&db_path).unwrap();

        let kp_id = b"lifecycle_kp";
        let kp_data = b"key_package_data";

        // Store
        storage
            .store_key_package(kp_id, kp_data, b"cred", None)
            .await
            .unwrap();

        // Load (automatically marks as used)
        let loaded = storage.load_key_package(kp_id).await.unwrap();
        assert_eq!(loaded, kp_data);

        // Try to load again (should fail - already used)
        let result = storage.load_key_package(kp_id).await;
        assert!(result.is_err(), "Used key packages should not be reloadable");

        // Cleanup
        let _ = std::fs::remove_file(&db_path);
    }

    #[tokio::test]
    async fn test_group_snapshot_isolation() {
        // Verify different groups are isolated
        let db_path = temp_db_path("isolation");
        let storage = SqlStorageProvider::new(&db_path).unwrap();

        use crate::core_mls::traits::storage::PersistedGroupSnapshot;

        // Store snapshots for different groups
        let snapshot1 = PersistedGroupSnapshot {
            group_id: b"group_1".to_vec(),
            epoch: 1,
            serialized_group: vec![1, 2, 3],
        };

        let snapshot2 = PersistedGroupSnapshot {
            group_id: b"group_2".to_vec(),
            epoch: 2,
            serialized_group: vec![4, 5, 6],
        };

        storage.save_group_snapshot(snapshot1.clone()).await.unwrap();
        storage.save_group_snapshot(snapshot2.clone()).await.unwrap();

        // Verify isolation
        let loaded1 = storage.load_group_snapshot(&snapshot1.group_id).await.unwrap();
        let loaded2 = storage.load_group_snapshot(&snapshot2.group_id).await.unwrap();

        assert_eq!(loaded1.epoch, 1);
        assert_eq!(loaded2.epoch, 2);
        assert_ne!(loaded1.serialized_group, loaded2.serialized_group);

        // Cleanup
        let _ = std::fs::remove_file(&db_path);
    }

    #[tokio::test]
    async fn test_storage_persistence() {
        // Verify data persists across storage instances
        let db_path = temp_db_path("persistence");
        
        {
            let storage = SqlStorageProvider::new(&db_path).unwrap();

            storage
                .store_key_package(b"persist_kp", b"data", b"cred", None)
                .await
                .unwrap();
        } // Drop storage

        // Reopen database
        {
            let storage = SqlStorageProvider::new(&db_path).unwrap();

            // Data should still be there
            let loaded = storage.load_key_package(b"persist_kp").await.unwrap();
            assert_eq!(loaded, b"data");
        }

        // Cleanup
        let _ = std::fs::remove_file(&db_path);
    }
}
