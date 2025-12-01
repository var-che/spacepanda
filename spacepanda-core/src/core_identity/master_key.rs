//! Master Identity Key
//!
//! Long-term Ed25519 keypair representing the user's global identity.
//! This key:
//! - Signs device authorizations
//! - Derives per-channel pseudonyms
//! - Signs identity proofs
//! - Never rotates (user identity anchor)

use crate::core_identity::keypair::{Keypair, KeyType};
use serde::{Deserialize, Serialize};
use hkdf::Hkdf;
use sha2::Sha256;

/// Master identity key - the user's long-term identity
#[derive(Clone, Serialize, Deserialize)]
pub struct MasterKey {
    /// Ed25519 keypair for signing
    keypair: Keypair,
}

impl MasterKey {
    /// Generate a new master key
    pub fn generate() -> Self {
        MasterKey {
            keypair: Keypair::generate(KeyType::Ed25519),
        }
    }

    /// Get public key bytes
    pub fn public_key(&self) -> &[u8] {
        self.keypair.public_key()
    }

    /// Sign a message with the master key
    pub fn sign(&self, msg: &[u8]) -> Vec<u8> {
        self.keypair.sign(msg)
    }

    /// Verify a signature using this master key's public key
    pub fn verify(&self, msg: &[u8], sig: &[u8]) -> bool {
        Keypair::verify(self.public_key(), msg, sig)
    }

    /// Verify a signature using a provided public key (static)
    pub fn verify_with_pubkey(pubkey: &[u8], msg: &[u8], sig: &[u8]) -> bool {
        Keypair::verify(pubkey, msg, sig)
    }

    /// Derive a channel pseudonym using HKDF
    /// 
    /// This creates unlinkable pseudonyms per channel:
    /// - Deterministic (same channel → same pseudonym)
    /// - Unlinkable (different channels → unrelated pseudonyms)
    /// - Cannot reverse to master key
    pub fn derive_pseudonym(&self, channel_id: &str) -> Vec<u8> {
        let hk = Hkdf::<Sha256>::new(
            Some(b"spacepanda-channel-pseudonym-v1"),
            self.keypair.secret_key()
        );
        
        let mut okm = vec![0u8; 32];
        hk.expand(channel_id.as_bytes(), &mut okm)
            .expect("HKDF expand failed");
        
        okm
    }

    /// Serialize to bytes (for keystore)
    pub fn to_bytes(&self) -> Vec<u8> {
        self.keypair.serialize()
    }

    /// Deserialize from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        Ok(MasterKey {
            keypair: Keypair::deserialize(bytes)?,
        })
    }

    /// Export as JSON (for backup/export)
    pub fn to_json(&self) -> Result<String, String> {
        serde_json::to_string(&self.keypair)
            .map_err(|e| format!("JSON serialization failed: {}", e))
    }

    /// Import from JSON
    pub fn from_json(json: &str) -> Result<Self, String> {
        let keypair: Keypair = serde_json::from_str(json)
            .map_err(|e| format!("JSON deserialization failed: {}", e))?;
        
        if keypair.key_type != KeyType::Ed25519 {
            return Err("Master key must be Ed25519".to_string());
        }
        
        Ok(MasterKey { keypair })
    }
}

impl std::fmt::Debug for MasterKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MasterKey")
            .field("public", &hex::encode(self.public_key()))
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_master_key_generation() {
        let mk = MasterKey::generate();
        assert_eq!(mk.public_key().len(), 32);
    }

    #[test]
    fn test_master_key_sign_verify() {
        let mk = MasterKey::generate();
        let msg = b"test message";
        let sig = mk.sign(msg);
        
        assert!(mk.verify(msg, &sig));
        assert!(!mk.verify(b"wrong message", &sig));
    }

    #[test]
    fn test_pseudonym_deterministic() {
        let mk = MasterKey::generate();
        let p1 = mk.derive_pseudonym("channel-123");
        let p2 = mk.derive_pseudonym("channel-123");
        
        assert_eq!(p1, p2);
    }

    #[test]
    fn test_pseudonym_unlinkable() {
        let mk = MasterKey::generate();
        let p1 = mk.derive_pseudonym("channel-1");
        let p2 = mk.derive_pseudonym("channel-2");
        
        assert_ne!(p1, p2);
    }

    #[test]
    fn test_pseudonym_unique_per_user() {
        let mk1 = MasterKey::generate();
        let mk2 = MasterKey::generate();
        
        let p1 = mk1.derive_pseudonym("room-1337");
        let p2 = mk2.derive_pseudonym("room-1337");
        
        assert_ne!(p1, p2);
    }

    #[test]
    fn test_export_import_roundtrip() {
        let mk = MasterKey::generate();
        let json = mk.to_json().unwrap();
        let restored = MasterKey::from_json(&json).unwrap();
        
        assert_eq!(mk.public_key(), restored.public_key());
        
        // Verify signing works identically
        let msg = b"test";
        let sig = mk.sign(msg);
        assert!(restored.verify(msg, &sig));
    }
}
