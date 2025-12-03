//! Group Operations
//!
//! Implements high-level group operations (add/remove members, send messages)
//! using OpenMLS primitives.

use crate::core_mls::{
    errors::{MlsError, MlsResult},
    events::MlsEvent,
};

use super::OpenMlsEngine;
use openmls::prelude::*;
use tls_codec::{Deserialize as TlsDeserialize, Serialize as TlsSerialize};

/// Group operations trait
///
/// Provides high-level MLS operations while abstracting OpenMLS details.
pub trait GroupOperations {
    /// Add members to the group
    ///
    /// Returns: (commit_message, optional_welcome_message)
    async fn add_members(
        &self,
        key_packages: Vec<Vec<u8>>,
    ) -> MlsResult<(Vec<u8>, Option<Vec<u8>>)>;

    /// Remove members from the group
    async fn remove_members(&self, leaf_indices: Vec<u32>) -> MlsResult<Vec<u8>>;

    /// Send an encrypted application message
    async fn send_message(&self, plaintext: &[u8]) -> MlsResult<Vec<u8>>;

    /// Process an incoming MLS message
    async fn process_message(&self, message: &[u8]) -> MlsResult<ProcessedMessage>;

    /// Commit pending proposals
    async fn commit_pending(&self) -> MlsResult<CommitResult>;
}

/// Result of processing a message
#[derive(Debug)]
pub enum ProcessedMessage {
    /// Application message (decrypted plaintext)
    Application(Vec<u8>),
    /// Proposal received (stored in group state)
    Proposal,
    /// Commit processed (epoch advanced)
    Commit { new_epoch: u64 },
}

/// Result of committing proposals
#[derive(Debug)]
pub struct CommitResult {
    /// Serialized commit message for existing members
    pub commit_message: Vec<u8>,
    /// Serialized welcome message for new members (if any)
    pub welcome_message: Option<Vec<u8>>,
    /// New epoch number
    pub new_epoch: u64,
}

impl GroupOperations for OpenMlsEngine {
    /// Add members to the group
    ///
    /// # Arguments
    /// * `key_packages` - Serialized KeyPackages for new members
    ///
    /// # Returns
    /// Tuple of (serialized commit message, optional serialized Welcome message)
    async fn add_members(
        &self,
        key_packages: Vec<Vec<u8>>,
    ) -> MlsResult<(Vec<u8>, Option<Vec<u8>>)> {
        let mut group = self.group.write().await;

        // Parse key packages using TlsDeserialize trait
        let parsed_packages: Vec<KeyPackage> = key_packages
            .iter()
            .map(|bytes| {
                // First deserialize to KeyPackageIn, then convert to KeyPackage
                let kp_in = KeyPackageIn::tls_deserialize(&mut bytes.as_slice()).map_err(|e| {
                    MlsError::InvalidMessage(format!("Invalid key package: {:?}", e))
                })?;
                kp_in
                    .validate(self.provider().crypto(), ProtocolVersion::default())
                    .map_err(|e| {
                        MlsError::InvalidMessage(format!("Key package validation failed: {:?}", e))
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;

        // Add members (creates proposals and commits them)
        let (commit_msg, welcome_msg, _group_info) = group
            .add_members(self.provider(), self.signature_keys(), &parsed_packages)
            .map_err(|e| MlsError::InvalidMessage(format!("Failed to add members: {:?}", e)))?;

        // Get group info for events
        let group_id = group.group_id().as_slice().to_vec();
        let new_epoch = group.epoch().as_u64();

        // Merge pending commit (CRITICAL!)
        group
            .merge_pending_commit(self.provider())
            .map_err(|e| MlsError::InvalidMessage(format!("Failed to merge commit: {:?}", e)))?;

        // Record join times for new members
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Find leaf indices of newly added members
        let mut new_member_indices = Vec::new();
        for member in group.members() {
            let member_identity = member.credential.serialized_content();
            for kp in &parsed_packages {
                let kp_identity = kp.leaf_node().credential().serialized_content();
                if member_identity == kp_identity {
                    new_member_indices.push(member.index.u32());
                    break;
                }
            }
        }

        // Drop the write lock before accessing member_join_times
        drop(group);

        // Record join times
        for leaf_index in &new_member_indices {
            self.record_join_time(*leaf_index, now).await;
        }

        // Emit MemberAdded events for each new member
        for key_package in &parsed_packages {
            // Extract member identity from credential
            let member_id = key_package.leaf_node().credential().serialized_content().to_vec();
            self.events().emit(MlsEvent::MemberAdded {
                group_id: group_id.clone(),
                member_id,
                epoch: new_epoch,
            });
        }

        // Serialize commit for transport
        let commit_bytes = commit_msg.tls_serialize_detached().map_err(|e| {
            MlsError::SerializationError(format!("Failed to serialize commit: {:?}", e))
        })?;

        // Serialize Welcome message - OpenMLS always returns one when adding members
        let welcome_bytes = welcome_msg.tls_serialize_detached().map_err(|e| {
            MlsError::SerializationError(format!("Failed to serialize Welcome: {:?}", e))
        })?;

        Ok((commit_bytes, Some(welcome_bytes)))
    }

    /// Remove members from the group
    ///
    /// # Arguments
    /// * `leaf_indices` - Leaf indices of members to remove
    ///
    /// # Returns
    /// Serialized commit message to broadcast to remaining members
    async fn remove_members(&self, leaf_indices: Vec<u32>) -> MlsResult<Vec<u8>> {
        let mut group = self.group.write().await;

        // Convert to LeafNodeIndex
        let indices: Vec<LeafNodeIndex> =
            leaf_indices.iter().map(|&idx| LeafNodeIndex::new(idx)).collect();

        let group_id = group.group_id().as_slice().to_vec();
        let new_epoch = group.epoch().as_u64();

        // Remove members
        let (commit_msg, _welcome_opt, _group_info) = group
            .remove_members(self.provider(), self.signature_keys(), &indices)
            .map_err(|e| MlsError::InvalidMessage(format!("Failed to remove members: {:?}", e)))?;

        // Merge pending commit
        group
            .merge_pending_commit(self.provider())
            .map_err(|e| MlsError::InvalidMessage(format!("Failed to merge commit: {:?}", e)))?;

        // Drop the write lock before emitting events
        drop(group);

        // Clean up join times for removed members
        for &idx in &leaf_indices {
            self.remove_join_time(idx).await;
        }

        // Emit MemberRemoved events for each removed member
        // Note: Using leaf index as member_id since credentials aren't easily accessible
        for &idx in &leaf_indices {
            self.events().emit(MlsEvent::MemberRemoved {
                group_id: group_id.clone(),
                member_id: idx.to_be_bytes().to_vec(), // Convert index to bytes
                epoch: new_epoch,
            });
        }

        // Serialize commit for transport
        commit_msg.tls_serialize_detached().map_err(|e| {
            MlsError::SerializationError(format!("Failed to serialize commit: {:?}", e))
        })
    }

    /// Send an encrypted application message
    ///
    /// # Arguments
    /// * `plaintext` - Message plaintext
    ///
    /// # Returns
    /// Serialized encrypted message for broadcast
    async fn send_message(&self, plaintext: &[u8]) -> MlsResult<Vec<u8>> {
        let mut group = self.group.write().await;

        // Create encrypted application message
        let message = group
            .create_message(self.provider(), self.signature_keys(), plaintext)
            .map_err(|e| MlsError::CryptoError(format!("Failed to encrypt message: {:?}", e)))?;

        // Serialize for transport
        message.tls_serialize_detached().map_err(|e| {
            MlsError::SerializationError(format!("Failed to serialize message: {:?}", e))
        })
    }

    /// Process an incoming MLS message
    ///
    /// # Arguments
    /// * `message` - Serialized MLS message
    ///
    /// # Returns
    /// Processed message content
    async fn process_message(&self, message: &[u8]) -> MlsResult<ProcessedMessage> {
        let mut group = self.group.write().await;

        // Parse message as MlsMessageIn and extract ProtocolMessage
        let mls_message = MlsMessageIn::tls_deserialize_exact(message).map_err(|e| {
            MlsError::InvalidMessage(format!("Failed to parse MLS message: {:?}", e))
        })?;

        // Convert to ProtocolMessage for processing
        let protocol_message = mls_message.try_into_protocol_message().map_err(|_| {
            MlsError::InvalidMessage(
                "Expected protocol message, got welcome or key package bundle".to_string(),
            )
        })?;

        // Process the message
        let processed = group
            .process_message(self.provider(), protocol_message)
            .map_err(|e| MlsError::InvalidMessage(format!("Failed to process message: {:?}", e)))?;

        // Handle based on content type
        let result = match processed.into_content() {
            ProcessedMessageContent::ApplicationMessage(app_msg) => {
                let plaintext = app_msg.into_bytes();
                ProcessedMessage::Application(plaintext)
            }
            ProcessedMessageContent::ProposalMessage(_proposal) => {
                // Proposal stored automatically in group's proposal store
                ProcessedMessage::Proposal
            }
            ProcessedMessageContent::StagedCommitMessage(staged_commit) => {
                // Merge the staged commit
                group.merge_staged_commit(self.provider(), *staged_commit).map_err(|e| {
                    MlsError::InvalidMessage(format!("Failed to merge staged commit: {:?}", e))
                })?;

                let new_epoch = group.epoch().as_u64();
                ProcessedMessage::Commit { new_epoch }
            }
            ProcessedMessageContent::ExternalJoinProposalMessage(_) => {
                // External join proposal stored
                ProcessedMessage::Proposal
            }
        };

        Ok(result)
    }

    /// Commit pending proposals
    ///
    /// # Returns
    /// Commit result with messages to send
    async fn commit_pending(&self) -> MlsResult<CommitResult> {
        let mut group = self.group.write().await;

        // Commit pending proposals
        let (commit_msg, welcome_opt, _group_info) = group
            .commit_to_pending_proposals(self.provider(), self.signature_keys())
            .map_err(|e| {
                MlsError::InvalidMessage(format!("Failed to commit proposals: {:?}", e))
            })?;

        // Merge the commit
        group
            .merge_pending_commit(self.provider())
            .map_err(|e| MlsError::InvalidMessage(format!("Failed to merge commit: {:?}", e)))?;

        let new_epoch = group.epoch().as_u64();

        // Serialize messages
        let commit_message = commit_msg
            .tls_serialize_detached()
            .map_err(|e| MlsError::Serialization(format!("Failed to serialize commit: {}", e)))?;

        let welcome_message = if let Some(welcome) = welcome_opt {
            Some(welcome.tls_serialize_detached().map_err(|e| {
                MlsError::Serialization(format!("Failed to serialize welcome: {}", e))
            })?)
        } else {
            None
        };

        Ok(CommitResult { commit_message, welcome_message, new_epoch })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_processed_message_variants() {
        let app_msg = ProcessedMessage::Application(vec![1, 2, 3]);
        assert!(matches!(app_msg, ProcessedMessage::Application(_)));

        let proposal_msg = ProcessedMessage::Proposal;
        assert!(matches!(proposal_msg, ProcessedMessage::Proposal));

        let commit_msg = ProcessedMessage::Commit { new_epoch: 1 };
        assert!(matches!(commit_msg, ProcessedMessage::Commit { .. }));
    }

    #[test]
    fn test_commit_result() {
        let result = CommitResult {
            commit_message: vec![1, 2, 3],
            welcome_message: Some(vec![4, 5, 6]),
            new_epoch: 2,
        };

        assert_eq!(result.new_epoch, 2);
        assert!(result.welcome_message.is_some());
    }
}
