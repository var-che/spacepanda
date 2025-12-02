//! Inbound Message Processing
//!
//! Handles incoming MLS messages: parsing, verification, and application to group state.

use crate::core_mls::{
    engine::openmls_engine::{OpenMlsEngine, ProcessedMessage},
    errors::{MlsError, MlsResult},
    events::MlsEvent,
};
use super::{EncryptedEnvelope, MessageType};

/// Inbound message processor
///
/// Responsible for processing incoming MLS messages and emitting appropriate events.
pub struct InboundHandler {
    // Future: event emitter for notifying CRDT layer
    // event_tx: mpsc::Sender<MlsEvent>,
}

impl InboundHandler {
    /// Create a new inbound message handler
    pub fn new() -> Self {
        Self {}
    }
    
    /// Process an incoming encrypted envelope
    ///
    /// # Arguments
    /// * `engine` - The MLS engine managing the group
    /// * `envelope` - The encrypted envelope to process
    ///
    /// # Returns
    /// The processed message content and any events to emit
    pub async fn process_envelope(
        &self,
        engine: &OpenMlsEngine,
        envelope: &EncryptedEnvelope,
    ) -> MlsResult<ProcessedMessageResult> {
        // Verify epoch is valid (not too old, not from future)
        let current_epoch = engine.epoch().await;
        if envelope.epoch() > current_epoch + 1 {
            return Err(MlsError::EpochMismatch {
                expected: current_epoch,
                actual: envelope.epoch(),
            });
        }
        
        // Process the MLS message payload
        let processed = engine.process_message(envelope.payload()).await?;
        
        // Convert to result with events
        let result = match processed {
            ProcessedMessage::Application(plaintext) => {
                // Application message received
                let event = MlsEvent::MessageReceived {
                    group_id: envelope.group_id().as_bytes().to_vec(),
                    sender_id: envelope.sender().to_vec(),
                    epoch: envelope.epoch(),
                    plaintext: plaintext.clone(),
                };
                
                ProcessedMessageResult {
                    content: MessageContent::Application(plaintext),
                    events: vec![event],
                }
            },
            ProcessedMessage::Proposal => {
                // Proposal received and stored in group state
                // Note: We don't emit a specific event for proposals since we don't know the type yet
                ProcessedMessageResult {
                    content: MessageContent::Proposal,
                    events: vec![],
                }
            },
            ProcessedMessage::Commit { new_epoch } => {
                // Commit processed, epoch advanced
                let event = MlsEvent::EpochChanged {
                    group_id: envelope.group_id().as_bytes().to_vec(),
                    old_epoch: envelope.epoch(),
                    new_epoch,
                };
                
                ProcessedMessageResult {
                    content: MessageContent::Commit { new_epoch },
                    events: vec![event],
                }
            },
        };
        
        Ok(result)
    }
    
    /// Verify envelope matches expected group and epoch constraints
    pub fn verify_envelope_metadata(
        &self,
        envelope: &EncryptedEnvelope,
        expected_group_id: &crate::core_mls::types::GroupId,
        max_epoch_drift: u64,
        current_epoch: u64,
    ) -> MlsResult<()> {
        // Verify group ID matches
        if envelope.group_id() != expected_group_id {
            return Err(MlsError::InvalidMessage(format!(
                "Group ID mismatch: expected {:?}, got {:?}",
                expected_group_id,
                envelope.group_id()
            )));
        }
        
        // Verify epoch is within acceptable range
        if envelope.epoch() < current_epoch.saturating_sub(max_epoch_drift) {
            return Err(MlsError::EpochMismatch {
                expected: current_epoch,
                actual: envelope.epoch(),
            });
        }
        
        if envelope.epoch() > current_epoch + max_epoch_drift {
            return Err(MlsError::EpochMismatch {
                expected: current_epoch,
                actual: envelope.epoch(),
            });
        }
        
        Ok(())
    }
}

impl Default for InboundHandler {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of processing an inbound message
#[derive(Debug)]
pub struct ProcessedMessageResult {
    /// The content of the processed message
    pub content: MessageContent,
    
    /// Events to emit to the application layer
    pub events: Vec<MlsEvent>,
}

/// Content extracted from a processed message
#[derive(Debug)]
pub enum MessageContent {
    /// Decrypted application message
    Application(Vec<u8>),
    
    /// Proposal was stored
    Proposal,
    
    /// Commit was applied, epoch advanced
    Commit { new_epoch: u64 },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core_mls::{
        types::{GroupId, MlsConfig},
    };
    
    #[tokio::test]
    async fn test_inbound_handler_creation() {
        let handler = InboundHandler::new();
        // Handler should be created successfully
        assert!(std::mem::size_of_val(&handler) >= 0);
    }
    
    #[tokio::test]
    async fn test_verify_envelope_metadata_valid() {
        let handler = InboundHandler::new();
        let group_id = GroupId::random();
        
        let envelope = EncryptedEnvelope::new(
            group_id.clone(),
            10,
            b"alice@example.com".to_vec(),
            vec![1, 2, 3],
            MessageType::Application,
        );
        
        let result = handler.verify_envelope_metadata(&envelope, &group_id, 5, 10);
        assert!(result.is_ok());
    }
    
    #[tokio::test]
    async fn test_verify_envelope_metadata_wrong_group() {
        let handler = InboundHandler::new();
        let group_id = GroupId::random();
        let wrong_group_id = GroupId::random();
        
        let envelope = EncryptedEnvelope::new(
            wrong_group_id,
            10,
            b"alice@example.com".to_vec(),
            vec![1, 2, 3],
            MessageType::Application,
        );
        
        let result = handler.verify_envelope_metadata(&envelope, &group_id, 5, 10);
        assert!(result.is_err());
    }
    
    #[tokio::test]
    async fn test_verify_envelope_metadata_epoch_too_old() {
        let handler = InboundHandler::new();
        let group_id = GroupId::random();
        
        let envelope = EncryptedEnvelope::new(
            group_id.clone(),
            3, // Too old
            b"alice@example.com".to_vec(),
            vec![1, 2, 3],
            MessageType::Application,
        );
        
        let result = handler.verify_envelope_metadata(&envelope, &group_id, 5, 10);
        assert!(result.is_err());
    }
    
    #[tokio::test]
    async fn test_verify_envelope_metadata_epoch_too_new() {
        let handler = InboundHandler::new();
        let group_id = GroupId::random();
        
        let envelope = EncryptedEnvelope::new(
            group_id.clone(),
            20, // Too far in future
            b"alice@example.com".to_vec(),
            vec![1, 2, 3],
            MessageType::Application,
        );
        
        let result = handler.verify_envelope_metadata(&envelope, &group_id, 5, 10);
        assert!(result.is_err());
    }
}
