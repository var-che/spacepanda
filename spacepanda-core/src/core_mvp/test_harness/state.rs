//! Server state management for the HTTP test harness
//!
//! Maintains in-memory state for identities, channels, and messages
//! across HTTP requests.

use crate::core_mvp::channel_manager::ChannelManager;
use crate::core_mvp::types::InviteToken;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Represents a user identity in the test harness
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Identity {
    pub identity_id: String,
    pub public_key: Vec<u8>,
}

/// Represents a received message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceivedMessage {
    pub message_id: String,
    pub sender_id: String,
    pub plaintext: String,
    pub timestamp: u64,
}

/// Server state shared across requests
#[derive(Clone)]
pub struct AppState {
    /// Current user identity (single-user per server instance)
    pub identity: Arc<RwLock<Option<Identity>>>,
    
    /// Channel manager for MLS operations
    pub channel_manager: Arc<ChannelManager>,
    
    /// Message history per channel (channel_id -> messages)
    pub message_history: Arc<RwLock<HashMap<String, Vec<ReceivedMessage>>>>,
    
    /// Invite tokens awaiting acceptance (for debugging/inspection)
    pub pending_invites: Arc<RwLock<HashMap<String, InviteToken>>>,
}

impl AppState {
    /// Create a new server state
    pub fn new(channel_manager: Arc<ChannelManager>) -> Self {
        Self {
            identity: Arc::new(RwLock::new(None)),
            channel_manager,
            message_history: Arc::new(RwLock::new(HashMap::new())),
            pending_invites: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// Set the current identity
    pub async fn set_identity(&self, identity: Identity) {
        *self.identity.write().await = Some(identity);
    }
    
    /// Get the current identity
    pub async fn get_identity(&self) -> Option<Identity> {
        self.identity.read().await.clone()
    }
    
    /// Add a message to history
    pub async fn add_message(&self, channel_id: String, message: ReceivedMessage) {
        let mut history = self.message_history.write().await;
        history.entry(channel_id).or_insert_with(Vec::new).push(message);
    }
    
    /// Get message history for a channel
    pub async fn get_messages(&self, channel_id: &str) -> Vec<ReceivedMessage> {
        let history = self.message_history.read().await;
        history.get(channel_id).cloned().unwrap_or_default()
    }
}
