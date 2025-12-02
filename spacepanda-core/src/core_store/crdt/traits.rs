/*
    traits.rs - Core CRDT trait definitions
    
    Defines the unified interface that all CRDT types must implement:
    - Apply operations
    - Merge with other replicas
    - Query current state
*/

use super::vector_clock::VectorClock;
use crate::core_store::store::errors::StoreResult;
use serde::{Deserialize, Serialize};

/// Core trait that all CRDTs must implement
pub trait Crdt: Clone + Send + Sync {
    /// The type of operations this CRDT accepts
    type Operation: Clone + Send + Sync;
    
    /// The type of value this CRDT represents
    type Value: Clone;
    
    /// Apply a local operation to this CRDT
    /// This is called when the local node performs an action
    fn apply(&mut self, op: Self::Operation) -> StoreResult<()>;
    
    /// Merge another CRDT state into this one
    /// This is called when receiving state from a remote peer
    fn merge(&mut self, other: &Self) -> StoreResult<()>;
    
    /// Get the current value/state
    fn value(&self) -> Self::Value;
    
    /// Get the vector clock for causal ordering
    fn vector_clock(&self) -> &VectorClock;
}

/// Trait for CRDTs that can be validated before applying
pub trait ValidatedCrdt: Crdt {
    /// Validate an operation before applying it
    fn validate(&self, op: &Self::Operation) -> StoreResult<()>;
}

/// Trait for CRDTs that support tombstones (deletion markers)
pub trait TombstoneCrdt: Crdt {
    /// Check if an element is tombstoned (deleted but retained for sync)
    fn is_tombstoned(&self, key: &str) -> bool;
    
    /// Garbage collect tombstones older than the given threshold
    fn gc_tombstones(&mut self, threshold_ms: u64) -> usize;
}

/// Metadata attached to every CRDT operation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OperationMetadata {
    /// Node ID that created this operation
    pub node_id: String,
    
    /// Vector clock at the time of creation
    pub vector_clock: VectorClock,
    
    /// Timestamp in milliseconds since epoch
    pub timestamp: u64,
    
    /// Optional signature over the operation
    pub signature: Option<Vec<u8>>,
}

impl OperationMetadata {
    pub fn new(node_id: String, vector_clock: VectorClock) -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        
        OperationMetadata {
            node_id,
            vector_clock,
            timestamp,
            signature: None,
        }
    }
    
    pub fn with_signature(mut self, signature: Vec<u8>) -> Self {
        self.signature = Some(signature);
        self
    }
    
    /// Check if this operation has a signature
    pub fn is_signed(&self) -> bool {
        self.signature.is_some()
    }
    
    /// Verify the signature on this operation
    /// Returns Ok(()) if signature is valid or if operation is unsigned but not required
    /// Returns Err if signature is invalid or missing when required
    pub fn verify_signature(&self, operation_data: &[u8], public_key: &[u8], context: &str, require_signature: bool) -> StoreResult<()> {
        use crate::core_store::crdt::signer::{PublicKey, Signature};
        use crate::core_store::crdt::signer::OperationVerifier;
        
        match &self.signature {
            Some(sig_bytes) => {
                // Has signature - verify it
                let public_key = PublicKey::from_bytes(public_key.to_vec())
                    .map_err(|e| crate::core_store::store::errors::StoreError::InvalidSignature(e))?;
                
                let verifier = OperationVerifier::new(public_key, context.to_string());
                let signature = Signature(sig_bytes.clone());
                
                if verifier.verify_operation(operation_data, &signature) {
                    Ok(())
                } else {
                    Err(crate::core_store::store::errors::StoreError::InvalidSignature(
                        "Signature verification failed".to_string()
                    ))
                }
            }
            None => {
                // No signature
                if require_signature {
                    Err(crate::core_store::store::errors::StoreError::InvalidSignature(
                        "Operation must be signed".to_string()
                    ))
                } else {
                    Ok(())
                }
            }
        }
    }
}

/// Generic CRDT operation wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrdtOperation<T> {
    /// The actual operation data
    pub data: T,
    
    /// Metadata for causal ordering and validation
    pub metadata: OperationMetadata,
}

impl<T> CrdtOperation<T> {
    pub fn new(data: T, metadata: OperationMetadata) -> Self {
        CrdtOperation { data, metadata }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_operation_metadata_creation() {
        let vc = VectorClock::new();
        let metadata = OperationMetadata::new("node1".to_string(), vc.clone());
        
        assert_eq!(metadata.node_id, "node1");
        assert_eq!(metadata.vector_clock, vc);
        assert!(metadata.timestamp > 0);
        assert!(metadata.signature.is_none());
    }
    
    #[test]
    fn test_operation_metadata_with_signature() {
        let vc = VectorClock::new();
        let sig = vec![1, 2, 3, 4];
        let metadata = OperationMetadata::new("node1".to_string(), vc)
            .with_signature(sig.clone());
        
        assert_eq!(metadata.signature, Some(sig));
    }
    
    #[test]
    fn test_crdt_operation_creation() {
        let vc = VectorClock::new();
        let metadata = OperationMetadata::new("node1".to_string(), vc);
        let op = CrdtOperation::new("test_data".to_string(), metadata.clone());
        
        assert_eq!(op.data, "test_data");
        assert_eq!(op.metadata, metadata);
    }
    
    #[test]
    fn test_signature_verification_valid() {
        use crate::core_identity::keypair::{Keypair, KeyType};
        use crate::core_store::crdt::signer::{SigningKey, OperationSigner};
        
        // Create keypair and signer
        let keypair = Keypair::generate(KeyType::Ed25519);
        let signing_key = SigningKey::from_keypair(keypair.clone());
        let signer = OperationSigner::new(signing_key, "channel_123".to_string());
        
        // Create operation data and sign it
        let operation_data = b"test operation";
        let signature = signer.sign_operation(operation_data);
        
        // Create metadata with signature
        let vc = VectorClock::new();
        let metadata = OperationMetadata::new("node1".to_string(), vc)
            .with_signature(signature.0);
        
        // Verify signature
        let result = metadata.verify_signature(
            operation_data,
            keypair.public_key(),
            "channel_123",
            true
        );
        
        assert!(result.is_ok(), "Valid signature should verify");
    }
    
    #[test]
    fn test_signature_verification_forged() {
        use crate::core_identity::keypair::{Keypair, KeyType};
        
        // Create two different keypairs
        let keypair1 = Keypair::generate(KeyType::Ed25519);
        let keypair2 = Keypair::generate(KeyType::Ed25519);
        
        // Sign with keypair1
        let operation_data = b"test operation";
        let signature = keypair1.sign(operation_data);
        
        // Create metadata with signature from keypair1
        let vc = VectorClock::new();
        let metadata = OperationMetadata::new("node1".to_string(), vc)
            .with_signature(signature);
        
        // Try to verify with keypair2's public key (should fail)
        let result = metadata.verify_signature(
            operation_data,
            keypair2.public_key(),
            "channel_123",
            true
        );
        
        assert!(result.is_err(), "Forged signature should be rejected");
    }
    
    #[test]
    fn test_signature_verification_unsigned_required() {
        let vc = VectorClock::new();
        let metadata = OperationMetadata::new("node1".to_string(), vc);
        
        let operation_data = b"test operation";
        let dummy_pubkey = vec![0u8; 32];
        
        // Should fail when signature required but missing
        let result = metadata.verify_signature(
            operation_data,
            &dummy_pubkey,
            "channel_123",
            true
        );
        
        assert!(result.is_err(), "Unsigned operation should be rejected when signature required");
    }
    
    #[test]
    fn test_signature_verification_unsigned_optional() {
        let vc = VectorClock::new();
        let metadata = OperationMetadata::new("node1".to_string(), vc);
        
        let operation_data = b"test operation";
        let dummy_pubkey = vec![0u8; 32];
        
        // Should pass when signature not required
        let result = metadata.verify_signature(
            operation_data,
            &dummy_pubkey,
            "channel_123",
            false
        );
        
        assert!(result.is_ok(), "Unsigned operation should pass when signature not required");
    }
}
