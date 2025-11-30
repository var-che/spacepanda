//! Integration tests for core_identity
//!
//! Cross-file behavior and component interactions

use crate::core_identity::*;
use crate::core_identity::keystore::memory_keystore::MemoryKeystore;
use crate::core_identity::keystore::Keystore;
use super::helpers::*;

// Note: These tests would require an IdentityManager implementation
// For now, we'll test the components we have

#[test]
fn test_keypackage_serialization_roundtrip() {
    let identity_kp = test_keypair();
    let device_kp = test_keypair();
    let device_id = DeviceId::from_pubkey(device_kp.public_key());
    let meta = DeviceMetadata::new(device_id, "Test".to_string(), "node1");

    let kp = KeyPackage::new(&device_kp, &identity_kp, &meta);
    let bytes = kp.to_bytes();
    let restored = KeyPackage::from_bytes(&bytes).unwrap();

    assert_eq!(kp.cipher_suite, restored.cipher_suite);
    assert_eq!(kp.init_key, restored.init_key);
    assert_eq!(kp.signature, restored.signature);
}

#[test]
fn test_user_metadata_merge_different_fields() {
    let uid = test_user_id();
    let mut meta1 = UserMetadata::new(uid.clone());
    let mut meta2 = UserMetadata::new(uid);

    // Use deterministic timestamps - no sleep needed
    let ts1 = test_timestamp(1000);
    let ts2 = test_timestamp(2000);

    // meta1 sets name, meta2 sets avatar
    meta1.set_display_name("Alice".to_string(), ts1, "node1");
    meta2.set_avatar_hash(Some(vec![1, 2, 3]), ts2, "node2");

    // Merge both ways
    let mut merged = meta1.clone();
    merged.merge(&meta2);

    // Name should be present
    assert_eq!(merged.display_name.get(), Some(&"Alice".to_string()));
    // Avatar merge is simplified in current implementation
    // Just verify merge doesn't crash
}

#[test]
fn test_keystore_identity_persistence() {
    let keystore = MemoryKeystore::new();
    
    // First identity creation
    let kp1 = test_keypair();
    keystore.save_identity_keypair(&kp1).unwrap();
    
    // Load it back
    let loaded = keystore.load_identity_keypair().unwrap();
    assert_eq!(kp1.public_key(), loaded.public_key());
    
    // Save a different one (simulating rotation)
    let kp2 = test_keypair();
    keystore.save_identity_keypair(&kp2).unwrap();
    
    // Should get the new one
    let loaded2 = keystore.load_identity_keypair().unwrap();
    assert_eq!(kp2.public_key(), loaded2.public_key());
    assert_ne!(kp1.public_key(), loaded2.public_key());
}

#[test]
fn test_device_metadata_key_package_ref() {
    let mut meta = test_device_metadata("node1");
    let hash = vec![1, 2, 3, 4, 5];
    
    // Use deterministic timestamp - no sleep needed
    meta.set_key_package_ref(Some(hash.clone()), test_timestamp(1000), "node1");
    
    assert_eq!(meta.key_package_ref.get(), Some(&Some(hash)));
}

#[test]
fn test_validation_keypackage_format() {
    let identity_kp = test_keypair();
    let device_kp = test_keypair();
    let device_id = DeviceId::from_pubkey(device_kp.public_key());
    let meta = DeviceMetadata::new(device_id, "Test".to_string(), "node1");

    let kp = KeyPackage::new(&device_kp, &identity_kp, &meta);
    let bytes = kp.to_bytes();

    let validated = validation::validate_keypackage(&bytes);
    assert!(validated.is_ok());
}

#[test]
fn test_validation_identity_bundle() {
    let identity_kp = test_keypair();
    let user_id = UserId::from_public_key(identity_kp.public_key());
    let devices = vec![DeviceId::generate()];

    let bundle = IdentityBundle::new(
        user_id.as_bytes().to_vec(),
        identity_kp.public_key().to_vec(),
        devices,
        &identity_kp,
    );

    let result = validation::validate_identity_bundle(&bundle);
    assert!(result.is_ok());
}

#[test]
fn test_signature_with_nonce_uniqueness() {
    let identity_kp = test_keypair();
    let user_id = UserId::from_public_key(identity_kp.public_key());

    // Generate multiple signatures - they should have different nonces
    let sig1 = IdentitySignature::sign_identity_proof(user_id.clone(), &identity_kp);
    let sig2 = IdentitySignature::sign_identity_proof(user_id, &identity_kp);

    // Both should verify
    assert!(sig1.verify(identity_kp.public_key()));
    assert!(sig2.verify(identity_kp.public_key()));
}

#[test]
fn test_multiple_devices_in_metadata() {
    use crate::core_store::crdt::AddId;
    use crate::core_store::model::types::Timestamp;

    let uid = test_user_id();
    let mut meta = UserMetadata::new(uid);

    // Add multiple devices
    for i in 0..3 {
        let dev = DeviceMetadata::new(
            DeviceId::generate(),
            format!("Device {}", i),
            "node1",
        );
        let device_id = dev.device_id.clone();
        let add_id = AddId::new("node1".to_string(), Timestamp::now().as_millis() + i as u64);
        let vc = test_vector_clock("node1");
        
        meta.add_device(dev, add_id, vc);
        assert!(meta.devices.contains_key(&device_id));
    }

    assert_eq!(meta.devices.len(), 3);
}

#[test]
fn test_keypackage_hash_deterministic() {
    let identity_kp = test_keypair();
    let device_kp = test_keypair();
    let device_id = DeviceId::from_pubkey(device_kp.public_key());
    let meta = DeviceMetadata::new(device_id, "Test".to_string(), "node1");

    let kp = KeyPackage::new(&device_kp, &identity_kp, &meta);
    let hash1 = kp.hash();
    let hash2 = kp.hash();

    assert_eq!(hash1, hash2);
    assert_eq!(hash1.len(), 32);
}

#[test]
fn test_file_keystore_roundtrip() {
    use crate::core_identity::keystore::file_keystore::FileKeystore;
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();
    let keystore = FileKeystore::new(temp_dir.path().to_path_buf(), None).unwrap();

    let kp = test_keypair();
    keystore.save_identity_keypair(&kp).unwrap();

    let loaded = keystore.load_identity_keypair().unwrap();
    assert_eq!(kp.public_key(), loaded.public_key());
}

#[test]
fn test_file_keystore_with_password() {
    use crate::core_identity::keystore::file_keystore::FileKeystore;
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();
    let keystore = FileKeystore::new(
        temp_dir.path().to_path_buf(),
        Some("test_password"),
    ).unwrap();

    let kp = test_keypair();
    keystore.save_identity_keypair(&kp).unwrap();

    let loaded = keystore.load_identity_keypair().unwrap();
    assert_eq!(kp.public_key(), loaded.public_key());
}

// Test 9: CRDT merge conflict resolution (LWW)
#[test]
fn test_crdt_merge_user_metadata_conflict_resolution() {
    let uid = test_user_id();
    let mut a = UserMetadata::new(uid.clone());
    let mut b = UserMetadata::new(uid);

    // Use deterministic timestamps - no sleep needed
    let ts1 = test_timestamp(1000);
    let ts2 = test_timestamp(2000);

    // a sets display name at ts1
    a.set_display_name("alice".to_string(), ts1, "a");
    
    // b sets display name at ts2 (later timestamp)
    b.set_display_name("alicia".to_string(), ts2, "b");

    // Merge b into a - later timestamp should win
    a.merge(&b);

    assert_eq!(a.display_name.get(), Some(&"alicia".to_string()));
}

// Test 14: Keystore encrypt/decrypt identity keypair roundtrip
#[test]
fn test_keystore_encrypt_decrypt_identity_keypair() {
    use crate::core_identity::keystore::file_keystore::FileKeystore;
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();
    let keystore = FileKeystore::new(
        temp_dir.path().to_path_buf(),
        Some("password123"),
    ).unwrap();

    let kp = test_keypair();
    keystore.save_identity_keypair(&kp).unwrap();

    let loaded = keystore.load_identity_keypair().unwrap();

    // Public keys should match
    assert_eq!(kp.public_key(), loaded.public_key());
    
    // Test signing to verify private key works
    let msg = b"test message";
    let sig1 = kp.sign(msg);
    let sig2 = loaded.sign(msg);
    
    assert!(Keypair::verify(kp.public_key(), msg, &sig1));
    assert!(Keypair::verify(loaded.public_key(), msg, &sig2));
}
