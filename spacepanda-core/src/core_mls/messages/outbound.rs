//! Outbound Message Building
//!
//! Converts user intents into properly formatted MLS messages wrapped in envelopes.

use super::{EncryptedEnvelope, MessageType};
use crate::core_mls::{engine::openmls_engine::OpenMlsEngine, errors::MlsResult};

#[cfg(test)]
use crate::core_mls::types::{GroupId, MlsConfig};
#[cfg(test)]
use openmls_rust_crypto::OpenMlsRustCrypto;
#[cfg(test)]
use std::sync::Arc;

/// Outbound message builder
///
/// Responsible for creating MLS messages from user actions and wrapping them
/// in envelopes for transport.
pub struct OutboundBuilder {
    /// Identity of this member (for sender field)
    identity: Vec<u8>,
}

impl OutboundBuilder {
    /// Create a new outbound message builder
    ///
    /// # Arguments
    /// * `identity` - The identity of the local user (sender)
    pub fn new(identity: Vec<u8>) -> Self {
        Self { identity }
    }

    /// Build an application message envelope
    ///
    /// # Arguments
    /// * `engine` - The MLS engine managing the group
    /// * `plaintext` - The plaintext message to encrypt
    ///
    /// # Returns
    /// An encrypted envelope ready for transport
    pub async fn build_application_message<P: openmls_traits::OpenMlsProvider + 'static>(
        &self,
        engine: &OpenMlsEngine<P>,
        plaintext: &[u8],
    ) -> MlsResult<EncryptedEnvelope> {
        let group_id = engine.group_id().await;
        let epoch = engine.epoch().await;

        // Encrypt the message
        let encrypted_payload = engine.send_message(plaintext).await?;

        // Wrap in envelope
        Ok(EncryptedEnvelope::new(
            group_id,
            epoch,
            self.identity.clone(),
            encrypted_payload,
            MessageType::Application,
        ))
    }

    /// Build a commit message envelope
    ///
    /// # Arguments
    /// * `engine` - The MLS engine managing the group
    ///
    /// # Returns
    /// An encrypted envelope ready for transport, plus optional Welcome messages
    pub async fn build_commit_message<P: openmls_traits::OpenMlsProvider + 'static>(
        &self,
        engine: &OpenMlsEngine<P>,
    ) -> MlsResult<(EncryptedEnvelope, Option<Vec<Vec<u8>>>)> {
        let group_id = engine.group_id().await;
        let epoch = engine.epoch().await;

        // Create commit for pending proposals
        let (commit_payload, welcome_messages) = engine.commit_pending().await?;

        // Wrap commit in envelope
        let envelope = EncryptedEnvelope::new(
            group_id,
            epoch,
            self.identity.clone(),
            commit_payload,
            MessageType::Commit,
        );

        Ok((envelope, welcome_messages))
    }

    /// Build proposal to add members
    ///
    /// Note: Currently returns the raw serialized proposal.
    /// In a full implementation, this would create a Proposal message.
    pub async fn build_add_proposal<P: openmls_traits::OpenMlsProvider + 'static>(
        &self,
        engine: &OpenMlsEngine<P>,
        key_packages: Vec<Vec<u8>>,
    ) -> MlsResult<EncryptedEnvelope> {
        let group_id = engine.group_id().await;
        let epoch = engine.epoch().await;

        // For now, we skip the proposal step and go straight to commit
        // In a production system, you'd first create proposals, then commit them
        use crate::core_mls::engine::group_ops::GroupOperations;
        let (commit_payload, _welcome) = engine.add_members(key_packages).await?;

        // Wrap in envelope as a commit (since add_members commits immediately)
        Ok(EncryptedEnvelope::new(
            group_id,
            epoch,
            self.identity.clone(),
            commit_payload,
            MessageType::Commit,
        ))
    }

    /// Build proposal to remove members
    ///
    /// Note: Currently returns the raw serialized proposal.
    /// In a full implementation, this would create a Proposal message.
    pub async fn build_remove_proposal<P: openmls_traits::OpenMlsProvider + 'static>(
        &self,
        engine: &OpenMlsEngine<P>,
        leaf_indices: Vec<u32>,
    ) -> MlsResult<EncryptedEnvelope> {
        let group_id = engine.group_id().await;
        let epoch = engine.epoch().await;

        // For now, we skip the proposal step and go straight to commit
        use crate::core_mls::engine::group_ops::GroupOperations;
        let commit_payload = engine.remove_members(leaf_indices).await?;

        // Wrap in envelope as a commit
        Ok(EncryptedEnvelope::new(
            group_id,
            epoch,
            self.identity.clone(),
            commit_payload,
            MessageType::Commit,
        ))
    }

    /// Get the sender identity
    pub fn identity(&self) -> &[u8] {
        &self.identity
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core_mls::types::MlsConfig;

    #[tokio::test]
    async fn test_outbound_builder_creation() {
        let builder = OutboundBuilder::new(b"alice@example.com".to_vec());
        assert_eq!(builder.identity(), b"alice@example.com");
    }

    #[tokio::test]
    async fn test_build_application_message() {
        let group_id = GroupId::random();
        let identity = b"alice@example.com".to_vec();
        let config = MlsConfig::default();
        let provider = Arc::new(OpenMlsRustCrypto::default());

        // Create a group
        let engine = OpenMlsEngine::create_group(group_id.clone(), identity.clone(), config, provider)
            .await
            .expect("Failed to create group");

        // Create builder
        let builder = OutboundBuilder::new(identity.clone());

        // Build application message
        let envelope = builder
            .build_application_message(&engine, b"Hello, World!")
            .await
            .expect("Failed to build message");

        // Verify envelope metadata
        assert_eq!(envelope.group_id(), &group_id);
        assert_eq!(envelope.epoch(), 0);
        assert_eq!(envelope.sender(), &identity[..]);
        assert_eq!(envelope.message_type(), MessageType::Application);
        assert!(!envelope.payload().is_empty());
    }

    #[tokio::test]
    async fn test_build_commit_message() {
        let group_id = GroupId::random();
        let identity = b"bob@example.com".to_vec();
        let config = MlsConfig::default();
        let provider = Arc::new(OpenMlsRustCrypto::default());

        // Create a group
        let engine = OpenMlsEngine::create_group(group_id.clone(), identity.clone(), config, provider)
            .await
            .expect("Failed to create group");

        // Create builder
        let builder = OutboundBuilder::new(identity.clone());

        // Build commit message
        let (envelope, _welcome) =
            builder.build_commit_message(&engine).await.expect("Failed to build commit");

        // Verify envelope metadata
        assert_eq!(envelope.group_id(), &group_id);
        assert_eq!(envelope.epoch(), 0); // Epoch before commit
        assert_eq!(envelope.sender(), &identity[..]);
        assert_eq!(envelope.message_type(), MessageType::Commit);
        assert!(!envelope.payload().is_empty());
    }
}
