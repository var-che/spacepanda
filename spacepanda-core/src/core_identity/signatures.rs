//! Signatures module
//!
//! Defines signed statements produced by the identity system.

use crate::core_identity::device_id::DeviceId;
use crate::core_identity::keypair::Keypair;
use crate::core_identity::user_id::UserId;
use serde::{Deserialize, Serialize};

/// Hash type
pub type Hash = Vec<u8>;

/// Signed statement types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IdentitySignature {
    /// Device ownership proof
    DeviceOwnership {
        device_id: DeviceId,
        user_id: UserId,
        timestamp: u64,
        nonce: Vec<u8>,
        signature: Vec<u8>,
    },
    /// Space ownership proof
    SpaceOwnership {
        space_id: String,
        user_id: UserId,
        timestamp: u64,
        nonce: Vec<u8>,
        signature: Vec<u8>,
    },
    /// Channel creation proof
    ChannelCreation {
        channel_id: String,
        user_id: UserId,
        timestamp: u64,
        nonce: Vec<u8>,
        signature: Vec<u8>,
    },
    /// Key package binding proof
    KeyPackage {
        keypackage_hash: Hash,
        user_id: UserId,
        device_id: DeviceId,
        timestamp: u64,
        nonce: Vec<u8>,
        signature: Vec<u8>,
    },
    /// Generic identity proof
    IdentityProof {
        user_id: UserId,
        pubkey: Vec<u8>,
        timestamp: u64,
        nonce: Vec<u8>,
        signature: Vec<u8>,
    },
}

impl IdentitySignature {
    /// Create a device ownership signature
    pub fn sign_device_ownership(
        device_id: DeviceId,
        user_id: UserId,
        identity_kp: &Keypair,
    ) -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let nonce = generate_nonce();

        let mut payload = Vec::new();
        payload.extend_from_slice(b"DEVICE_OWNERSHIP");
        payload.extend_from_slice(device_id.as_bytes());
        payload.extend_from_slice(user_id.as_bytes());
        payload.extend_from_slice(&timestamp.to_le_bytes());
        payload.extend_from_slice(&nonce);

        let signature = identity_kp.sign(&payload);

        IdentitySignature::DeviceOwnership {
            device_id,
            user_id,
            timestamp,
            nonce,
            signature,
        }
    }

    /// Create a space ownership signature
    pub fn sign_space_ownership(space_id: String, user_id: UserId, identity_kp: &Keypair) -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let nonce = generate_nonce();

        let mut payload = Vec::new();
        payload.extend_from_slice(b"SPACE_OWNERSHIP");
        payload.extend_from_slice(space_id.as_bytes());
        payload.extend_from_slice(user_id.as_bytes());
        payload.extend_from_slice(&timestamp.to_le_bytes());
        payload.extend_from_slice(&nonce);

        let signature = identity_kp.sign(&payload);

        IdentitySignature::SpaceOwnership {
            space_id,
            user_id,
            timestamp,
            nonce,
            signature,
        }
    }

    /// Create a channel creation signature
    pub fn sign_channel_creation(
        channel_id: String,
        user_id: UserId,
        identity_kp: &Keypair,
    ) -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let nonce = generate_nonce();

        let mut payload = Vec::new();
        payload.extend_from_slice(b"CHANNEL_CREATION");
        payload.extend_from_slice(channel_id.as_bytes());
        payload.extend_from_slice(user_id.as_bytes());
        payload.extend_from_slice(&timestamp.to_le_bytes());
        payload.extend_from_slice(&nonce);

        let signature = identity_kp.sign(&payload);

        IdentitySignature::ChannelCreation {
            channel_id,
            user_id,
            timestamp,
            nonce,
            signature,
        }
    }

    /// Create a key package binding signature
    pub fn sign_keypackage_binding(
        keypackage_hash: Hash,
        user_id: UserId,
        device_id: DeviceId,
        identity_kp: &Keypair,
    ) -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let nonce = generate_nonce();

        let mut payload = Vec::new();
        payload.extend_from_slice(b"KEYPACKAGE_BINDING");
        payload.extend_from_slice(&keypackage_hash);
        payload.extend_from_slice(user_id.as_bytes());
        payload.extend_from_slice(device_id.as_bytes());
        payload.extend_from_slice(&timestamp.to_le_bytes());
        payload.extend_from_slice(&nonce);

        let signature = identity_kp.sign(&payload);

        IdentitySignature::KeyPackage {
            keypackage_hash,
            user_id,
            device_id,
            timestamp,
            nonce,
            signature,
        }
    }

    /// Create an identity proof signature
    pub fn sign_identity_proof(user_id: UserId, identity_kp: &Keypair) -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let nonce = generate_nonce();
        let pubkey = identity_kp.public_key().to_vec();

        let mut payload = Vec::new();
        payload.extend_from_slice(b"IDENTITY_PROOF");
        payload.extend_from_slice(user_id.as_bytes());
        payload.extend_from_slice(&pubkey);
        payload.extend_from_slice(&timestamp.to_le_bytes());
        payload.extend_from_slice(&nonce);

        let signature = identity_kp.sign(&payload);

        IdentitySignature::IdentityProof {
            user_id,
            pubkey,
            timestamp,
            nonce,
            signature,
        }
    }

    /// Verify this signature
    pub fn verify(&self, pubkey: &[u8]) -> bool {
        match self {
            IdentitySignature::DeviceOwnership {
                device_id,
                user_id,
                timestamp,
                nonce,
                signature,
            } => {
                let mut payload = Vec::new();
                payload.extend_from_slice(b"DEVICE_OWNERSHIP");
                payload.extend_from_slice(device_id.as_bytes());
                payload.extend_from_slice(user_id.as_bytes());
                payload.extend_from_slice(&timestamp.to_le_bytes());
                payload.extend_from_slice(nonce);
                Keypair::verify(pubkey, &payload, signature)
            }
            IdentitySignature::SpaceOwnership {
                space_id,
                user_id,
                timestamp,
                nonce,
                signature,
            } => {
                let mut payload = Vec::new();
                payload.extend_from_slice(b"SPACE_OWNERSHIP");
                payload.extend_from_slice(space_id.as_bytes());
                payload.extend_from_slice(user_id.as_bytes());
                payload.extend_from_slice(&timestamp.to_le_bytes());
                payload.extend_from_slice(nonce);
                Keypair::verify(pubkey, &payload, signature)
            }
            IdentitySignature::ChannelCreation {
                channel_id,
                user_id,
                timestamp,
                nonce,
                signature,
            } => {
                let mut payload = Vec::new();
                payload.extend_from_slice(b"CHANNEL_CREATION");
                payload.extend_from_slice(channel_id.as_bytes());
                payload.extend_from_slice(user_id.as_bytes());
                payload.extend_from_slice(&timestamp.to_le_bytes());
                payload.extend_from_slice(nonce);
                Keypair::verify(pubkey, &payload, signature)
            }
            IdentitySignature::KeyPackage {
                keypackage_hash,
                user_id,
                device_id,
                timestamp,
                nonce,
                signature,
            } => {
                let mut payload = Vec::new();
                payload.extend_from_slice(b"KEYPACKAGE_BINDING");
                payload.extend_from_slice(keypackage_hash);
                payload.extend_from_slice(user_id.as_bytes());
                payload.extend_from_slice(device_id.as_bytes());
                payload.extend_from_slice(&timestamp.to_le_bytes());
                payload.extend_from_slice(nonce);
                Keypair::verify(pubkey, &payload, signature)
            }
            IdentitySignature::IdentityProof {
                user_id,
                pubkey: proof_pubkey,
                timestamp,
                nonce,
                signature,
            } => {
                let mut payload = Vec::new();
                payload.extend_from_slice(b"IDENTITY_PROOF");
                payload.extend_from_slice(user_id.as_bytes());
                payload.extend_from_slice(proof_pubkey);
                payload.extend_from_slice(&timestamp.to_le_bytes());
                payload.extend_from_slice(nonce);
                Keypair::verify(pubkey, &payload, signature)
            }
        }
    }

    /// Get timestamp from signature (for replay protection)
    pub fn timestamp(&self) -> u64 {
        match self {
            IdentitySignature::DeviceOwnership { timestamp, .. }
            | IdentitySignature::SpaceOwnership { timestamp, .. }
            | IdentitySignature::ChannelCreation { timestamp, .. }
            | IdentitySignature::KeyPackage { timestamp, .. }
            | IdentitySignature::IdentityProof { timestamp, .. } => *timestamp,
        }
    }
}

/// Generate a random nonce for replay protection
fn generate_nonce() -> Vec<u8> {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    (0..16).map(|_| rng.random()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core_identity::keypair::KeyType;

    #[test]
    fn test_device_ownership_signature() {
        let identity_kp = Keypair::generate(KeyType::Ed25519);
        let device_id = DeviceId::generate();
        let user_id = UserId::from_public_key(identity_kp.public_key());

        let sig = IdentitySignature::sign_device_ownership(
            device_id,
            user_id,
            &identity_kp,
        );

        assert!(sig.verify(identity_kp.public_key()));
    }

    #[test]
    fn test_space_ownership_signature() {
        let identity_kp = Keypair::generate(KeyType::Ed25519);
        let user_id = UserId::from_public_key(identity_kp.public_key());

        let sig = IdentitySignature::sign_space_ownership(
            "space123".to_string(),
            user_id,
            &identity_kp,
        );

        assert!(sig.verify(identity_kp.public_key()));
    }

    #[test]
    fn test_replay_protection() {
        let identity_kp = Keypair::generate(KeyType::Ed25519);
        let user_id = UserId::from_public_key(identity_kp.public_key());

        let sig1 = IdentitySignature::sign_identity_proof(user_id.clone(), &identity_kp);
        
        // Sleep briefly to ensure different timestamp
        std::thread::sleep(std::time::Duration::from_millis(10));
        
        let sig2 = IdentitySignature::sign_identity_proof(user_id, &identity_kp);

        // Different nonces and timestamps mean different signatures possible
        // Just verify both are valid
        assert!(sig1.verify(identity_kp.public_key()));
        assert!(sig2.verify(identity_kp.public_key()));
    }
}
