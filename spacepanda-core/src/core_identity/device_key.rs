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
use rand::Rng;

/// Device key version number
pub type KeyVersion = u64;

/// Challenge for device proof-of-possession
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceChallenge {
    /// Random nonce (32 bytes)
    pub nonce: Vec<u8>,
    /// Timestamp when challenge was created
    pub timestamp: u64,
    /// Device ID this challenge is for
    pub device_id: DeviceId,
}

impl DeviceChallenge {
    /// Generate a new challenge for a device
    pub fn generate(device_id: DeviceId) -> Self {
        let mut rng = rand::rng();
        let mut nonce = vec![0u8; 32];
        rng.fill(&mut nonce[..]);
        
        DeviceChallenge {
            nonce,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("Time went backwards")
                .as_secs(),
            device_id,
        }
    }
    
    /// Get the message to be signed for proof-of-possession
    pub fn to_message(&self) -> Vec<u8> {
        let mut msg = Vec::new();
        msg.extend_from_slice(b"DEVICE_PROOF_OF_POSSESSION_V1:");
        msg.extend_from_slice(&self.nonce);
        msg.extend_from_slice(&self.timestamp.to_le_bytes());
        msg.extend_from_slice(self.device_id.as_bytes());
        msg
    }
}

/// Proof of possession response from device
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofOfPossession {
    /// The challenge that was signed
    pub challenge: DeviceChallenge,
    /// Device's signature over the challenge
    pub signature: Vec<u8>,
    /// Device public key that signed the challenge
    pub device_public_key: Vec<u8>,
}

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
    /// 
    /// ⚠️ DEPRECATED: Use `register_with_proof_of_possession` instead for security.
    /// This method does not verify device ownership and should only be used in tests.
    #[deprecated(since = "0.2.0", note = "Use register_with_proof_of_possession for production")]
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
    
    /// Register a new device with proof-of-possession (SECURE)
    /// 
    /// This is a 3-step protocol:
    /// 1. Device generates keypair locally (private key never leaves device)
    /// 2. Master generates challenge for device's public key
    /// 3. Device signs challenge and master validates proof before creating binding
    /// 
    /// Returns DeviceKey if proof is valid, error otherwise.
    pub fn register_with_proof_of_possession(
        master_key: &MasterKey,
        proof: &ProofOfPossession,
    ) -> Result<Self, String> {
        // Validate proof of possession
        Self::validate_proof_of_possession(proof)?;
        
        // Check challenge age (reject if > 5 minutes old)
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs();
        const MAX_CHALLENGE_AGE_SECS: u64 = 300; // 5 minutes
        if now - proof.challenge.timestamp > MAX_CHALLENGE_AGE_SECS {
            return Err("Challenge expired (> 5 minutes old)".to_string());
        }
        
        // Proof validated - create device key with master binding
        let device_id = proof.challenge.device_id.clone();
        let current_version = 1;
        
        // Reconstruct keypair from public key (device will have private key)
        // Note: We only store the public key here - private key remains on device
        let active_key = Keypair::from_public_key(&proof.device_public_key)?;
        
        // Create master binding
        let mut msg = Vec::new();
        msg.extend_from_slice(device_id.as_bytes());
        msg.extend_from_slice(&(current_version as u64).to_le_bytes());
        msg.extend_from_slice(&proof.device_public_key);
        let master_binding = master_key.sign(&msg);
        
        Ok(DeviceKey {
            device_id,
            current_version,
            active_key,
            archived_keys: HashMap::new(),
            master_binding,
            signature_counter: 0,
        })
    }
    
    /// Validate a proof-of-possession
    pub fn validate_proof_of_possession(proof: &ProofOfPossession) -> Result<(), String> {
        // Verify device signed the challenge correctly
        let challenge_msg = proof.challenge.to_message();
        
        if !Keypair::verify(&proof.device_public_key, &challenge_msg, &proof.signature) {
            return Err("Invalid proof-of-possession signature".to_string());
        }
        
        Ok(())
    }
    
    /// Create proof-of-possession for a challenge (device-side)
    /// 
    /// Device receives challenge, signs it with private key, and returns proof.
    pub fn create_proof_of_possession(
        challenge: DeviceChallenge,
        device_keypair: &Keypair,
    ) -> ProofOfPossession {
        let challenge_msg = challenge.to_message();
        let signature = device_keypair.sign(&challenge_msg);
        
        ProofOfPossession {
            challenge,
            signature,
            device_public_key: device_keypair.public_key().to_vec(),
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
    
    // ===== NEW PROOF-OF-POSSESSION TESTS =====
    
    #[test]
    fn test_proof_of_possession_valid() {
        let mk = MasterKey::generate();
        
        // Device generates keypair locally
        let device_keypair = Keypair::generate(KeyType::Ed25519);
        let device_id = DeviceId::generate();
        
        // Step 1: Master generates challenge
        let challenge = DeviceChallenge::generate(device_id.clone());
        
        // Step 2: Device creates proof
        let proof = DeviceKey::create_proof_of_possession(
            challenge,
            &device_keypair,
        );
        
        // Step 3: Master validates proof and registers device
        let result = DeviceKey::register_with_proof_of_possession(&mk, &proof);
        assert!(result.is_ok(), "Valid proof should be accepted");
        
        let device_key = result.unwrap();
        assert_eq!(device_key.device_id(), &device_id);
        assert_eq!(device_key.public_key(), device_keypair.public_key());
    }
    
    #[test]
    fn test_proof_of_possession_wrong_device_key() {
        // This test demonstrates that proof-of-possession works correctly:
        // An attacker can create a valid proof for THEIR key, but not for victim's key
        
        let mk = MasterKey::generate();
        
        // Legitimate device generates keypair
        let legitimate_keypair = Keypair::generate(KeyType::Ed25519);
        let device_id = DeviceId::generate();
        
        // Attacker generates different keypair
        let attacker_keypair = Keypair::generate(KeyType::Ed25519);
        
        // Master generates challenge for device_id
        let challenge = DeviceChallenge::generate(device_id.clone());
        
        // Attacker creates proof with THEIR keypair
        let attacker_proof = DeviceKey::create_proof_of_possession(
            challenge.clone(),
            &attacker_keypair,
        );
        
        // The proof IS valid (attacker proved they own their key)
        let validation = DeviceKey::validate_proof_of_possession(&attacker_proof);
        assert!(validation.is_ok(), "Attacker's proof is cryptographically valid");
        
        // BUT when registered, it binds the ATTACKER'S public key, not victim's
        let registered = DeviceKey::register_with_proof_of_possession(&mk, &attacker_proof).unwrap();
        assert_eq!(
            registered.public_key(),
            attacker_keypair.public_key(),
            "Device is bound to attacker's key, not victim's"
        );
        
        // Attacker CANNOT create a valid proof for the legitimate device's key
        // without having its private key
        let mut forged_proof = attacker_proof.clone();
        forged_proof.device_public_key = legitimate_keypair.public_key().to_vec();
        
        let validation2 = DeviceKey::validate_proof_of_possession(&forged_proof);
        assert!(validation2.is_err(), "Forged public key should fail validation");
    }
    
    #[test]
    fn test_proof_of_possession_forged_signature() {
        let mk = MasterKey::generate();
        let device_id = DeviceId::generate();
        let challenge = DeviceChallenge::generate(device_id);
        
        // Create proof with forged signature
        let device_keypair = Keypair::generate(KeyType::Ed25519);
        let mut proof = DeviceKey::create_proof_of_possession(
            challenge,
            &device_keypair,
        );
        
        // Corrupt the signature
        proof.signature[0] ^= 0xFF;
        
        let result = DeviceKey::validate_proof_of_possession(&proof);
        assert!(result.is_err(), "Forged signature should be rejected");
        assert!(result.unwrap_err().contains("Invalid proof-of-possession"));
    }
    
    #[test]
    fn test_proof_of_possession_expired_challenge() {
        let mk = MasterKey::generate();
        let device_keypair = Keypair::generate(KeyType::Ed25519);
        let device_id = DeviceId::generate();
        
        // Create old challenge (> 5 minutes ago)
        let mut challenge = DeviceChallenge::generate(device_id);
        challenge.timestamp -= 600; // 10 minutes ago
        
        let proof = DeviceKey::create_proof_of_possession(
            challenge,
            &device_keypair,
        );
        
        let result = DeviceKey::register_with_proof_of_possession(&mk, &proof);
        assert!(result.is_err(), "Expired challenge should be rejected");
        assert!(result.unwrap_err().contains("expired"));
    }
    
    #[test]
    fn test_proof_of_possession_cannot_forge_for_others_key() {
        // Core security property: Cannot prove possession of a key you don't own
        
        let mk = MasterKey::generate();
        let device_id = DeviceId::generate();
        
        // Legitimate device keypair (attacker doesn't have this private key)
        let victim_keypair = Keypair::generate(KeyType::Ed25519);
        
        // Attacker has their own keypair
        let attacker_keypair = Keypair::generate(KeyType::Ed25519);
        
        // Master generates challenge
        let challenge = DeviceChallenge::generate(device_id.clone());
        
        // Attacker tries to create proof claiming they own victim's key
        // They sign with THEIR key but claim it's the victim's public key
        let mut forged_proof = DeviceKey::create_proof_of_possession(
            challenge,
            &attacker_keypair,
        );
        
        // Replace public key with victim's key
        forged_proof.device_public_key = victim_keypair.public_key().to_vec();
        
        // Validation MUST fail (signature won't match public key)
        let result = DeviceKey::validate_proof_of_possession(&forged_proof);
        assert!(
            result.is_err(),
            "Cannot prove possession of someone else's key"
        );
        
        // The ONLY way to prove possession is to actually have the private key
        let valid_proof = DeviceKey::create_proof_of_possession(
            DeviceChallenge::generate(device_id),
            &victim_keypair,
        );
        assert!(DeviceKey::validate_proof_of_possession(&valid_proof).is_ok());
    }
    
    #[test]
    fn test_challenge_message_format() {
        let device_id = DeviceId::generate();
        let challenge = DeviceChallenge::generate(device_id.clone());
        
        let msg = challenge.to_message();
        
        // Verify message includes all required components
        assert!(msg.starts_with(b"DEVICE_PROOF_OF_POSSESSION_V1:"));
        assert!(msg.len() > 32 + 8 + 16); // nonce + timestamp + device_id
    }
}
