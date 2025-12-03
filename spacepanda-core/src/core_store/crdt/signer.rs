/*
    signer.rs - CRDT operation signing and verification

    Signs CRDT operations to ensure authenticity and prevent tampering.
    Uses Ed25519 signatures integrated with core_identity.
*/

use crate::core_identity::keypair::Keypair;
use serde::{Deserialize, Serialize};

/// Signature over a CRDT operation
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Signature(pub Vec<u8>);

/// Public key for signature verification (32 bytes Ed25519)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicKey(pub Vec<u8>);

/// Signing key for creating signatures (wraps real Ed25519 keypair)
#[derive(Clone)]
pub struct SigningKey {
    keypair: Keypair,
}

impl SigningKey {
    /// Create from an existing Ed25519 keypair
    pub fn from_keypair(keypair: Keypair) -> Self {
        SigningKey { keypair }
    }

    /// Sign data using Ed25519
    pub fn sign(&self, data: &[u8]) -> Signature {
        Signature(self.keypair.sign(data))
    }

    /// Get the public key
    pub fn public_key(&self) -> PublicKey {
        PublicKey(self.keypair.public_key().to_vec())
    }
}

impl PublicKey {
    /// Create from raw bytes
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, String> {
        if bytes.len() != 32 {
            return Err("Public key must be 32 bytes".to_string());
        }
        Ok(PublicKey(bytes))
    }

    /// Verify a signature using Ed25519
    pub fn verify(&self, data: &[u8], signature: &Signature) -> bool {
        if signature.0.len() != 64 {
            return false; // Ed25519 signatures are 64 bytes
        }
        Keypair::verify(&self.0, data, &signature.0)
    }
}

/// Signs CRDT operations for a specific channel/context
pub struct OperationSigner {
    signing_key: SigningKey,
    context: String, // Channel ID or context identifier
}

impl OperationSigner {
    /// Create a new operation signer for a context
    pub fn new(signing_key: SigningKey, context: String) -> Self {
        OperationSigner { signing_key, context }
    }

    /// Sign operation data
    pub fn sign_operation(&self, operation_bytes: &[u8]) -> Signature {
        // Include context in signature
        let mut data = self.context.as_bytes().to_vec();
        data.extend_from_slice(operation_bytes);

        self.signing_key.sign(&data)
    }

    /// Get the public key
    pub fn public_key(&self) -> PublicKey {
        self.signing_key.public_key()
    }
}

/// Verifies CRDT operation signatures
pub struct OperationVerifier {
    public_key: PublicKey,
    context: String,
}

impl OperationVerifier {
    /// Create a new operation verifier
    pub fn new(public_key: PublicKey, context: String) -> Self {
        OperationVerifier { public_key, context }
    }

    /// Verify an operation signature
    pub fn verify_operation(&self, operation_bytes: &[u8], signature: &Signature) -> bool {
        // Include context in verification
        let mut data = self.context.as_bytes().to_vec();
        data.extend_from_slice(operation_bytes);

        self.public_key.verify(&data, signature)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core_identity::keypair::KeyType;

    #[test]
    fn test_signing_key_creation() {
        let keypair = Keypair::generate(KeyType::Ed25519);
        let key = SigningKey::from_keypair(keypair);
        let pubkey = key.public_key();

        assert_eq!(pubkey.0.len(), 32);
    }

    #[test]
    fn test_sign_and_verify() {
        let keypair = Keypair::generate(KeyType::Ed25519);
        let key = SigningKey::from_keypair(keypair);
        let data = b"test data";

        let signature = key.sign(data);
        assert_eq!(signature.0.len(), 64); // Ed25519 signatures are 64 bytes

        let public_key = key.public_key();
        assert!(public_key.verify(data, &signature));

        // Wrong data should fail
        assert!(!public_key.verify(b"wrong data", &signature));
    }

    #[test]
    fn test_operation_signer() {
        let keypair = Keypair::generate(KeyType::Ed25519);
        let key = SigningKey::from_keypair(keypair);
        let signer = OperationSigner::new(key, "channel_123".to_string());

        let operation = b"some operation data";
        let signature = signer.sign_operation(operation);

        assert_eq!(signature.0.len(), 64);

        // Verify with matching context
        let verifier = OperationVerifier::new(signer.public_key(), "channel_123".to_string());
        assert!(verifier.verify_operation(operation, &signature));

        // Wrong context should fail
        let wrong_verifier = OperationVerifier::new(signer.public_key(), "channel_456".to_string());
        assert!(!wrong_verifier.verify_operation(operation, &signature));
    }

    #[test]
    fn test_forged_signature_rejected() {
        let keypair1 = Keypair::generate(KeyType::Ed25519);
        let keypair2 = Keypair::generate(KeyType::Ed25519);

        let key1 = SigningKey::from_keypair(keypair1);
        let key2 = SigningKey::from_keypair(keypair2);

        let data = b"test data";
        let signature = key1.sign(data);

        // Different public key should reject signature
        let public_key2 = key2.public_key();
        assert!(!public_key2.verify(data, &signature));
    }
}
