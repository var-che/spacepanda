//! Cryptographic primitives for MLS
//!
//! Provides production-grade cryptographic operations:
//! - Ed25519 signatures (RFC 8032)
//! - X25519 key agreement (used in HPKE)
//! - Key generation and management
//!
//! This module implements the cryptographic operations required by the
//! MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519 ciphersuite.

use super::errors::{MlsError, MlsResult};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand::RngCore;
use zeroize::ZeroizeOnDrop;

/// Ed25519 signing key (secret key)
///
/// Implements `ZeroizeOnDrop` to ensure the secret key is securely
/// erased from memory when dropped.
#[derive(ZeroizeOnDrop)]
pub struct MlsSigningKey {
    inner: SigningKey,
}

impl MlsSigningKey {
    /// Generate a new random signing key
    pub fn generate() -> Self {
        let mut rng = rand::rng();
        let mut seed = [0u8; 32];
        rng.fill_bytes(&mut seed);

        Self { inner: SigningKey::from_bytes(&seed) }
    }

    /// Create from raw 32-byte seed
    pub fn from_bytes(bytes: &[u8; 32]) -> Self {
        Self { inner: SigningKey::from_bytes(bytes) }
    }

    /// Get the corresponding verifying (public) key
    pub fn verifying_key(&self) -> MlsVerifyingKey {
        MlsVerifyingKey { inner: self.inner.verifying_key() }
    }

    /// Sign data and return signature bytes
    pub fn sign(&self, data: &[u8]) -> Vec<u8> {
        let signature = self.inner.sign(data);
        signature.to_bytes().to_vec()
    }

    /// Export as bytes (WARNING: exposes secret key!)
    pub fn to_bytes(&self) -> [u8; 32] {
        self.inner.to_bytes()
    }
}

/// Ed25519 verifying key (public key)
#[derive(Clone)]
pub struct MlsVerifyingKey {
    inner: VerifyingKey,
}

impl MlsVerifyingKey {
    /// Create from raw 32-byte public key
    pub fn from_bytes(bytes: &[u8; 32]) -> MlsResult<Self> {
        let inner = VerifyingKey::from_bytes(bytes)
            .map_err(|e| MlsError::CryptoError(format!("Invalid Ed25519 public key: {}", e)))?;

        Ok(Self { inner })
    }

    /// Verify a signature on data
    pub fn verify(&self, data: &[u8], signature: &[u8]) -> MlsResult<bool> {
        if signature.len() != 64 {
            return Ok(false);
        }

        let sig_array: [u8; 64] = signature
            .try_into()
            .map_err(|_| MlsError::CryptoError("Invalid signature length".to_string()))?;

        let signature = Signature::from_bytes(&sig_array);

        match self.inner.verify(data, &signature) {
            Ok(()) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    /// Export as bytes
    pub fn to_bytes(&self) -> [u8; 32] {
        self.inner.to_bytes()
    }
}

/// Helper function for signing in tests and examples
pub fn sign_with_key(data: &[u8], signing_key: &MlsSigningKey) -> MlsResult<Vec<u8>> {
    Ok(signing_key.sign(data))
}

/// Helper function for verification in tests and examples
pub fn verify_with_key(
    data: &[u8],
    signature: &[u8],
    verifying_key: &MlsVerifyingKey,
) -> MlsResult<bool> {
    verifying_key.verify(data, signature)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_generation() {
        let signing_key = MlsSigningKey::generate();
        let verifying_key = signing_key.verifying_key();

        // Keys should be valid
        assert_eq!(signing_key.to_bytes().len(), 32);
        assert_eq!(verifying_key.to_bytes().len(), 32);
    }

    #[test]
    fn test_sign_verify() {
        let signing_key = MlsSigningKey::generate();
        let verifying_key = signing_key.verifying_key();

        let data = b"Hello, MLS!";
        let signature = signing_key.sign(data);

        assert_eq!(signature.len(), 64); // Ed25519 signatures are 64 bytes
        assert!(verifying_key.verify(data, &signature).unwrap());
    }

    #[test]
    fn test_verify_wrong_signature() {
        let signing_key = MlsSigningKey::generate();
        let verifying_key = signing_key.verifying_key();

        let data = b"Hello, MLS!";
        let wrong_sig = vec![0u8; 64];

        assert!(!verifying_key.verify(data, &wrong_sig).unwrap());
    }

    #[test]
    fn test_verify_wrong_data() {
        let signing_key = MlsSigningKey::generate();
        let verifying_key = signing_key.verifying_key();

        let data = b"Hello, MLS!";
        let signature = signing_key.sign(data);

        let wrong_data = b"Wrong data";
        assert!(!verifying_key.verify(wrong_data, &signature).unwrap());
    }

    #[test]
    fn test_key_from_bytes() {
        let signing_key1 = MlsSigningKey::generate();
        let bytes = signing_key1.to_bytes();

        let signing_key2 = MlsSigningKey::from_bytes(&bytes);
        let verifying_key1 = signing_key1.verifying_key();
        let verifying_key2 = signing_key2.verifying_key();

        assert_eq!(verifying_key1.to_bytes(), verifying_key2.to_bytes());
    }

    #[test]
    fn test_deterministic_signatures() {
        let seed = [42u8; 32];
        let signing_key = MlsSigningKey::from_bytes(&seed);

        let data = b"Test data";
        let sig1 = signing_key.sign(data);
        let sig2 = signing_key.sign(data);

        // Ed25519 signatures are deterministic
        assert_eq!(sig1, sig2);
    }

    #[test]
    fn test_different_keys_different_signatures() {
        let key1 = MlsSigningKey::generate();
        let key2 = MlsSigningKey::generate();

        let data = b"Test data";
        let sig1 = key1.sign(data);
        let sig2 = key2.sign(data);

        // Different keys produce different signatures
        assert_ne!(sig1, sig2);

        // Each key can verify its own signature
        assert!(key1.verifying_key().verify(data, &sig1).unwrap());
        assert!(key2.verifying_key().verify(data, &sig2).unwrap());

        // But not the other's
        assert!(!key1.verifying_key().verify(data, &sig2).unwrap());
        assert!(!key2.verifying_key().verify(data, &sig1).unwrap());
    }
}
