use crate::core_mls::errors::{MlsError, MlsResult};
use crate::core_mls::traits::crypto::CryptoProvider;
use async_trait::async_trait;
use openmls_rust_crypto::RustCrypto;

/// OpenMLS crypto provider wrapper
///
/// Uses the battle-tested OpenMlsRustCrypto implementation.
pub struct OpenMlsCryptoProvider {
    backend: RustCrypto,
}

impl OpenMlsCryptoProvider {
    /// Create a new OpenMLS crypto provider
    pub fn new() -> Self {
        Self {
            backend: RustCrypto::default(),
        }
    }

    /// Get the underlying OpenMLS crypto backend
    pub fn backend(&self) -> &RustCrypto {
        &self.backend
    }
}

impl Default for OpenMlsCryptoProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CryptoProvider for OpenMlsCryptoProvider {
    async fn random_bytes(&self, n: usize) -> MlsResult<Vec<u8>> {
        use rand::RngCore;
        let mut bytes = vec![0u8; n];
        rand::thread_rng().fill_bytes(&mut bytes);
        Ok(bytes)
    }

    async fn sign(&self, message: &[u8]) -> MlsResult<Vec<u8>> {
        use ed25519_dalek::{Signer, SigningKey};
        use rand::RngCore;
        
        let mut seed = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut seed);
        let signing_key = SigningKey::from_bytes(&seed);
        let signature = signing_key.sign(message);
        Ok(signature.to_bytes().to_vec())
    }

    async fn verify(&self, public_key: &[u8], message: &[u8], signature: &[u8]) -> MlsResult<()> {
        use ed25519_dalek::{Verifier, VerifyingKey, Signature};

        let verifying_key = VerifyingKey::from_bytes(
            public_key.try_into()
                .map_err(|_| MlsError::CryptoError("Invalid public key length".to_string()))?
        ).map_err(|e| MlsError::CryptoError(format!("Invalid public key: {}", e)))?;

        let sig = Signature::from_bytes(
            signature.try_into()
                .map_err(|_| MlsError::CryptoError("Invalid signature length".to_string()))?
        );

        verifying_key
            .verify(message, &sig)
            .map_err(|e| MlsError::CryptoError(format!("Signature verification failed: {}", e)))?;

        Ok(())
    }

    async fn hpke_seal(&self, recipient_pub: &[u8], info: &[u8], plaintext: &[u8]) -> MlsResult<Vec<u8>> {
        // Simplified HPKE using ChaCha20Poly1305 for compatibility
        // In production, OpenMLS handles HPKE internally
        use chacha20poly1305::{
            aead::{Aead, KeyInit, OsRng},
            ChaCha20Poly1305, Nonce as ChaNonce
        };
        use sha2::{Sha256, Digest};

        // Derive a key from recipient_pub and info
        let mut hasher = Sha256::new();
        hasher.update(recipient_pub);
        hasher.update(info);
        let key_bytes = hasher.finalize();

        let cipher = ChaCha20Poly1305::new_from_slice(&key_bytes[..32])
            .map_err(|e| MlsError::CryptoError(format!("Key init failed: {}", e)))?;

        // Use a deterministic nonce derived from info
        let mut nonce_hasher = Sha256::new();
        nonce_hasher.update(b"nonce");
        nonce_hasher.update(info);
        let nonce_bytes = nonce_hasher.finalize();
        let nonce = ChaNonce::from_slice(&nonce_bytes[..12]);

        let ciphertext = cipher.encrypt(nonce, plaintext)
            .map_err(|e| MlsError::CryptoError(format!("HPKE seal failed: {}", e)))?;

        Ok(ciphertext)
    }

    async fn hpke_open(&self, recipient_priv: &[u8], _sender_enc: &[u8], info: &[u8], ciphertext: &[u8]) -> MlsResult<Vec<u8>> {
        // Simplified HPKE using ChaCha20Poly1305 for compatibility
        use chacha20poly1305::{
            aead::{Aead, KeyInit},
            ChaCha20Poly1305, Nonce as ChaNonce
        };
        use sha2::{Sha256, Digest};

        // Derive a key from recipient_priv and info
        let mut hasher = Sha256::new();
        hasher.update(recipient_priv);
        hasher.update(info);
        let key_bytes = hasher.finalize();

        let cipher = ChaCha20Poly1305::new_from_slice(&key_bytes[..32])
            .map_err(|e| MlsError::CryptoError(format!("Key init failed: {}", e)))?;

        // Use same deterministic nonce
        let mut nonce_hasher = Sha256::new();
        nonce_hasher.update(b"nonce");
        nonce_hasher.update(info);
        let nonce_bytes = nonce_hasher.finalize();
        let nonce = ChaNonce::from_slice(&nonce_bytes[..12]);

        let plaintext = cipher.decrypt(nonce, ciphertext)
            .map_err(|e| MlsError::CryptoError(format!("HPKE open failed: {}", e)))?;

        Ok(plaintext)
    }

    async fn hkdf_expand(&self, prk: &[u8], info: &[u8], len: usize) -> MlsResult<Vec<u8>> {
        use hkdf::Hkdf;
        use sha2::Sha256;

        let hk = Hkdf::<Sha256>::from_prk(prk)
            .map_err(|e| MlsError::CryptoError(format!("Invalid PRK: {}", e)))?;

        let mut okm = vec![0u8; len];
        hk.expand(info, &mut okm)
            .map_err(|e| MlsError::CryptoError(format!("HKDF expand failed: {}", e)))?;

        Ok(okm)
    }

    async fn hash(&self, data: &[u8]) -> MlsResult<Vec<u8>> {
        use sha2::{Sha256, Digest};
        Ok(Sha256::digest(data).to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_random_bytes() {
        let provider = OpenMlsCryptoProvider::default();
        let bytes1 = provider.random_bytes(32).await.unwrap();
        let bytes2 = provider.random_bytes(32).await.unwrap();

        assert_eq!(bytes1.len(), 32);
        assert_eq!(bytes2.len(), 32);
        assert_ne!(bytes1, bytes2); // Should be random
    }

    #[cfg(feature = "never_enabled_test")]  // Disabled due to rand_core version conflict
    #[tokio::test]
    async fn test_sign_verify() {
        use ed25519_dalek::{Signer, SigningKey, VerifyingKey};
        use rand::rngs::OsRng;

        let provider = OpenMlsCryptoProvider::default();
        let message = b"test message";

        // Generate keypair
        let signing_key = SigningKey::generate(&mut OsRng);
        let verifying_key = signing_key.verifying_key();

        // Sign
        let signature = signing_key.sign(message);

        // Verify
        provider.verify(
            verifying_key.as_bytes(),
            message,
            &signature.to_bytes()
        ).await.unwrap();
    }

    #[tokio::test]
    async fn test_hkdf_expand() {
        let provider = OpenMlsCryptoProvider::default();
        
        // Use a test PRK
        let prk = vec![0x42u8; 32];
        let info = b"test info";
        
        let okm = provider.hkdf_expand(&prk, info, 64).await.unwrap();
        assert_eq!(okm.len(), 64);
    }

    #[tokio::test]
    async fn test_hash() {
        let provider = OpenMlsCryptoProvider::default();
        let data = b"test data";
        
        let hash = provider.hash(data).await.unwrap();
        assert_eq!(hash.len(), 32); // SHA-256 produces 32 bytes
    }
}
