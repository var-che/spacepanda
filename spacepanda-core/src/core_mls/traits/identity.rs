//! Identity Bridge Trait
//!
//! Bridges the application identity system with MLS credentials.

use async_trait::async_trait;
use crate::core_mls::errors::MlsResult;

/// Member identifier (typically public key fingerprint or user ID)
pub type MemberId = Vec<u8>;

/// Group identifier
pub type GroupId = Vec<u8>;

/// Identity bridge trait
///
/// Provides stable binding from application identity (user account, certificate chain,
/// device id) to MLS CredentialBundle.
///
/// This is the integration point between `core_identity` and MLS.
#[async_trait]
pub trait IdentityBridge: Send + Sync {
    /// Return the local member id
    ///
    /// This should be a stable identifier (e.g., public key fingerprint, user ID).
    /// MLS uses this to identify members in the group.
    async fn local_member_id(&self) -> MlsResult<MemberId>;

    /// Export the credential bundle bytes that MLS expects
    ///
    /// This should return a serialized MLS credential (e.g., BasicCredential, X.509, etc.).
    /// The format depends on the credential type used by the MLS group.
    async fn export_credential_bundle(&self) -> MlsResult<Vec<u8>>;

    /// Validate a remote credential bundle
    ///
    /// Should perform:
    /// - Certificate chain validation (if using X.509)
    /// - Revocation checks
    /// - Policy enforcement
    ///
    /// # Arguments
    /// * `credential_bundle` - Serialized credential from a remote member
    ///
    /// # Returns
    /// `Ok(())` if valid, `Err` if invalid or untrusted
    async fn validate_remote_credential(&self, credential_bundle: &[u8]) -> MlsResult<()>;

    /// Sign data for MLS operations
    ///
    /// Used when MLS requires a signature from the identity key.
    ///
    /// # Arguments
    /// * `message` - Data to sign
    ///
    /// # Returns
    /// Signature bytes
    async fn sign_for_mls(&self, message: &[u8]) -> MlsResult<Vec<u8>>;

    /// Get public key bytes for this identity
    ///
    /// Returns the public key that corresponds to the signing key.
    async fn public_key(&self) -> MlsResult<Vec<u8>> {
        // Default implementation - can be overridden
        // Extract from credential bundle
        let bundle = self.export_credential_bundle().await?;
        // This is a simplified default - real implementation depends on credential format
        Ok(bundle)
    }
}
