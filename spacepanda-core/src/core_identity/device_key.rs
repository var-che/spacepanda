//! Device Key Module
//!
//! Per-device Ed25519 keypairs with rotation support.
//! Each device has:
//! - Current active keypair
//! - Archive of rotated keypairs (for historical signature verification)
//! - Master key binding (authorization proof)
//! - Rotation prevents future signing but preserves verification

use crate::core_identity::keypair::{Keypair, KeyType};
use crate::core_identity::master_key::MasterKey;
use crate::core_identity::device_id::DeviceId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Device key version number
pub type KeyVersion = u64;

/// Device keypair with rotation support
#[derive(Clone, Serialize, Deserialize)]
pub struct DeviceKey {
    /// Device identifier (stable across rotations)
    pub device_id: DeviceId,
    /// Current active key version
    current_version: KeyVersion,
    /// Active keypair for signing
    active_key: Keypair,
    /// Archived keypairs (for verifying old signatures)
    /// Maps version → public key
    archived_keys: HashMap<KeyVersion, Vec<u8>>,
    /// Master key binding signature
    /// Signs: device_id || current_version || active_public_key
    master_binding: Vec<u8>,
    /// Flag indicating if this key has been rotated (cannot sign anymore)
    is_rotated: bool,
}

/// Device key binding - proves device is authorized by master key
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceKeyBinding {
    pub device_id: DeviceId,
    pub key_version: KeyVersion,
    pub device_public_key: Vec<u8>,
    pub master_signature: Vec<u8>,
}

impl DeviceKeyBinding {
    /// Create a new device key binding signed by master key
    pub fn new(master_key: &MasterKey, device_id: DeviceId, key_version: KeyVersion, device_public_key: Vec<u8>) -> Self {
        // Construct binding message: device_id || version || public_key
        let mut msg = Vec::new();
        msg.extend_from_slice(device_id.as_bytes());
        msg.extend_from_slice(&(key_version as u64).to_le_bytes());
        msg.extend_from_slice(&device_public_key);
        
        let master_signature = master_key.sign(&msg);
        
        DeviceKeyBinding {
            device_id,
            key_version,
            device_public_key,
            master_signature,
        }
    }

    /// Verify the binding against a master public key
    pub fn verify(&self, master_pubkey: &[u8]) -> bool {
        let mut msg = Vec::new();
        msg.extend_from_slice(self.device_id.as_bytes());
        msg.extend_from_slice(&(self.key_version as u64).to_le_bytes());
        msg.extend_from_slice(&self.device_public_key);
        
        MasterKey::verify_with_pubkey(master_pubkey, &msg, &self.master_signature)
    }
}

impl DeviceKey {
    /// Generate a new device key authorized by master key
    pub fn generate(master_key: &MasterKey) -> Self {
        let device_id = DeviceId::generate();
        let active_key = Keypair::generate(KeyType::Ed25519);
        let current_version = 1;
        
        // Create master binding
        let mut msg = Vec::new();
        msg.extend_from_slice(device_id.as_bytes());
        msg.extend_from_slice(&(current_version as u64).to_le_bytes());
        msg.extend_from_slice(active_key.public_key());
        let master_binding = master_key.sign(&msg);
        
        DeviceKey {
            device_id,
            current_version,
            active_key,
            archived_keys: HashMap::new(),
            master_binding,
            is_rotated: false,
        }
    }

    /// Get the device ID
    pub fn device_id(&self) -> &DeviceId {
        &self.device_id
    }

    /// Get current key version
    pub fn version(&self) -> KeyVersion {
        self.current_version
    }

    /// Get active public key
    pub fn public_key(&self) -> &[u8] {
        self.active_key.public_key()
    }

    /// Sign a message with the current active key
    /// Fails if the key has been rotated
    pub fn sign(&self, msg: &[u8]) -> Result<Vec<u8>, String> {
        if self.is_rotated {
            return Err("Cannot sign with rotated key - device key has been replaced".to_string());
        }
        
        Ok(self.active_key.sign(msg))
    }

    /// Verify a signature using current or archived keys
    /// This allows verifying old signatures even after rotation
    pub fn verify(&self, msg: &[u8], sig: &[u8]) -> bool {
        // Try current key first
        if Keypair::verify(self.active_key.public_key(), msg, sig) {
            return true;
        }
        
        // Try archived keys
        for archived_pubkey in self.archived_keys.values() {
            if Keypair::verify(archived_pubkey, msg, sig) {
                return true;
            }
        }
        
        false
    }

    /// Rotate the device key (creates new keypair, archives old one)
    /// Master key must re-sign the new binding
    pub fn rotate(&mut self, master_key: &MasterKey) -> DeviceKeyBinding {
        // Archive current key
        self.archived_keys.insert(
            self.current_version,
            self.active_key.public_key().to_vec()
        );
        
        // Mark current key as rotated (cannot sign anymore)
        self.is_rotated = true;
        
        // Generate new keypair
        let new_key = Keypair::generate(KeyType::Ed25519);
        let new_version = self.current_version + 1;
        
        // Create new master binding
        let mut msg = Vec::new();
        msg.extend_from_slice(self.device_id.as_bytes());
        msg.extend_from_slice(&(new_version as u64).to_le_bytes());
        msg.extend_from_slice(new_key.public_key());
        let new_binding = master_key.sign(&msg);
        
        // Update state
        self.current_version = new_version;
        self.active_key = new_key;
        self.master_binding = new_binding.clone();
        self.is_rotated = false; // New key is now active
        
        DeviceKeyBinding {
            device_id: self.device_id.clone(),
            key_version: new_version,
            device_public_key: self.active_key.public_key().to_vec(),
            master_signature: new_binding,
        }
    }

    /// Check if this device key is currently active (can sign)
    pub fn is_active(&self) -> bool {
        !self.is_rotated
    }

    /// Get the device key binding (for advertisement)
    pub fn binding(&self) -> DeviceKeyBinding {
        DeviceKeyBinding {
            device_id: self.device_id.clone(),
            key_version: self.current_version,
            device_public_key: self.active_key.public_key().to_vec(),
            master_signature: self.master_binding.clone(),
        }
    }

    /// Export to bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        bincode::serialize(self).expect("Failed to serialize device key")
    }

    /// Import from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        bincode::deserialize(bytes).map_err(|e| format!("Failed to deserialize: {}", e))
    }
}

impl std::fmt::Debug for DeviceKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeviceKey")
            .field("device_id", &self.device_id)
            .field("version", &self.current_version)
            .field("public", &hex::encode(self.public_key()))
            .field("is_active", &self.is_active())
            .field("archived_count", &self.archived_keys.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_key_generation() {
        let mk = MasterKey::generate();
        let dk = DeviceKey::generate(&mk);
        
        assert_eq!(dk.version(), 1);
        assert!(dk.is_active());
        assert_eq!(dk.public_key().len(), 32);
    }

    #[test]
    fn test_device_key_sign_verify() {
        let mk = MasterKey::generate();
        let dk = DeviceKey::generate(&mk);
        
        let msg = b"test message";
        let sig = dk.sign(msg).unwrap();
        
        assert!(dk.verify(msg, &sig));
        assert!(!dk.verify(b"wrong message", &sig));
    }

    #[test]
    fn test_device_key_rotation() {
        let mk = MasterKey::generate();
        let mut dk = DeviceKey::generate(&mk);
        
        let msg = b"old message";
        let old_sig = dk.sign(msg).unwrap();
        let old_pubkey = dk.public_key().to_vec();
        let old_version = dk.version();
        
        // Rotate
        let new_binding = dk.rotate(&mk);
        
        // New version incremented
        assert_eq!(dk.version(), old_version + 1);
        assert_ne!(dk.public_key(), old_pubkey.as_slice());
        
        // Old signature still verifiable (archived)
        assert!(dk.verify(msg, &old_sig));
        
        // Cannot sign with rotated key before rotation
        // (the old DeviceKey instance would be marked is_rotated=true if we kept it)
        
        // New binding verifies
        assert!(new_binding.verify(mk.public_key()));
    }

    #[test]
    fn test_binding_verification() {
        let mk = MasterKey::generate();
        let dk = DeviceKey::generate(&mk);
        
        let binding = dk.binding();
        assert!(binding.verify(mk.public_key()));
        
        // Wrong master key fails
        let mk2 = MasterKey::generate();
        assert!(!binding.verify(mk2.public_key()));
    }
}
