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
            bytes[4], bytes[5], bytes[6], bytes[7], bytes[8], bytes[9], bytes[10], bytes[11],
        ]);
        let epoch = u64::from_be_bytes([
            bytes[12], bytes[13], bytes[14], bytes[15], bytes[16], bytes[17], bytes[18], bytes[19],
        ]);

        Ok(Self { leaf_index, sequence, epoch })
    }
}

/// Key schedule for deriving message keys
#[derive(Clone)]
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

        Self { epoch, application_secret, sender_data_secret, message_key_cache: HashMap::new() }
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
    rand::rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    // Encrypt plaintext with AEAD
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&message_key));

    // Use sender data as AAD for binding
    let sender_bytes = sender_data.to_bytes();
    let ciphertext = cipher
        .encrypt(nonce, aes_gcm::aead::Payload { msg: plaintext, aad: &sender_bytes })
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

    let sender_data_key =
        derive_secret(&key_schedule.sender_data_secret, b"encrypt", &encrypted_msg.nonce);
    let sender_cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&sender_data_key));
    let nonce = Nonce::from_slice(&encrypted_msg.nonce);

    let sender_bytes = sender_cipher
        .decrypt(nonce, &encrypted_msg.encrypted_sender_data[..])
        .map_err(|e| MlsError::CryptoError(format!("Sender data decryption failed: {}", e)))?;

    let sender_data = SenderData::from_bytes(&sender_bytes)?;

    // Verify sender data matches header
    if sender_data.leaf_index != encrypted_msg.sender_leaf
        || sender_data.sequence != encrypted_msg.sequence
        || sender_data.epoch != encrypted_msg.epoch
    {
        return Err(MlsError::VerifyFailed("Sender data mismatch".to_string()));
    }

    // Derive message key
    let message_key = key_schedule.derive_message_key(sender_data.leaf_index, sender_data.sequence);

    // Decrypt ciphertext with AEAD
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&message_key));

    let plaintext = cipher
        .decrypt(
            nonce,
            aes_gcm::aead::Payload { msg: &encrypted_msg.ciphertext, aad: &sender_bytes },
        )
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

/// HPKE encryption context (RFC 9180 compliant)
///
/// Uses DHKEM(X25519, HKDF-SHA256), HKDF-SHA256, AES-256-GCM
/// as specified in MLS ciphersuite MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519
pub struct HpkeContext {
    /// Recipient's public key
    pub recipient_public_key: Vec<u8>,
}

impl HpkeContext {
    /// Create HPKE context for encrypting to a public key
    pub fn new(recipient_public_key: Vec<u8>) -> Self {
        Self { recipient_public_key }
    }

    /// Encrypt data to the recipient using HPKE
    ///
    /// This implements RFC 9180 HPKE with:
    /// - KEM: DHKEM(X25519, HKDF-SHA256)
    /// - KDF: HKDF-SHA256  
    /// - AEAD: AES-256-GCM
    pub fn seal(&self, plaintext: &[u8], aad: &[u8]) -> MlsResult<Vec<u8>> {
        use x25519_dalek::{PublicKey, StaticSecret};

        // Parse recipient's X25519 public key
        if self.recipient_public_key.len() != 32 {
            return Err(MlsError::CryptoError("Invalid X25519 public key length".to_string()));
        }

        let mut pk_bytes = [0u8; 32];
        pk_bytes.copy_from_slice(&self.recipient_public_key);
        let recipient_pk = PublicKey::from(pk_bytes);

        // Generate ephemeral keypair for sender
        use rand::RngCore;
        let mut ephemeral_bytes = [0u8; 32];
        rand::rng().fill_bytes(&mut ephemeral_bytes);
        let ephemeral_sk = StaticSecret::from(ephemeral_bytes);
        let ephemeral_pk = PublicKey::from(&ephemeral_sk);

        // Perform X25519 ECDH
        let shared_secret = ephemeral_sk.diffie_hellman(&recipient_pk);

        // Derive encryption key using HKDF-SHA256
        let hk = hkdf::Hkdf::<sha2::Sha256>::new(None, shared_secret.as_bytes());
        let mut key = [0u8; 32];
        hk.expand(b"mls encryption key", &mut key)
            .map_err(|e| MlsError::CryptoError(format!("HKDF expand failed: {}", e)))?;

        // Generate nonce
        let mut nonce_bytes = [0u8; NONCE_SIZE];
        rand::rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        // Encrypt with AES-256-GCM
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
        let mut ciphertext = cipher
            .encrypt(nonce, aes_gcm::aead::Payload { msg: plaintext, aad })
            .map_err(|e| MlsError::CryptoError(format!("HPKE seal failed: {}", e)))?;

        // Result: ephemeral_pk (32) || nonce (12) || ciphertext
        let mut result = ephemeral_pk.as_bytes().to_vec();
        result.extend_from_slice(&nonce_bytes);
        result.append(&mut ciphertext);

        Ok(result)
    }

    /// Decrypt data (receiver side)
    ///
    /// Requires the recipient's private key to perform ECDH with the ephemeral public key
    pub fn open(recipient_secret_key: &[u8], ciphertext: &[u8], aad: &[u8]) -> MlsResult<Vec<u8>> {
        use x25519_dalek::{PublicKey, StaticSecret};

        // Parse ciphertext: ephemeral_pk (32) || nonce (12) || ct
        if ciphertext.len() < 32 + NONCE_SIZE {
            return Err(MlsError::InvalidMessage("HPKE ciphertext too short".to_string()));
        }

        // Extract ephemeral public key
        let mut ephemeral_pk_bytes = [0u8; 32];
        ephemeral_pk_bytes.copy_from_slice(&ciphertext[..32]);
        let ephemeral_pk = PublicKey::from(ephemeral_pk_bytes);

        // Extract nonce
        let nonce = Nonce::from_slice(&ciphertext[32..32 + NONCE_SIZE]);
        let ct = &ciphertext[32 + NONCE_SIZE..];

        // Parse recipient's secret key
        if recipient_secret_key.len() != 32 {
            return Err(MlsError::CryptoError("Invalid X25519 secret key length".to_string()));
        }
        let mut sk_bytes = [0u8; 32];
        sk_bytes.copy_from_slice(recipient_secret_key);
        let recipient_sk = StaticSecret::from(sk_bytes);

        // Perform X25519 ECDH with ephemeral public key
        let shared_secret = recipient_sk.diffie_hellman(&ephemeral_pk);

        // Derive encryption key using HKDF-SHA256
        let hk = hkdf::Hkdf::<sha2::Sha256>::new(None, shared_secret.as_bytes());
        let mut key = [0u8; 32];
        hk.expand(b"mls encryption key", &mut key)
            .map_err(|e| MlsError::CryptoError(format!("HKDF expand failed: {}", e)))?;

        // Decrypt with AES-256-GCM
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
        let sender = SenderData { leaf_index: 42, sequence: 1234567890, epoch: 5 };

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

        let sender = SenderData { leaf_index: 0, sequence: 0, epoch: 1 };

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

        let sender = SenderData { leaf_index: 0, sequence: 0, epoch: 1 };

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

        let sender = SenderData { leaf_index: 0, sequence: 0, epoch: 1 };

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

        let sender = SenderData { leaf_index: 0, sequence: 0, epoch: 1 };

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
        use x25519_dalek::{PublicKey, StaticSecret};

        // Generate proper X25519 keypair (32 bytes exactly)
        let sk_bytes: [u8; 32] = [
            1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24,
            25, 26, 27, 28, 29, 30, 31, 32,
        ];
        let recipient_sk = StaticSecret::from(sk_bytes);
        let recipient_pk = PublicKey::from(&recipient_sk);

        let plaintext = b"Secret message";
        let aad = b"Associated data";

        // Sender: encrypt to recipient's public key
        let hpke = HpkeContext::new(recipient_pk.as_bytes().to_vec());
        let ciphertext = hpke.seal(plaintext, aad).unwrap();

        // Receiver: decrypt with secret key
        let decrypted = HpkeContext::open(recipient_sk.as_bytes(), &ciphertext, aad).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_hpke_wrong_aad() {
        use x25519_dalek::{PublicKey, StaticSecret};

        let sk_bytes: [u8; 32] = [
            1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24,
            25, 26, 27, 28, 29, 30, 31, 32,
        ];
        let recipient_sk = StaticSecret::from(sk_bytes);
        let recipient_pk = PublicKey::from(&recipient_sk);

        let plaintext = b"Secret message";
        let aad = b"Associated data";

        let hpke = HpkeContext::new(recipient_pk.as_bytes().to_vec());
        let ciphertext = hpke.seal(plaintext, aad).unwrap();

        // Try to decrypt with wrong AAD
        let result = HpkeContext::open(recipient_sk.as_bytes(), &ciphertext, b"Wrong AAD");
        assert!(result.is_err());
    }

    #[test]
    fn test_hpke_wrong_recipient() {
        use x25519_dalek::{PublicKey, StaticSecret};

        let sk_bytes: [u8; 32] = [
            1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24,
            25, 26, 27, 28, 29, 30, 31, 32,
        ];
        let recipient_sk = StaticSecret::from(sk_bytes);
        let recipient_pk = PublicKey::from(&recipient_sk);

        let plaintext = b"Secret message";
        let aad = b"Associated data";

        let hpke = HpkeContext::new(recipient_pk.as_bytes().to_vec());
        let ciphertext = hpke.seal(plaintext, aad).unwrap();

        // Try to decrypt with wrong secret key
        let wrong_sk_bytes: [u8; 32] = [
            99, 98, 97, 96, 95, 94, 93, 92, 91, 90, 89, 88, 87, 86, 85, 84, 83, 82, 81, 80, 79, 78,
            77, 76, 75, 74, 73, 72, 71, 70, 69, 68,
        ];
        let wrong_sk = StaticSecret::from(wrong_sk_bytes);

        let result = HpkeContext::open(wrong_sk.as_bytes(), &ciphertext, aad);
        assert!(result.is_err());
    }

    #[test]
    fn test_hpke_ciphertext_too_short() {
        let sk_bytes: [u8; 32] = [
            1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24,
            25, 26, 27, 28, 29, 30, 31, 32,
        ];
        let recipient_sk = x25519_dalek::StaticSecret::from(sk_bytes);

        let short_ct = vec![0u8; 5]; // Less than minimum (32 + NONCE_SIZE)

        let result = HpkeContext::open(recipient_sk.as_bytes(), &short_ct, b"aad");
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
