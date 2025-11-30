//! File-based keystore with encryption at rest

use super::{Keystore, KeystoreError};
use crate::core_identity::device_id::DeviceId;
use crate::core_identity::keypair::Keypair;
use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use argon2::{Argon2, PasswordHasher};
use argon2::password_hash::{PasswordHash, SaltString};
use std::fs;
use std::path::PathBuf;

/// File-based encrypted keystore
pub struct FileKeystore {
    /// Directory where keys are stored
    base_path: PathBuf,
    /// Master key derived from password
    master_key: Option<Vec<u8>>,
}

impl FileKeystore {
    /// Create a new file keystore at the given path
    pub fn new(base_path: PathBuf, password: Option<&str>) -> Result<Self, KeystoreError> {
        // Create directory if it doesn't exist
        fs::create_dir_all(&base_path)?;

        // Derive master key from password if provided
        let master_key = password.map(|pwd| derive_key(pwd));

        Ok(FileKeystore {
            base_path,
            master_key,
        })
    }

    /// Get path for identity keypair
    fn identity_path(&self) -> PathBuf {
        self.base_path.join("identity.bin.enc")
    }

    /// Get path for device keypair
    fn device_path(&self, device_id: &DeviceId) -> PathBuf {
        self.base_path
            .join(format!("device-{}.bin.enc", device_id.to_string()))
    }

    /// Encrypt data
    fn encrypt(&self, data: &[u8]) -> Result<Vec<u8>, KeystoreError> {
        if let Some(key) = &self.master_key {
            let cipher = Aes256Gcm::new_from_slice(key)
                .map_err(|e| KeystoreError::Encryption(format!("Invalid key: {}", e)))?;

            let nonce = Nonce::from_slice(&[0u8; 12]); // In production, use random nonce
            let ciphertext = cipher
                .encrypt(nonce, data)
                .map_err(|e| KeystoreError::Encryption(format!("Encryption failed: {}", e)))?;

            Ok(ciphertext)
        } else {
            // No encryption if no password
            Ok(data.to_vec())
        }
    }

    /// Decrypt data
    fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>, KeystoreError> {
        if let Some(key) = &self.master_key {
            let cipher = Aes256Gcm::new_from_slice(key)
                .map_err(|e| KeystoreError::Decryption(format!("Invalid key: {}", e)))?;

            let nonce = Nonce::from_slice(&[0u8; 12]); // Must match encryption nonce
            let plaintext = cipher
                .decrypt(nonce, data)
                .map_err(|e| KeystoreError::Decryption(format!("Decryption failed: {}", e)))?;

            Ok(plaintext)
        } else {
            // No decryption if no password
            Ok(data.to_vec())
        }
    }

    /// Write file atomically (write to temp, then rename)
    fn write_atomic(&self, path: &PathBuf, data: &[u8]) -> Result<(), KeystoreError> {
        let temp_path = path.with_extension("tmp");
        fs::write(&temp_path, data)?;
        fs::rename(temp_path, path)?;
        Ok(())
    }
}

impl Keystore for FileKeystore {
    fn load_identity_keypair(&self) -> Result<Keypair, KeystoreError> {
        let path = self.identity_path();
        if !path.exists() {
            return Err(KeystoreError::NotFound("Identity keypair not found".to_string()));
        }

        let encrypted = fs::read(&path)?;
        let decrypted = self.decrypt(&encrypted)?;
        Keypair::deserialize(&decrypted)
            .map_err(|e| KeystoreError::Serialization(e))
    }

    fn save_identity_keypair(&self, kp: &Keypair) -> Result<(), KeystoreError> {
        let serialized = kp.serialize();
        let encrypted = self.encrypt(&serialized)?;
        let path = self.identity_path();
        self.write_atomic(&path, &encrypted)
    }

    fn load_device_keypair(&self, device_id: &DeviceId) -> Result<Keypair, KeystoreError> {
        let path = self.device_path(device_id);
        if !path.exists() {
            return Err(KeystoreError::NotFound(format!(
                "Device keypair not found: {}",
                device_id
            )));
        }

        let encrypted = fs::read(&path)?;
        let decrypted = self.decrypt(&encrypted)?;
        Keypair::deserialize(&decrypted)
            .map_err(|e| KeystoreError::Serialization(e))
    }

    fn save_device_keypair(
        &self,
        device_id: &DeviceId,
        kp: &Keypair,
    ) -> Result<(), KeystoreError> {
        let serialized = kp.serialize();
        let encrypted = self.encrypt(&serialized)?;
        let path = self.device_path(device_id);
        self.write_atomic(&path, &encrypted)
    }

    fn list_devices(&self) -> Result<Vec<DeviceId>, KeystoreError> {
        let mut devices = Vec::new();

        for entry in fs::read_dir(&self.base_path)? {
            let entry = entry?;
            let filename = entry.file_name();
            let filename_str = filename.to_string_lossy();

            if filename_str.starts_with("device-") && filename_str.ends_with(".bin.enc") {
                // Extract device ID from filename
                let id_str = filename_str
                    .strip_prefix("device-")
                    .and_then(|s| s.strip_suffix(".bin.enc"))
                    .unwrap_or("");

                if let Ok(device_id) = DeviceId::from_string(id_str) {
                    devices.push(device_id);
                }
            }
        }

        Ok(devices)
    }

    fn rotate_master_key(&self, _password: &str) -> Result<(), KeystoreError> {
        // TODO: Implement key rotation
        // 1. Load all keys with old master key
        // 2. Derive new master key from new password
        // 3. Re-encrypt all keys with new master key
        // 4. Write back to disk atomically
        Err(KeystoreError::Encryption(
            "Key rotation not yet implemented".to_string(),
        ))
    }
}

/// Derive encryption key from password using Argon2
fn derive_key(password: &str) -> Vec<u8> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();

    // In production, store the salt with the encrypted data
    let hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .expect("Failed to hash password");

    // Extract 32 bytes for AES-256
    hash.hash.unwrap().as_bytes()[0..32].to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core_identity::keypair::KeyType;
    use tempfile::TempDir;

    #[test]
    fn test_create_and_load_identity_keypair() {
        let temp_dir = TempDir::new().unwrap();
        let keystore = FileKeystore::new(temp_dir.path().to_path_buf(), Some("password123"))
            .unwrap();

        let kp = Keypair::generate(KeyType::Ed25519);
        keystore.save_identity_keypair(&kp).unwrap();

        let loaded = keystore.load_identity_keypair().unwrap();
        assert_eq!(kp.public_key(), loaded.public_key());
    }

    #[test]
    fn test_create_and_load_device_keypair() {
        let temp_dir = TempDir::new().unwrap();
        let keystore = FileKeystore::new(temp_dir.path().to_path_buf(), None).unwrap();

        let device_id = DeviceId::generate();
        let kp = Keypair::generate(KeyType::Ed25519);

        keystore.save_device_keypair(&device_id, &kp).unwrap();

        let loaded = keystore.load_device_keypair(&device_id).unwrap();
        assert_eq!(kp.public_key(), loaded.public_key());
    }

    #[test]
    fn test_list_devices() {
        let temp_dir = TempDir::new().unwrap();
        let keystore = FileKeystore::new(temp_dir.path().to_path_buf(), None).unwrap();

        let device1 = DeviceId::generate();
        let device2 = DeviceId::generate();

        keystore
            .save_device_keypair(&device1, &Keypair::generate(KeyType::Ed25519))
            .unwrap();
        keystore
            .save_device_keypair(&device2, &Keypair::generate(KeyType::Ed25519))
            .unwrap();

        let devices = keystore.list_devices().unwrap();
        assert_eq!(devices.len(), 2);
    }

    #[test]
    fn test_not_found_error() {
        let temp_dir = TempDir::new().unwrap();
        let keystore = FileKeystore::new(temp_dir.path().to_path_buf(), None).unwrap();

        let result = keystore.load_identity_keypair();
        assert!(result.is_err());
    }
}
