//! Bundles module
//!
//! Create MLS KeyPackage and identity bundles for publication.

use crate::core_identity::device_id::DeviceId;
use crate::core_identity::keypair::Keypair;
use crate::core_identity::metadata::DeviceMetadata;
use blake3;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Hash type for content addressing
pub type Hash = Vec<u8>;

/// MLS KeyPackage for device authentication and group joining
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyPackage {
    /// Cipher suite identifier
    pub cipher_suite: String,
    /// X25519 public key for key exchange
    pub init_key: Vec<u8>,
    /// Encrypted leaf secret (optional)
    pub leaf_secret_encryption: Option<Vec<u8>>,
    /// Identity credential (signed by identity key)
    pub credential: Vec<u8>,
    /// Extensions (capabilities, version, etc.)
    pub extensions: Vec<Extension>,
    /// Signature over the package
    pub signature: Vec<u8>,
}

/// MLS extension
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Extension {
    /// Extension type identifier
    pub extension_type: String,
    /// Extension data
    pub data: Vec<u8>,
}

impl KeyPackage {
    /// Create a new key package
    pub fn new(
        device_kp: &Keypair,
        identity_kp: &Keypair,
        device_metadata: &DeviceMetadata,
    ) -> Self {
        // Generate init key for ECDH
        let init_key = device_kp.public_key().to_vec();

        // Create credential containing device identity
        let credential = Self::create_credential(device_kp, identity_kp);

        // Add extensions with device capabilities
        let extensions = vec![
            Extension {
                extension_type: "capabilities".to_string(),
                data: bincode::serialize(device_metadata.capabilities.get().unwrap_or(&HashMap::new()))
                    .unwrap_or_default(),
            },
            Extension {
                extension_type: "device_id".to_string(),
                data: device_metadata.device_id.as_bytes().to_vec(),
            },
        ];

        // Create signature over package body
        let body = Self::package_body(&init_key, &credential, &extensions);
        let signature = identity_kp.sign(&body);

        KeyPackage {
            cipher_suite: "MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519".to_string(),
            init_key,
            leaf_secret_encryption: None,
            credential,
            extensions,
            signature,
        }
    }

    /// Create credential signed by identity key
    fn create_credential(device_kp: &Keypair, identity_kp: &Keypair) -> Vec<u8> {
        let mut cred = Vec::new();
        cred.extend_from_slice(b"MLS_CREDENTIAL_v1");
        cred.extend_from_slice(identity_kp.public_key());
        cred.extend_from_slice(device_kp.public_key());

        let sig = identity_kp.sign(&cred);
        cred.extend_from_slice(&sig);
        cred
    }

    /// Create package body for signing
    fn package_body(init_key: &[u8], credential: &[u8], extensions: &[Extension]) -> Vec<u8> {
        let mut body = Vec::new();
        body.extend_from_slice(init_key);
        body.extend_from_slice(credential);

        for ext in extensions {
            body.extend_from_slice(ext.extension_type.as_bytes());
            body.extend_from_slice(&ext.data);
        }

        body
    }

    /// Serialize to bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        bincode::serialize(self).expect("Failed to serialize KeyPackage")
    }

    /// Deserialize from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        bincode::deserialize(bytes).map_err(|e| format!("Failed to deserialize: {}", e))
    }

    /// Compute hash of this key package
    pub fn hash(&self) -> Hash {
        let hash = blake3::hash(&self.to_bytes());
        hash.as_bytes()[0..32].to_vec()
    }

    /// Verify signature on this key package
    pub fn verify(&self, identity_pubkey: &[u8]) -> bool {
        let body = Self::package_body(&self.init_key, &self.credential, &self.extensions);
        Keypair::verify(identity_pubkey, &body, &self.signature)
    }
}

/// Device bundle containing key package and metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceBundle {
    /// The key package
    pub key_package: KeyPackage,
    /// Device metadata
    pub device_metadata: DeviceMetadata,
    /// Signature over bundle
    pub signature: Vec<u8>,
}

impl DeviceBundle {
    /// Create a new device bundle
    pub fn new(
        key_package: KeyPackage,
        device_metadata: DeviceMetadata,
        identity_kp: &Keypair,
    ) -> Self {
        let mut body = Vec::new();
        body.extend_from_slice(&key_package.to_bytes());
        body.extend_from_slice(&bincode::serialize(&device_metadata).unwrap_or_default());

        let signature = identity_kp.sign(&body);

        DeviceBundle {
            key_package,
            device_metadata,
            signature,
        }
    }

    /// Verify this bundle
    pub fn verify(&self, identity_pubkey: &[u8]) -> bool {
        let mut body = Vec::new();
        body.extend_from_slice(&self.key_package.to_bytes());
        body.extend_from_slice(&bincode::serialize(&self.device_metadata).unwrap_or_default());

        Keypair::verify(identity_pubkey, &body, &self.signature)
    }
}

/// Identity bundle containing user info and devices
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityBundle {
    /// User ID
    pub user_id: Vec<u8>,
    /// Identity public key
    pub public_key: Vec<u8>,
    /// List of device IDs
    pub devices: Vec<DeviceId>,
    /// Signature over bundle
    pub signature: Vec<u8>,
}

impl IdentityBundle {
    /// Create a new identity bundle
    pub fn new(
        user_id: Vec<u8>,
        public_key: Vec<u8>,
        devices: Vec<DeviceId>,
        identity_kp: &Keypair,
    ) -> Self {
        let mut body = Vec::new();
        body.extend_from_slice(&user_id);
        body.extend_from_slice(&public_key);
        for device in &devices {
            body.extend_from_slice(device.as_bytes());
        }

        let signature = identity_kp.sign(&body);

        IdentityBundle {
            user_id,
            public_key,
            devices,
            signature,
        }
    }

    /// Verify this bundle
    pub fn verify(&self) -> bool {
        let mut body = Vec::new();
        body.extend_from_slice(&self.user_id);
        body.extend_from_slice(&self.public_key);
        for device in &self.devices {
            body.extend_from_slice(device.as_bytes());
        }

        Keypair::verify(&self.public_key, &body, &self.signature)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core_identity::keypair::KeyType;

    #[test]
    fn test_keypackage_creation() {
        let device_kp = Keypair::generate(KeyType::Ed25519);
        let identity_kp = Keypair::generate(KeyType::Ed25519);

        let device_id = DeviceId::generate();
        let device_meta = DeviceMetadata::new(device_id, "Test Device".to_string(), "node1");

        let kp = KeyPackage::new(&device_kp, &identity_kp, &device_meta);
        assert!(!kp.init_key.is_empty());
        assert!(!kp.credential.is_empty());
        assert!(!kp.signature.is_empty());
    }

    #[test]
    fn test_keypackage_sign_and_verify() {
        let device_kp = Keypair::generate(KeyType::Ed25519);
        let identity_kp = Keypair::generate(KeyType::Ed25519);

        let device_id = DeviceId::generate();
        let device_meta = DeviceMetadata::new(device_id, "Test Device".to_string(), "node1");

        let kp = KeyPackage::new(&device_kp, &identity_kp, &device_meta);
        assert!(kp.verify(identity_kp.public_key()));
    }

    #[test]
    fn test_keypackage_hash() {
        let device_kp = Keypair::generate(KeyType::Ed25519);
        let identity_kp = Keypair::generate(KeyType::Ed25519);

        let device_id = DeviceId::generate();
        let device_meta = DeviceMetadata::new(device_id, "Test Device".to_string(), "node1");

        let kp = KeyPackage::new(&device_kp, &identity_kp, &device_meta);
        let hash1 = kp.hash();
        let hash2 = kp.hash();
        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 32);
    }

    #[test]
    fn test_device_bundle() {
        let device_kp = Keypair::generate(KeyType::Ed25519);
        let identity_kp = Keypair::generate(KeyType::Ed25519);

        let device_id = DeviceId::generate();
        let device_meta = DeviceMetadata::new(device_id, "Test Device".to_string(), "node1");

        let kp = KeyPackage::new(&device_kp, &identity_kp, &device_meta);
        let bundle = DeviceBundle::new(kp, device_meta, &identity_kp);

        assert!(bundle.verify(identity_kp.public_key()));
    }

    #[test]
    fn test_identity_bundle() {
        let identity_kp = Keypair::generate(KeyType::Ed25519);
        let user_id = vec![1, 2, 3, 4];
        let devices = vec![DeviceId::generate(), DeviceId::generate()];

        let bundle = IdentityBundle::new(
            user_id,
            identity_kp.public_key().to_vec(),
            devices,
            &identity_kp,
        );

        assert!(bundle.verify());
    }
}
