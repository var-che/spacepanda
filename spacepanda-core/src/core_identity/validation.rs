//! Validation module
//!
//! Stateless validators for incoming identity artifacts.

use crate::core_identity::bundles::{DeviceBundle, IdentityBundle, KeyPackage};
use crate::core_identity::signatures::IdentitySignature;
use crate::core_identity::user_id::UserId;
use thiserror::Error;

/// Validation errors
#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("Invalid signature")]
    InvalidSignature,

    #[error("Timestamp out of range: {0}")]
    TimestampOutOfRange(String),

    #[error("Unknown user: {0}")]
    UnknownUser(String),

    #[error("Bad format: {0}")]
    BadFormat(String),

    #[error("Replay attack detected")]
    ReplayAttack,

    #[error("Invalid key format")]
    InvalidKeyFormat,

    #[error("Credential verification failed")]
    CredentialVerificationFailed,
}

/// Maximum allowed timestamp skew (1 hour in seconds)
const MAX_TIMESTAMP_SKEW: u64 = 3600;

/// Validate a key package
pub fn validate_keypackage(kp_bytes: &[u8]) -> Result<KeyPackage, ValidationError> {
    let kp = KeyPackage::from_bytes(kp_bytes)
        .map_err(|e| ValidationError::BadFormat(format!("Failed to parse: {}", e)))?;

    // Check key formats
    if kp.init_key.len() != 32 {
        return Err(ValidationError::InvalidKeyFormat);
    }

    // Check cipher suite
    if kp.cipher_suite.is_empty() {
        return Err(ValidationError::BadFormat("Empty cipher suite".to_string()));
    }

    // Signature verification requires identity public key
    // This is a basic format check only
    if kp.signature.len() != 64 {
        return Err(ValidationError::InvalidSignature);
    }

    Ok(kp)
}

/// Validate a device bundle
pub fn validate_device_bundle(
    bundle: &DeviceBundle,
    expected_user: &UserId,
    identity_pubkey: &[u8],
) -> Result<(), ValidationError> {
    // Verify bundle signature
    if !bundle.verify(identity_pubkey) {
        return Err(ValidationError::InvalidSignature);
    }

    // Verify key package signature
    if !bundle.key_package.verify(identity_pubkey) {
        return Err(ValidationError::InvalidSignature);
    }

    // Verify device metadata is recent
    if let Some(last_seen) = bundle.device_metadata.last_seen.get() {
        validate_timestamp(last_seen.as_millis() / 1000)?;
    }

    Ok(())
}

/// Validate an identity bundle
pub fn validate_identity_bundle(bundle: &IdentityBundle) -> Result<(), ValidationError> {
    // Verify self-signature
    if !bundle.verify() {
        return Err(ValidationError::InvalidSignature);
    }

    // Check public key format
    if bundle.public_key.len() != 32 {
        return Err(ValidationError::InvalidKeyFormat);
    }

    // Check user ID derivation
    let derived_user_id = UserId::from_public_key(&bundle.public_key);
    if derived_user_id.as_bytes() != &bundle.user_id {
        return Err(ValidationError::BadFormat("User ID doesn't match public key".to_string()));
    }

    Ok(())
}

/// Validate an identity signature
pub fn validate_signature(sig: &IdentitySignature, pubkey: &[u8]) -> Result<(), ValidationError> {
    // Verify cryptographic signature
    if !sig.verify(pubkey) {
        return Err(ValidationError::InvalidSignature);
    }

    // Verify timestamp is within acceptable range
    validate_timestamp(sig.timestamp())?;

    Ok(())
}

/// Validate timestamp is within acceptable range
pub fn validate_timestamp(timestamp: u64) -> Result<(), ValidationError> {
    use std::time::{SystemTime, UNIX_EPOCH};

    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();

    // Check if timestamp is too far in the future
    if timestamp > now + MAX_TIMESTAMP_SKEW {
        return Err(ValidationError::TimestampOutOfRange(
            "Timestamp too far in future".to_string(),
        ));
    }

    // Check if timestamp is too far in the past
    if timestamp + MAX_TIMESTAMP_SKEW < now {
        return Err(ValidationError::TimestampOutOfRange("Timestamp too old".to_string()));
    }

    Ok(())
}

/// Replay protection using a simple in-memory set of seen signatures
/// In production, use a time-windowed bloom filter or similar
pub struct ReplayProtection {
    seen_signatures: std::collections::HashSet<Vec<u8>>,
}

impl ReplayProtection {
    pub fn new() -> Self {
        ReplayProtection { seen_signatures: std::collections::HashSet::new() }
    }

    /// Check if signature has been seen before
    pub fn check(&mut self, sig_bytes: &[u8]) -> Result<(), ValidationError> {
        if self.seen_signatures.contains(sig_bytes) {
            return Err(ValidationError::ReplayAttack);
        }
        self.seen_signatures.insert(sig_bytes.to_vec());
        Ok(())
    }

    /// Clear old entries (simple implementation - in production use time-based expiry)
    pub fn cleanup(&mut self, max_size: usize) {
        if self.seen_signatures.len() > max_size {
            self.seen_signatures.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core_identity::bundles::KeyPackage;
    use crate::core_identity::device_id::DeviceId;
    use crate::core_identity::keypair::{KeyType, Keypair};
    use crate::core_identity::metadata::DeviceMetadata;
    use crate::core_identity::signatures::IdentitySignature;

    #[test]
    fn test_validate_keypackage() {
        let device_kp = Keypair::generate(KeyType::Ed25519);
        let identity_kp = Keypair::generate(KeyType::Ed25519);
        let device_id = DeviceId::generate();
        let device_meta = DeviceMetadata::new(device_id, "Test".to_string(), "node1");

        let kp = KeyPackage::new(&device_kp, &identity_kp, &device_meta);
        let bytes = kp.to_bytes();

        let validated = validate_keypackage(&bytes);
        assert!(validated.is_ok());
    }

    #[test]
    fn test_validate_signature() {
        let identity_kp = Keypair::generate(KeyType::Ed25519);
        let user_id = UserId::from_public_key(identity_kp.public_key());

        let sig = IdentitySignature::sign_identity_proof(user_id, &identity_kp);

        let result = validate_signature(&sig, identity_kp.public_key());
        assert!(result.is_ok());
    }

    #[test]
    fn test_replay_protection() {
        let mut rp = ReplayProtection::new();

        let sig_bytes = vec![1, 2, 3, 4, 5];

        assert!(rp.check(&sig_bytes).is_ok());
        assert!(rp.check(&sig_bytes).is_err()); // Should detect replay
    }

    #[test]
    fn test_timestamp_validation() {
        use std::time::{SystemTime, UNIX_EPOCH};

        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();

        // Current time should be valid
        assert!(validate_timestamp(now).is_ok());

        // Far future should be invalid
        assert!(validate_timestamp(now + 10000).is_err());

        // Far past should be invalid
        assert!(validate_timestamp(now - 10000).is_err());
    }
}
