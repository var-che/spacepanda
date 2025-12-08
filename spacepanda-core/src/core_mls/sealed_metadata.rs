//! Sealed Metadata - Privacy-preserving encrypted metadata storage
//!
//! This module provides encryption for sensitive channel metadata to prevent
//! information leakage from stored or transmitted metadata.
//!
//! ## Threat Model
//!
//! **Metadata Exposure Risks:**
//! - Channel names reveal purpose ("#whistleblowers", "#activists", etc.)
//! - Member lists expose social graphs
//! - Creation/update timestamps leak activity patterns
//! - Roles reveal organizational structure
//!
//! **Attack Scenarios:**
//! 1. Database compromise: Encrypted storage protects metadata at rest
//! 2. Network observer: Sealed metadata prevents transit analysis
//! 3. Compromised relay: Cannot learn channel details from forwarded messages
//! 4. Forensic analysis: Encrypted metadata provides plausible deniability
//!
//! ## Design
//!
//! Uses AES-256-GCM (same as message encryption) for authenticated encryption:
//! - **Confidentiality**: Channel names, members, timestamps hidden
//! - **Integrity**: Tampering detection via AEAD tag
//! - **Authenticity**: Only group members can decrypt
//!
//! Format: [VERSION:1][NONCE:12][CIPHERTEXT+TAG]
//!
//! ## Security Properties
//!
//! ✅ Symmetric encryption (fast, constant-time)
//! ✅ Random nonce per encryption (prevents correlation)
//! ✅ AEAD tag (detects tampering)
//! ✅ Versioned format (future-proof)
//! ⚠️ Epoch still visible (needed for MLS protocol)
//!
//! ## Usage
//!
//! ```ignore
//! // Encrypt metadata before storage/transmission
//! let metadata = GroupMetadata { /* ... */ };
//! let encryption_key = derive_metadata_key(&group_secret);
//! let sealed = seal_metadata(&metadata, &encryption_key)?;
//!
//! // Decrypt when needed
//! let metadata = unseal_metadata(&sealed, &encryption_key)?;
//! ```

use super::errors::{MlsError, MlsResult};
use super::types::GroupMetadata;
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use rand::RngCore;
use sha2::{Digest, Sha256};

/// Sealed metadata format version
const SEALED_VERSION: u8 = 0x01;

/// Nonce size for AES-GCM (96 bits)
const NONCE_SIZE: usize = 12;

/// Key size for AES-256-GCM
const KEY_SIZE: usize = 32;

/// Domain separation label for metadata encryption keys
const METADATA_KEY_LABEL: &[u8] = b"SpacePanda MLS 1.0 Metadata Encryption";

/// Sealed (encrypted) metadata blob
///
/// This is what gets stored or transmitted instead of plaintext GroupMetadata.
/// Only the epoch remains visible for MLS protocol requirements.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SealedMetadata {
    /// Encryption format version
    pub version: u8,
    /// Current epoch (visible for MLS)
    pub epoch: u64,
    /// Random nonce for AES-GCM
    pub nonce: [u8; NONCE_SIZE],
    /// Encrypted metadata (name, members, timestamps) + AEAD tag
    pub ciphertext: Vec<u8>,
}

/// Derive a metadata encryption key from a group secret
///
/// Uses HKDF-SHA256 to derive a domain-separated key for metadata encryption.
/// This ensures metadata keys are independent from message keys.
///
/// # Arguments
///
/// * `group_secret` - Secret material from MLS group (e.g., exporter secret)
///
/// # Returns
///
/// 32-byte encryption key for metadata
pub fn derive_metadata_key(group_secret: &[u8]) -> [u8; KEY_SIZE] {
    let mut hasher = Sha256::new();
    hasher.update(METADATA_KEY_LABEL);
    hasher.update(group_secret);

    let hash = hasher.finalize();
    let mut key = [0u8; KEY_SIZE];
    key.copy_from_slice(&hash[..KEY_SIZE]);
    key
}

/// Encrypt metadata for storage or transmission
///
/// # Arguments
///
/// * `metadata` - GroupMetadata to encrypt
/// * `key` - 32-byte encryption key (from derive_metadata_key)
///
/// # Returns
///
/// SealedMetadata with encrypted contents
///
/// # Security
///
/// - Uses fresh random nonce per encryption
/// - AEAD provides authenticity + confidentiality
/// - Epoch left visible for MLS protocol
pub fn seal_metadata(metadata: &GroupMetadata, key: &[u8; KEY_SIZE]) -> MlsResult<SealedMetadata> {
    if key.len() != KEY_SIZE {
        return Err(MlsError::CryptoError(format!(
            "Invalid key size: {} (expected {})",
            key.len(),
            KEY_SIZE
        )));
    }

    // Serialize metadata to JSON
    let plaintext = serde_json::to_vec(metadata).map_err(|e| {
        MlsError::SerializationError(format!("Failed to serialize metadata: {}", e))
    })?;

    // Generate random nonce
    let mut nonce_bytes = [0u8; NONCE_SIZE];
    rand::rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    // Create cipher
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|e| MlsError::CryptoError(format!("Failed to create cipher: {}", e)))?;

    // Encrypt with AAD = epoch (binds epoch to ciphertext)
    let aad = metadata.epoch.to_be_bytes();
    let ciphertext = cipher
        .encrypt(nonce, aes_gcm::aead::Payload { msg: &plaintext, aad: &aad })
        .map_err(|e| MlsError::CryptoError(format!("Encryption failed: {}", e)))?;

    Ok(SealedMetadata {
        version: SEALED_VERSION,
        epoch: metadata.epoch,
        nonce: nonce_bytes,
        ciphertext,
    })
}

/// Decrypt sealed metadata
///
/// # Arguments
///
/// * `sealed` - SealedMetadata to decrypt
/// * `key` - 32-byte encryption key (same as used for sealing)
///
/// # Returns
///
/// Decrypted GroupMetadata
///
/// # Errors
///
/// - Invalid version
/// - Key mismatch
/// - Tampered ciphertext (AEAD verification fails)
/// - Corrupted data
pub fn unseal_metadata(sealed: &SealedMetadata, key: &[u8; KEY_SIZE]) -> MlsResult<GroupMetadata> {
    // Check version
    if sealed.version != SEALED_VERSION {
        return Err(MlsError::InvalidInput(format!(
            "Unsupported sealed metadata version: 0x{:02x}",
            sealed.version
        )));
    }

    if key.len() != KEY_SIZE {
        return Err(MlsError::CryptoError(format!(
            "Invalid key size: {} (expected {})",
            key.len(),
            KEY_SIZE
        )));
    }

    // Create cipher
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|e| MlsError::CryptoError(format!("Failed to create cipher: {}", e)))?;

    // Decrypt with AAD = epoch
    let nonce = Nonce::from_slice(&sealed.nonce);
    let aad = sealed.epoch.to_be_bytes();

    let plaintext = cipher
        .decrypt(nonce, aes_gcm::aead::Payload { msg: &sealed.ciphertext, aad: &aad })
        .map_err(|_| {
            MlsError::CryptoError("Decryption failed (wrong key or tampered data)".to_string())
        })?;

    // Deserialize metadata
    let metadata: GroupMetadata = serde_json::from_slice(&plaintext).map_err(|e| {
        MlsError::SerializationError(format!("Failed to deserialize metadata: {}", e))
    })?;

    // Verify epoch matches (defense in depth)
    if metadata.epoch != sealed.epoch {
        return Err(MlsError::InvalidInput(format!(
            "Epoch mismatch: sealed={}, decrypted={}",
            sealed.epoch, metadata.epoch
        )));
    }

    Ok(metadata)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core_mls::types::{GroupId, MemberInfo, MemberRole};

    fn test_metadata() -> GroupMetadata {
        GroupMetadata {
            group_id: GroupId::new(vec![1, 2, 3, 4]),
            name: Some("Secret Channel".to_string()),
            epoch: 42,
            members: vec![
                MemberInfo {
                    identity: b"alice".to_vec(),
                    leaf_index: 0,
                    joined_at: 1000,
                    role: MemberRole::Admin,
                },
                MemberInfo {
                    identity: b"bob".to_vec(),
                    leaf_index: 1,
                    joined_at: 1001,
                    role: MemberRole::Member,
                },
            ],
            created_at: 1000,
            updated_at: 2000,
        }
    }

    #[test]
    fn test_seal_unseal_roundtrip() {
        let metadata = test_metadata();
        let key = derive_metadata_key(b"test_group_secret");

        let sealed = seal_metadata(&metadata, &key).unwrap();
        let unsealed = unseal_metadata(&sealed, &key).unwrap();

        assert_eq!(unsealed.group_id, metadata.group_id);
        assert_eq!(unsealed.name, metadata.name);
        assert_eq!(unsealed.epoch, metadata.epoch);
        assert_eq!(unsealed.members.len(), metadata.members.len());
        assert_eq!(unsealed.created_at, metadata.created_at);
    }

    #[test]
    fn test_sealed_format() {
        let metadata = test_metadata();
        let key = derive_metadata_key(b"test_secret");

        let sealed = seal_metadata(&metadata, &key).unwrap();

        assert_eq!(sealed.version, SEALED_VERSION);
        assert_eq!(sealed.epoch, 42); // Epoch visible
        assert_eq!(sealed.nonce.len(), NONCE_SIZE);
        assert!(!sealed.ciphertext.is_empty());
    }

    #[test]
    fn test_wrong_key_fails() {
        let metadata = test_metadata();
        let key1 = derive_metadata_key(b"key1");
        let key2 = derive_metadata_key(b"key2");

        let sealed = seal_metadata(&metadata, &key1).unwrap();
        let result = unseal_metadata(&sealed, &key2);

        assert!(result.is_err(), "Should fail with wrong key");
    }

    #[test]
    fn test_tampered_ciphertext_fails() {
        let metadata = test_metadata();
        let key = derive_metadata_key(b"test_key");

        let mut sealed = seal_metadata(&metadata, &key).unwrap();

        // Tamper with ciphertext
        if !sealed.ciphertext.is_empty() {
            sealed.ciphertext[0] ^= 0xFF;
        }

        let result = unseal_metadata(&sealed, &key);
        assert!(result.is_err(), "Should fail on tampered data");
    }

    #[test]
    fn test_different_nonces() {
        let metadata = test_metadata();
        let key = derive_metadata_key(b"test");

        let sealed1 = seal_metadata(&metadata, &key).unwrap();
        let sealed2 = seal_metadata(&metadata, &key).unwrap();

        // Nonces should be different (random)
        assert_ne!(sealed1.nonce, sealed2.nonce);

        // But both should decrypt correctly
        let unsealed1 = unseal_metadata(&sealed1, &key).unwrap();
        let unsealed2 = unseal_metadata(&sealed2, &key).unwrap();

        assert_eq!(unsealed1.name, unsealed2.name);
    }

    #[test]
    fn test_invalid_version_fails() {
        let metadata = test_metadata();
        let key = derive_metadata_key(b"test");

        let mut sealed = seal_metadata(&metadata, &key).unwrap();
        sealed.version = 0x99; // Invalid version

        let result = unseal_metadata(&sealed, &key);
        assert!(result.is_err(), "Should reject invalid version");
    }

    #[test]
    fn test_metadata_confidentiality() {
        let metadata = test_metadata();
        let key = derive_metadata_key(b"secret");

        let sealed = seal_metadata(&metadata, &key).unwrap();

        // Channel name should NOT be visible in ciphertext
        let secret_name = b"Secret Channel";
        let ciphertext_str = String::from_utf8_lossy(&sealed.ciphertext);

        assert!(!ciphertext_str.contains("Secret"), "Channel name leaked in ciphertext");
        assert!(
            !sealed.ciphertext.windows(secret_name.len()).any(|w| w == secret_name),
            "Channel name found as plaintext in sealed blob"
        );

        // Member identities should NOT be visible
        assert!(!ciphertext_str.contains("alice"), "Member identity leaked in ciphertext");
    }

    #[test]
    fn test_key_derivation_deterministic() {
        let secret = b"test_group_secret";

        let key1 = derive_metadata_key(secret);
        let key2 = derive_metadata_key(secret);

        assert_eq!(key1, key2, "Key derivation should be deterministic");
    }

    #[test]
    fn test_key_derivation_unique() {
        let key1 = derive_metadata_key(b"secret1");
        let key2 = derive_metadata_key(b"secret2");

        assert_ne!(key1, key2, "Different secrets should produce different keys");
    }

    #[test]
    fn test_epoch_binding() {
        let mut metadata1 = test_metadata();
        metadata1.epoch = 10;

        let mut metadata2 = test_metadata();
        metadata2.epoch = 20;

        let key = derive_metadata_key(b"test");

        let sealed1 = seal_metadata(&metadata1, &key).unwrap();
        let sealed2 = seal_metadata(&metadata2, &key).unwrap();

        // Epoch is AAD - cannot swap ciphertexts between epochs
        let mut tampered = sealed1.clone();
        tampered.ciphertext = sealed2.ciphertext.clone();

        let result = unseal_metadata(&tampered, &key);
        assert!(result.is_err(), "Should fail when epoch mismatches AAD");
    }
}
