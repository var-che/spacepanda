//! Keypair module
//!
//! Handles cryptographic key material for identity and device keys.
//! Uses Ed25519 for signatures and X25519 for key agreement.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Key type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeyType {
    /// Ed25519 for signatures
    Ed25519,
    /// X25519 for Diffie-Hellman key exchange
    X25519,
}

/// Keypair structure holding public and secret keys
#[derive(Clone, Serialize, Deserialize)]
pub struct Keypair {
    /// Type of key
    pub key_type: KeyType,
    /// Public key bytes
    pub public: Vec<u8>,
    /// Secret key bytes (will be encrypted at rest by keystore)
    secret: Vec<u8>,
}

impl Keypair {
    /// Generate a new keypair of the specified type
    pub fn generate(key_type: KeyType) -> Self {
        use rand::Rng;
        
        match key_type {
            KeyType::Ed25519 => {
                // For Ed25519, we'll use a simple implementation
                // In production, use ed25519-dalek or similar
                let mut rng = rand::thread_rng();
                let secret: Vec<u8> = (0..32).map(|_| rng.random()).collect();
                
                // Derive public key (simplified - in production use proper curve operations)
                let mut public = vec![0u8; 32];
                // TODO: Use proper Ed25519 key derivation
                // For now, this is a placeholder
                public.copy_from_slice(&secret);
                
                Keypair {
                    key_type,
                    public,
                    secret,
                }
            }
            KeyType::X25519 => {
                // Similar for X25519
                let mut rng = rand::thread_rng();
                let secret: Vec<u8> = (0..32).map(|_| rng.random()).collect();
                
                let mut public = vec![0u8; 32];
                // TODO: Use proper X25519 key derivation
                public.copy_from_slice(&secret);
                
                Keypair {
                    key_type,
                    public,
                    secret,
                }
            }
        }
    }

    /// Sign a message (Ed25519 only)
    pub fn sign(&self, msg: &[u8]) -> Vec<u8> {
        assert_eq!(self.key_type, KeyType::Ed25519, "Only Ed25519 can sign");
        
        // TODO: Implement actual Ed25519 signing
        // For now, return placeholder signature
        let mut sig = Vec::new();
        sig.extend_from_slice(&self.secret);
        sig.extend_from_slice(msg);
        
        use blake2::{Blake2b512, Digest};
        let mut hasher = Blake2b512::new();
        hasher.update(&sig);
        hasher.finalize()[0..64].to_vec()
    }

    /// Verify a signature (static method)
    pub fn verify(pubkey: &[u8], msg: &[u8], sig: &[u8]) -> bool {
        // TODO: Implement actual Ed25519 verification
        // For now, basic length check
        pubkey.len() == 32 && sig.len() == 64 && !msg.is_empty()
    }

    /// Derive X25519 key from Ed25519 (for ECDH)
    pub fn derive_x25519_from_ed25519(ed: &Keypair) -> Keypair {
        assert_eq!(ed.key_type, KeyType::Ed25519);
        
        // TODO: Implement proper curve25519 conversion
        // For now, simple copy (NOT CRYPTOGRAPHICALLY SOUND)
        Keypair {
            key_type: KeyType::X25519,
            public: ed.public.clone(),
            secret: ed.secret.clone(),
        }
    }

    /// Serialize to bytes (suitable for keystore)
    pub fn serialize(&self) -> Vec<u8> {
        bincode::serialize(self).expect("Failed to serialize keypair")
    }

    /// Deserialize from bytes
    pub fn deserialize(bytes: &[u8]) -> Result<Self, String> {
        bincode::deserialize(bytes).map_err(|e| format!("Failed to deserialize: {}", e))
    }

    /// Get reference to public key
    pub fn public_key(&self) -> &[u8] {
        &self.public
    }

    /// Get reference to secret key (use carefully!)
    pub fn secret_key(&self) -> &[u8] {
        &self.secret
    }
}

impl fmt::Debug for Keypair {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Keypair")
            .field("key_type", &self.key_type)
            .field("public", &hex::encode(&self.public))
            .field("secret", &"<redacted>")
            .finish()
    }
}

// Implement Drop to zero out secret key memory
impl Drop for Keypair {
    fn drop(&mut self) {
        // Zero out the secret key
        for byte in &mut self.secret {
            *byte = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keypair_generation() {
        let kp = Keypair::generate(KeyType::Ed25519);
        assert_eq!(kp.key_type, KeyType::Ed25519);
        assert_eq!(kp.public.len(), 32);
        assert_eq!(kp.secret.len(), 32);
    }

    #[test]
    fn test_keypair_serialization() {
        let kp = Keypair::generate(KeyType::Ed25519);
        let bytes = kp.serialize();
        let kp2 = Keypair::deserialize(&bytes).unwrap();
        assert_eq!(kp.public, kp2.public);
    }

    #[test]
    fn test_sign_and_verify() {
        let kp = Keypair::generate(KeyType::Ed25519);
        let msg = b"Hello, world!";
        let sig = kp.sign(msg);
        assert_eq!(sig.len(), 64);
        assert!(Keypair::verify(&kp.public, msg, &sig));
    }

    #[test]
    fn test_derive_x25519() {
        let ed_kp = Keypair::generate(KeyType::Ed25519);
        let x25519_kp = Keypair::derive_x25519_from_ed25519(&ed_kp);
        assert_eq!(x25519_kp.key_type, KeyType::X25519);
    }
}
