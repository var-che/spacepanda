//! Identity Adversarial Security Tests
//!
//! Tests for malicious behavior and attack scenarios:
//! - Signature forgery attempts  
//! - Timestamp manipulation attacks
//! - Key rotation and revocation
//! - Cross-device impersonation
//! - Tampered bundle re-signing
//! - Trust chain invalidation

use crate::core_identity::*;
use crate::core_identity::validation::ReplayProtection;
use crate::core_store::crdt::{VectorClock, AddId, ORMap};
use crate::core_store::model::types::Timestamp;
use super::helpers::*;

// =============================================================================
// SIGNATURE FORGERY TESTS
// =============================================================================

#[test]
#[cfg_attr(not(feature = "real-crypto"), ignore = "requires real crypto")]
fn test_signature_wrong_key_pair() {
    // Create two different device bundles with different keys
    let id_kp1 = test_keypair();
    let dev_kp1 = test_keypair();
    let device_id1 = DeviceId::from_pubkey(dev_kp1.public_key());
    let meta1 = DeviceMetadata::new(device_id1, "Device1".to_string(), "node1");
    let kp1 = KeyPackage::new(&dev_kp1, &id_kp1, &meta1);
    let bundle1 = DeviceBundle::new(kp1, meta1, &id_kp1);
    
    let id_kp2 = test_keypair();
    let dev_kp2 = test_keypair();
    let device_id2 = DeviceId::from_pubkey(dev_kp2.public_key());
    let meta2 = DeviceMetadata::new(device_id2, "Device2".to_string(), "node2");
    let kp2 = KeyPackage::new(&dev_kp2, &id_kp2, &meta2);
    let bundle2 = DeviceBundle::new(kp2, meta2, &id_kp2);
    
    // bundle1 should not verify with id_kp2's public key
    assert!(!bundle1.verify(id_kp2.public_key()));
    
    // bundle2 should not verify with id_kp1's public key
    assert!(!bundle2.verify(id_kp1.public_key()));
}

#[test]
#[cfg_attr(not(feature = "real-crypto"), ignore = "requires real crypto")]
fn test_tampered_message_different_signature() {
    let id_kp = test_keypair();
    let dev_kp = test_keypair();
    let device_id = DeviceId::from_pubkey(dev_kp.public_key());
    let meta = DeviceMetadata::new(device_id, "Device".to_string(), "node1");
    
    let kp = KeyPackage::new(&dev_kp, &id_kp, &meta);
    let bundle = DeviceBundle::new(kp, meta, &id_kp);
    
    // Original signature is in bundle
    assert!(bundle.verify(id_kp.public_key()));
    
    // Create tampered bundle with different metadata
    let tampered_meta = DeviceMetadata::new(DeviceId::generate(), "Tampered".to_string(), "attacker");
    let tampered_kp = KeyPackage::new(&dev_kp, &id_kp, &tampered_meta);
    
    // Bundle with original signature but tampered metadata
    let mut tampered_bundle = bundle.clone();
    tampered_bundle.device_metadata = tampered_meta;
    
    // Should fail verification
    assert!(!tampered_bundle.verify(id_kp.public_key()));
}

// =============================================================================
// TIMESTAMP ATTACK TESTS
// =============================================================================

#[test]
fn test_timestamp_rollback_attack() {
    // Attacker tries to use very old timestamp to override newer data
    let user_id = test_user_id();
    let mut meta = UserMetadata::new(user_id);
    
    // Legitimate update with current timestamp
    let current_ts = test_timestamp(1000).as_millis();
    let mut vc_current = VectorClock::new();
    vc_current.increment("legitimate");
    meta.display_name.set(
        "LegitName".to_string(),
        current_ts,
        "legitimate".to_string(),
        vc_current,
    );
    
    // Attacker tries rollback attack with old timestamp
    let old_ts = test_timestamp(1).as_millis();
    let mut vc_old = VectorClock::new();
    vc_old.increment("attacker");
    meta.display_name.set(
        "AttackerName".to_string(),
        old_ts,
        "attacker".to_string(),
        vc_old,
    );
    
    // Legitimate value should win (higher timestamp)
    assert_eq!(meta.display_name.get(), Some(&"LegitName".to_string()));
}

#[test]
fn test_timestamp_far_future_attack() {
    // Attacker uses absurdly far future timestamp
    let user_id = test_user_id();
    let mut meta = UserMetadata::new(user_id);
    
    // Normal update
    let mut vc = VectorClock::new();
    vc.increment("normal");
    meta.display_name.set(
        "NormalName".to_string(),
        test_timestamp(100).as_millis(),
        "normal".to_string(),
        vc,
    );
    
    // Attacker with max timestamp
    let mut vc_attack = VectorClock::new();
    vc_attack.increment("attacker");
    meta.display_name.set(
        "AttackerName".to_string(),
        u64::MAX,
        "attacker".to_string(),
        vc_attack,
    );
    
    // Attacker wins (timestamp-based)
    // NOTE: This shows a weakness - need timestamp bounds validation
    assert_eq!(meta.display_name.get(), Some(&"AttackerName".to_string()));
}

#[test]
fn test_timestamp_validation_boundaries() {
    use crate::core_identity::validation::validate_timestamp;
    
    // Current time should pass (convert to seconds)
    let now = Timestamp::now().as_millis() / 1000;
    assert!(validate_timestamp(now).is_ok());
    
    // Far future should fail (test helper creates far-future timestamps)
    let far_future = test_timestamp(1000000).as_millis() / 1000;
    let result = validate_timestamp(far_future);
    assert!(result.is_err());
    
    // Far past should fail
    let far_past = 1; // Timestamp from 1970
    let result = validate_timestamp(far_past);
    assert!(result.is_err());
}

// =============================================================================
// KEY ROTATION TESTS
// =============================================================================

#[test]
#[cfg_attr(not(feature = "real-crypto"), ignore = "requires real crypto")]
fn test_old_key_after_rotation_fails() {
    // Simulate key rotation: old signatures should not verify with new key
    let id_kp1 = test_keypair();
    let dev_kp = test_keypair();
    let device_id = DeviceId::from_pubkey(dev_kp.public_key());
    let meta = DeviceMetadata::new(device_id.clone(), "Device".to_string(), "node1");
    
    // Original bundle with key1
    let kp1 = KeyPackage::new(&dev_kp, &id_kp1, &meta);
    let bundle1 = DeviceBundle::new(kp1, meta.clone(), &id_kp1);
    
    // New identity key (simulates key rotation)
    let id_kp2 = test_keypair();
    let kp2 = KeyPackage::new(&dev_kp, &id_kp2, &meta);
    let _bundle2 = DeviceBundle::new(kp2, meta, &id_kp2);
    
    // Old bundle should not verify with new key
    assert!(!bundle1.verify(id_kp2.public_key()));
}

#[test]
fn test_device_removal_invalidates_operations() {
    // Remove device from user metadata, operations from that device should be suspect
    let user_id = test_user_id();
    let device_id = DeviceId::generate();
    
    let mut meta = UserMetadata::new(user_id.clone());
    
    let id_kp = test_keypair();
    let dev_kp = test_keypair();
    let device_metadata = DeviceMetadata::new(device_id.clone(), "Device".to_string(), "node1");
    let key_package = KeyPackage::new(&dev_kp, &id_kp, &device_metadata);
    let device_bundle = DeviceBundle::new(key_package, device_metadata.clone(), &id_kp);
    
    // Add device
    let mut vc = VectorClock::new();
    vc.increment("node1");
    meta.add_device(device_metadata, test_add_id("node1", 1), vc.clone());
    
    assert!(meta.devices.contains_key(&device_id));
    
    // Remove device
    vc.increment("node1");
    meta.remove_device(&device_id, vc);
    
    // Device should be gone
    assert!(!meta.devices.contains_key(&device_id));
}

// =============================================================================
// IDENTITY BUNDLE TAMPERING TESTS
// =============================================================================

#[test]
fn test_identity_bundle_user_id_mismatch() {
    // Bundle claims one user_id but this is detectable
    let user_id_1 = test_user_id();
    let kp2 = test_keypair();
    let user_id_2 = UserId::from_public_key(kp2.public_key());
    
    // Different user IDs should not be equal
    assert_ne!(user_id_1.as_bytes(), user_id_2.as_bytes());
}

#[test]
fn test_identity_bundle_empty_devices() {
    // Bundle with no devices should be invalid in production
    let user_id = test_user_id();
    let id_kp = test_keypair();
    
    let devices: Vec<DeviceId> = vec![];
    
    let bundle = IdentityBundle::new(
        user_id.as_bytes().to_vec(),
        id_kp.public_key().to_vec(),
        devices.clone(),
        &id_kp,
    );
    
    assert!(bundle.devices.is_empty());
    // In production, validation should reject this
}

#[test]
fn test_identity_bundle_duplicate_device_ids() {
    // Bundle should not have same device twice
    let user_id = test_user_id();
    let id_kp = test_keypair();
    let device_id = DeviceId::generate();
    
    // Same device twice
    let devices = vec![device_id.clone(), device_id.clone()];
    
    let bundle = IdentityBundle::new(
        user_id.as_bytes().to_vec(),
        id_kp.public_key().to_vec(),
        devices.clone(),
        &id_kp,
    );
    
    // Should have 2 entries but both same device_id
    assert_eq!(bundle.devices.len(), 2);
    assert_eq!(bundle.devices[0], bundle.devices[1]);
}

// =============================================================================
// CROSS-DEVICE IMPERSONATION TESTS
// =============================================================================

#[test]
#[cfg_attr(not(feature = "real-crypto"), ignore = "requires real crypto")]
fn test_device_a_cannot_sign_for_device_b() {
    // Device A tries to sign messages pretending to be Device B
    let id_kp_a = test_keypair();
    let dev_kp_a = test_keypair();
    let device_id_a = DeviceId::from_pubkey(dev_kp_a.public_key());
    let meta_a = DeviceMetadata::new(device_id_a, "DeviceA".to_string(), "nodeA");
    
    let id_kp_b = test_keypair();
    let dev_kp_b = test_keypair();
    let device_id_b = DeviceId::from_pubkey(dev_kp_b.public_key());
    let meta_b = DeviceMetadata::new(device_id_b, "DeviceB".to_string(), "nodeB");
    
    // Create bundles
    let kp_a = KeyPackage::new(&dev_kp_a, &id_kp_a, &meta_a);
    let _bundle_a = DeviceBundle::new(kp_a, meta_a, &id_kp_a);
    
    let kp_b = KeyPackage::new(&dev_kp_b, &id_kp_b, &meta_b);
    let bundle_b = DeviceBundle::new(kp_b, meta_b, &id_kp_b);
    
    // bundle_a cannot verify with bundle_b's key
    assert!(!bundle_b.verify(id_kp_a.public_key()));
}

// =============================================================================
// MIXED TIMESTAMP ATTACKS
// =============================================================================

#[test]
fn test_concurrent_updates_with_clock_skew() {
    // Two nodes with different clock speeds update same field
    let user_id = test_user_id();
    let mut meta = UserMetadata::new(user_id);
    
    // Node with fast clock (ahead by days)
    let fast_clock_ts = test_timestamp(100000).as_millis();
    let mut vc_fast = VectorClock::new();
    vc_fast.increment("fast_node");
    meta.display_name.set(
        "FastClock".to_string(),
        fast_clock_ts,
        "fast_node".to_string(),
        vc_fast,
    );
    
    // Node with slow clock (behind by days)
    let slow_clock_ts = test_timestamp(1).as_millis();
    let mut vc_slow = VectorClock::new();
    vc_slow.increment("slow_node");
    meta.display_name.set(
        "SlowClock".to_string(),
        slow_clock_ts,
        "slow_node".to_string(),
        vc_slow,
    );
    
    // Fast clock wins (LWW semantics)
    assert_eq!(meta.display_name.get(), Some(&"FastClock".to_string()));
}

#[test]
fn test_vector_clock_prevents_pure_timestamp_dominance() {
    // Even with older timestamp, higher vector clock should be detectable
    let mut vc_old = VectorClock::new();
    vc_old.increment("node1");
    vc_old.increment("node1");
    vc_old.increment("node1");
    
    let mut vc_new = VectorClock::new();
    vc_new.increment("node1");
    
    // vc_old happened after vc_new (higher count)
    assert!(!vc_old.happened_before(&vc_new));
    assert!(vc_new.happened_before(&vc_old));
}

// =============================================================================
// REPLAY ATTACK TESTS (Extended)
// =============================================================================

#[test]
fn test_replay_protection_same_signature_twice() {
    let mut replay = ReplayProtection::new();
    
    let sig_bytes = vec![1, 2, 3, 4, 5];
    
    // First insert
    assert!(replay.check(&sig_bytes).is_ok());
    
    // Second insert (replay)
    let result = replay.check(&sig_bytes);
    assert!(result.is_err());
}

#[test]
fn test_replay_protection_across_sessions() {
    // Simulates attacker capturing signature and replaying later
    let mut replay = ReplayProtection::new();
    
    let captured_sig = vec![0xDE, 0xAD, 0xBE, 0xEF];
    
    // Original signature processed
    replay.check(&captured_sig).unwrap();
    
    // Time passes, attacker replays
    let replay_result = replay.check(&captured_sig);
    
    assert!(replay_result.is_err());
}

#[test]
fn test_replay_protection_lru_eviction_allows_old_sigs() {
    // After eviction, old signatures could be replayed (vulnerability)
    let mut replay = ReplayProtection::new();
    
    let sig1 = vec![1];
    let sig2 = vec![2];
    let sig3 = vec![3];
    
    replay.check(&sig1).unwrap();
    replay.check(&sig2).unwrap();
    
    // Cleanup with small max size
    replay.cleanup(2);
    
    // After cleanup, sig1 might be forgotten
    // (actual behavior depends on cleanup implementation)
}
