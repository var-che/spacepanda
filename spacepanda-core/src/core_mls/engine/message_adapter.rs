//! Message Adapter
//!
//! Converts between OpenMLS message formats and our wire format for transport.

use crate::core_mls::{
    errors::{MlsError, MlsResult},
    traits::transport::{MessageType, WireMessage},
    types::GroupId,
};

use openmls::prelude::*;
use tls_codec::Deserialize as TlsDeserialize;

/// Wire format version
const WIRE_FORMAT_VERSION: u8 = 1;

/// Message adapter for OpenMLS messages
pub struct MessageAdapter;

/// Wire format wrapper
pub struct WireFormat;

impl MessageAdapter {
    /// Convert OpenMLS message to wire format
    ///
    /// # Arguments
    /// * `message_bytes` - Serialized OpenMLS message
    /// * `group_id` - Group identifier
    /// * `epoch` - Current epoch
    /// * `msg_type` - Message type
    pub fn to_wire(
        message_bytes: Vec<u8>,
        group_id: &GroupId,
        epoch: u64,
        msg_type: MessageType,
    ) -> MlsResult<WireMessage> {
        Ok(WireMessage {
            group_id: group_id.as_bytes().to_vec(),
            epoch,
            payload: message_bytes,
            msg_type,
        })
    }

    /// Convert wire format to OpenMLS message
    ///
    /// # Arguments
    /// * `wire` - Wire message
    pub fn from_wire(wire: &WireMessage) -> MlsResult<MlsMessageIn> {
        MlsMessageIn::tls_deserialize_exact(&wire.payload).map_err(|e| {
            MlsError::InvalidMessage(format!("Failed to deserialize message: {:?}", e))
        })
    }

    /// Extract group ID from wire message
    pub fn extract_group_id(wire: &WireMessage) -> GroupId {
        GroupId::new(wire.group_id.clone())
    }

    /// Extract epoch from wire message
    pub fn extract_epoch(wire: &WireMessage) -> u64 {
        wire.epoch
    }
}

impl WireFormat {
    /// Get the current wire format version
    pub fn version() -> u8 {
        WIRE_FORMAT_VERSION
    }

    /// Check if a wire message version is compatible
    pub fn is_compatible(version: u8) -> bool {
        version == WIRE_FORMAT_VERSION
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wire_format_version() {
        assert_eq!(WireFormat::version(), 1);
        assert!(WireFormat::is_compatible(1));
        assert!(!WireFormat::is_compatible(2));
    }

    #[test]
    fn test_group_id_conversion() {
        let test_bytes = vec![1u8; 32];
        let group_id = GroupId::new(test_bytes.clone());

        assert_eq!(group_id.as_bytes(), test_bytes.as_slice());
    }

    #[test]
    fn test_wire_message_creation() {
        let group_id = GroupId::new(vec![1u8; 32]);
        let message_bytes = vec![1, 2, 3, 4];

        let wire =
            MessageAdapter::to_wire(message_bytes.clone(), &group_id, 42, MessageType::Application)
                .unwrap();

        assert_eq!(wire.epoch, 42);
        assert_eq!(wire.payload, message_bytes);
        assert_eq!(wire.msg_type, MessageType::Application);
    }
}
