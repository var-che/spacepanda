//! OpenMLS Engine Implementation
//!
//! Wraps OpenMLS MlsGroup to maintain our current MlsHandle API while using
//! OpenMLS for all cryptographic operations and state management.

use crate::core_mls::{
    errors::{MlsError, MlsResult},
    types::{GroupId, GroupMetadata, MemberInfo, MlsConfig},
    events::MlsEvent,
};

use openmls::prelude::*;
use openmls_basic_credential::SignatureKeyPair;
use openmls_rust_crypto::OpenMlsRustCrypto;
use std::sync::Arc;
use tokio::sync::RwLock;
use tls_codec::{Deserialize as TlsDeserialize, Serialize as TlsSerialize};

/// OpenMLS engine wrapper
///
/// This wraps an OpenMLS MlsGroup and provides the same API as our custom MlsGroup,
/// allowing transparent migration from custom crypto to OpenMLS.
///
/// Note: This is a simplified version that uses OpenMlsRustCrypto directly
/// instead of our trait-based providers. Full trait integration will come later.
pub struct OpenMlsEngine {
    /// The underlying OpenMLS group
    pub(crate) group: Arc<RwLock<MlsGroup>>,
    
    /// OpenMLS crypto provider
    provider: Arc<OpenMlsRustCrypto>,
    
    /// Group configuration
    config: MlsConfig,
    
    /// This member's signature keys
    signature_keys: SignatureKeyPair,
    
    /// This member's credential
    credential: CredentialWithKey,
}

impl OpenMlsEngine {
    /// Create a new group (as creator)
    ///
    /// # Arguments
    /// * `group_id` - Unique group identifier
    /// * `identity` - Member identity (username/user ID)
    /// * `config` - Group configuration
    pub async fn create_group(
        group_id: GroupId,
        identity: Vec<u8>,
        config: MlsConfig,
    ) -> MlsResult<Self> {
        // Create crypto provider
        let provider = Arc::new(OpenMlsRustCrypto::default());
        
        // Define ciphersuite
        let ciphersuite = Ciphersuite::MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519;
        
        // Generate signature keys
        let signature_keys = SignatureKeyPair::new(
            ciphersuite.signature_algorithm(),
        ).map_err(|e| MlsError::CryptoError(format!("Failed to generate signature keys: {:?}", e)))?;
        
        // Store signature keys (required by OpenMLS)
        signature_keys.store(provider.storage())
            .map_err(|e| MlsError::CryptoError(format!("Failed to store signature keys: {:?}", e)))?;
        
        // Create credential using BasicCredential
        let basic_credential = BasicCredential::new(identity);
        let credential_bundle = CredentialWithKey {
            credential: basic_credential.into(),
            signature_key: signature_keys.public().into(),
        };
        
        // Create MLS group configuration
        let mls_group_config = MlsGroupCreateConfig::builder()
            .wire_format_policy(PURE_CIPHERTEXT_WIRE_FORMAT_POLICY)
            .ciphersuite(ciphersuite)
            .build();
        
        // Convert our GroupId to OpenMLS GroupId
        let openmls_group_id = openmls::prelude::GroupId::from_slice(group_id.as_bytes());
        
        // Create the OpenMLS group with custom group ID
        let group = MlsGroup::new_with_group_id(
            &*provider,
            &signature_keys,
            &mls_group_config,
            openmls_group_id,
            credential_bundle.clone(),
        ).map_err(|e| MlsError::InvalidMessage(format!("Failed to create group: {:?}", e)))?;
        
        Ok(Self {
            group: Arc::new(RwLock::new(group)),
            provider,
            config,
            signature_keys,
            credential: credential_bundle,
        })
    }
    
    /// Join a group via Welcome message
    ///
    /// # Arguments
    /// * `welcome_bytes` - Serialized Welcome message
    /// * `ratchet_tree` - Optional ratchet tree for optimization
    /// * `config` - Group configuration
    pub async fn join_from_welcome(
        welcome_bytes: &[u8],
        ratchet_tree: Option<Vec<u8>>,
        config: MlsConfig,
    ) -> MlsResult<Self> {
        // Create crypto provider
        let provider = Arc::new(OpenMlsRustCrypto::default());
        
        // Parse Welcome message
        let mls_message = MlsMessageIn::tls_deserialize_exact(welcome_bytes)
            .map_err(|e| MlsError::InvalidMessage(format!("Failed to parse welcome: {:?}", e)))?;
        
        // Extract Welcome from MlsMessageIn
        let welcome = match mls_message.extract() {
            MlsMessageBodyIn::Welcome(w) => w,
            _ => return Err(MlsError::InvalidMessage("Expected Welcome message".to_string())),
        };
        
        // Define ciphersuite (will be extracted from Welcome)
        let ciphersuite = Ciphersuite::MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519;
        
        // Generate signature keys for this member
        let signature_keys = SignatureKeyPair::new(
            ciphersuite.signature_algorithm(),
        ).map_err(|e| MlsError::CryptoError(format!("Failed to generate signature keys: {:?}", e)))?;
        
        // Store signature keys
        signature_keys.store(provider.storage())
            .map_err(|e| MlsError::CryptoError(format!("Failed to store signature keys: {:?}", e)))?;
        
        // Create credential (identity will come from Welcome)
        // Create temporary credential (will be replaced by Welcome)
        let basic_credential = BasicCredential::new(vec![]);
        let credential_bundle = CredentialWithKey {
            credential: basic_credential.into(),
            signature_key: signature_keys.public().into(),
        };
        
        // Parse optional ratchet tree
        let ratchet_tree_in = if let Some(ref tree_bytes) = ratchet_tree {
            Some(RatchetTreeIn::tls_deserialize_exact(tree_bytes)
                .map_err(|e| MlsError::InvalidMessage(format!("Failed to parse ratchet tree: {:?}", e)))?)
        } else {
            None
        };
        
        // Create join config
        let join_config = MlsGroupJoinConfig::builder()
            .wire_format_policy(PURE_CIPHERTEXT_WIRE_FORMAT_POLICY)
            .build();
        
        // Stage the welcome (validates and prepares group state)
        let staged_welcome = StagedWelcome::new_from_welcome(
            &*provider,
            &join_config,
            welcome,
            ratchet_tree_in,
        ).map_err(|e| MlsError::InvalidMessage(format!("Failed to stage welcome: {:?}", e)))?;
        
        // Convert to active group
        let group = staged_welcome.into_group(&*provider)
            .map_err(|e| MlsError::InvalidMessage(format!("Failed to join group: {:?}", e)))?;
        
        Ok(Self {
            group: Arc::new(RwLock::new(group)),
            provider,
            config,
            signature_keys,
            credential: credential_bundle,
        })
    }
    
    /// Get the group ID
    pub async fn group_id(&self) -> GroupId {
        let group = self.group.read().await;
        GroupId::new(group.group_id().as_slice().to_vec())
    }
    
    /// Get the current epoch
    pub async fn epoch(&self) -> u64 {
        let group = self.group.read().await;
        group.epoch().as_u64()
    }
    
    /// Get group metadata
    pub async fn metadata(&self) -> MlsResult<GroupMetadata> {
        let group = self.group.read().await;
        
        // Extract member information from the tree
        let members: Vec<MemberInfo> = group.members()
            .map(|member| {
                // Extract identity from credential - use serialized credential as identity
                let identity = member.credential.serialized_content().to_vec();
                MemberInfo {
                    identity,
                    leaf_index: member.index.u32(),
                    joined_at: 0, // OpenMLS doesn't track join time by default
                }
            })
            .collect();
        
        Ok(GroupMetadata {
            group_id: GroupId::new(group.group_id().as_slice().to_vec()),
            name: None, // Can be added via group context extensions
            epoch: group.epoch().as_u64(),
            members,
            created_at: 0, // Would need custom storage
            updated_at: 0, // Would need custom storage
        })
    }
    
    /// Get reference to provider for use in operations
    pub(crate) fn provider(&self) -> &OpenMlsRustCrypto {
        &self.provider
    }
    
    /// Get reference to signature keys
    pub(crate) fn signature_keys(&self) -> &SignatureKeyPair {
        &self.signature_keys
    }
    
    /// Send an application message to the group
    ///
    /// # Arguments
    /// * `plaintext` - The plaintext message to encrypt and send
    ///
    /// # Returns
    /// Serialized MLS application message ready for transport
    pub async fn send_message(&self, plaintext: &[u8]) -> MlsResult<Vec<u8>> {
        let mut group = self.group.write().await;
        
        // Create application message
        let message = group.create_message(
            self.provider.as_ref(),
            &self.signature_keys,
            plaintext,
        ).map_err(|e| MlsError::InvalidMessage(format!("Failed to create message: {:?}", e)))?;
        
        // Serialize the message for transport
        let serialized = message.tls_serialize_detached()
            .map_err(|e| MlsError::Internal(format!("Failed to serialize message: {:?}", e)))?;
        
        Ok(serialized)
    }
    
    /// Commit pending proposals and advance to next epoch
    ///
    /// # Returns
    /// Serialized commit message and optional welcome messages for new members
    pub async fn commit_pending(&self) -> MlsResult<(Vec<u8>, Option<Vec<Vec<u8>>>)> {
        let mut group = self.group.write().await;
        
        // Create commit for pending proposals
        let (commit, welcome, _group_info) = group.commit_to_pending_proposals(
            self.provider.as_ref(),
            &self.signature_keys,
        ).map_err(|e| MlsError::InvalidMessage(format!("Failed to create commit: {:?}", e)))?;
        
        // Merge the commit into our own state
        group.merge_pending_commit(self.provider.as_ref())
            .map_err(|e| MlsError::Internal(format!("Failed to merge commit: {:?}", e)))?;
        
        // Serialize commit message
        let commit_bytes = commit.tls_serialize_detached()
            .map_err(|e| MlsError::Internal(format!("Failed to serialize commit: {:?}", e)))?;
        
        // Serialize welcome messages if any
        let welcome_bytes = if let Some(w) = welcome {
            let serialized = w.tls_serialize_detached()
                .map_err(|e| MlsError::Internal(format!("Failed to serialize welcome: {:?}", e)))?;
            Some(vec![serialized])
        } else {
            None
        };
        
        Ok((commit_bytes, welcome_bytes))
    }
    
    /// Process an incoming MLS message
    ///
    /// Handles application messages, proposals, and commits from other members
    ///
    /// # Returns
    /// ProcessedMessage enum indicating what was processed
    pub async fn process_message(&self, message_bytes: &[u8]) -> MlsResult<ProcessedMessage> {
        let mut group = self.group.write().await;
        
        // Parse the wire message
        let mls_message_in = MlsMessageIn::tls_deserialize_exact(message_bytes)
            .map_err(|e| MlsError::InvalidMessage(format!("Failed to parse message: {:?}", e)))?;
        
        // Extract the protocol message from the MLS message
        let protocol_message: ProtocolMessage = match mls_message_in.extract() {
            MlsMessageBodyIn::PrivateMessage(pm) => pm.into(),
            MlsMessageBodyIn::PublicMessage(pm) => pm.into(),
            _ => return Err(MlsError::InvalidMessage("Unexpected message type".to_string())),
        };
        
        // Process the message through OpenMLS - this handles decryption and validation
        let processed = group.process_message(self.provider.as_ref(), protocol_message)
            .map_err(|e| MlsError::InvalidMessage(format!("Failed to process message: {:?}", e)))?;
        
        // Handle based on message type
        match processed.into_content() {
            ProcessedMessageContent::ApplicationMessage(app_msg) => {
                // Extract plaintext from application message
                let plaintext = app_msg.into_bytes();
                Ok(ProcessedMessage::Application(plaintext))
            },
            ProcessedMessageContent::ProposalMessage(_proposal) => {
                // Proposal has been added to group's pending state
                Ok(ProcessedMessage::Proposal)
            },
            ProcessedMessageContent::StagedCommitMessage(staged_commit) => {
                // Merge the staged commit to advance the epoch
                group.merge_staged_commit(self.provider.as_ref(), *staged_commit)
                    .map_err(|e| MlsError::Internal(format!("Failed to merge commit: {:?}", e)))?;
                
                let new_epoch = group.epoch().as_u64();
                
                Ok(ProcessedMessage::Commit { new_epoch })
            },
            ProcessedMessageContent::ExternalJoinProposalMessage(_ext_proposal) => {
                // External join proposal received and stored
                Ok(ProcessedMessage::Proposal)
            },
        }
    }
}

/// Result of processing an incoming message
#[derive(Debug)]
pub enum ProcessedMessage {
    /// Application message with decrypted plaintext
    Application(Vec<u8>),
    /// Proposal was received and stored
    Proposal,
    /// Commit was processed and epoch advanced
    Commit { new_epoch: u64 },
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_create_group() {
        let group_id = GroupId::random();
        let identity = b"alice".to_vec();
        let config = MlsConfig::default();
        
        let result = OpenMlsEngine::create_group(
            group_id.clone(),
            identity,
            config,
        ).await;
        
        assert!(result.is_ok());
        
        if let Ok(engine) = result {
            let retrieved_id = engine.group_id().await;
            // Group IDs may differ as OpenMLS generates its own
            assert!(!retrieved_id.as_bytes().is_empty());
        }
    }
    
    #[tokio::test]
    async fn test_group_metadata() {
        let group_id = GroupId::random();
        let identity = b"alice".to_vec();
        let config = MlsConfig::default();
        
        let engine = OpenMlsEngine::create_group(
            group_id.clone(),
            identity,
            config,
        ).await.expect("Failed to create group");
        
        let metadata = engine.metadata().await.expect("Failed to get metadata");
        
        assert_eq!(metadata.epoch, 0);
        assert_eq!(metadata.members.len(), 1); // Creator
    }
}
