//! Message Serializer Trait
//!
//! Handles conversion between internal message types and wire format.

use async_trait::async_trait;
use crate::core_mls::errors::MlsResult;
use super::transport::WireMessage;

/// Outbound message (to be serialized)
///
/// Internal representation of a message before serialization.
#[derive(Clone, Debug)]
pub struct OutboundMessage {
    pub group_id: Vec<u8>,
    pub epoch: u64,
    pub content: Vec<u8>,
    pub msg_type: super::transport::MessageType,
}

/// Inbound message (after deserialization)
///
/// Internal representation of a received message.
#[derive(Clone, Debug)]
pub struct InboundMessage {
    pub group_id: Vec<u8>,
    pub epoch: u64,
    pub content: Vec<u8>,
    pub msg_type: super::transport::MessageType,
}

/// Message serialization trait
///
/// Handles conversion between internal message types and wire format.
/// This enables:
/// - Protocol version evolution
/// - Wire format changes without affecting internal logic
/// - Custom encoding schemes (compression, etc.)
#[async_trait]
pub trait MessageSerializer: Send + Sync {
    /// Serialize engine-specific message into WireMessage
    ///
    /// # Arguments
    /// * `msg` - The outbound message to serialize
    ///
    /// # Returns
    /// Wire-format message ready for transport
    async fn serialize(&self, msg: &OutboundMessage) -> MlsResult<WireMessage>;

    /// Deserialize raw WireMessage to inbound typed message
    ///
    /// # Arguments
    /// * `wire` - The received wire message
    ///
    /// # Returns
    /// Deserialized inbound message
    async fn deserialize(&self, wire: &WireMessage) -> MlsResult<InboundMessage>;

    /// Get protocol version for compatibility checking
    fn protocol_version(&self) -> u8 {
        1 // Default version
    }
}
