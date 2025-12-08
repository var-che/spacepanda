//! Persistent OpenMLS Provider
//!
//! Extends OpenMLS RustCrypto provider with file-based persistence for MLS group state.
//! Uses the built-in MemoryStorage persistence feature to save/load the entire key store.

use crate::core_mls::errors::{MlsError, MlsResult};
use openmls_memory_storage::MemoryStorage;
use openmls_rust_crypto::RustCrypto;
use openmls_traits::OpenMlsProvider;
use std::fs::File;
use std::path::{Path, PathBuf};

/// Persistent OpenMLS provider that can save/load state to/from disk
#[derive(Debug)]
pub struct PersistentProvider {
    crypto: RustCrypto,
    storage: MemoryStorage,
    storage_path: PathBuf,
}

impl Default for PersistentProvider {
    fn default() -> Self {
        Self {
            crypto: RustCrypto::default(),
            storage: MemoryStorage::default(),
            storage_path: PathBuf::from("/tmp/openmls_storage.json"),
        }
    }
}

impl PersistentProvider {
    /// Create a new persistent provider with a specific storage path
    pub fn new<P: AsRef<Path>>(storage_path: P) -> MlsResult<Self> {
        let storage_path = storage_path.as_ref().to_path_buf();
        let mut storage = MemoryStorage::default();

        // Try to load existing storage if it exists
        if storage_path.exists() {
            tracing::info!("Loading existing OpenMLS storage from {:?}", storage_path);
            let file = File::open(&storage_path)
                .map_err(|e| MlsError::Storage(format!("Failed to open storage file: {}", e)))?;

            storage
                .load_from_file(&file)
                .map_err(|e| MlsError::Storage(format!("Failed to load storage: {}", e)))?;

            tracing::info!("Successfully loaded OpenMLS storage");
        } else {
            tracing::info!("No existing storage found at {:?}, starting fresh", storage_path);

            // Ensure parent directory exists
            if let Some(parent) = storage_path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    MlsError::Storage(format!("Failed to create storage directory: {}", e))
                })?;
            }
        }

        Ok(Self { crypto: RustCrypto::default(), storage, storage_path })
    }

    /// Save the current storage state to disk
    pub fn save(&self) -> MlsResult<()> {
        tracing::debug!("Saving OpenMLS storage to {:?}", self.storage_path);

        let file = File::create(&self.storage_path)
            .map_err(|e| MlsError::Storage(format!("Failed to create storage file: {}", e)))?;

        self.storage
            .save_to_file(&file)
            .map_err(|e| MlsError::Storage(format!("Failed to save storage: {}", e)))?;

        tracing::debug!("Successfully saved OpenMLS storage");
        Ok(())
    }

    /// Get the storage path
    pub fn storage_path(&self) -> &Path {
        &self.storage_path
    }
}

impl OpenMlsProvider for PersistentProvider {
    type CryptoProvider = RustCrypto;
    type RandProvider = RustCrypto;
    type StorageProvider = MemoryStorage;

    fn storage(&self) -> &Self::StorageProvider {
        &self.storage
    }

    fn crypto(&self) -> &Self::CryptoProvider {
        &self.crypto
    }

    fn rand(&self) -> &Self::RandProvider {
        &self.crypto
    }
}

// For test utilities
impl Clone for PersistentProvider {
    fn clone(&self) -> Self {
        Self {
            crypto: RustCrypto::default(),
            storage: self.storage.clone(),
            storage_path: self.storage_path.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_persistent_provider_new() {
        let temp_dir = TempDir::new().unwrap();
        let storage_path = temp_dir.path().join("test_storage.json");

        let provider = PersistentProvider::new(&storage_path).unwrap();
        assert_eq!(provider.storage_path(), storage_path);
    }

    #[test]
    fn test_persistent_provider_save_and_load() {
        let temp_dir = TempDir::new().unwrap();
        let storage_path = temp_dir.path().join("test_storage.json");

        // Create provider and save
        {
            let provider = PersistentProvider::new(&storage_path).unwrap();
            provider.save().unwrap();
        }

        // Verify file exists
        assert!(storage_path.exists());

        // Load in new provider
        let provider2 = PersistentProvider::new(&storage_path).unwrap();
        assert_eq!(provider2.storage_path(), storage_path);
    }
}
