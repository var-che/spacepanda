//! Transport Bridge Trait
//!
//! Defines the interface for sending/receiving MLS messages via DHT/Router.

use async_trait::async_trait;
use crate::core_mls::errors::MlsResult;

/// Group identifier
pub type GroupId = Vec<u8>;

/// Wire message wrapper
///
/// Encapsulates serialized MLS messages for transport over DHT.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct WireMessage {
    pub group_id: GroupId,
    pub epoch: u64,
    pub payload: Vec<u8>, // Raw MLS wire bytes
    pub msg_type: MessageType,
}

/// MLS message type enumeration
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub enum MessageType {
    Commit,
    Welcome,
    Application,
    Proposal,
}

/// DHT/Router bridge trait
///
/// Provides transport layer integration for MLS messages.
/// In production, this wraps the DHT subsystem.
/// In tests, this can use in-memory channels.
#[async_trait]
pub trait DhtBridge: Send + Sync {
    /// Publish MLS wire message to DHT under group namespace
    ///
    /// # Arguments
    /// * `group_id` - The group to publish to
    /// * `wire` - The serialized message
    ///
    /// # Implementation Notes
    /// Should attach routing metadata (signed envelope, TTL, etc.)
    async fn publish(&self, group_id: &GroupId, wire: WireMessage) -> MlsResult<()>;

    /// Subscribe to inbound MLS messages for a group
    ///
    /// Returns a channel receiver for incoming messages.
    /// The receiver should be polled to get new messages.
    ///
    /// # Arguments
    /// * `group_id` - The group to subscribe to
    ///
    /// # Returns
    /// A receiver that yields `WireMessage` as they arrive
    async fn subscribe(&self, group_id: &GroupId) -> MlsResult<tokio::sync::mpsc::Receiver<WireMessage>>;

    /// Unsubscribe from group messages
    ///
    /// # Arguments
    /// * `group_id` - The group to unsubscribe from
    async fn unsubscribe(&self, group_id: &GroupId) -> MlsResult<()>;

    /// Send message directly to a specific peer
    ///
    /// Optional: For direct delivery (e.g., Welcome messages)
    ///
    /// # Arguments
    /// * `peer_id` - The peer to send to
    /// * `wire` - The message to send
    async fn send_direct(&self, peer_id: &[u8], wire: WireMessage) -> MlsResult<()> {
        // Default implementation uses publish (can be overridden for optimization)
        let group_id = wire.group_id.clone();
        self.publish(&group_id, wire).await
    }
}
