//! File-based keystore with encryption at rest
//!
//! Encrypted File Format:
//! ```
//! [Magic: 8 bytes "SPKS0001"]
//! [Version: 1 byte]
//! [Salt: 16 bytes]
//! [Nonce: 12 bytes]
//! [Ciphertext + AEAD tag: variable]
//! ```

use super::{Keystore, KeystoreError};
use crate::core_identity::device_id::DeviceId;
use crate::core_identity::keypair::Keypair;
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use argon2::{Argon2, Params, PasswordHasher};
use argon2::password_hash::{PasswordHash, SaltString};
use rand::RngCore;
use std::fs;
use std::path::PathBuf;

/// Magic header for encrypted keystore files
const MAGIC_HEADER: &[u8; 8] = b"SPKS0001";

/// Current keystore format version
const FORMAT_VERSION: u8 = 1;

/// Salt length for Argon2 KDF (16 bytes = 128 bits)
const SALT_LEN: usize = 16;

/// Nonce length for AES-GCM (12 bytes = 96 bits)
const NONCE_LEN: usize = 12;

/// Header size: magic(8) + version(1) + salt(16) + nonce(12) = 37 bytes
const HEADER_SIZE: usize = 8 + 1 + SALT_LEN + NONCE_LEN;

/// File-based encrypted keystore
pub struct FileKeystore {
    /// Directory where keys are stored
    base_path: PathBuf,
    /// Encryption password (stored for re-encryption)
    password: Option<String>,
}

impl FileKeystore {
    /// Create a new file keystore at the given path
    pub fn new(base_path: PathBuf, password: Option<&str>) -> Result<Self, KeystoreError> {
        // Create directory if it doesn't exist
        fs::create_dir_all(&base_path)?;

        Ok(FileKeystore {
            base_path,
            password: password.map(|s| s.to_string()),
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

    /// Encrypt data with AEAD (AES-256-GCM)
    /// 
    /// Returns: [magic][version][salt][nonce][ciphertext+tag]
    fn encrypt(&self, data: &[u8]) -> Result<Vec<u8>, KeystoreError> {
        if let Some(password) = &self.password {
            // Generate random salt for this encryption
            let mut salt = [0u8; SALT_LEN];
            rand::thread_rng().fill_bytes(&mut salt);
            
            // Derive key from password using Argon2
            let key = derive_key_from_password(password, &salt)?;
            
            // Generate random nonce
            let mut nonce_bytes = [0u8; NONCE_LEN];
            rand::thread_rng().fill_bytes(&mut nonce_bytes);
            let nonce = Nonce::from_slice(&nonce_bytes);
            
            // Encrypt with AES-256-GCM (provides both confidentiality and integrity)
            let cipher = Aes256Gcm::new_from_slice(&key)
                .map_err(|e| KeystoreError::Encryption(format!("Invalid key: {}", e)))?;
            
            let ciphertext = cipher
                .encrypt(nonce, data)
                .map_err(|e| KeystoreError::Encryption(format!("Encryption failed: {}", e)))?;
            
            // Build encrypted file format: [magic][version][salt][nonce][ciphertext+tag]
            let mut result = Vec::with_capacity(HEADER_SIZE + ciphertext.len());
            result.extend_from_slice(MAGIC_HEADER);
            result.push(FORMAT_VERSION);
            result.extend_from_slice(&salt);
            result.extend_from_slice(&nonce_bytes);
            result.extend_from_slice(&ciphertext);
            
            Ok(result)
        } else {
            // No encryption if no password - still add unencrypted marker header
            let mut result = Vec::with_capacity(9 + data.len());
            result.extend_from_slice(b"SPKS_RAW");
            result.push(FORMAT_VERSION);
            result.extend_from_slice(data);
            Ok(result)
        }
    }

    /// Decrypt data with AEAD (AES-256-GCM)
    /// 
    /// Expects: [magic][version][salt][nonce][ciphertext+tag]
    fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>, KeystoreError> {
        // Check minimum size
        if data.len() < 9 {
            return Err(KeystoreError::Decryption("File too short".to_string()));
        }
        
        // Check if this is unencrypted format
        if &data[0..8] == b"SPKS_RAW" {
            if self.password.is_some() {
                return Err(KeystoreError::Decryption(
                    "Encrypted keystore expected, found unencrypted".to_string()
                ));
            }
            // Skip header and return plaintext
            return Ok(data[9..].to_vec());
        }
        
        // Verify magic header
        if &data[0..8] != MAGIC_HEADER {
            return Err(KeystoreError::Decryption("Invalid magic header".to_string()));
        }
        
        // Verify version
        let version = data[8];
        if version != FORMAT_VERSION {
            return Err(KeystoreError::Decryption(
                format!("Unsupported version: {}", version)
            ));
        }
        
        // Check minimum encrypted size
        if data.len() < HEADER_SIZE + 16 {
            // 16 is minimum: AEAD tag size
            return Err(KeystoreError::Decryption("Truncated file".to_string()));
        }
        
        if let Some(password) = &self.password {
            // Extract salt
            let salt = &data[9..9+SALT_LEN];
            
            // Extract nonce
            let nonce_bytes = &data[9+SALT_LEN..9+SALT_LEN+NONCE_LEN];
            let nonce = Nonce::from_slice(nonce_bytes);
            
            // Extract ciphertext (includes AEAD tag)
            let ciphertext = &data[HEADER_SIZE..];
            
            // Derive key from password and salt
            let key = derive_key_from_password(password, salt)?;
            
            // Decrypt with AES-256-GCM (verifies integrity via AEAD tag)
            let cipher = Aes256Gcm::new_from_slice(&key)
                .map_err(|e| KeystoreError::Decryption(format!("Invalid key: {}", e)))?;
            
            let plaintext = cipher
                .decrypt(nonce, ciphertext)
                .map_err(|_| KeystoreError::InvalidPassword)?; // AEAD tag mismatch = wrong password or corrupted
            
            Ok(plaintext)
        } else {
            Err(KeystoreError::Decryption(
                "Password required to decrypt".to_string()
            ))
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

/// Derive 256-bit encryption key from password using Argon2id
fn derive_key_from_password(password: &str, salt: &[u8]) -> Result<Vec<u8>, KeystoreError> {
    // Use Argon2id with secure parameters
    let params = Params::new(
        19 * 1024,  // 19 MiB memory cost
        2,          // 2 iterations
        1,          // 1 thread (for determinism)
        Some(32),   // 32-byte output (256 bits for AES-256)
    ).map_err(|e| KeystoreError::Encryption(format!("Invalid Argon2 params: {}", e)))?;
    
    let argon2 = Argon2::new(
        argon2::Algorithm::Argon2id,
        argon2::Version::V0x13,
        params,
    );
    
    // Hash password with salt
    let mut key = vec![0u8; 32];
    argon2
        .hash_password_into(password.as_bytes(), salt, &mut key)
        .map_err(|e| KeystoreError::Encryption(format!("Key derivation failed: {}", e)))?;
    
    Ok(key)
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

    #[test]
    fn test_corrupted_aead_tag() {
        let temp_dir = TempDir::new().unwrap();
        let keystore = FileKeystore::new(temp_dir.path().to_path_buf(), Some("password123"))
            .unwrap();

        let kp = Keypair::generate(KeyType::Ed25519);
        keystore.save_identity_keypair(&kp).unwrap();

        // Read encrypted file
        let path = keystore.identity_path();
        let mut encrypted = fs::read(&path).unwrap();

        // Corrupt the AEAD tag (last 16 bytes)
        let len = encrypted.len();
        encrypted[len - 1] ^= 0xFF; // Flip bits in last byte of tag

        // Write corrupted data back
        fs::write(&path, &encrypted).unwrap();

        // Attempt to load should fail due to AEAD tag verification
        let result = keystore.load_identity_keypair();
        assert!(result.is_err());
        
        // Should report as invalid password (AEAD failure)
        match result {
            Err(KeystoreError::InvalidPassword) => {
                // Expected - AEAD tag mismatch
            }
            _ => panic!("Expected InvalidPassword error for corrupted AEAD tag"),
        }
    }

    #[test]
    fn test_corrupted_ciphertext() {
        let temp_dir = TempDir::new().unwrap();
        let keystore = FileKeystore::new(temp_dir.path().to_path_buf(), Some("password123"))
            .unwrap();

        let kp = Keypair::generate(KeyType::Ed25519);
        keystore.save_identity_keypair(&kp).unwrap();

        // Read encrypted file
        let path = keystore.identity_path();
        let mut encrypted = fs::read(&path).unwrap();

        // Corrupt ciphertext (middle of file, not header or tag)
        let mid = HEADER_SIZE + 10;
        encrypted[mid] ^= 0xFF;

        fs::write(&path, &encrypted).unwrap();

        // Should fail AEAD verification
        let result = keystore.load_identity_keypair();
        assert!(result.is_err());
        match result {
            Err(KeystoreError::InvalidPassword) => {
                // Expected - AEAD integrity check failed
            }
            _ => panic!("Expected InvalidPassword for corrupted ciphertext"),
        }
    }

    #[test]
    fn test_wrong_passphrase() {
        let temp_dir = TempDir::new().unwrap();
        let keystore1 = FileKeystore::new(temp_dir.path().to_path_buf(), Some("correct_password"))
            .unwrap();

        let kp = Keypair::generate(KeyType::Ed25519);
        keystore1.save_identity_keypair(&kp).unwrap();

        // Try to load with wrong password
        let keystore2 = FileKeystore::new(temp_dir.path().to_path_buf(), Some("wrong_password"))
            .unwrap();

        let result = keystore2.load_identity_keypair();
        assert!(result.is_err());
        match result {
            Err(KeystoreError::InvalidPassword) => {
                // Expected
            }
            _ => panic!("Expected InvalidPassword error"),
        }
    }

    #[test]
    fn test_truncated_file() {
        let temp_dir = TempDir::new().unwrap();
        let keystore = FileKeystore::new(temp_dir.path().to_path_buf(), Some("password123"))
            .unwrap();

        let kp = Keypair::generate(KeyType::Ed25519);
        keystore.save_identity_keypair(&kp).unwrap();

        let path = keystore.identity_path();
        let encrypted = fs::read(&path).unwrap();

        // Truncate file to less than header size
        fs::write(&path, &encrypted[0..10]).unwrap();

        let result = keystore.load_identity_keypair();
        assert!(result.is_err());
        match result {
            Err(KeystoreError::Decryption(msg)) => {
                assert!(msg.contains("too short") || msg.contains("Truncated"));
            }
            _ => panic!("Expected Decryption error for truncated file"),
        }
    }

    #[test]
    fn test_invalid_magic_header() {
        let temp_dir = TempDir::new().unwrap();
        let keystore = FileKeystore::new(temp_dir.path().to_path_buf(), Some("password123"))
            .unwrap();

        let kp = Keypair::generate(KeyType::Ed25519);
        keystore.save_identity_keypair(&kp).unwrap();

        let path = keystore.identity_path();
        let mut encrypted = fs::read(&path).unwrap();

        // Corrupt magic header
        encrypted[0] = b'X';

        fs::write(&path, &encrypted).unwrap();

        let result = keystore.load_identity_keypair();
        assert!(result.is_err());
        match result {
            Err(KeystoreError::Decryption(msg)) => {
                assert!(msg.contains("magic"));
            }
            _ => panic!("Expected Decryption error for invalid magic"),
        }
    }

    #[test]
    fn test_unsupported_version() {
        let temp_dir = TempDir::new().unwrap();
        let keystore = FileKeystore::new(temp_dir.path().to_path_buf(), Some("password123"))
            .unwrap();

        let kp = Keypair::generate(KeyType::Ed25519);
        keystore.save_identity_keypair(&kp).unwrap();

        let path = keystore.identity_path();
        let mut encrypted = fs::read(&path).unwrap();

        // Change version to unsupported value
        encrypted[8] = 99;

        fs::write(&path, &encrypted).unwrap();

        let result = keystore.load_identity_keypair();
        assert!(result.is_err());
        match result {
            Err(KeystoreError::Decryption(msg)) => {
                assert!(msg.contains("version"));
            }
            _ => panic!("Expected Decryption error for unsupported version"),
        }
    }

    #[test]
    fn test_nonce_uniqueness() {
        // Encrypt same data multiple times, verify nonces are different
        let temp_dir = TempDir::new().unwrap();
        let keystore = FileKeystore::new(temp_dir.path().to_path_buf(), Some("password123"))
            .unwrap();

        let data = b"test data";
        let encrypted1 = keystore.encrypt(data).unwrap();
        let encrypted2 = keystore.encrypt(data).unwrap();

        // Nonces should be different (bytes 25-37)
        let nonce1 = &encrypted1[9+SALT_LEN..9+SALT_LEN+NONCE_LEN];
        let nonce2 = &encrypted2[9+SALT_LEN..9+SALT_LEN+NONCE_LEN];

        assert_ne!(nonce1, nonce2, "Nonces must be unique for each encryption");

        // Ciphertexts should also be different due to different nonces/salts
        assert_ne!(encrypted1, encrypted2, "Ciphertexts must differ");
    }

    #[test]
    fn test_salt_uniqueness() {
        // Encrypt same data multiple times, verify salts are different
        let temp_dir = TempDir::new().unwrap();
        let keystore = FileKeystore::new(temp_dir.path().to_path_buf(), Some("password123"))
            .unwrap();

        let data = b"test data";
        let encrypted1 = keystore.encrypt(data).unwrap();
        let encrypted2 = keystore.encrypt(data).unwrap();

        // Salts should be different (bytes 9-25)
        let salt1 = &encrypted1[9..9+SALT_LEN];
        let salt2 = &encrypted2[9..9+SALT_LEN];

        assert_ne!(salt1, salt2, "Salts must be unique for each encryption");
    }

    #[test]
    fn test_unencrypted_mode() {
        let temp_dir = TempDir::new().unwrap();
        let keystore = FileKeystore::new(temp_dir.path().to_path_buf(), None).unwrap();

        let kp = Keypair::generate(KeyType::Ed25519);
        keystore.save_identity_keypair(&kp).unwrap();

        let loaded = keystore.load_identity_keypair().unwrap();
        assert_eq!(kp.public_key(), loaded.public_key());

        // Verify file starts with SPKS_RAW marker
        let path = keystore.identity_path();
        let data = fs::read(&path).unwrap();
        assert_eq!(&data[0..8], b"SPKS_RAW");
    }

    #[test]
    fn test_encrypted_keystore_rejects_unencrypted_file() {
        let temp_dir = TempDir::new().unwrap();
        
        // Save with no password
        let keystore1 = FileKeystore::new(temp_dir.path().to_path_buf(), None).unwrap();
        let kp = Keypair::generate(KeyType::Ed25519);
        keystore1.save_identity_keypair(&kp).unwrap();

        // Try to load with password
        let keystore2 = FileKeystore::new(temp_dir.path().to_path_buf(), Some("password"))
            .unwrap();
        
        let result = keystore2.load_identity_keypair();
        assert!(result.is_err());
        match result {
            Err(KeystoreError::Decryption(msg)) => {
                assert!(msg.contains("unencrypted"));
            }
            _ => panic!("Expected error when encrypted keystore loads unencrypted file"),
        }
    }
}
