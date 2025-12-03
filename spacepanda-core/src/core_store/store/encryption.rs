/*
    encryption.rs - At-rest encryption for local storage

    Encrypts all data written to disk using AES-256-GCM.
    Keys are derived from user passphrase or hardware-backed keystore.

    Security properties:
    - Authenticated encryption (AEAD)
    - Unique nonce per encryption
    - Key derivation with Argon2id
*/

use crate::core_store::store::errors::{StoreError, StoreResult};
use aes_gcm::aead::OsRng;
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Key, Nonce,
};
use argon2::{
    password_hash::{PasswordHasher, SaltString},
    Argon2,
};

/// Manages encryption/decryption of data at rest
pub struct EncryptionManager {
    cipher: Aes256Gcm,
}

impl EncryptionManager {
    /// Create a new encryption manager with a random key
    pub fn new() -> StoreResult<Self> {
        // In production, this should come from a keystore or user passphrase
        let key = Aes256Gcm::generate_key(&mut OsRng);
        let cipher = Aes256Gcm::new(&key);

        Ok(EncryptionManager { cipher })
    }

    /// Create from a passphrase
    pub fn from_passphrase(passphrase: &str) -> StoreResult<Self> {
        // Derive key from passphrase using Argon2id
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();

        let password_hash = argon2
            .hash_password(passphrase.as_bytes(), &salt)
            .map_err(|e| StoreError::EncryptionError(e.to_string()))?;

        let key_bytes = password_hash.hash.unwrap();
        let key_slice = &key_bytes.as_bytes()[..32];
        let key = Key::<Aes256Gcm>::from_slice(key_slice);

        let cipher = Aes256Gcm::new(key);

        Ok(EncryptionManager { cipher })
    }

    /// Encrypt data
    pub fn encrypt(&self, plaintext: &[u8]) -> StoreResult<Vec<u8>> {
        // Generate random nonce
        use aes_gcm::aead::rand_core::RngCore;
        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        // Encrypt
        let ciphertext = self
            .cipher
            .encrypt(nonce, plaintext)
            .map_err(|e| StoreError::EncryptionError(e.to_string()))?;

        // Prepend nonce to ciphertext
        let mut result = nonce_bytes.to_vec();
        result.extend_from_slice(&ciphertext);

        Ok(result)
    }

    /// Decrypt data
    pub fn decrypt(&self, ciphertext: &[u8]) -> StoreResult<Vec<u8>> {
        if ciphertext.len() < 12 {
            return Err(StoreError::EncryptionError("Invalid ciphertext length".to_string()));
        }

        // Extract nonce
        let nonce = Nonce::from_slice(&ciphertext[..12]);

        // Decrypt
        let plaintext = self
            .cipher
            .decrypt(nonce, &ciphertext[12..])
            .map_err(|e| StoreError::EncryptionError(e.to_string()))?;

        Ok(plaintext)
    }
}

impl Default for EncryptionManager {
    fn default() -> Self {
        Self::new().expect("Failed to create EncryptionManager")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encryption_manager_creation() {
        let manager = EncryptionManager::new();
        assert!(manager.is_ok());
    }

    #[test]
    fn test_encrypt_decrypt() {
        let manager = EncryptionManager::new().unwrap();

        let plaintext = b"Hello, World!";
        let ciphertext = manager.encrypt(plaintext).unwrap();

        assert_ne!(plaintext.to_vec(), ciphertext);

        let decrypted = manager.decrypt(&ciphertext).unwrap();
        assert_eq!(plaintext.to_vec(), decrypted);
    }

    #[test]
    fn test_from_passphrase() {
        let manager = EncryptionManager::from_passphrase("my-secret-passphrase");
        assert!(manager.is_ok());
    }

    #[test]
    fn test_nonce_uniqueness() {
        let manager = EncryptionManager::new().unwrap();

        let plaintext = b"test";
        let ciphertext1 = manager.encrypt(plaintext).unwrap();
        let ciphertext2 = manager.encrypt(plaintext).unwrap();

        // Different nonces should produce different ciphertexts
        assert_ne!(ciphertext1, ciphertext2);

        // But both should decrypt to the same plaintext
        assert_eq!(manager.decrypt(&ciphertext1).unwrap(), plaintext);
        assert_eq!(manager.decrypt(&ciphertext2).unwrap(), plaintext);
    }

    #[test]
    fn test_invalid_ciphertext() {
        let manager = EncryptionManager::new().unwrap();

        let result = manager.decrypt(b"invalid");
        assert!(result.is_err());
    }
}
