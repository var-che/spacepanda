//! Crypto Provider Trait
//!
//! Defines the cryptographic operations interface for MLS.
//! In production, this wraps OpenMLS crypto providers.
//! In tests, this can use deterministic mock implementations.

use async_trait::async_trait;
use crate::core_mls::errors::MlsResult;

/// Cryptographic operations provider for MLS
///
/// This trait abstracts cryptographic operations so we can:
/// - Use OpenMlsRustCrypto in production
/// - Use deterministic mocks in tests
/// - Swap crypto implementations without changing MLS logic
///
/// Note: OpenMLS requires certain operations (random, signature, HPKE, hash).
/// We expose only what the engine needs and leave heavy lifting to the underlying provider.
#[async_trait]
pub trait CryptoProvider: Send + Sync {
    /// Return cryptographically secure random bytes
    ///
    /// # Arguments
    /// * `n` - Number of random bytes to generate
    ///
    /// # Security
    /// MUST use a cryptographically secure RNG (e.g., OsRng)
    async fn random_bytes(&self, n: usize) -> MlsResult<Vec<u8>>;

    /// Sign a message using the node's private credential key (Ed25519)
    ///
    /// # Arguments
    /// * `message` - The message to sign
    ///
    /// # Returns
    /// The signature bytes
    async fn sign(&self, message: &[u8]) -> MlsResult<Vec<u8>>;

    /// Verify a signature with a public key
    ///
    /// # Arguments
    /// * `public_key` - The public key bytes
    /// * `message` - The message that was signed
    /// * `signature` - The signature to verify
    ///
    /// # Returns
    /// `Ok(())` if valid, `Err` if invalid
    async fn verify(&self, public_key: &[u8], message: &[u8], signature: &[u8]) -> MlsResult<()>;

    /// HPKE encrypt (sender encapsulates for recipient pubkey)
    ///
    /// # Arguments
    /// * `recipient_pub` - Recipient's public key
    /// * `info` - Additional authenticated data
    /// * `plaintext` - Data to encrypt
    ///
    /// # Returns
    /// Encrypted ciphertext
    async fn hpke_seal(&self, recipient_pub: &[u8], info: &[u8], plaintext: &[u8]) -> MlsResult<Vec<u8>>;

    /// HPKE decrypt (recipient decapsulates)
    ///
    /// # Arguments
    /// * `recipient_priv` - Recipient's private key
    /// * `sender_enc` - Sender's encapsulated key
    /// * `info` - Additional authenticated data
    /// * `ciphertext` - Data to decrypt
    ///
    /// # Returns
    /// Decrypted plaintext
    async fn hpke_open(&self, recipient_priv: &[u8], sender_enc: &[u8], info: &[u8], ciphertext: &[u8]) -> MlsResult<Vec<u8>>;

    /// KDF (HKDF extract/expand) helper
    ///
    /// # Arguments
    /// * `prk` - Pseudorandom key (from extract phase)
    /// * `info` - Context-specific info string
    /// * `len` - Desired output length in bytes
    ///
    /// # Returns
    /// Derived key material
    async fn hkdf_expand(&self, prk: &[u8], info: &[u8], len: usize) -> MlsResult<Vec<u8>>;

    /// Hash function (SHA-256 or ciphersuite-specific)
    ///
    /// # Arguments
    /// * `data` - Data to hash
    ///
    /// # Returns
    /// Hash digest
    async fn hash(&self, data: &[u8]) -> MlsResult<Vec<u8>> {
        // Default implementation - can be overridden
        use sha2::{Sha256, Digest};
        Ok(Sha256::digest(data).to_vec())
    }
}
