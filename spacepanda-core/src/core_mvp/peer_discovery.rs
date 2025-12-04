/// Peer Discovery Service
///
/// Provides mechanisms to discover network peer IDs for user identities.
/// This is used by the NetworkLayer to map user identities to peer addresses.
///
/// # Privacy-First Architecture
///
/// **SECURITY DECISION**: This module previously included DHT-based peer discovery,
/// which has been REMOVED due to critical privacy risks:
///
/// ## DHT Metadata Leakage Threats
/// - DHT queries expose WHO is looking for WHOM and WHEN
/// - Global observers can build social graphs from lookup patterns
/// - User activity patterns become visible to DHT node operators
/// - Online status inference from query timing
/// - Violates privacy-first mission (similar to Signal/Session's approach)
///
/// ## Current Approach: Invite-Based Peer Exchange
/// Peer IDs are now exchanged securely through encrypted InviteTokens:
/// - No metadata leakage to third parties
/// - Perfect forward secrecy via MLS encryption
/// - Only communicating parties learn peer IDs
/// - Prevents social graph analysis
///
/// ## Future Considerations
/// If DHT is needed for relay/bootstrap discovery (not user discovery):
/// - Route all DHT queries through onion circuits (anonymous lookups)
/// - Use blind cryptographic queries (PIR)
/// - Store only relay addresses, never user identity mappings

use crate::core_router::session_manager::PeerId;
use crate::core_store::model::types::UserId;
use async_trait::async_trait;
use std::sync::Arc;

/// Trait for peer discovery services
#[async_trait]
pub trait PeerDiscovery: Send + Sync {
    /// Look up the peer ID for a given user identity
    ///
    /// # Arguments
    /// * `identity` - User's MLS identity bytes
    ///
    /// # Returns
    /// The peer ID if found, or None if the user is not registered
    async fn lookup_peer_id(&self, identity: &[u8]) -> Result<Option<PeerId>, String>;

    /// Look up peer ID by UserId
    ///
    /// # Arguments
    /// * `user_id` - User ID to lookup
    ///
    /// # Returns
    /// The peer ID if found, or None if the user is not registered
    async fn lookup_peer_id_by_user(&self, user_id: &UserId) -> Result<Option<PeerId>, String> {
        self.lookup_peer_id(user_id.0.as_bytes()).await
    }

    /// Register our own peer ID for our identity
    ///
    /// # Arguments
    /// * `identity` - Our MLS identity bytes
    /// * `peer_id` - Our network peer ID
    async fn register_self(&self, identity: &[u8], peer_id: PeerId) -> Result<(), String>;
}

/// No-op peer discovery for testing/local-only mode
pub struct NoPeerDiscovery;

#[async_trait]
impl PeerDiscovery for NoPeerDiscovery {
    async fn lookup_peer_id(&self, _identity: &[u8]) -> Result<Option<PeerId>, String> {
        Ok(None)
    }

    async fn register_self(&self, _identity: &[u8], _peer_id: PeerId) -> Result<(), String> {
        Ok(())
    }
}

// DHT-based peer discovery REMOVED for privacy reasons
// 
// SECURITY RATIONALE:
// - DHT queries expose metadata: who is looking for whom, when, and social graphs
// - Global observer can track user activity patterns and relationships
// - Violates privacy-first mission (Signal/Session use invite-based exchange, not DHT)
//
// ALTERNATIVE APPROACH:
// Peer IDs are now exchanged through encrypted invite tokens.
// This provides perfect forward secrecy and prevents metadata leakage.

/// Type alias for peer discovery service
pub type PeerDiscoveryService = Arc<dyn PeerDiscovery>;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_no_peer_discovery() {
        let discovery = NoPeerDiscovery;
        let result = discovery.lookup_peer_id(b"test_identity").await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }
}
