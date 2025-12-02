//! File-based Storage Provider
//!
//! Implements `StorageProvider` using encrypted file storage.
//! Bridges to the existing FileKeystore infrastructure.

use crate::core_mls::errors::{MlsError, MlsResult};
use crate::core_mls::traits::storage::{GroupId, PersistedGroupSnapshot, StorageProvider};
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use argon2::Argon2;
use rand::RngCore;
use zeroize::Zeroizing;

/// Magic header for encrypted snapshot files
const MAGIC_HEADER: &[u8; 8] = b"MLSS0001";

/// Current snapshot format version
const FORMAT_VERSION: u8 = 1;

/// Salt length for Argon2 KDF
const SALT_LEN: usize = 16;

/// Nonce length for AES-GCM
const NONCE_LEN: usize = 12;

/// File-based storage provider with encryption at rest
pub struct FileStorageProvider {
    /// Base directory for snapshots
    base_path: PathBuf,
    /// Optional encryption password
    password: Option<Zeroizing<String>>,
    /// In-memory cache for faster access
    cache: Arc<RwLock<HashMap<Vec<u8>, PersistedGroupSnapshot>>>,
}

impl FileStorageProvider {
    /// Create a new file storage provider
    ///
    /// # Arguments
    /// * `base_path` - Directory to store snapshots
    /// * `password` - Optional password for encryption at rest
    pub fn new(base_path: PathBuf, password: Option<&str>) -> MlsResult<Self> {
        // Create directory if it doesn't exist
        std::fs::create_dir_all(&base_path).map_err(|e| {
            MlsError::Storage(format!("Failed to create storage directory: {}", e))
        })?;

        Ok(Self {
            base_path,
            password: password.map(|s| Zeroizing::new(s.to_string())),
            cache: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Get file path for a group snapshot
    fn snapshot_path(&self, group_id: &GroupId) -> PathBuf {
        let hex_id = hex::encode(group_id);
        self.base_path.join(format!("group-{}.snapshot", hex_id))
    }

    /// Get file path for a blob
    fn blob_path(&self, key: &str) -> PathBuf {
        self.base_path.join(format!("{}.blob", key))
    }

    /// Encrypt data with AES-256-GCM
    fn encrypt(&self, data: &[u8]) -> MlsResult<Vec<u8>> {
        if let Some(password) = &self.password {
            // Generate random salt
            let mut salt = [0u8; SALT_LEN];
            rand::thread_rng().fill_bytes(&mut salt);

            // Derive key from password
            let mut key = Zeroizing::new([0u8; 32]);
            Argon2::default()
                .hash_password_into(password.as_bytes(), &salt, &mut *key)
                .map_err(|e| MlsError::Storage(format!("Key derivation failed: {}", e)))?;

            // Generate random nonce
            let mut nonce_bytes = [0u8; NONCE_LEN];
            rand::thread_rng().fill_bytes(&mut nonce_bytes);
            let nonce = Nonce::from_slice(&nonce_bytes);

            // Encrypt
            let cipher = Aes256Gcm::new_from_slice(&key)
                .map_err(|e| MlsError::Storage(format!("Invalid key: {}", e)))?;

            let ciphertext = cipher
                .encrypt(nonce, data)
                .map_err(|e| MlsError::Storage(format!("Encryption failed: {}", e)))?;

            // Build: [magic][version][salt][nonce][ciphertext+tag]
            let mut result = Vec::with_capacity(8 + 1 + SALT_LEN + NONCE_LEN + ciphertext.len());
            result.extend_from_slice(MAGIC_HEADER);
            result.push(FORMAT_VERSION);
            result.extend_from_slice(&salt);
            result.extend_from_slice(&nonce_bytes);
            result.extend_from_slice(&ciphertext);

            Ok(result)
        } else {
            // No encryption - just add header
            let mut result = Vec::with_capacity(9 + data.len());
            result.extend_from_slice(MAGIC_HEADER);
            result.push(FORMAT_VERSION);
            result.extend_from_slice(data);
            Ok(result)
        }
    }

    /// Decrypt data with AES-256-GCM
    fn decrypt(&self, encrypted: &[u8]) -> MlsResult<Vec<u8>> {
        // Verify magic header
        if encrypted.len() < 9 || &encrypted[0..8] != MAGIC_HEADER {
            return Err(MlsError::Storage("Invalid snapshot format".to_string()));
        }

        let version = encrypted[8];
        if version != FORMAT_VERSION {
            return Err(MlsError::Storage(format!(
                "Unsupported format version: {}",
                version
            )));
        }

        if let Some(password) = &self.password {
            // Encrypted format
            if encrypted.len() < 8 + 1 + SALT_LEN + NONCE_LEN {
                return Err(MlsError::Storage("Truncated encrypted data".to_string()));
            }

            let salt = &encrypted[9..9 + SALT_LEN];
            let nonce_bytes = &encrypted[9 + SALT_LEN..9 + SALT_LEN + NONCE_LEN];
            let ciphertext = &encrypted[9 + SALT_LEN + NONCE_LEN..];

            // Derive key
            let mut key = Zeroizing::new([0u8; 32]);
            Argon2::default()
                .hash_password_into(password.as_bytes(), salt, &mut *key)
                .map_err(|e| MlsError::Storage(format!("Key derivation failed: {}", e)))?;

            // Decrypt
            let cipher = Aes256Gcm::new_from_slice(&key)
                .map_err(|e| MlsError::Storage(format!("Invalid key: {}", e)))?;

            let nonce = Nonce::from_slice(nonce_bytes);
            let plaintext = cipher
                .decrypt(nonce, ciphertext)
                .map_err(|e| MlsError::Storage(format!("Decryption failed: {}", e)))?;

            Ok(plaintext)
        } else {
            // No encryption - just strip header
            Ok(encrypted[9..].to_vec())
        }
    }
}

#[async_trait]
impl StorageProvider for FileStorageProvider {
    async fn save_group_snapshot(&self, snapshot: PersistedGroupSnapshot) -> MlsResult<()> {
        // Serialize snapshot
        let serialized = bincode::serialize(&snapshot)
            .map_err(|e| MlsError::Storage(format!("Serialization failed: {}", e)))?;

        // Encrypt
        let encrypted = self.encrypt(&serialized)?;

        // Write to temp file then atomic rename
        let path = self.snapshot_path(&snapshot.group_id);
        let temp_path = path.with_extension("tmp");

        tokio::fs::write(&temp_path, encrypted)
            .await
            .map_err(|e| MlsError::Storage(format!("Write failed: {}", e)))?;

        tokio::fs::rename(&temp_path, &path)
            .await
            .map_err(|e| MlsError::Storage(format!("Atomic rename failed: {}", e)))?;

        // Update cache
        let mut cache = self.cache.write().await;
        cache.insert(snapshot.group_id.clone(), snapshot);

        Ok(())
    }

    async fn load_group_snapshot(&self, group_id: &GroupId) -> MlsResult<PersistedGroupSnapshot> {
        // Check cache first
        {
            let cache = self.cache.read().await;
            if let Some(snapshot) = cache.get(group_id) {
                return Ok(snapshot.clone());
            }
        }

        // Load from disk
        let path = self.snapshot_path(group_id);
        let encrypted = tokio::fs::read(&path)
            .await
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    MlsError::NotFound(format!("Group snapshot not found: {}", hex::encode(group_id)))
                } else {
                    MlsError::Storage(format!("Read failed: {}", e))
                }
            })?;

        // Decrypt
        let decrypted = self.decrypt(&encrypted)?;

        // Deserialize
        let snapshot: PersistedGroupSnapshot = bincode::deserialize(&decrypted)
            .map_err(|e| MlsError::Storage(format!("Deserialization failed: {}", e)))?;

        // Update cache
        let mut cache = self.cache.write().await;
        cache.insert(group_id.to_vec(), snapshot.clone());

        Ok(snapshot)
    }

    async fn delete_group_snapshot(&self, group_id: &GroupId) -> MlsResult<()> {
        let path = self.snapshot_path(group_id);
        
        tokio::fs::remove_file(&path)
            .await
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    MlsError::NotFound(format!("Group snapshot not found: {}", hex::encode(group_id)))
                } else {
                    MlsError::Storage(format!("Delete failed: {}", e))
                }
            })?;

        // Remove from cache
        let mut cache = self.cache.write().await;
        cache.remove(group_id);

        Ok(())
    }

    async fn put_blob(&self, key: &str, data: &[u8]) -> MlsResult<()> {
        let encrypted = self.encrypt(data)?;
        let path = self.blob_path(key);

        tokio::fs::write(&path, encrypted)
            .await
            .map_err(|e| MlsError::Storage(format!("Blob write failed: {}", e)))?;

        Ok(())
    }

    async fn get_blob(&self, key: &str) -> MlsResult<Vec<u8>> {
        let path = self.blob_path(key);
        let encrypted = tokio::fs::read(&path)
            .await
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    MlsError::NotFound(format!("Blob not found: {}", key))
                } else {
                    MlsError::Storage(format!("Blob read failed: {}", e))
                }
            })?;

        self.decrypt(&encrypted)
    }

    async fn list_groups(&self) -> MlsResult<Vec<GroupId>> {
        let mut groups = Vec::new();

        let mut entries = tokio::fs::read_dir(&self.base_path)
            .await
            .map_err(|e| MlsError::Storage(format!("Failed to read directory: {}", e)))?;

        while let Some(entry) = entries.next_entry()
            .await
            .map_err(|e| MlsError::Storage(format!("Failed to read entry: {}", e)))? {
            
            let path = entry.path();
            if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                if filename.starts_with("group-") && filename.ends_with(".snapshot") {
                    // Extract hex group ID
                    let hex_id = &filename[6..filename.len() - 9]; // Remove "group-" and ".snapshot"
                    if let Ok(group_id) = hex::decode(hex_id) {
                        groups.push(group_id);
                    }
                }
            }
        }

        Ok(groups)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_save_and_load_snapshot() {
        let temp_dir = TempDir::new().unwrap();
        let provider = FileStorageProvider::new(temp_dir.path().to_path_buf(), None).unwrap();

        let snapshot = PersistedGroupSnapshot {
            group_id: vec![1, 2, 3, 4],
            epoch: 5,
            serialized_group: vec![10, 20, 30],
        };

        provider.save_group_snapshot(snapshot.clone()).await.unwrap();
        let loaded = provider.load_group_snapshot(&snapshot.group_id).await.unwrap();

        assert_eq!(loaded.group_id, snapshot.group_id);
        assert_eq!(loaded.epoch, snapshot.epoch);
        assert_eq!(loaded.serialized_group, snapshot.serialized_group);
    }

    #[tokio::test]
    async fn test_encrypted_snapshot() {
        let temp_dir = TempDir::new().unwrap();
        let provider = FileStorageProvider::new(
            temp_dir.path().to_path_buf(),
            Some("test-password"),
        ).unwrap();

        let snapshot = PersistedGroupSnapshot {
            group_id: vec![5, 6, 7, 8],
            epoch: 10,
            serialized_group: vec![40, 50, 60],
        };

        provider.save_group_snapshot(snapshot.clone()).await.unwrap();
        let loaded = provider.load_group_snapshot(&snapshot.group_id).await.unwrap();

        assert_eq!(loaded.epoch, snapshot.epoch);
    }

    #[tokio::test]
    async fn test_delete_snapshot() {
        let temp_dir = TempDir::new().unwrap();
        let provider = FileStorageProvider::new(temp_dir.path().to_path_buf(), None).unwrap();

        let snapshot = PersistedGroupSnapshot {
            group_id: vec![9, 10, 11],
            epoch: 1,
            serialized_group: vec![70, 80],
        };

        provider.save_group_snapshot(snapshot.clone()).await.unwrap();
        provider.delete_group_snapshot(&snapshot.group_id).await.unwrap();

        let result = provider.load_group_snapshot(&snapshot.group_id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_blob_storage() {
        let temp_dir = TempDir::new().unwrap();
        let provider = FileStorageProvider::new(temp_dir.path().to_path_buf(), None).unwrap();

        let data = vec![1, 2, 3, 4, 5];
        provider.put_blob("test-key", &data).await.unwrap();
        let retrieved = provider.get_blob("test-key").await.unwrap();

        assert_eq!(retrieved, data);
    }

    #[tokio::test]
    async fn test_list_groups() {
        let temp_dir = TempDir::new().unwrap();
        let provider = FileStorageProvider::new(temp_dir.path().to_path_buf(), None).unwrap();

        let snapshot1 = PersistedGroupSnapshot {
            group_id: vec![1, 2, 3],
            epoch: 1,
            serialized_group: vec![10],
        };
        let snapshot2 = PersistedGroupSnapshot {
            group_id: vec![4, 5, 6],
            epoch: 2,
            serialized_group: vec![20],
        };

        provider.save_group_snapshot(snapshot1.clone()).await.unwrap();
        provider.save_group_snapshot(snapshot2.clone()).await.unwrap();

        let groups = provider.list_groups().await.unwrap();
        assert_eq!(groups.len(), 2);
        assert!(groups.contains(&snapshot1.group_id));
        assert!(groups.contains(&snapshot2.group_id));
    }
}
