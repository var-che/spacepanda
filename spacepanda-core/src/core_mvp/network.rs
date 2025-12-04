//! Network Layer for ChannelManager
//!
//! Integrates the P2P router with MLS channels to enable actual multi-user messaging.
//!
//! # Architecture
//!
//! ```text
//! ┌──────────────────────────────────────┐
//! │       ChannelManager (MLS)           │
//! │    • send_message() → encrypt        │
//! │    • receive_message() → decrypt     │
//! └─────────────┬────────────────────────┘
//!               │
//!               ▼
//! ┌─────────────────────────────────────┐
//! │      NetworkLayer                   │
//! │  • broadcast_to_channel()           │
//! │  • route_message_to_peer()          │
//! │  • handle_incoming_message()        │
//! └─────────────┬───────────────────────┘
//!               │
//!               ▼
//! ┌─────────────────────────────────────┐
//! │      RouterHandle (P2P)             │
//! │  • send_direct()                    │
//! │  • listen(), dial()                 │
//! │  • peer discovery                   │
//! └─────────────────────────────────────┘
//! ```

use crate::core_mvp::errors::{MvpError, MvpResult};
use crate::core_router::{PeerId, RouterEvent, RouterHandle};
use crate::core_store::model::types::{ChannelId, UserId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};

/// Network message types for MLS channels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChannelNetworkMessage {
    /// Encrypted application message (MLS ciphertext)
    EncryptedMessage {
        channel_id: String,
        ciphertext: Vec<u8>,
        sender_id: String,
    },
    /// Commit message (group state change)
    Commit {
        channel_id: String,
        commit_data: Vec<u8>,
    },
    /// Proposal message
    Proposal {
        channel_id: String,
        proposal_data: Vec<u8>,
    },
    /// Peer wants to join a channel
    JoinRequest {
        channel_id: String,
        key_package: Vec<u8>,
    },
}

/// Maps channel members to their network peer IDs
type ChannelMemberMap = HashMap<ChannelId, HashMap<UserId, PeerId>>;

/// Network layer for P2P messaging
pub struct NetworkLayer {
    /// Router handle for P2P communication
    router: RouterHandle,
    
    /// Maps channel members to their peer IDs
    channel_members: Arc<RwLock<ChannelMemberMap>>,
    
    /// Channel for incoming messages
    incoming_tx: mpsc::Sender<IncomingMessage>,
    
    /// Our peer ID
    local_peer_id: PeerId,
}

/// Incoming message from the network
#[derive(Debug)]
pub struct IncomingMessage {
    pub channel_id: ChannelId,
    pub ciphertext: Vec<u8>,
    pub sender_peer_id: PeerId,
}

impl NetworkLayer {
    /// Create a new network layer
    ///
    /// # Arguments
    /// * `router` - Router handle for P2P communication
    /// * `local_peer_id` - Our peer ID on the network
    ///
    /// # Returns
    /// Network layer and receiver for incoming messages
    pub fn new(
        router: RouterHandle,
        local_peer_id: PeerId,
    ) -> (Self, mpsc::Receiver<IncomingMessage>) {
        let (incoming_tx, incoming_rx) = mpsc::channel(100);
        
        let network = Self {
            router,
            channel_members: Arc::new(RwLock::new(HashMap::new())),
            incoming_tx,
            local_peer_id,
        };
        
        (network, incoming_rx)
    }
    
    /// Start listening on an address
    pub async fn listen(&self, addr: &str) -> MvpResult<()> {
        self.router
            .listen(addr.to_string())
            .await
            .map_err(|e| MvpError::NetworkError(format!("Failed to listen: {}", e)))
    }
    
    /// Connect to a peer
    pub async fn dial(&self, addr: &str) -> MvpResult<()> {
        self.router
            .dial(addr.to_string())
            .await
            .map_err(|e| MvpError::NetworkError(format!("Failed to dial: {}", e)))
    }
    
    /// Register a channel member
    ///
    /// Maps a user ID to their network peer ID for message routing
    pub async fn register_channel_member(
        &self,
        channel_id: &ChannelId,
        user_id: UserId,
        peer_id: PeerId,
    ) {
        let user_id_str = user_id.0.clone();
        let peer_id_debug = format!("{:?}", peer_id);
        let mut members = self.channel_members.write().await;
        members
            .entry(channel_id.clone())
            .or_insert_with(HashMap::new)
            .insert(user_id, peer_id);
        
        info!(
            channel_id = %channel_id,
            user_id = %user_id_str,
            peer_id = %peer_id_debug,
            "Registered channel member"
        );
    }
    
    /// Broadcast encrypted message to all channel members
    ///
    /// # Arguments
    /// * `channel_id` - Target channel
    /// * `ciphertext` - Encrypted MLS message
    /// * `sender_id` - Sender's user ID
    pub async fn broadcast_message(
        &self,
        channel_id: &ChannelId,
        ciphertext: Vec<u8>,
        sender_id: &UserId,
    ) -> MvpResult<()> {
        let members = self.channel_members.read().await;
        
        let channel_members = members.get(channel_id).ok_or_else(|| {
            MvpError::ChannelNotFound(channel_id.0.clone())
        })?;
        
        let message = ChannelNetworkMessage::EncryptedMessage {
            channel_id: channel_id.0.clone(),
            ciphertext: ciphertext.clone(),
            sender_id: sender_id.0.clone(),
        };
        
        let message_bytes = serde_json::to_vec(&message)
            .map_err(|e| MvpError::SerializationError(format!("Failed to serialize: {}", e)))?;
        
        let mut sent_count = 0;
        let mut error_count = 0;
        
        // Send to all members except ourselves
        for (user_id, peer_id) in channel_members.iter() {
            if user_id == sender_id {
                continue; // Don't send to ourselves
            }
            
            match self.router.send_direct(peer_id.clone(), message_bytes.clone()).await {
                Ok(_) => {
                    sent_count += 1;
                    debug!(
                        channel_id = %channel_id,
                        peer_id = ?peer_id,
                        "Sent message to peer"
                    );
                }
                Err(e) => {
                    error_count += 1;
                    warn!(
                        channel_id = %channel_id,
                        peer_id = ?peer_id,
                        error = %e,
                        "Failed to send message to peer"
                    );
                }
            }
        }
        
        info!(
            channel_id = %channel_id,
            sent = sent_count,
            errors = error_count,
            total_members = channel_members.len(),
            "Broadcast complete"
        );
        
        if error_count > 0 && sent_count == 0 {
            return Err(MvpError::NetworkError(format!(
                "Failed to send to any channel members ({} errors)",
                error_count
            )));
        }
        
        Ok(())
    }
    
    /// Send commit message to channel members
    pub async fn broadcast_commit(
        &self,
        channel_id: &ChannelId,
        commit_data: Vec<u8>,
    ) -> MvpResult<()> {
        let members = self.channel_members.read().await;
        
        let channel_members = members.get(channel_id).ok_or_else(|| {
            MvpError::ChannelNotFound(channel_id.0.clone())
        })?;
        
        let message = ChannelNetworkMessage::Commit {
            channel_id: channel_id.0.clone(),
            commit_data,
        };
        
        let message_bytes = serde_json::to_vec(&message)
            .map_err(|e| MvpError::SerializationError(format!("Failed to serialize: {}", e)))?;
        
        // Send to all members
        for (_user_id, peer_id) in channel_members.iter() {
            if let Err(e) = self.router.send_direct(peer_id.clone(), message_bytes.clone()).await {
                warn!(peer_id = ?peer_id, error = %e, "Failed to send commit");
            }
        }
        
        Ok(())
    }
    
    /// Start processing incoming network events
    ///
    /// This spawns a background task that listens for router events
    /// and forwards channel messages to the incoming message channel
    pub fn spawn_event_processor(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            loop {
                match self.router.next_event().await {
                    Some(RouterEvent::DataReceived(peer_id, data)) => {
                        if let Err(e) = self.handle_incoming_data(peer_id, data).await {
                            error!(error = %e, "Failed to handle incoming data");
                        }
                    }
                    Some(RouterEvent::PeerConnected(peer_id)) => {
                        info!(peer_id = ?peer_id, "Peer connected");
                    }
                    Some(RouterEvent::PeerDisconnected(peer_id)) => {
                        info!(peer_id = ?peer_id, "Peer disconnected");
                    }
                    Some(RouterEvent::Listening(addr)) => {
                        info!(addr = %addr, "Router listening");
                    }
                    None => {
                        warn!("Router event channel closed");
                        break;
                    }
                }
            }
        })
    }
    
    /// Handle incoming data from a peer
    async fn handle_incoming_data(&self, peer_id: PeerId, data: Vec<u8>) -> MvpResult<()> {
        // Deserialize network message
        let message: ChannelNetworkMessage = serde_json::from_slice(&data)
            .map_err(|e| MvpError::InvalidMessage(format!("Failed to deserialize: {}", e)))?;
        
        match message {
            ChannelNetworkMessage::EncryptedMessage {
                channel_id,
                ciphertext,
                sender_id: _,
            } => {
                // Forward to channel manager for decryption
                let incoming = IncomingMessage {
                    channel_id: ChannelId(channel_id),
                    ciphertext,
                    sender_peer_id: peer_id,
                };
                
                if let Err(e) = self.incoming_tx.send(incoming).await {
                    error!(error = %e, "Failed to forward incoming message");
                }
            }
            ChannelNetworkMessage::Commit { channel_id, commit_data } => {
                debug!(
                    channel_id = %channel_id,
                    size = commit_data.len(),
                    "Received commit message"
                );
                // TODO: Forward to channel manager for processing
            }
            ChannelNetworkMessage::Proposal { channel_id, proposal_data } => {
                debug!(
                    channel_id = %channel_id,
                    size = proposal_data.len(),
                    "Received proposal message"
                );
                // TODO: Forward to channel manager for processing
            }
            ChannelNetworkMessage::JoinRequest { channel_id, key_package } => {
                debug!(
                    channel_id = %channel_id,
                    size = key_package.len(),
                    "Received join request"
                );
                // TODO: Forward to channel manager for processing
            }
        }
        
        Ok(())
    }
    
    /// Get list of peers in a channel
    pub async fn get_channel_peers(&self, channel_id: &ChannelId) -> Vec<PeerId> {
        let members = self.channel_members.read().await;
        members
            .get(channel_id)
            .map(|m| m.values().cloned().collect())
            .unwrap_or_default()
    }
    
    /// Get our local peer ID
    pub fn local_peer_id(&self) -> &PeerId {
        &self.local_peer_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_network_layer_creation() {
        let (router, _handle) = RouterHandle::new();
        let peer_id = PeerId(vec![1, 2, 3, 4]);
        
        let (_network, _rx) = NetworkLayer::new(router, peer_id);
        
        // Network layer created successfully
    }
    
    #[tokio::test]
    async fn test_register_channel_member() {
        let (router, _handle) = RouterHandle::new();
        let peer_id = PeerId(vec![1, 2, 3, 4]);
        let (network, _rx) = NetworkLayer::new(router, peer_id);
        
        let channel_id = ChannelId("test-channel".to_string());
        let user_id = UserId("user123".to_string());
        let member_peer_id = PeerId(vec![5, 6, 7, 8]);
        
        network.register_channel_member(&channel_id, user_id.clone(), member_peer_id.clone()).await;
        
        let peers = network.get_channel_peers(&channel_id).await;
        assert_eq!(peers.len(), 1);
        assert_eq!(peers[0], member_peer_id);
    }
}
