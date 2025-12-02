//! Identity Bridge Implementation
//!
//! Bridges core_identity with MLS credential requirements.

use crate::core_mls::errors::{MlsError, MlsResult};
use crate::core_mls::traits::identity::{IdentityBridge, MemberId};
use crate::core_identity::{Keypair, KeyType};
use async_trait::async_trait;
use std::sync::Arc;

/// Identity bridge implementation
///
/// Wraps core_identity keypairs to provide MLS credentials.
pub struct IdentityBridgeImpl {
    keypair: Arc<Keypair>,
}

impl IdentityBridgeImpl {
    /// Create a new identity bridge
    ///
    /// # Arguments
    /// * `keypair` - The identity keypair to use
    pub fn new(keypair: Arc<Keypair>) -> Self {
        Self { keypair }
    }

    /// Create from an existing keypair
    pub fn from_keypair(keypair: Keypair) -> Self {
        Self {
            keypair: Arc::new(keypair),
        }
    }

    /// Generate a new identity with a random keypair
    pub fn generate() -> MlsResult<Self> {
        let keypair = Keypair::generate(KeyType::Ed25519);
        Ok(Self::from_keypair(keypair))
    }

    /// Get the underlying keypair
    pub fn keypair(&self) -> &Keypair {
        &self.keypair
    }
}

#[async_trait]
impl IdentityBridge for IdentityBridgeImpl {
    async fn local_member_id(&self) -> MlsResult<MemberId> {
        // Use public key bytes as member ID
        Ok(self.keypair.public_key().to_vec())
    }

    async fn export_credential_bundle(&self) -> MlsResult<Vec<u8>> {
        // For BasicCredential, we export the public key
        // In a more sophisticated setup, this would be a full X.509 cert chain
        Ok(self.keypair.public_key().to_vec())
    }

    async fn validate_remote_credential(&self, credential_bundle: &[u8]) -> MlsResult<()> {
        // Basic validation: check that it's a valid Ed25519 public key
        if credential_bundle.len() != 32 {
            return Err(MlsError::InvalidInput(format!(
                "Invalid credential length: expected 32, got {}",
                credential_bundle.len()
            )));
        }

        // Additional validation could check:
        // - Certificate chain validity
        // - Revocation status
        // - Trust anchors
        // - Policy compliance

        Ok(())
    }

    async fn sign_for_mls(&self, message: &[u8]) -> MlsResult<Vec<u8>> {
        Ok(self.keypair.sign(message))
    }

    async fn public_key(&self) -> MlsResult<Vec<u8>> {
        Ok(self.keypair.public_key().to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_identity_bridge_creation() {
        let bridge = IdentityBridgeImpl::generate().unwrap();
        let member_id = bridge.local_member_id().await.unwrap();
        
        assert_eq!(member_id.len(), 32); // Ed25519 public key
    }

    #[tokio::test]
    async fn test_export_credential() {
        let bridge = IdentityBridgeImpl::generate().unwrap();
        let credential = bridge.export_credential_bundle().await.unwrap();
        
        assert_eq!(credential.len(), 32);
    }

    #[tokio::test]
    async fn test_validate_credential() {
        let bridge = IdentityBridgeImpl::generate().unwrap();
        let credential = bridge.export_credential_bundle().await.unwrap();
        
        // Should accept valid credential
        bridge.validate_remote_credential(&credential).await.unwrap();
        
        // Should reject invalid length
        let invalid = vec![0u8; 16];
        assert!(bridge.validate_remote_credential(&invalid).await.is_err());
    }

    #[tokio::test]
    async fn test_sign_and_verify() {
        let bridge = IdentityBridgeImpl::generate().unwrap();
        let message = b"test message";
        
        let signature = bridge.sign_for_mls(message).await.unwrap();
        let public_key = bridge.public_key().await.unwrap();
        
        // Verify using the static method
        assert!(Keypair::verify(&public_key, message, &signature));
    }
}
