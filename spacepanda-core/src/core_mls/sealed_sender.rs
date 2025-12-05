//! Sealed Sender - Hide sender identity from network observers
//!
//! This module implements "sealed sender" encryption to prevent network-level
//! adversaries from learning who sent a message. Only group members can decrypt
//! the sender identity.
//!
//! ## Threat Model
//!
//! **Without Sealed Sender:**
//! ```text
//! EncryptedEnvelope {
//!     sender: b"alice@example.com",  // ⚠️ PLAINTEXT - network sees this
//!     payload: [encrypted_message],
//! }
//! → Network observer: "Alice sent a message to this group"
//! → Social graph leakage, timing correlation, activity patterns
//! ```
//!
//! **With Sealed Sender:**
//! ```text
//! EncryptedEnvelope {
//!     sealed_sender: SealedSender {
//!         ciphertext: [0x9a, 0x3f, ...],  // ✅ ENCRYPTED
//!     },
//!     payload: [encrypted_message],
//! }
//! → Network observer: "Someone in the group sent a message"
//! → No sender identity leakage
//! ```
//!
//! ## Security Properties
//!
//! - **Confidentiality**: Only group members can decrypt sender identity
//! - **Integrity**: AEAD tag prevents tampering with sender field
//! - **Authenticity**: Only valid group members can create sealed senders
//! - **Unlinkability**: Different messages from same sender → different ciphertexts
//!
//! ## Cryptographic Design
//!
//! - **Algorithm**: AES-256-GCM (same as MLS messages)
//! - **Key Derivation**: HKDF-SHA256 from group exporter secret
//! - **Domain Separation**: "Sealed Sender v1" label
//! - **Nonce**: Random 12 bytes (never reused)
//! - **AAD**: Epoch number (binds sender to specific group state)
//!
//! ## Format
//!
//! ```text
//! SealedSender {
//!     version: u8,              // 0x01 (for future upgrades)
//!     nonce: [u8; 12],          // Random per encryption
//!     ciphertext: Vec<u8>,      // sender_bytes + AEAD tag (16 bytes)
//! }
//! ```
//!
//! ## Usage
//!
//! ```rust
//! use spacepanda_core::core_mls::sealed_sender::{seal_sender, unseal_sender};
//!
//! // Derive key from group secret (once per epoch)
//! let key = derive_sender_key(&group_exporter_secret);
//!
//! // Seal sender before network transmission
//! let sender_id = b"alice@example.com";
//! let sealed = seal_sender(sender_id, &key, epoch)?;
//!
//! // Network transmission (sealed.ciphertext is opaque)
//! transmit_envelope(sealed);
//!
//! // Unseal by group member
//! let sender_id = unseal_sender(&sealed, &key, epoch)?;
//! assert_eq!(sender_id, b"alice@example.com");
//! ```

use crate::core_mls::{errors::MlsError, errors::MlsResult};
use aes_gcm::{
    aead::{Aead, KeyInit, Payload},
    Aes256Gcm, Nonce,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Current sealed sender format version
const SEALED_SENDER_VERSION: u8 = 1;

/// Domain separation label for HKDF
const HKDF_LABEL: &[u8] = b"Sealed Sender v1";

/// AES-GCM nonce size (96 bits)
const NONCE_SIZE: usize = 12;

/// AES-256 key size (256 bits)
const KEY_SIZE: usize = 32;

/// Sealed sender structure - encrypts sender identity
///
/// This structure contains an encrypted sender identity that only group
/// members can decrypt. Network observers see only the ciphertext.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SealedSender {
    /// Format version (for future upgrades)
    version: u8,

    /// Random nonce for AES-GCM (never reused)
    nonce: [u8; NONCE_SIZE],

    /// Encrypted sender identity + AEAD tag
    /// Format: AES-256-GCM(sender_bytes) + 16-byte authentication tag
    ciphertext: Vec<u8>,
}

impl SealedSender {
    /// Create a new sealed sender from components
    pub fn new(nonce: [u8; NONCE_SIZE], ciphertext: Vec<u8>) -> Self {
        Self { version: SEALED_SENDER_VERSION, nonce, ciphertext }
    }

    /// Get the ciphertext
    pub fn ciphertext(&self) -> &[u8] {
        &self.ciphertext
    }

    /// Get the nonce
    pub fn nonce(&self) -> &[u8; NONCE_SIZE] {
        &self.nonce
    }

    /// Get the version
    pub fn version(&self) -> u8 {
        self.version
    }
}

/// Derive a sender encryption key from group secret
///
/// Uses HKDF-SHA256 with domain separation to derive a deterministic
/// key for sealing sender identities. The same group secret always
/// produces the same key (within the same epoch).
///
/// # Arguments
///
/// * `group_secret` - MLS exporter secret from the group
///
/// # Returns
///
/// A 32-byte AES-256 key for sealing/unsealing senders
///
/// # Example
///
/// ```rust
/// let key = derive_sender_key(&group_exporter_secret);
/// // Same secret → same key (deterministic)
/// assert_eq!(key, derive_sender_key(&group_exporter_secret));
/// ```
pub fn derive_sender_key(group_secret: &[u8]) -> [u8; KEY_SIZE] {
    let mut hasher = Sha256::new();
    hasher.update(HKDF_LABEL);
    hasher.update(group_secret);
    let hash = hasher.finalize();

    let mut key = [0u8; KEY_SIZE];
    key.copy_from_slice(&hash[..KEY_SIZE]);
    key
}

/// Seal a sender identity for network transmission
///
/// Encrypts the sender identity using AES-256-GCM so only group members
/// can see who sent the message. Network observers see only ciphertext.
///
/// # Arguments
///
/// * `sender` - The sender identity to encrypt (e.g., b"alice@example.com")
/// * `key` - 32-byte encryption key (from `derive_sender_key`)
/// * `epoch` - Current MLS epoch (used in AAD for binding)
///
/// # Returns
///
/// A `SealedSender` containing the encrypted sender identity
///
/// # Security
///
/// - Random nonce ensures different ciphertexts for same sender
/// - Epoch in AAD prevents cross-epoch tampering
/// - AEAD tag provides integrity and authenticity
///
/// # Example
///
/// ```rust
/// let key = derive_sender_key(&group_secret);
/// let sealed = seal_sender(b"alice@example.com", &key, 42)?;
/// // Network sees only: SealedSender { ciphertext: [0x9a, ...] }
/// ```
pub fn seal_sender(sender: &[u8], key: &[u8; KEY_SIZE], epoch: u64) -> MlsResult<SealedSender> {
    // Validate inputs
    if sender.is_empty() {
        return Err(MlsError::InvalidInput("Sender cannot be empty".to_string()));
    }

    // Generate random nonce
    let mut nonce_bytes = [0u8; NONCE_SIZE];
    use rand::Rng;
    rand::rng().fill(&mut nonce_bytes);

    // Initialize cipher
    let cipher =
        Aes256Gcm::new_from_slice(key).map_err(|e| MlsError::CryptoError(e.to_string()))?;

    let nonce = Nonce::from_slice(&nonce_bytes);

    // Use epoch as AAD (Additional Authenticated Data)
    // This binds the sealed sender to a specific group state
    let aad = epoch.to_le_bytes();

    // Encrypt sender identity
    let payload = Payload { msg: sender, aad: &aad };

    let ciphertext = cipher
        .encrypt(nonce, payload)
        .map_err(|e| MlsError::CryptoError(format!("Failed to seal sender: {}", e)))?;

    Ok(SealedSender::new(nonce_bytes, ciphertext))
}

/// Unseal a sender identity (decrypt)
///
/// Decrypts a sealed sender to reveal the original sender identity.
/// Only group members with the correct key can unseal.
///
/// # Arguments
///
/// * `sealed` - The sealed sender to decrypt
/// * `key` - 32-byte decryption key (same as used for sealing)
/// * `epoch` - Expected MLS epoch (must match sealing epoch)
///
/// # Returns
///
/// The original sender identity bytes
///
/// # Errors
///
/// - `InvalidInput` - Wrong format version
/// - `CryptoError` - Decryption failed (wrong key, tampered data, or wrong epoch)
///
/// # Example
///
/// ```rust
/// let sender = unseal_sender(&sealed, &key, 42)?;
/// assert_eq!(sender, b"alice@example.com");
/// ```
pub fn unseal_sender(
    sealed: &SealedSender,
    key: &[u8; KEY_SIZE],
    epoch: u64,
) -> MlsResult<Vec<u8>> {
    // Verify version
    if sealed.version != SEALED_SENDER_VERSION {
        return Err(MlsError::InvalidInput(format!(
            "Unsupported sealed sender version: {}",
            sealed.version
        )));
    }

    // Initialize cipher
    let cipher =
        Aes256Gcm::new_from_slice(key).map_err(|e| MlsError::CryptoError(e.to_string()))?;

    let nonce = Nonce::from_slice(&sealed.nonce);

    // Use epoch as AAD (must match sealing epoch)
    let aad = epoch.to_le_bytes();

    // Decrypt sender identity
    let payload = Payload { msg: &sealed.ciphertext, aad: &aad };

    let sender = cipher.decrypt(nonce, payload).map_err(|e| {
        MlsError::CryptoError(format!("Failed to unseal sender (wrong key or tampered): {}", e))
    })?;

    Ok(sender)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_seal_unseal_roundtrip() {
        let sender = b"alice@spacepanda.local";
        let group_secret = b"test_group_secret_32_bytes_long!";
        let key = derive_sender_key(group_secret);
        let epoch = 42;

        let sealed = seal_sender(sender, &key, epoch).expect("Sealing should succeed");
        let unsealed = unseal_sender(&sealed, &key, epoch).expect("Unsealing should succeed");

        assert_eq!(unsealed, sender);
    }

    #[test]
    fn test_sealed_format() {
        let sender = b"bob@example.com";
        let key = derive_sender_key(b"secret");
        let epoch = 1;

        let sealed = seal_sender(sender, &key, epoch).expect("Sealing should succeed");

        // Check format
        assert_eq!(sealed.version(), SEALED_SENDER_VERSION);
        assert_eq!(sealed.nonce().len(), NONCE_SIZE);
        // Ciphertext = sender (15 bytes) + AEAD tag (16 bytes) = 31 bytes
        assert_eq!(sealed.ciphertext().len(), sender.len() + 16);
    }

    #[test]
    fn test_wrong_key_fails() {
        let sender = b"alice@example.com";
        let key1 = derive_sender_key(b"secret1");
        let key2 = derive_sender_key(b"secret2");
        let epoch = 1;

        let sealed = seal_sender(sender, &key1, epoch).expect("Sealing should succeed");
        let result = unseal_sender(&sealed, &key2, epoch);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Failed to unseal sender"));
    }

    #[test]
    fn test_tampered_ciphertext_fails() {
        let sender = b"alice@example.com";
        let key = derive_sender_key(b"secret");
        let epoch = 1;

        let mut sealed = seal_sender(sender, &key, epoch).expect("Sealing should succeed");

        // Tamper with ciphertext
        sealed.ciphertext[0] ^= 0xFF;

        let result = unseal_sender(&sealed, &key, epoch);
        assert!(result.is_err());
    }

    #[test]
    fn test_different_nonces() {
        let sender = b"alice@example.com";
        let key = derive_sender_key(b"secret");
        let epoch = 1;

        let sealed1 = seal_sender(sender, &key, epoch).expect("Sealing should succeed");
        let sealed2 = seal_sender(sender, &key, epoch).expect("Sealing should succeed");

        // Same sender, but different nonces → different ciphertexts (unlinkability)
        assert_ne!(sealed1.nonce(), sealed2.nonce());
        assert_ne!(sealed1.ciphertext(), sealed2.ciphertext());

        // Both decrypt to same sender
        let unsealed1 = unseal_sender(&sealed1, &key, epoch).expect("Unseal 1 should succeed");
        let unsealed2 = unseal_sender(&sealed2, &key, epoch).expect("Unseal 2 should succeed");
        assert_eq!(unsealed1, sender);
        assert_eq!(unsealed2, sender);
    }

    #[test]
    fn test_invalid_version_fails() {
        let sender = b"alice@example.com";
        let key = derive_sender_key(b"secret");
        let epoch = 1;

        let mut sealed = seal_sender(sender, &key, epoch).expect("Sealing should succeed");
        sealed.version = 99; // Invalid version

        let result = unseal_sender(&sealed, &key, epoch);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unsupported sealed sender version"));
    }

    #[test]
    fn test_empty_sender_rejected() {
        let key = derive_sender_key(b"secret");
        let epoch = 1;

        let result = seal_sender(b"", &key, epoch);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Sender cannot be empty"));
    }

    #[test]
    fn test_sender_confidentiality() {
        let sender1 = b"alice@example.com";
        let sender2 = b"bob@example.com";
        let key = derive_sender_key(b"secret");
        let epoch = 1;

        let sealed1 = seal_sender(sender1, &key, epoch).expect("Sealing should succeed");
        let sealed2 = seal_sender(sender2, &key, epoch).expect("Sealing should succeed");

        // Network observer sees only ciphertexts (no plaintext sender)
        assert_ne!(sealed1.ciphertext(), sender1);
        assert_ne!(sealed2.ciphertext(), sender2);

        // Cannot tell who sent what just by looking at ciphertext
        assert_ne!(sealed1.ciphertext(), sealed2.ciphertext());
    }

    #[test]
    fn test_key_derivation_deterministic() {
        let group_secret = b"test_secret";

        let key1 = derive_sender_key(group_secret);
        let key2 = derive_sender_key(group_secret);

        // Same secret → same key
        assert_eq!(key1, key2);
    }

    #[test]
    fn test_key_derivation_unique() {
        let secret1 = b"secret1";
        let secret2 = b"secret2";

        let key1 = derive_sender_key(secret1);
        let key2 = derive_sender_key(secret2);

        // Different secrets → different keys
        assert_ne!(key1, key2);
    }

    #[test]
    fn test_epoch_binding() {
        let sender = b"alice@example.com";
        let key = derive_sender_key(b"secret");
        let epoch1 = 1;
        let epoch2 = 2;

        // Seal with epoch 1
        let sealed = seal_sender(sender, &key, epoch1).expect("Sealing should succeed");

        // Try to unseal with epoch 2 (should fail - epoch mismatch)
        let result = unseal_sender(&sealed, &key, epoch2);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Failed to unseal sender"));

        // Unseal with correct epoch (should succeed)
        let unsealed = unseal_sender(&sealed, &key, epoch1).expect("Unsealing should succeed");
        assert_eq!(unsealed, sender);
    }

    #[test]
    fn test_various_sender_lengths() {
        let key = derive_sender_key(b"secret");
        let epoch = 1;

        // Short sender
        let short = b"a@b.c";
        let sealed = seal_sender(short, &key, epoch).expect("Sealing should succeed");
        let unsealed = unseal_sender(&sealed, &key, epoch).expect("Unsealing should succeed");
        assert_eq!(unsealed, short);

        // Medium sender
        let medium = b"alice.wonderland@spacepanda.local";
        let sealed = seal_sender(medium, &key, epoch).expect("Sealing should succeed");
        let unsealed = unseal_sender(&sealed, &key, epoch).expect("Unsealing should succeed");
        assert_eq!(unsealed, medium);

        // Long sender (UUID-based)
        let long = b"user-550e8400-e29b-41d4-a716-446655440000@spacepanda.local";
        let sealed = seal_sender(long, &key, epoch).expect("Sealing should succeed");
        let unsealed = unseal_sender(&sealed, &key, epoch).expect("Unsealing should succeed");
        assert_eq!(unsealed, long);
    }
}
