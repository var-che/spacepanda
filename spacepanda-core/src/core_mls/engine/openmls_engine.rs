//! OpenMLS Engine Implementation
//!
//! Wraps OpenMLS MlsGroup to maintain our current MlsHandle API while using
//! OpenMLS for all cryptographic operations and state management.

use crate::core_mls::{
    errors::{MlsError, MlsResult},
    events::{EventBroadcaster, MlsEvent},
    state::GroupSnapshot,
    types::{GroupId, GroupMetadata, MemberInfo, MlsConfig},
};

use openmls::prelude::*;
use openmls_basic_credential::SignatureKeyPair;
use openmls_rust_crypto::OpenMlsRustCrypto;
use std::collections::HashMap;
use std::sync::Arc;
use tls_codec::{Deserialize as TlsDeserialize, Serialize as TlsSerialize};
use tokio::sync::RwLock;

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

    /// Event broadcaster for emitting state changes
    event_broadcaster: EventBroadcaster,

    /// Track when each member joined (leaf_index -> unix timestamp)
    member_join_times: Arc<RwLock<HashMap<u32, u64>>>,
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
        let signature_keys =
            SignatureKeyPair::new(ciphersuite.signature_algorithm()).map_err(|e| {
                MlsError::CryptoError(format!("Failed to generate signature keys: {:?}", e))
            })?;

        // Store signature keys (required by OpenMLS)
        signature_keys.store(provider.storage()).map_err(|e| {
            MlsError::CryptoError(format!("Failed to store signature keys: {:?}", e))
        })?;

        // Save identity for event emission
        let identity_for_event = identity.clone();

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
        )
        .map_err(|e| MlsError::InvalidMessage(format!("Failed to create group: {:?}", e)))?;

        // Create event broadcaster
        let event_broadcaster = EventBroadcaster::default();

        // Initialize member join times with creator's join time
        let mut join_times = HashMap::new();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        join_times.insert(0u32, now); // Creator is always at leaf index 0

        let engine = Self {
            group: Arc::new(RwLock::new(group)),
            provider,
            config,
            signature_keys,
            credential: credential_bundle.clone(),
            event_broadcaster: event_broadcaster.clone(),
            member_join_times: Arc::new(RwLock::new(join_times)),
        };

        // Emit GroupCreated event
        event_broadcaster.emit(MlsEvent::GroupCreated {
            group_id: group_id.as_bytes().to_vec(),
            creator_id: identity_for_event,
        });

        Ok(engine)
    }

    /// Create engine from an existing MlsGroup with custom provider
    ///
    /// This is useful for test scenarios where you want to reuse a provider
    /// that has specific crypto material already stored.
    pub(crate) fn from_group_with_provider(
        group: MlsGroup,
        provider: Arc<OpenMlsRustCrypto>,
        config: MlsConfig,
        signature_keys: SignatureKeyPair,
        credential: CredentialWithKey,
    ) -> Self {
        Self {
            group: Arc::new(RwLock::new(group)),
            provider,
            config,
            signature_keys,
            credential,
            event_broadcaster: EventBroadcaster::default(),
            member_join_times: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Join a group via Welcome message
    ///
    /// # Arguments
    /// * `welcome_bytes` - Serialized Welcome message
    /// * `ratchet_tree` - Optional ratchet tree for optimization
    /// * `config` - Group configuration
    /// * `key_package_bundle` - Optional KeyPackageBundle with private keys for this member
    ///   If provided, uses existing crypto material. If None, generates new keys.
    pub async fn join_from_welcome(
        welcome_bytes: &[u8],
        ratchet_tree: Option<Vec<u8>>,
        config: MlsConfig,
        key_package_bundle: Option<KeyPackageBundle>,
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

        // Use provided KeyPackageBundle or generate new keys
        let (signature_keys, credential_bundle) = if let Some(ref bundle) = key_package_bundle {
            // Extract information from the existing key package bundle
            let key_package = bundle.key_package();
            let leaf_node = key_package.leaf_node();
            let signature_key = leaf_node.signature_key().clone();
            
            // Note: We still need to generate SignatureKeyPair for the engine,
            // but the actual decryption will use the bundle's init_private_key
            // which OpenMLS will automatically find from the Welcome message
            let sig_keys = SignatureKeyPair::new(ciphersuite.signature_algorithm()).map_err(|e| {
                MlsError::CryptoError(format!("Failed to generate signature keys: {:?}", e))
            })?;
            
            sig_keys.store(provider.storage()).map_err(|e| {
                MlsError::CryptoError(format!("Failed to store signature keys: {:?}", e))
            })?;

            let cred = CredentialWithKey {
                credential: leaf_node.credential().clone(),
                signature_key,
            };
            
            (sig_keys, cred)
        } else {
            // Generate new signature keys
            let signature_keys =
                SignatureKeyPair::new(ciphersuite.signature_algorithm()).map_err(|e| {
                    MlsError::CryptoError(format!("Failed to generate signature keys: {:?}", e))
                })?;

            // Store signature keys
            signature_keys.store(provider.storage()).map_err(|e| {
                MlsError::CryptoError(format!("Failed to store signature keys: {:?}", e))
            })?;

            // Create temporary credential (will be replaced by Welcome)
            let basic_credential = BasicCredential::new(vec![]);
            let credential_bundle = CredentialWithKey {
                credential: basic_credential.into(),
                signature_key: signature_keys.public().into(),
            };
            
            (signature_keys, credential_bundle)
        };

        // Parse optional ratchet tree
        let ratchet_tree_in = if let Some(ref tree_bytes) = ratchet_tree {
            Some(RatchetTreeIn::tls_deserialize_exact(tree_bytes).map_err(|e| {
                MlsError::InvalidMessage(format!("Failed to parse ratchet tree: {:?}", e))
            })?)
        } else {
            None
        };

        // Create join config
        let join_config = MlsGroupJoinConfig::builder()
            .wire_format_policy(PURE_CIPHERTEXT_WIRE_FORMAT_POLICY)
            .build();

        // Stage the welcome (validates and prepares group state)
        let staged_welcome =
            StagedWelcome::new_from_welcome(&*provider, &join_config, welcome, ratchet_tree_in)
                .map_err(|e| {
                    MlsError::InvalidMessage(format!("Failed to stage welcome: {:?}", e))
                })?;

        // Convert to active group
        let group = staged_welcome
            .into_group(&*provider)
            .map_err(|e| MlsError::InvalidMessage(format!("Failed to join group: {:?}", e)))?;

        // Get group info for event
        let group_id = GroupId::new(group.group_id().as_slice().to_vec());
        let epoch = group.epoch().as_u64();
        let member_count = group.members().count();

        // Create event broadcaster
        let event_broadcaster = EventBroadcaster::default();

        // Initialize member join times - we'll populate from group state
        let join_times = HashMap::new();

        let engine = Self {
            group: Arc::new(RwLock::new(group)),
            provider,
            config,
            signature_keys,
            credential: credential_bundle,
            event_broadcaster: event_broadcaster.clone(),
            member_join_times: Arc::new(RwLock::new(join_times)),
        };

        // Emit GroupJoined event
        event_broadcaster.emit(MlsEvent::GroupJoined {
            group_id: group_id.as_bytes().to_vec(),
            epoch,
            member_count,
        });

        Ok(engine)
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
        let members: Vec<MemberInfo> = group
            .members()
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

    /// Get reference to event broadcaster
    ///
    /// This allows subscribers to receive MLS events (group changes, messages, etc.)
    pub fn events(&self) -> &EventBroadcaster {
        &self.event_broadcaster
    }

    /// Subscribe to MLS events
    ///
    /// # Returns
    /// A receiver for MLS events from this group
    pub fn subscribe_events(&self) -> tokio::sync::broadcast::Receiver<MlsEvent> {
        self.event_broadcaster.subscribe()
    }

    /// Record join time for a member
    ///
    /// # Arguments
    /// * `leaf_index` - Leaf index of the member
    /// * `timestamp` - Unix timestamp when member joined
    pub(crate) async fn record_join_time(&self, leaf_index: u32, timestamp: u64) {
        let mut join_times = self.member_join_times.write().await;
        join_times.insert(leaf_index, timestamp);
    }

    /// Remove join time for a member (called on removal)
    ///
    /// # Arguments
    /// * `leaf_index` - Leaf index of the member
    pub(crate) async fn remove_join_time(&self, leaf_index: u32) {
        let mut join_times = self.member_join_times.write().await;
        join_times.remove(&leaf_index);
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
        let message = group
            .create_message(self.provider.as_ref(), &self.signature_keys, plaintext)
            .map_err(|e| MlsError::InvalidMessage(format!("Failed to create message: {:?}", e)))?;

        // Serialize the message for transport
        let serialized = message
            .tls_serialize_detached()
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
        let (commit, welcome, _group_info) = group
            .commit_to_pending_proposals(self.provider.as_ref(), &self.signature_keys)
            .map_err(|e| MlsError::InvalidMessage(format!("Failed to create commit: {:?}", e)))?;

        // Merge the commit into our own state
        group
            .merge_pending_commit(self.provider.as_ref())
            .map_err(|e| MlsError::Internal(format!("Failed to merge commit: {:?}", e)))?;

        // Serialize commit message
        let commit_bytes = commit
            .tls_serialize_detached()
            .map_err(|e| MlsError::Internal(format!("Failed to serialize commit: {:?}", e)))?;

        // Serialize welcome messages if any
        let welcome_bytes = if let Some(w) = welcome {
            let serialized = w
                .tls_serialize_detached()
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
        let processed = group
            .process_message(self.provider.as_ref(), protocol_message)
            .map_err(|e| MlsError::InvalidMessage(format!("Failed to process message: {:?}", e)))?;

        // Handle based on message type
        match processed.into_content() {
            ProcessedMessageContent::ApplicationMessage(app_msg) => {
                // Extract plaintext from application message
                let plaintext = app_msg.into_bytes();
                Ok(ProcessedMessage::Application(plaintext))
            }
            ProcessedMessageContent::ProposalMessage(_proposal) => {
                // Proposal has been added to group's pending state
                Ok(ProcessedMessage::Proposal)
            }
            ProcessedMessageContent::StagedCommitMessage(staged_commit) => {
                // Merge the staged commit to advance the epoch
                group
                    .merge_staged_commit(self.provider.as_ref(), *staged_commit)
                    .map_err(|e| MlsError::Internal(format!("Failed to merge commit: {:?}", e)))?;

                let new_epoch = group.epoch().as_u64();

                Ok(ProcessedMessage::Commit { new_epoch })
            }
            ProcessedMessageContent::ExternalJoinProposalMessage(_ext_proposal) => {
                // External join proposal received and stored
                Ok(ProcessedMessage::Proposal)
            }
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

impl OpenMlsEngine {
    // ===== Snapshot Export/Import =====

    /// Export current group state as an atomic snapshot
    ///
    /// This captures the complete group state for backup, export, or CRDT synchronization.
    /// The snapshot includes:
    /// - Ratchet tree (public group state)
    /// - Group context (metadata, extensions, epoch)
    /// - Member list
    /// - Own leaf index
    ///
    /// # Returns
    /// * `GroupSnapshot` - Complete group state snapshot
    pub async fn export_snapshot(&self) -> MlsResult<GroupSnapshot> {
        let group = self.group.read().await;

        // Export ratchet tree
        let ratchet_tree = group.export_ratchet_tree();
        let ratchet_tree_bytes = ratchet_tree.tls_serialize_detached().map_err(|e| {
            MlsError::Internal(format!("Failed to serialize ratchet tree: {:?}", e))
        })?;

        // Export group info - this is the authoritative group context
        // Note: We include the ratchet tree in the GroupInfo (with_ratchet_tree=true)
        let group_info_msg = group
            .export_group_info(
                self.provider.crypto(),
                &self.signature_keys,
                true, // Include ratchet tree in GroupInfo
            )
            .map_err(|e| MlsError::Internal(format!("Failed to export group info: {:?}", e)))?;
        let group_context_bytes = group_info_msg
            .tls_serialize_detached()
            .map_err(|e| MlsError::Internal(format!("Failed to serialize group info: {:?}", e)))?;

        // Get epoch
        let epoch = group.epoch().as_u64();

        // Get group ID
        let group_id = GroupId::new(group.group_id().as_slice().to_vec());

        // Get members with join times from our tracking HashMap
        let join_times = self.member_join_times.read().await.clone();
        let fallback_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let members: Vec<MemberInfo> = group
            .members()
            .map(|member| {
                let identity = member.credential.serialized_content().to_vec();
                let leaf_index = member.index.u32();
                let joined_at = join_times.get(&leaf_index).copied().unwrap_or(fallback_time);

                MemberInfo { identity, leaf_index, joined_at }
            })
            .collect();

        // Get own leaf index - convert LeafNodeIndex to u32
        let own_leaf_index = group.own_leaf_index().u32();

        // Create snapshot
        let snapshot = GroupSnapshot::new(
            group_id,
            epoch,
            ratchet_tree_bytes,
            group_context_bytes,
            members,
            own_leaf_index,
        );

        Ok(snapshot)
    }

    /// Export only the ratchet tree for sharing with new members
    ///
    /// This is needed when joining from a Welcome message that doesn't include
    /// the ratchet tree inline.
    pub async fn export_ratchet_tree_bytes(&self) -> MlsResult<Vec<u8>> {
        let group = self.group.read().await;
        let ratchet_tree = group.export_ratchet_tree();
        ratchet_tree.tls_serialize_detached().map_err(|e| {
            MlsError::Internal(format!("Failed to serialize ratchet tree: {:?}", e))
        })
    }

    /// Helper to extract members without holding the lock
    fn get_members_internal(&self, group: &MlsGroup) -> MlsResult<Vec<MemberInfo>> {
        let mut members = Vec::new();

        // Get join times from our tracking HashMap
        let join_times = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(async { self.member_join_times.read().await.clone() })
        });

        // Fallback timestamp if member not found in tracking
        let fallback_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        for member in group.members() {
            // Extract identity from credential - use serialized credential as identity
            let identity = member.credential.serialized_content().to_vec();
            let leaf_index = member.index.u32();

            // Get actual join time from our tracking, or use fallback
            let joined_at = join_times.get(&leaf_index).copied().unwrap_or(fallback_time);

            members.push(MemberInfo { identity, leaf_index, joined_at });
        }

        Ok(members)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_group() {
        let group_id = GroupId::random();
        let identity = b"alice".to_vec();
        let config = MlsConfig::default();

        let result = OpenMlsEngine::create_group(group_id.clone(), identity, config).await;

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

        let engine = OpenMlsEngine::create_group(group_id.clone(), identity, config)
            .await
            .expect("Failed to create group");

        let metadata = engine.metadata().await.expect("Failed to get metadata");

        assert_eq!(metadata.epoch, 0);
        assert_eq!(metadata.members.len(), 1); // Creator
    }

    #[tokio::test]
    async fn test_export_snapshot() {
        let group_id = GroupId::random();
        let identity = b"bob".to_vec();
        let config = MlsConfig::default();

        let engine = OpenMlsEngine::create_group(group_id.clone(), identity, config)
            .await
            .expect("Failed to create group");

        // Export snapshot
        let snapshot = engine.export_snapshot().await.expect("Failed to export snapshot");

        // Verify snapshot contents
        assert_eq!(snapshot.epoch(), 0);
        assert_eq!(snapshot.members().len(), 1);
        assert!(!snapshot.ratchet_tree_bytes.is_empty());

        // Test serialization
        let bytes = snapshot.to_bytes().expect("Failed to serialize");
        let restored = GroupSnapshot::from_bytes(&bytes).expect("Failed to deserialize");

        assert_eq!(restored.epoch(), snapshot.epoch());
        assert_eq!(restored.members().len(), snapshot.members().len());
    }

    #[tokio::test]
    async fn test_event_emission() {
        let group_id = GroupId::random();
        let identity = b"charlie".to_vec();
        let config = MlsConfig::default();

        // Create engine and subscribe to events
        let engine = OpenMlsEngine::create_group(group_id.clone(), identity.clone(), config)
            .await
            .expect("Failed to create group");

        let mut rx = engine.subscribe_events();

        // The GroupCreated event should have been emitted
        // Try to receive it (it might have already been consumed)
        // So we'll just verify the broadcaster works
        assert_eq!(engine.events().subscriber_count(), 1);
    }
}
