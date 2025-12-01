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
    /// Signature counter for replay protection (increments with each signature)
    signature_counter: u64,
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
            signature_counter: 0,
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
    /// Returns (signature, counter) for replay protection
    pub fn sign(&mut self, msg: &[u8]) -> Result<(Vec<u8>, u64), String> {
        // Increment counter for replay protection
        self.signature_counter += 1;
        
        // Construct message with counter: version || counter || msg
        let mut full_msg = Vec::new();
        full_msg.extend_from_slice(&(self.current_version as u64).to_le_bytes());
        full_msg.extend_from_slice(&self.signature_counter.to_le_bytes());
        full_msg.extend_from_slice(msg);
        
        let signature = self.active_key.sign(&full_msg);
        Ok((signature, self.signature_counter))
    }
    
    /// Sign a message without replay protection (for testing/legacy)
    /// WARNING: Prefer `sign()` which includes counter
    pub fn sign_raw(&self, msg: &[u8]) -> Result<Vec<u8>, String> {
        Ok(self.active_key.sign(msg))
    }

    /// Verify a signature with replay protection
    /// Checks version, counter, and signature validity
    pub fn verify_with_counter(&self, msg: &[u8], sig: &[u8], version: KeyVersion, counter: u64) -> bool {
        // Reconstruct signed message: version || counter || msg
        let mut full_msg = Vec::new();
        full_msg.extend_from_slice(&(version as u64).to_le_bytes());
        full_msg.extend_from_slice(&counter.to_le_bytes());
        full_msg.extend_from_slice(msg);
        
        // Try current key if version matches
        if version == self.current_version {
            if Keypair::verify(self.active_key.public_key(), &full_msg, sig) {
                return true;
            }
        }
        
        // Try archived key for this version
        if let Some(archived_pubkey) = self.archived_keys.get(&version) {
            if Keypair::verify(archived_pubkey, &full_msg, sig) {
                return true;
            }
        }
        
        false
    }
    
    /// Verify a signature without counter (legacy/testing)
    /// This allows verifying old signatures even after rotation
    pub fn verify_raw(&self, msg: &[u8], sig: &[u8]) -> bool {
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
    /// Resets signature counter to 0
    pub fn rotate(&mut self, master_key: &MasterKey) -> DeviceKeyBinding {
        // Archive current key
        self.archived_keys.insert(
            self.current_version,
            self.active_key.public_key().to_vec()
        );
        
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
        self.signature_counter = 0; // Reset counter for new key
        
        DeviceKeyBinding {
            device_id: self.device_id.clone(),
            key_version: new_version,
            device_public_key: self.active_key.public_key().to_vec(),
            master_signature: new_binding,
        }
    }

    /// Check if a specific version is archived (rotated away)
    pub fn is_version_archived(&self, version: KeyVersion) -> bool {
        self.archived_keys.contains_key(&version)
    }
    
    /// Get current signature counter
    pub fn counter(&self) -> u64 {
        self.signature_counter
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
            .field("counter", &self.signature_counter)
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
        assert_eq!(dk.counter(), 0);
        assert_eq!(dk.public_key().len(), 32);
    }

    #[test]
    fn test_device_key_sign_verify() {
        let mk = MasterKey::generate();
        let mut dk = DeviceKey::generate(&mk);
        
        let msg = b"test message";
        let (sig, counter) = dk.sign(msg).unwrap();
        
        assert_eq!(counter, 1);
        assert!(dk.verify_with_counter(msg, &sig, dk.version(), counter));
        assert!(!dk.verify_with_counter(b"wrong message", &sig, dk.version(), counter));
    }

    #[test]
    fn test_device_key_rotation() {
        let mk = MasterKey::generate();
        let mut dk = DeviceKey::generate(&mk);
        
        let msg = b"old message";
        let (old_sig, old_counter) = dk.sign(msg).unwrap();
        let old_pubkey = dk.public_key().to_vec();
        let old_version = dk.version();
        
        // Rotate
        let new_binding = dk.rotate(&mk);
        
        // New version incremented
        assert_eq!(dk.version(), old_version + 1);
        assert_ne!(dk.public_key(), old_pubkey.as_slice());
        
        // Counter reset after rotation
        assert_eq!(dk.counter(), 0);
        
        // Old signature still verifiable (archived) with correct version
        assert!(dk.verify_with_counter(msg, &old_sig, old_version, old_counter));
        
        // Old version is now archived
        assert!(dk.is_version_archived(old_version));
        
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
