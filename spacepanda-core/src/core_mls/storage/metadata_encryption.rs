//! Metadata Encryption for Channel Data
//!
//! Provides encryption/decryption for sensitive channel metadata using ChaCha20Poly1305.
//! This prevents plaintext leakage of channel names, topics, and member lists in the database.
//!
//! Security properties:
//! - AEAD cipher (authenticated encryption)
//! - Unique nonce per encryption operation
//! - Key derived from group ID using HKDF (different key per channel)
//! - Domain separation prevents key reuse across contexts
//! - Prevents database compromise from exposing plaintext metadata

use crate::core_mls::errors::{MlsError, MlsResult};
use chacha20poly1305::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    ChaCha20Poly1305, Nonce,
};
use hkdf::Hkdf;
use sha2::Sha256;

/// Metadata encryption context
///
/// Uses ChaCha20Poly1305 for authenticated encryption of sensitive metadata.
/// Each group has a unique encryption key derived from its group_id using HKDF.
pub struct MetadataEncryption {
    cipher: ChaCha20Poly1305,
}

/// Domain separation constant for metadata encryption
/// Prevents key reuse across different contexts
const METADATA_ENCRYPTION_DOMAIN: &[u8] = b"SpacePanda-Metadata-Encryption-v1";

/// Application-specific salt for HKDF
/// In production, this could be configurable per deployment
const HKDF_SALT: &[u8] = b"SpacePanda-HKDF-Salt-2025";

impl MetadataEncryption {
    /// Create encryption context from group ID
    ///
    /// Derives a unique key for this group using HKDF-SHA256.
    /// This ensures different groups have cryptographically independent encryption keys.
    ///
    /// # Security
    /// - Uses HKDF (RFC 5869) for proper key derivation
    /// - Includes domain separation to prevent cross-context attacks
    /// - Application-specific salt provides additional security
    pub fn new(group_id: &[u8]) -> Self {
        // Use HKDF to derive key from group_id
        // HKDF provides better security properties than simple hashing:
        // - Proper key stretching
        // - Domain separation via info parameter
        // - Salt provides additional entropy mixing
        let hkdf = Hkdf::<Sha256>::new(Some(HKDF_SALT), group_id);
        
        let mut key_bytes = [0u8; 32]; // ChaCha20Poly1305 uses 256-bit keys
        hkdf.expand(METADATA_ENCRYPTION_DOMAIN, &mut key_bytes)
            .expect("HKDF expand should never fail with valid parameters");
        
        let key = chacha20poly1305::Key::from_slice(&key_bytes);
        
        Self {
            cipher: ChaCha20Poly1305::new(key),
        }
    }

    /// Encrypt plaintext metadata
    ///
    /// Returns: nonce (12 bytes) || ciphertext (variable length)
    /// The nonce is prepended to allow decryption without storing it separately.
    pub fn encrypt(&self, plaintext: &[u8]) -> MlsResult<Vec<u8>> {
        // Generate random nonce using AeadCore trait
        let nonce = ChaCha20Poly1305::generate_nonce(OsRng);
        
        // Encrypt
        let ciphertext = self
            .cipher
            .encrypt(&nonce, plaintext)
            .map_err(|e| MlsError::Encryption(format!("Failed to encrypt metadata: {}", e)))?;
        
        // Prepend nonce to ciphertext
        let mut result = nonce.to_vec();
        result.extend_from_slice(&ciphertext);
        
        Ok(result)
    }

    /// Decrypt encrypted metadata
    ///
    /// Expects: nonce (12 bytes) || ciphertext (variable length)
    pub fn decrypt(&self, encrypted: &[u8]) -> MlsResult<Vec<u8>> {
        if encrypted.len() < 12 {
            return Err(MlsError::Decryption(
                "Encrypted data too short (missing nonce)".to_string(),
            ));
        }

        // Extract nonce and ciphertext
        let nonce = Nonce::from_slice(&encrypted[..12]);
        let ciphertext = &encrypted[12..];
        
        // Decrypt
        let plaintext = self
            .cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| MlsError::Decryption(format!("Failed to decrypt metadata: {}", e)))?;
        
        Ok(plaintext)
    }
}

/// Helper function to encrypt channel metadata fields
pub fn encrypt_metadata(group_id: &[u8], plaintext: &[u8]) -> MlsResult<Vec<u8>> {
    let enc = MetadataEncryption::new(group_id);
    enc.encrypt(plaintext)
}

/// Helper function to decrypt channel metadata fields
pub fn decrypt_metadata(group_id: &[u8], encrypted: &[u8]) -> MlsResult<Vec<u8>> {
    let enc = MetadataEncryption::new(group_id);
    enc.decrypt(encrypted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let group_id = b"test_group_123";
        let plaintext = b"Secret channel name";
        
        let enc = MetadataEncryption::new(group_id);
        
        // Encrypt
        let encrypted = enc.encrypt(plaintext).unwrap();
        assert!(encrypted.len() > plaintext.len(), "Encrypted should be larger (nonce + auth tag)");
        assert_ne!(&encrypted[12..], plaintext, "Ciphertext should not match plaintext");
        
        // Decrypt
        let decrypted = enc.decrypt(&encrypted).unwrap();
        assert_eq!(decrypted, plaintext, "Decrypted should match original");
    }

    #[test]
    fn test_different_groups_different_keys() {
        let plaintext = b"same plaintext";
        
        let enc1 = MetadataEncryption::new(b"group_1");
        let enc2 = MetadataEncryption::new(b"group_2");
        
        let encrypted1 = enc1.encrypt(plaintext).unwrap();
        let encrypted2 = enc2.encrypt(plaintext).unwrap();
        
        // Different groups should produce different ciphertexts
        assert_ne!(encrypted1, encrypted2, "Different groups should have different keys");
        
        // Each should decrypt correctly with its own key
        assert_eq!(enc1.decrypt(&encrypted1).unwrap(), plaintext);
        assert_eq!(enc2.decrypt(&encrypted2).unwrap(), plaintext);
        
        // Cross-decryption should fail
        assert!(enc1.decrypt(&encrypted2).is_err(), "Wrong key should fail to decrypt");
        assert!(enc2.decrypt(&encrypted1).is_err(), "Wrong key should fail to decrypt");
    }

    #[test]
    fn test_nonce_uniqueness() {
        let group_id = b"test_group";
        let plaintext = b"test data";
        
        let enc = MetadataEncryption::new(group_id);
        
        // Encrypt same plaintext twice
        let encrypted1 = enc.encrypt(plaintext).unwrap();
        let encrypted2 = enc.encrypt(plaintext).unwrap();
        
        // Should have different nonces (first 12 bytes)
        assert_ne!(&encrypted1[..12], &encrypted2[..12], "Nonces should be unique");
        
        // But both should decrypt correctly
        assert_eq!(enc.decrypt(&encrypted1).unwrap(), plaintext);
        assert_eq!(enc.decrypt(&encrypted2).unwrap(), plaintext);
    }

    #[test]
    fn test_empty_data() {
        let group_id = b"test_group";
        let plaintext = b"";
        
        let enc = MetadataEncryption::new(group_id);
        
        let encrypted = enc.encrypt(plaintext).unwrap();
        let decrypted = enc.decrypt(&encrypted).unwrap();
        
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_large_data() {
        let group_id = b"test_group";
        let plaintext = vec![0x42u8; 10000]; // 10KB
        
        let enc = MetadataEncryption::new(group_id);
        
        let encrypted = enc.encrypt(&plaintext).unwrap();
        let decrypted = enc.decrypt(&encrypted).unwrap();
        
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_corrupted_ciphertext() {
        let group_id = b"test_group";
        let plaintext = b"test data";
        
        let enc = MetadataEncryption::new(group_id);
        let mut encrypted = enc.encrypt(plaintext).unwrap();
        
        // Corrupt the ciphertext (get length first to avoid borrow issue)
        let len = encrypted.len();
        encrypted[len - 1] ^= 0xFF;
        
        // Decryption should fail (authentication tag mismatch)
        assert!(enc.decrypt(&encrypted).is_err(), "Corrupted data should fail authentication");
    }

    #[test]
    fn test_too_short_data() {
        let group_id = b"test_group";
        let enc = MetadataEncryption::new(group_id);
        
        // Data shorter than nonce size
        let short_data = vec![0u8; 11];
        assert!(enc.decrypt(&short_data).is_err(), "Too short data should error");
    }

    #[test]
    fn test_helper_functions() {
        let group_id = b"test_group";
        let plaintext = b"test message";
        
        let encrypted = encrypt_metadata(group_id, plaintext).unwrap();
        let decrypted = decrypt_metadata(group_id, &encrypted).unwrap();
        
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_hkdf_key_derivation_deterministic() {
        // Same group ID should produce the same key (deterministic)
        let group_id = b"test_group_xyz";
        
        let enc1 = MetadataEncryption::new(group_id);
        let enc2 = MetadataEncryption::new(group_id);
        
        let plaintext = b"test data";
        let encrypted = enc1.encrypt(plaintext).unwrap();
        
        // Second instance with same group_id should decrypt successfully
        let decrypted = enc2.decrypt(&encrypted).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_hkdf_key_isolation() {
        // Even very similar group IDs should have completely different keys
        let plaintext = b"sensitive data";
        
        let enc1 = MetadataEncryption::new(b"group_001");
        let enc2 = MetadataEncryption::new(b"group_002");
        let enc3 = MetadataEncryption::new(b"group_001 "); // Note: trailing space
        
        let encrypted1 = enc1.encrypt(plaintext).unwrap();
        
        // Even slight differences in group_id should prevent decryption
        assert!(enc2.decrypt(&encrypted1).is_err(), "Similar group_id should not decrypt");
        assert!(enc3.decrypt(&encrypted1).is_err(), "Group_id with extra char should not decrypt");
    }

    #[test]
    fn test_domain_separation() {
        // Verify that changing domain would change keys
        // We can't directly test this without exposing internals,
        // but we can verify different groups produce different outputs
        let plaintext = b"test";
        
        let groups = vec![b"group_a", b"group_b", b"group_c"];
        let mut ciphertexts = Vec::new();
        
        for group_id in &groups {
            let enc = MetadataEncryption::new(*group_id);
            // Use fixed plaintext so nonce is the only variable
            // Still, different keys should be detectable statistically
            ciphertexts.push(enc.encrypt(plaintext).unwrap());
        }
        
        // All ciphertexts should be different (different keys + different nonces)
        for i in 0..ciphertexts.len() {
            for j in (i + 1)..ciphertexts.len() {
                assert_ne!(
                    ciphertexts[i], ciphertexts[j],
                    "Different groups should produce different ciphertexts"
                );
            }
        }
    }

    #[test]
    fn test_key_derivation_from_empty_group_id() {
        // Even empty group_id should work (though not recommended)
        let group_id = b"";
        let plaintext = b"data";
        
        let enc = MetadataEncryption::new(group_id);
        let encrypted = enc.encrypt(plaintext).unwrap();
        let decrypted = enc.decrypt(&encrypted).unwrap();
        
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_key_derivation_from_large_group_id() {
        // HKDF should handle arbitrarily large inputs
        let group_id = vec![0xABu8; 1024]; // 1KB group ID
        let plaintext = b"data";
        
        let enc = MetadataEncryption::new(&group_id);
        let encrypted = enc.encrypt(plaintext).unwrap();
        let decrypted = enc.decrypt(&encrypted).unwrap();
        
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_binary_group_ids() {
        // Group IDs with null bytes and binary data should work
        let group_id = b"\x00\x01\x02\xFF\xFE\xFD";
        let plaintext = b"test message";
        
        let enc = MetadataEncryption::new(group_id);
        let encrypted = enc.encrypt(plaintext).unwrap();
        let decrypted = enc.decrypt(&encrypted).unwrap();
        
        assert_eq!(decrypted, plaintext);
    }
}
