//! Message Lifecycle Management
//!
//! This module handles the wrapping and unwrapping of MLS messages with additional
//! metadata for routing and processing in the SpacePanda ecosystem.

pub mod inbound;
pub mod outbound;

use crate::core_mls::{
    errors::MlsResult,
    types::GroupId,
};
use serde::{Deserialize, Serialize};

/// Wire format envelope for MLS messages
///
/// Wraps raw MLS protocol messages with metadata needed for routing
/// and processing in the distributed system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedEnvelope {
    /// The group this message belongs to
    pub group_id: GroupId,
    
    /// The epoch when this message was created
    pub epoch: u64,
    
    /// Identity of the sender (for authentication/routing)
    pub sender: Vec<u8>,
    
    /// The actual MLS protocol message (serialized)
    pub payload: Vec<u8>,
    
    /// Message type hint for routing optimization
    pub message_type: MessageType,
}

/// Type of MLS message for routing optimization
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum MessageType {
    /// Application message (encrypted user data)
    Application,
    /// Proposal (add/remove/update member)
    Proposal,
    /// Commit (finalizes proposals, advances epoch)
    Commit,
    /// Welcome message (for new members)
    Welcome,
}

impl EncryptedEnvelope {
    /// Create a new envelope wrapping an MLS message
    pub fn new(
        group_id: GroupId,
        epoch: u64,
        sender: Vec<u8>,
        payload: Vec<u8>,
        message_type: MessageType,
    ) -> Self {
        Self {
            group_id,
            epoch,
            sender,
            payload,
            message_type,
        }
    }
    
    /// Serialize the envelope for transport
    pub fn to_bytes(&self) -> MlsResult<Vec<u8>> {
        bincode::serialize(self)
            .map_err(|e| crate::core_mls::errors::MlsError::SerializationError(
                format!("Failed to serialize envelope: {}", e)
            ))
    }
    
    /// Deserialize an envelope from bytes
    pub fn from_bytes(bytes: &[u8]) -> MlsResult<Self> {
        bincode::deserialize(bytes)
            .map_err(|e| crate::core_mls::errors::MlsError::SerializationError(
                format!("Failed to deserialize envelope: {}", e)
            ))
    }
    
    /// Extract the MLS payload
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }
    
    /// Get the group ID
    pub fn group_id(&self) -> &GroupId {
        &self.group_id
    }
    
    /// Get the epoch
    pub fn epoch(&self) -> u64 {
        self.epoch
    }
    
    /// Get the sender identity
    pub fn sender(&self) -> &[u8] {
        &self.sender
    }
    
    /// Get the message type
    pub fn message_type(&self) -> MessageType {
        self.message_type
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_envelope_serialization() {
        let group_id = GroupId::random();
        let envelope = EncryptedEnvelope::new(
            group_id.clone(),
            42,
            b"alice@example.com".to_vec(),
            vec![1, 2, 3, 4, 5],
            MessageType::Application,
        );
        
        let bytes = envelope.to_bytes().expect("Serialization should succeed");
        let deserialized = EncryptedEnvelope::from_bytes(&bytes)
            .expect("Deserialization should succeed");
        
        assert_eq!(deserialized.group_id, group_id);
        assert_eq!(deserialized.epoch, 42);
        assert_eq!(deserialized.sender, b"alice@example.com");
        assert_eq!(deserialized.payload, vec![1, 2, 3, 4, 5]);
        assert_eq!(deserialized.message_type, MessageType::Application);
    }
    
    #[test]
    fn test_envelope_accessors() {
        let group_id = GroupId::random();
        let envelope = EncryptedEnvelope::new(
            group_id.clone(),
            10,
            b"bob@example.com".to_vec(),
            vec![0xDE, 0xAD, 0xBE, 0xEF],
            MessageType::Commit,
        );
        
        assert_eq!(envelope.group_id(), &group_id);
        assert_eq!(envelope.epoch(), 10);
        assert_eq!(envelope.sender(), b"bob@example.com");
        assert_eq!(envelope.payload(), &[0xDE, 0xAD, 0xBE, 0xEF]);
        assert_eq!(envelope.message_type(), MessageType::Commit);
    }
}
