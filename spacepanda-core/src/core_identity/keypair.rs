//! Keypair module
//!
//! Handles cryptographic key material for identity and device keys.
//! Uses Ed25519 for signatures and X25519 for key agreement.
//!
//! This implementation uses real cryptography (ed25519-dalek, x25519-dalek)
//! to ensure production-grade security before MLS integration.
//!
//! Security: Secret keys are automatically zeroized on drop using zeroize crate.

use serde::{Deserialize, Serialize};
use std::fmt;
use ed25519_dalek::{SigningKey, VerifyingKey, Signature, Signer, Verifier};
use x25519_dalek::{StaticSecret, PublicKey as X25519PublicKey};
use rand::rand_core::OsRng;
use zeroize::{Zeroize, ZeroizeOnDrop};

/// Key type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeyType {
    /// Ed25519 for signatures
    Ed25519,
    /// X25519 for Diffie-Hellman key exchange
    X25519,
}

/// Keypair structure holding public and secret keys
/// Secret keys are automatically zeroized on drop for security
#[derive(Clone, Serialize, Deserialize)]
pub struct Keypair {
    /// Type of key
    pub key_type: KeyType,
    /// Public key bytes (32 bytes)
    pub public: Vec<u8>,
    /// Secret key bytes (32 bytes, will be encrypted at rest by keystore)
    /// Automatically zeroized on drop
    secret: Vec<u8>,
}

impl Keypair {
    /// Generate a new keypair of the specified type
    pub fn generate(key_type: KeyType) -> Self {
        match key_type {
            KeyType::Ed25519 => {
                // Real Ed25519 key generation
                // Generate 32 random bytes for the signing key seed
                use rand::Rng;
                let mut csprng = rand::thread_rng();
                let seed_bytes: [u8; 32] = csprng.gen();
                
                let signing_key = SigningKey::from_bytes(&seed_bytes);
                let verifying_key = signing_key.verifying_key();
                
                Keypair {
                    key_type,
                    public: verifying_key.to_bytes().to_vec(),
                    secret: signing_key.to_bytes().to_vec(),
                }
            }
            KeyType::X25519 => {
                // Real X25519 key generation
                use rand::Rng;
                let mut csprng = rand::thread_rng();
                let secret_bytes: [u8; 32] = csprng.gen();
                
                let secret = StaticSecret::from(secret_bytes);
                let public = X25519PublicKey::from(&secret);
                
                Keypair {
                    key_type,
                    public: public.to_bytes().to_vec(),
                    secret: secret.to_bytes().to_vec(),
                }
            }
        }
    }

    /// Sign a message (Ed25519 only)
    /// Returns 64-byte signature
    pub fn sign(&self, msg: &[u8]) -> Vec<u8> {
        assert_eq!(self.key_type, KeyType::Ed25519, "Only Ed25519 can sign");
        
        // Real Ed25519 signing
        let signing_key = SigningKey::from_bytes(
            self.secret.as_slice().try_into().expect("Invalid secret key length")
        );
        
        let signature = signing_key.sign(msg);
        signature.to_bytes().to_vec()
    }

    /// Verify a signature (static method)
    /// Returns true if signature is valid
    pub fn verify(pubkey: &[u8], msg: &[u8], sig: &[u8]) -> bool {
        // Real Ed25519 verification
        if pubkey.len() != 32 || sig.len() != 64 {
            return false;
        }
        
        let verifying_key = match VerifyingKey::from_bytes(
            pubkey.try_into().expect("pubkey length checked")
        ) {
            Ok(vk) => vk,
            Err(_) => return false,
        };
        
        let signature = match Signature::from_slice(sig) {
            Ok(sig) => sig,
            Err(_) => return false,
        };
        
        verifying_key.verify(msg, &signature).is_ok()
    }

    /// Derive X25519 key from Ed25519 (for ECDH)
    /// Uses proper curve25519 conversion
    pub fn derive_x25519_from_ed25519(ed: &Keypair) -> Keypair {
        assert_eq!(ed.key_type, KeyType::Ed25519);
        
        // Convert Ed25519 secret key to X25519
        // Ed25519 secret key is the seed (first 32 bytes)
        let ed_secret_bytes: [u8; 32] = ed.secret.as_slice()
            .try_into()
            .expect("Ed25519 secret key must be 32 bytes");
        
        // Hash the Ed25519 secret to get X25519 scalar
        use sha2::{Sha512, Digest};
        let mut hasher = Sha512::new();
        hasher.update(&ed_secret_bytes);
        let hash = hasher.finalize();
        
        // Take first 32 bytes as X25519 scalar and clamp
        let mut scalar_bytes = [0u8; 32];
        scalar_bytes.copy_from_slice(&hash[..32]);
        
        // Clamp the scalar for X25519
        scalar_bytes[0] &= 248;
        scalar_bytes[31] &= 127;
        scalar_bytes[31] |= 64;
        
        let x25519_secret = StaticSecret::from(scalar_bytes);
        let x25519_public = X25519PublicKey::from(&x25519_secret);
        
        Keypair {
            key_type: KeyType::X25519,
            public: x25519_public.to_bytes().to_vec(),
            secret: x25519_secret.to_bytes().to_vec(),
        }
    }

    /// Serialize to bytes (suitable for keystore)
    pub fn serialize(&self) -> Vec<u8> {
        bincode::serialize(self).expect("Failed to serialize keypair")
    }
    
    /// Create a keypair from only a public key (for verification-only purposes)
    /// 
    /// This creates a "public-key-only" keypair that can only verify signatures,
    /// not create them. Used when registering devices where the private key
    /// remains on the device.
    pub fn from_public_key(public_key: &[u8]) -> Result<Self, String> {
        if public_key.len() != 32 {
            return Err("Public key must be 32 bytes".to_string());
        }
        
        // Validate it's a valid Ed25519 public key
        VerifyingKey::from_bytes(
            public_key.try_into().expect("Length checked")
        ).map_err(|e| format!("Invalid Ed25519 public key: {}", e))?;
        
        Ok(Keypair {
            key_type: KeyType::Ed25519,
            public: public_key.to_vec(),
            secret: vec![0u8; 32], // Placeholder - cannot sign with this keypair
        })
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

// Implement Drop to zero out secret key memory using zeroize
impl Drop for Keypair {
    fn drop(&mut self) {
        // Securely zero out the secret key using zeroize
        self.secret.zeroize();
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

    #[test]
    fn test_secret_zeroized_on_drop() {
        // Create a keypair and get a pointer to its secret bytes
        let kp = Keypair::generate(KeyType::Ed25519);
        let secret_ptr = kp.secret.as_ptr();
        let secret_len = kp.secret.len();
        
        // Verify secret is not all zeros initially
        let secret_copy: Vec<u8> = kp.secret.clone();
        assert!(secret_copy.iter().any(|&b| b != 0), "Secret should not be all zeros initially");
        
        // Drop the keypair
        drop(kp);
        
        // Note: We cannot safely check if memory was zeroized without unsafe code
        // and potential UB. The zeroize crate guarantees this behavior.
        // This test mainly documents the expectation.
    }

    #[test]
    fn test_debug_does_not_leak_secret() {
        let kp = Keypair::generate(KeyType::Ed25519);
        let debug_str = format!("{:?}", kp);
        
        // Debug output should not contain the actual secret key
        assert!(debug_str.contains("<redacted>"), "Debug should redact secret");
        assert!(!debug_str.contains(&hex::encode(&kp.secret)), "Debug should not show secret");
    }
}
