//! Message encryption and key derivation for MLS
//!
//! This module implements:
//! - HPKE (Hybrid Public Key Encryption) for encrypting to public keys
//! - Key derivation for application secrets
//! - Message AEAD encryption/decryption
//! - Sender data encryption (AAD binding)

use super::errors::{MlsError, MlsResult};
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Key, Nonce,
};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

/// Size of encryption key (256 bits for AES-256-GCM)
const KEY_SIZE: usize = 32;

/// Size of nonce (96 bits for AES-GCM)
const NONCE_SIZE: usize = 12;

/// Reuse key label for HKDF
const REUSE_KEY_LABEL: &[u8] = b"SpacePanda MLS 1.0 Reuse Key";

/// Application secret label for HKDF
const APP_SECRET_LABEL: &[u8] = b"SpacePanda MLS 1.0 Application Secret";

/// Sender data key label
const SENDER_DATA_LABEL: &[u8] = b"SpacePanda MLS 1.0 Sender Data";

/// Message key label
const MESSAGE_KEY_LABEL: &[u8] = b"SpacePanda MLS 1.0 Message Key";

/// Encrypted message with metadata
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EncryptedMessage {
    /// Epoch this message was encrypted in
    pub epoch: u64,
    /// Sender's leaf index
    pub sender_leaf: u32,
    /// Sender's sequence number (for replay protection)
    pub sequence: u64,
    /// Encrypted sender data (AAD)
    pub encrypted_sender_data: Vec<u8>,
    /// Nonce for AEAD
    pub nonce: Vec<u8>,
    /// Ciphertext (includes AEAD tag)
    pub ciphertext: Vec<u8>,
}

/// Sender authentication data
#[derive(Debug, Clone)]
pub struct SenderData {
    /// Sender's leaf index
    pub leaf_index: u32,
    /// Sequence number
    pub sequence: u64,
    /// Epoch
    pub epoch: u64,
}

impl SenderData {
    /// Serialize to bytes for encryption
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(20);
        bytes.extend_from_slice(&self.leaf_index.to_be_bytes());
        bytes.extend_from_slice(&self.sequence.to_be_bytes());
        bytes.extend_from_slice(&self.epoch.to_be_bytes());
        bytes
    }

    /// Deserialize from bytes
    pub fn from_bytes(bytes: &[u8]) -> MlsResult<Self> {
        if bytes.len() != 20 {
            return Err(MlsError::InvalidMessage("Invalid sender data length".to_string()));
        }

        let leaf_index = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let sequence = u64::from_be_bytes([
            bytes[4], bytes[5], bytes[6], bytes[7],
            bytes[8], bytes[9], bytes[10], bytes[11],
        ]);
        let epoch = u64::from_be_bytes([
            bytes[12], bytes[13], bytes[14], bytes[15],
            bytes[16], bytes[17], bytes[18], bytes[19],
        ]);

        Ok(Self {
            leaf_index,
            sequence,
            epoch,
        })
    }
}

/// Key schedule for deriving message keys
pub struct KeySchedule {
    /// Current epoch
    pub epoch: u64,
    /// Application secret for this epoch
    pub application_secret: Vec<u8>,
    /// Sender data secret
    pub sender_data_secret: Vec<u8>,
    /// Cache of derived message keys (leaf_index, sequence) -> key
    message_key_cache: HashMap<(u32, u64), Vec<u8>>,
}

impl KeySchedule {
    /// Create new key schedule from application secret
    pub fn new(epoch: u64, application_secret: Vec<u8>) -> Self {
        let sender_data_secret = derive_secret(&application_secret, SENDER_DATA_LABEL, &[]);
        
        Self {
            epoch,
            application_secret,
            sender_data_secret,
            message_key_cache: HashMap::new(),
        }
    }

    /// Derive message key for a specific sender and sequence
    pub fn derive_message_key(&mut self, leaf_index: u32, sequence: u64) -> Vec<u8> {
        // Check cache first
        let cache_key = (leaf_index, sequence);
        if let Some(key) = self.message_key_cache.get(&cache_key) {
            return key.clone();
        }

        // Derive: message_key = HKDF(app_secret, "message_key" || leaf_index || sequence)
        let mut context = Vec::with_capacity(12);
        context.extend_from_slice(&leaf_index.to_be_bytes());
        context.extend_from_slice(&sequence.to_be_bytes());

        let key = derive_secret(&self.application_secret, MESSAGE_KEY_LABEL, &context);
        
        // Cache for reuse (same key used for encryption and decryption)
        self.message_key_cache.insert(cache_key, key.clone());
        
        key
    }

    /// Derive reuse key for HPKE (placeholder - would use actual HPKE in production)
    pub fn derive_reuse_key(&self) -> Vec<u8> {
        derive_secret(&self.application_secret, REUSE_KEY_LABEL, &[])
    }

    /// Clear message key cache (after epoch advance)
    pub fn clear_cache(&mut self) {
        self.message_key_cache.clear();
    }
}

/// Encrypt an application message
pub fn encrypt_message(
    key_schedule: &mut KeySchedule,
    sender_data: SenderData,
    plaintext: &[u8],
) -> MlsResult<EncryptedMessage> {
    // Derive message key
    let message_key = key_schedule.derive_message_key(sender_data.leaf_index, sender_data.sequence);
    
    // Generate nonce (in production, use proper nonce generation)
    let mut nonce_bytes = [0u8; NONCE_SIZE];
    use rand::RngCore;
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    // Encrypt plaintext with AEAD
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&message_key));
    
    // Use sender data as AAD for binding
    let sender_bytes = sender_data.to_bytes();
    let ciphertext = cipher
        .encrypt(nonce, aes_gcm::aead::Payload {
            msg: plaintext,
            aad: &sender_bytes,
        })
        .map_err(|e| MlsError::CryptoError(format!("AEAD encryption failed: {}", e)))?;

    // Encrypt sender data separately (for confidentiality)
    let sender_data_key = derive_secret(&key_schedule.sender_data_secret, b"encrypt", &nonce_bytes);
    let sender_cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&sender_data_key));
    
    // Use empty AAD for sender data encryption
    let encrypted_sender_data = sender_cipher
        .encrypt(nonce, &sender_bytes[..])
        .map_err(|e| MlsError::CryptoError(format!("Sender data encryption failed: {}", e)))?;

    Ok(EncryptedMessage {
        epoch: sender_data.epoch,
        sender_leaf: sender_data.leaf_index,
        sequence: sender_data.sequence,
        encrypted_sender_data,
        nonce: nonce_bytes.to_vec(),
        ciphertext,
    })
}

/// Decrypt an application message
pub fn decrypt_message(
    key_schedule: &mut KeySchedule,
    encrypted_msg: &EncryptedMessage,
) -> MlsResult<Vec<u8>> {
    // Verify epoch matches
    if encrypted_msg.epoch != key_schedule.epoch {
        return Err(MlsError::EpochMismatch {
            expected: key_schedule.epoch,
            actual: encrypted_msg.epoch,
        });
    }

    // Decrypt sender data first
    if encrypted_msg.nonce.len() != NONCE_SIZE {
        return Err(MlsError::InvalidMessage("Invalid nonce size".to_string()));
    }
    
    let sender_data_key = derive_secret(&key_schedule.sender_data_secret, b"encrypt", &encrypted_msg.nonce);
    let sender_cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&sender_data_key));
    let nonce = Nonce::from_slice(&encrypted_msg.nonce);
    
    let sender_bytes = sender_cipher
        .decrypt(nonce, &encrypted_msg.encrypted_sender_data[..])
        .map_err(|e| MlsError::CryptoError(format!("Sender data decryption failed: {}", e)))?;

    let sender_data = SenderData::from_bytes(&sender_bytes)?;

    // Verify sender data matches header
    if sender_data.leaf_index != encrypted_msg.sender_leaf 
        || sender_data.sequence != encrypted_msg.sequence 
        || sender_data.epoch != encrypted_msg.epoch {
        return Err(MlsError::VerifyFailed("Sender data mismatch".to_string()));
    }

    // Derive message key
    let message_key = key_schedule.derive_message_key(sender_data.leaf_index, sender_data.sequence);
    
    // Decrypt ciphertext with AEAD
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&message_key));
    
    let plaintext = cipher
        .decrypt(nonce, aes_gcm::aead::Payload {
            msg: &encrypted_msg.ciphertext,
            aad: &sender_bytes,
        })
        .map_err(|e| MlsError::CryptoError(format!("AEAD decryption failed: {}", e)))?;

    Ok(plaintext)
}

/// Derive secret using HKDF-Expand (simplified)
///
/// In production MLS, this would use proper HKDF from RFC 5869.
/// Here we use a simplified version with SHA-256.
fn derive_secret(secret: &[u8], label: &[u8], context: &[u8]) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(secret);
    hasher.update(label);
    hasher.update(context);
    hasher.finalize().to_vec()
}

/// HPKE encryption context (placeholder)
///
/// In production MLS, this would use RFC 9180 HPKE.
/// This is a simplified version for the prototype.
pub struct HpkeContext {
    /// Recipient's public key
    pub recipient_public_key: Vec<u8>,
    /// Shared secret (from key agreement)
    shared_secret: Vec<u8>,
}

impl HpkeContext {
    /// Create HPKE context for encrypting to a public key
    ///
    /// In production, this would perform X25519 key agreement.
    /// For now, we use a placeholder that hashes the public key.
    pub fn new(recipient_public_key: Vec<u8>) -> Self {
        // Placeholder: hash public key to derive shared secret
        let mut hasher = Sha256::new();
        hasher.update(b"HPKE shared secret");
        hasher.update(&recipient_public_key);
        let shared_secret = hasher.finalize().to_vec();

        Self {
            recipient_public_key,
            shared_secret,
        }
    }

    /// Encrypt data to the recipient
    pub fn seal(&self, plaintext: &[u8], aad: &[u8]) -> MlsResult<Vec<u8>> {
        // Derive encryption key from shared secret
        let key = derive_secret(&self.shared_secret, b"HPKE encryption", aad);
        
        // Generate nonce
        let mut nonce_bytes = [0u8; NONCE_SIZE];
        use rand::RngCore;
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        // Encrypt with AEAD
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
        let mut ciphertext = cipher
            .encrypt(nonce, aes_gcm::aead::Payload { msg: plaintext, aad })
            .map_err(|e| MlsError::CryptoError(format!("HPKE seal failed: {}", e)))?;

        // Prepend nonce to ciphertext
        let mut result = nonce_bytes.to_vec();
        result.append(&mut ciphertext);
        
        Ok(result)
    }

    /// Decrypt data (receiver side)
    pub fn open(recipient_public_key: &[u8], ciphertext: &[u8], aad: &[u8]) -> MlsResult<Vec<u8>> {
        if ciphertext.len() < NONCE_SIZE {
            return Err(MlsError::InvalidMessage("HPKE ciphertext too short".to_string()));
        }

        // Extract nonce
        let nonce = Nonce::from_slice(&ciphertext[..NONCE_SIZE]);
        let ct = &ciphertext[NONCE_SIZE..];

        // Derive shared secret (same as sender)
        let mut hasher = Sha256::new();
        hasher.update(b"HPKE shared secret");
        hasher.update(recipient_public_key);
        let shared_secret = hasher.finalize();

        // Derive encryption key
        let key = derive_secret(&shared_secret, b"HPKE encryption", aad);

        // Decrypt with AEAD
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
        let plaintext = cipher
            .decrypt(nonce, aes_gcm::aead::Payload { msg: ct, aad })
            .map_err(|e| MlsError::CryptoError(format!("HPKE open failed: {}", e)))?;

        Ok(plaintext)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sender_data_roundtrip() {
        let sender = SenderData {
            leaf_index: 42,
            sequence: 1234567890,
            epoch: 5,
        };

        let bytes = sender.to_bytes();
        assert_eq!(bytes.len(), 20);

        let decoded = SenderData::from_bytes(&bytes).unwrap();
        assert_eq!(decoded.leaf_index, 42);
        assert_eq!(decoded.sequence, 1234567890);
        assert_eq!(decoded.epoch, 5);
    }

    #[test]
    fn test_sender_data_invalid_length() {
        let result = SenderData::from_bytes(&[0u8; 10]);
        assert!(result.is_err());
    }

    #[test]
    fn test_key_schedule_message_key_derivation() {
        let app_secret = vec![0u8; 32];
        let mut ks = KeySchedule::new(1, app_secret);

        let key1 = ks.derive_message_key(0, 0);
        assert_eq!(key1.len(), 32);

        // Same inputs should give same key (from cache)
        let key2 = ks.derive_message_key(0, 0);
        assert_eq!(key1, key2);

        // Different inputs should give different keys
        let key3 = ks.derive_message_key(0, 1);
        assert_ne!(key1, key3);

        let key4 = ks.derive_message_key(1, 0);
        assert_ne!(key1, key4);
    }

    #[test]
    fn test_encrypt_decrypt_message() {
        let app_secret = vec![1u8; 32];
        let mut ks = KeySchedule::new(1, app_secret);

        let sender = SenderData {
            leaf_index: 0,
            sequence: 0,
            epoch: 1,
        };

        let plaintext = b"Hello, MLS!";
        let encrypted = encrypt_message(&mut ks, sender.clone(), plaintext).unwrap();

        assert_eq!(encrypted.epoch, 1);
        assert_eq!(encrypted.sender_leaf, 0);
        assert_eq!(encrypted.sequence, 0);
        assert_eq!(encrypted.nonce.len(), NONCE_SIZE);
        assert!(encrypted.ciphertext.len() > plaintext.len()); // Has AEAD tag

        // Decrypt
        let decrypted = decrypt_message(&mut ks, &encrypted).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_decrypt_wrong_epoch() {
        let app_secret = vec![1u8; 32];
        let mut ks = KeySchedule::new(1, app_secret);

        let sender = SenderData {
            leaf_index: 0,
            sequence: 0,
            epoch: 1,
        };

        let encrypted = encrypt_message(&mut ks, sender, b"test").unwrap();

        // Advance epoch
        ks.epoch = 2;

        let result = decrypt_message(&mut ks, &encrypted);
        assert!(matches!(result, Err(MlsError::EpochMismatch { .. })));
    }

    #[test]
    fn test_decrypt_corrupted_ciphertext() {
        let app_secret = vec![1u8; 32];
        let mut ks = KeySchedule::new(1, app_secret);

        let sender = SenderData {
            leaf_index: 0,
            sequence: 0,
            epoch: 1,
        };

        let mut encrypted = encrypt_message(&mut ks, sender, b"test").unwrap();

        // Corrupt ciphertext
        encrypted.ciphertext[0] ^= 0xFF;

        let result = decrypt_message(&mut ks, &encrypted);
        assert!(result.is_err());
    }

    #[test]
    fn test_decrypt_corrupted_sender_data() {
        let app_secret = vec![1u8; 32];
        let mut ks = KeySchedule::new(1, app_secret);

        let sender = SenderData {
            leaf_index: 0,
            sequence: 0,
            epoch: 1,
        };

        let mut encrypted = encrypt_message(&mut ks, sender, b"test").unwrap();

        // Corrupt encrypted sender data
        encrypted.encrypted_sender_data[0] ^= 0xFF;

        let result = decrypt_message(&mut ks, &encrypted);
        assert!(result.is_err());
    }

    #[test]
    fn test_message_key_cache_cleared() {
        let app_secret = vec![1u8; 32];
        let mut ks = KeySchedule::new(1, app_secret);

        ks.derive_message_key(0, 0);
        assert_eq!(ks.message_key_cache.len(), 1);

        ks.clear_cache();
        assert_eq!(ks.message_key_cache.len(), 0);
    }

    #[test]
    fn test_hpke_seal_open() {
        let recipient_pk = b"recipient_public_key".to_vec();
        let plaintext = b"Secret message";
        let aad = b"Associated data";

        // Sender: encrypt to recipient
        let hpke = HpkeContext::new(recipient_pk.clone());
        let ciphertext = hpke.seal(plaintext, aad).unwrap();

        // Receiver: decrypt
        let decrypted = HpkeContext::open(&recipient_pk, &ciphertext, aad).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_hpke_wrong_aad() {
        let recipient_pk = b"recipient_public_key".to_vec();
        let plaintext = b"Secret message";
        let aad = b"Associated data";

        let hpke = HpkeContext::new(recipient_pk.clone());
        let ciphertext = hpke.seal(plaintext, aad).unwrap();

        // Try to decrypt with wrong AAD
        let result = HpkeContext::open(&recipient_pk, &ciphertext, b"Wrong AAD");
        assert!(result.is_err());
    }

    #[test]
    fn test_hpke_wrong_recipient() {
        let recipient_pk = b"recipient_public_key".to_vec();
        let plaintext = b"Secret message";
        let aad = b"Associated data";

        let hpke = HpkeContext::new(recipient_pk.clone());
        let ciphertext = hpke.seal(plaintext, aad).unwrap();

        // Try to decrypt with wrong public key
        let wrong_pk = b"wrong_public_key";
        let result = HpkeContext::open(wrong_pk, &ciphertext, aad);
        assert!(result.is_err());
    }

    #[test]
    fn test_hpke_ciphertext_too_short() {
        let recipient_pk = b"recipient_public_key".to_vec();
        let short_ct = vec![0u8; 5]; // Less than NONCE_SIZE

        let result = HpkeContext::open(&recipient_pk, &short_ct, b"aad");
        assert!(matches!(result, Err(MlsError::InvalidMessage(_))));
    }

    #[test]
    fn test_derive_secret_deterministic() {
        let secret = b"my_secret";
        let label = b"label";
        let context = b"context";

        let derived1 = derive_secret(secret, label, context);
        let derived2 = derive_secret(secret, label, context);
        assert_eq!(derived1, derived2);
        assert_eq!(derived1.len(), 32); // SHA-256 output
    }

    #[test]
    fn test_derive_secret_different_inputs() {
        let secret = b"my_secret";

        let d1 = derive_secret(secret, b"label1", b"context");
        let d2 = derive_secret(secret, b"label2", b"context");
        let d3 = derive_secret(secret, b"label1", b"different_context");

        assert_ne!(d1, d2);
        assert_ne!(d1, d3);
        assert_ne!(d2, d3);
    }

    #[test]
    fn test_reuse_key_derivation() {
        let app_secret = vec![42u8; 32];
        let ks = KeySchedule::new(1, app_secret);

        let reuse_key = ks.derive_reuse_key();
        assert_eq!(reuse_key.len(), 32);
    }
}
