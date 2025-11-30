//! Identity Edge-Case Tests
//!
//! Tests for signature replay, key rotation, trust chains, and cryptographic edge cases

use crate::core_identity::*;
use crate::core_identity::validation::{ReplayProtection, ValidationError};
use super::helpers::*;

// =============================================================================
// SIGNATURE REPLAY ATTACK PREVENTION
// =============================================================================

#[test]
fn test_signature_replay_attack_rejected() {
    let mut replay_protection = ReplayProtection::new();
    let kp = test_keypair();
    let msg = b"hello";
    
    let sig = kp.sign(msg);
    
    // First time should succeed
    assert!(replay_protection.check(&sig).is_ok());
    
    // Replay should be rejected
    assert!(matches!(
        replay_protection.check(&sig),
        Err(ValidationError::ReplayAttack)
    ));
}

#[test]
fn test_replay_protection_different_signatures_allowed() {
    let mut replay_protection = ReplayProtection::new();
    let kp = test_keypair();
    
    let sig1 = kp.sign(b"message1");
    let sig2 = kp.sign(b"message2");
    
    assert!(replay_protection.check(&sig1).is_ok());
    assert!(replay_protection.check(&sig2).is_ok());
}

#[test]
fn test_replay_protection_cleanup_allows_reinsertion() {
    let mut replay_protection = ReplayProtection::new();
    
    // Fill with many signatures
    for i in 0..500u32 {
        let sig = i.to_le_bytes().to_vec();
        let _ = replay_protection.check(&sig);
    }
    
    // Cleanup to small size
    replay_protection.cleanup(10);
    
    // Old signatures should be forgotten and can be reused
    let old_sig = 1u32.to_le_bytes().to_vec();
    assert!(replay_protection.check(&old_sig).is_ok());
}

// =============================================================================
// EXPIRED CREDENTIALS
// =============================================================================

#[test]
fn test_expired_timestamp_rejected() {
    use std::time::Duration;
    use crate::core_store::model::types::Timestamp;
    
    // validate_timestamp expects seconds
    let now_secs = Timestamp::now().as_millis() / 1000;
    let expired = now_secs - Duration::from_secs(7200).as_secs();
    
    assert!(matches!(
        validation::validate_timestamp(expired),
        Err(ValidationError::TimestampOutOfRange(_))
    ));
}

#[test]
fn test_future_timestamp_rejected() {
    use std::time::Duration;
    use crate::core_store::model::types::Timestamp;
    
    // validate_timestamp expects seconds
    let now_secs = Timestamp::now().as_millis() / 1000;
    let future = now_secs + Duration::from_secs(7200).as_secs();
    
    assert!(matches!(
        validation::validate_timestamp(future),
        Err(ValidationError::TimestampOutOfRange(_))
    ));
}

#[test]
fn test_current_timestamp_accepted() {
    use crate::core_store::model::types::Timestamp;
    
    // validate_timestamp expects seconds, convert from millis
    let current = Timestamp::now().as_millis() / 1000;
    assert!(validation::validate_timestamp(current).is_ok());
}

// =============================================================================
// KEY PACKAGE TAMPERING DETECTION
// =============================================================================

#[test]
#[cfg_attr(not(feature = "real-crypto"), ignore = "requires real crypto for tamper detection")]
fn test_keypackage_rejects_tampered_payload() {
    let identity = test_keypair();
    let device = test_keypair();
    let device_id = DeviceId::from_pubkey(device.public_key());
    let meta = DeviceMetadata::new(device_id, "Test".to_string(), "node1");
    
    let mut kp = KeyPackage::new(&device, &identity, &meta);
    
    // Tamper with the public key after signing
    if !kp.init_key.is_empty() {
        kp.init_key[0] ^= 0x55;
    }
    
    // Verification should fail (with real crypto)
    assert!(!kp.verify(identity.public_key()));
}

#[test]
fn test_keypackage_serialization_corrupted_data() {
    let identity = test_keypair();
    let device = test_keypair();
    let device_id = DeviceId::from_pubkey(device.public_key());
    let meta = DeviceMetadata::new(device_id, "Test".to_string(), "node1");
    
    let kp = KeyPackage::new(&device, &identity, &meta);
    let mut bytes = kp.to_bytes();
    
    // Corrupt a byte in the middle
    if bytes.len() > 20 {
        bytes[10] ^= 0xFF;
    }
    
    // Deserialization may fail or produce invalid data
    let result = KeyPackage::from_bytes(&bytes);
    
    // Either fails to parse or produces different hash
    if let Ok(corrupted_kp) = result {
        // If it parsed, hash should differ
        assert_ne!(kp.hash(), corrupted_kp.hash());
    }
}

// =============================================================================
// IDENTITY BUNDLE VALIDATION
// =============================================================================

#[test]
fn test_identity_bundle_mismatched_user_id() {
    let id_kp = test_keypair();
    let wrong_user_id = vec![0u8; 32]; // Wrong user ID
    
    let bundle = IdentityBundle::new(
        wrong_user_id,
        id_kp.public_key().to_vec(),
        vec![DeviceId::generate()],
        &id_kp,
    );
    
    // Validation should detect mismatch
    let result = validation::validate_identity_bundle(&bundle);
    assert!(result.is_err());
}

#[test]
fn test_identity_bundle_empty_devices_list() {
    let id_kp = test_keypair();
    let user_id = UserId::from_public_key(id_kp.public_key());
    
    let bundle = IdentityBundle::new(
        user_id.as_bytes().to_vec(),
        id_kp.public_key().to_vec(),
        vec![], // No devices
        &id_kp,
    );
    
    // Should still verify (empty device list is valid)
    assert!(bundle.verify());
}

// =============================================================================
// DETACHED SIGNATURE VALIDATION
// =============================================================================

#[test]
#[cfg_attr(not(feature = "real-crypto"), ignore = "requires real crypto")]
fn test_detached_signature_wrong_payload() {
    let kp = test_keypair();
    let sig = kp.sign(b"hello");
    
    // Verify with different payload should fail
    assert!(!Keypair::verify(kp.public_key(), b"goodbye", &sig));
}

#[test]
fn test_signature_wrong_length_rejected() {
    let kp = test_keypair();
    
    // Signature too short
    let bad_sig = vec![0u8; 10];
    assert!(!Keypair::verify(kp.public_key(), b"test", &bad_sig));
    
    // Signature too long
    let bad_sig2 = vec![0u8; 100];
    assert!(!Keypair::verify(kp.public_key(), b"test", &bad_sig2));
}

// =============================================================================
// IDENTITY SIGNATURE TYPES
// =============================================================================

#[test]
fn test_all_identity_signature_types_verify() {
    let identity_kp = test_keypair();
    let user_id = UserId::from_public_key(identity_kp.public_key());
    let device_id = DeviceId::generate();
    
    // Test all 5 signature types
    let sigs = vec![
        IdentitySignature::sign_device_ownership(
            device_id.clone(),
            user_id.clone(),
            &identity_kp,
        ),
        IdentitySignature::sign_space_ownership(
            "space123".to_string(),
            user_id.clone(),
            &identity_kp,
        ),
        IdentitySignature::sign_channel_creation(
            "channel456".to_string(),
            user_id.clone(),
            &identity_kp,
        ),
        IdentitySignature::sign_keypackage_binding(
            vec![1, 2, 3],
            user_id.clone(),
            device_id,
            &identity_kp,
        ),
        IdentitySignature::sign_identity_proof(user_id, &identity_kp),
    ];
    
    // All should verify
    for sig in sigs {
        assert!(sig.verify(identity_kp.public_key()));
    }
}

#[test]
#[cfg_attr(not(feature = "real-crypto"), ignore = "requires real crypto")]
fn test_identity_signature_wrong_key_fails() {
    let kp1 = test_keypair();
    let kp2 = test_keypair();
    let user_id = UserId::from_public_key(kp1.public_key());
    
    let sig = IdentitySignature::sign_identity_proof(user_id, &kp1);
    
    // Verification with wrong key should fail
    assert!(!sig.verify(kp2.public_key()));
}

// =============================================================================
// CONCURRENT PROFILE UPDATES (CRDT LWW)
// =============================================================================

#[test]
fn test_identity_profile_concurrent_edits_lww() {
    let uid = test_user_id();
    let mut meta_a = UserMetadata::new(uid.clone());
    let mut meta_b = UserMetadata::new(uid);
    
    // Concurrent updates with different timestamps
    meta_a.set_display_name("Vlad".to_string(), test_timestamp(1000), "nodeA");
    meta_b.set_display_name("Vladimir".to_string(), test_timestamp(2000), "nodeB");
    
    // Merge
    meta_a.merge(&meta_b);
    
    // Later timestamp wins
    assert_eq!(meta_a.display_name.get(), Some(&"Vladimir".to_string()));
}

#[test]
fn test_identity_profile_equal_timestamp_resolution() {
    let uid = test_user_id();
    let mut meta_a = UserMetadata::new(uid.clone());
    let mut meta_b = UserMetadata::new(uid);
    
    let same_ts = test_timestamp(1000);
    
    // Same timestamp, different nodes
    meta_a.set_display_name("NameA".to_string(), same_ts, "nodeA");
    meta_b.set_display_name("NameB".to_string(), same_ts, "nodeB");
    
    meta_a.merge(&meta_b);
    
    // Node ID determines winner (lexicographic)
    // "nodeB" > "nodeA"
    assert_eq!(meta_a.display_name.get(), Some(&"NameB".to_string()));
}

// =============================================================================
// DEVICE BUNDLE VERIFICATION
// =============================================================================

#[test]
fn test_device_bundle_roundtrip_verifies() {
    let id = test_keypair();
    let dev = test_keypair();
    let device_id = DeviceId::from_pubkey(dev.public_key());
    let meta = DeviceMetadata::new(device_id, "Test Device".to_string(), "node1");
    
    let kp = KeyPackage::new(&dev, &id, &meta);
    let bundle = DeviceBundle::new(kp, meta, &id);
    
    assert!(bundle.verify(id.public_key()));
}

#[test]
#[cfg_attr(not(feature = "real-crypto"), ignore = "requires real crypto")]
fn test_device_bundle_tampered_metadata_fails() {
    let id = test_keypair();
    let dev = test_keypair();
    let device_id = DeviceId::from_pubkey(dev.public_key());
    let meta = DeviceMetadata::new(device_id, "Original".to_string(), "node1");
    
    let kp = KeyPackage::new(&dev, &id, &meta);
    let mut bundle = DeviceBundle::new(kp, meta, &id);
    
    // Tamper with metadata after signing
    let tampered_id = DeviceId::generate();
    bundle.device_metadata = DeviceMetadata::new(tampered_id, "Tampered".to_string(), "node1");
    
    // Verification should fail
    assert!(!bundle.verify(id.public_key()));
}
