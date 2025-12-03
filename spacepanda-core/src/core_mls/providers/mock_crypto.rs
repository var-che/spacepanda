//! Mock Crypto Provider
//!
//! Deterministic implementation for testing.

use crate::core_mls::errors::MlsResult;
use crate::core_mls::traits::crypto::CryptoProvider;
use async_trait::async_trait;
use sha2::{Digest, Sha256};

/// Mock crypto provider with deterministic operations (for testing)
pub struct MockCryptoProvider {
    /// Seed for deterministic random number generation
    seed: u64,
}

impl MockCryptoProvider {
    /// Create a new mock crypto provider
    pub fn new(seed: u64) -> Self {
        Self { seed }
    }

    /// Simple deterministic "random" number generator
    fn deterministic_bytes(&self, n: usize, offset: u64) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(n);
        let mut state = self.seed.wrapping_add(offset);

        for _ in 0..n {
            // Simple LCG
            state = state.wrapping_mul(1103515245).wrapping_add(12345);
            bytes.push((state >> 16) as u8);
        }

        bytes
    }
}

impl Default for MockCryptoProvider {
    fn default() -> Self {
        Self::new(42)
    }
}

#[async_trait]
impl CryptoProvider for MockCryptoProvider {
    async fn random_bytes(&self, n: usize) -> MlsResult<Vec<u8>> {
        Ok(self.deterministic_bytes(n, 0))
    }

    async fn sign(&self, message: &[u8]) -> MlsResult<Vec<u8>> {
        // Mock signature: hash(seed || message)
        let mut hasher = Sha256::new();
        hasher.update(&self.seed.to_le_bytes());
        hasher.update(message);
        Ok(hasher.finalize().to_vec())
    }

    async fn verify(&self, _public_key: &[u8], message: &[u8], signature: &[u8]) -> MlsResult<()> {
        // Mock verification: recompute signature and compare
        let expected = self.sign(message).await?;
        if expected == signature {
            Ok(())
        } else {
            Err(crate::core_mls::errors::MlsError::CryptoError(
                "Mock signature verification failed".to_string(),
            ))
        }
    }

    async fn hpke_seal(
        &self,
        _recipient_pub: &[u8],
        info: &[u8],
        plaintext: &[u8],
    ) -> MlsResult<Vec<u8>> {
        // Mock HPKE: XOR with deterministic key
        let key = self.deterministic_bytes(plaintext.len(), info.len() as u64);
        let ciphertext: Vec<u8> = plaintext.iter().zip(key.iter()).map(|(p, k)| p ^ k).collect();
        Ok(ciphertext)
    }

    async fn hpke_open(
        &self,
        _recipient_priv: &[u8],
        _sender_enc: &[u8],
        info: &[u8],
        ciphertext: &[u8],
    ) -> MlsResult<Vec<u8>> {
        // Mock HPKE: XOR with same deterministic key (symmetric)
        let key = self.deterministic_bytes(ciphertext.len(), info.len() as u64);
        let plaintext: Vec<u8> = ciphertext.iter().zip(key.iter()).map(|(c, k)| c ^ k).collect();
        Ok(plaintext)
    }

    async fn hkdf_expand(&self, prk: &[u8], info: &[u8], len: usize) -> MlsResult<Vec<u8>> {
        // Mock HKDF: hash(prk || info) repeated
        let mut hasher = Sha256::new();
        hasher.update(prk);
        hasher.update(info);
        let base = hasher.finalize();

        let mut result = Vec::with_capacity(len);
        let mut counter = 0u8;

        while result.len() < len {
            let mut hasher = Sha256::new();
            hasher.update(&base);
            hasher.update(&[counter]);
            let chunk = hasher.finalize();

            let remaining = len - result.len();
            let to_take = remaining.min(chunk.len());
            result.extend_from_slice(&chunk[..to_take]);

            counter = counter.wrapping_add(1);
        }

        Ok(result)
    }

    async fn hash(&self, data: &[u8]) -> MlsResult<Vec<u8>> {
        Ok(Sha256::digest(data).to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_deterministic_random() {
        let provider1 = MockCryptoProvider::new(42);
        let provider2 = MockCryptoProvider::new(42);

        let bytes1 = provider1.random_bytes(32).await.unwrap();
        let bytes2 = provider2.random_bytes(32).await.unwrap();

        assert_eq!(bytes1, bytes2); // Should be deterministic
    }

    #[tokio::test]
    async fn test_sign_verify() {
        let provider = MockCryptoProvider::default();
        let message = b"test message";

        let signature = provider.sign(message).await.unwrap();
        provider.verify(&[], message, &signature).await.unwrap();
    }

    #[tokio::test]
    async fn test_hpke_roundtrip() {
        let provider = MockCryptoProvider::default();
        let plaintext = b"secret data";
        let info = b"context";

        let ciphertext = provider.hpke_seal(&[], info, plaintext).await.unwrap();
        let decrypted = provider.hpke_open(&[], &[], info, &ciphertext).await.unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[tokio::test]
    async fn test_hkdf_expand() {
        let provider = MockCryptoProvider::default();
        let prk = b"pseudo-random key";
        let info = b"info";

        let okm = provider.hkdf_expand(prk, info, 64).await.unwrap();
        assert_eq!(okm.len(), 64);
    }
}
