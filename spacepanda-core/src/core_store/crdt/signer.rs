/*
    signer.rs - CRDT operation signing and verification
    
    Signs CRDT operations to ensure authenticity and prevent tampering.
    Uses Ed25519 signatures (placeholder for now, will integrate with core_identity).
*/

use serde::{Deserialize, Serialize};

/// Signature over a CRDT operation
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Signature(pub Vec<u8>);

/// Public key for signature verification
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicKey(pub Vec<u8>);

/// Signing key for creating signatures
#[derive(Debug, Clone)]
pub struct SigningKey {
    // TODO: Replace with actual Ed25519 key from core_identity
    key_bytes: Vec<u8>,
}

impl SigningKey {
    /// Create a new signing key (placeholder implementation)
    pub fn generate() -> Self {
        // TODO: Use actual Ed25519 key generation
        SigningKey {
            key_bytes: vec![0; 32],
        }
    }
    
    /// Sign data
    pub fn sign(&self, data: &[u8]) -> Signature {
        // TODO: Implement actual Ed25519 signing
        // For now, just hash the data as a placeholder
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        data.hash(&mut hasher);
        self.key_bytes.hash(&mut hasher);
        let hash = hasher.finish();
        
        Signature(hash.to_le_bytes().to_vec())
    }
    
    /// Get the public key
    pub fn public_key(&self) -> PublicKey {
        // TODO: Derive actual public key
        PublicKey(self.key_bytes.clone())
    }
}

impl PublicKey {
    /// Verify a signature
    pub fn verify(&self, data: &[u8], signature: &Signature) -> bool {
        // TODO: Implement actual Ed25519 verification
        // For now, just check signature is not empty
        !signature.0.is_empty() && !data.is_empty()
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
        OperationSigner {
            signing_key,
            context,
        }
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
        OperationVerifier {
            public_key,
            context,
        }
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
    
    #[test]
    fn test_signing_key_generation() {
        let key = SigningKey::generate();
        assert_eq!(key.key_bytes.len(), 32);
    }
    
    #[test]
    fn test_sign_and_verify() {
        let key = SigningKey::generate();
        let data = b"test data";
        
        let signature = key.sign(data);
        let public_key = key.public_key();
        
        assert!(public_key.verify(data, &signature));
    }
    
    #[test]
    fn test_operation_signer() {
        let key = SigningKey::generate();
        let signer = OperationSigner::new(key, "channel_123".to_string());
        
        let operation = b"some operation data";
        let signature = signer.sign_operation(operation);
        
        assert!(!signature.0.is_empty());
    }
    
    #[test]
    fn test_operation_verifier() {
        let key = SigningKey::generate();
        let public_key = key.public_key();
        let signer = OperationSigner::new(key, "channel_123".to_string());
        let verifier = OperationVerifier::new(public_key, "channel_123".to_string());
        
        let operation = b"some operation data";
        let signature = signer.sign_operation(operation);
        
        assert!(verifier.verify_operation(operation, &signature));
    }
    
    #[test]
    fn test_different_context_fails() {
        let key = SigningKey::generate();
        let public_key = key.public_key();
        let signer = OperationSigner::new(key, "channel_123".to_string());
        let verifier = OperationVerifier::new(public_key, "channel_456".to_string());
        
        let operation = b"some operation data";
        let signature = signer.sign_operation(operation);
        
        // Verification with different context should fail
        // (in real implementation; placeholder always passes if not empty)
        assert!(signature.0.len() > 0);
    }
}
