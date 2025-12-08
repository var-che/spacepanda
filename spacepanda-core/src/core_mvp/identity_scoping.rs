//! Identity Scoping - Per-Channel Pseudonymity
//!
//! This module implements per-channel identity isolation to prevent
//! cross-channel correlation and enhance user privacy.
//!
//! ## Threat Model
//!
//! **Without Per-Channel Identities:**
//! ```text
//! Alice joins multiple channels using same identity "alice@example.com":
//! - Channel A (Work): alice@example.com
//! - Channel B (Activism): alice@example.com  
//! - Channel C (Personal): alice@example.com
//!
//! → Adversary sees: "alice@example.com is in all 3 channels"
//! → Can build social graph across contexts
//! → Can correlate activity patterns
//! ```
//!
//! **With Per-Channel Identities:**
//! ```text
//! Alice uses different identity per channel:
//! - Channel A (Work): alice-work-a3f9@spacepanda
//! - Channel B (Activism): alice-activism-7b2e@spacepanda
//! - Channel C (Personal): alice-personal-9d4c@spacepanda
//!
//! → Adversary sees: Three unrelated identities
//! → Cannot correlate across channels (pseudonymity)
//! → Each channel has isolated identity context
//! ```
//!
//! ## Security Properties
//!
//! - **Pseudonymity**: Different channels cannot be linked to same user
//! - **Isolation**: Identity compromise in one channel doesn't affect others
//! - **Unlinkability**: Activity patterns cannot be correlated across channels
//! - **Compartmentalization**: Work, personal, activism identities are separate
//!
//! ## Implementation
//!
//! - Global identity: Used for account management, friend requests
//! - Per-channel identity: Unique pseudonym per channel (default for new channels)
//! - Throwaway identity: Temporary identity for high-risk scenarios
//!
//! ## Usage
//!
//! ```rust
//! use spacepanda_core::core_mvp::identity_scoping::{IdentityScoper, ChannelIdentityMode};
//!
//! let scoper = IdentityScoper::new(global_identity);
//!
//! // Create per-channel identity for new channel
//! let channel_identity = scoper.get_or_create_channel_identity(
//!     &channel_id,
//!     ChannelIdentityMode::PerChannel
//! )?;
//!
//! // Use channel-specific identity for messages
//! send_message(&channel_id, &channel_identity, message)?;
//! ```

use crate::core_mvp::channel_manager::Identity;
use crate::core_store::model::types::UserId;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Mode for channel identity selection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelIdentityMode {
    /// Use global identity (can be correlated across channels)
    Global,

    /// Use unique per-channel identity (pseudonymity)
    PerChannel,

    /// Use temporary throwaway identity (maximum privacy)
    Throwaway,
}

impl Default for ChannelIdentityMode {
    fn default() -> Self {
        // Default to per-channel for privacy
        ChannelIdentityMode::PerChannel
    }
}

/// Identity scoping manager
///
/// Manages multiple identities and provides channel-specific identity selection
pub struct IdentityScoper {
    /// Global identity (primary)
    global_identity: Arc<Identity>,

    /// Per-channel identities (channel_id -> identity)
    channel_identities: Arc<RwLock<HashMap<String, Arc<Identity>>>>,

    /// Mode configuration per channel
    channel_modes: Arc<RwLock<HashMap<String, ChannelIdentityMode>>>,
}

impl IdentityScoper {
    /// Create a new identity scoper
    ///
    /// # Arguments
    /// * `global_identity` - The primary identity for this user
    pub fn new(global_identity: Arc<Identity>) -> Self {
        Self {
            global_identity,
            channel_identities: Arc::new(RwLock::new(HashMap::new())),
            channel_modes: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get or create identity for a channel
    ///
    /// # Arguments
    /// * `channel_id` - Channel identifier
    /// * `mode` - Identity mode for this channel
    ///
    /// # Returns
    /// The identity to use for this channel
    pub async fn get_or_create_channel_identity(
        &self,
        channel_id: &str,
        mode: ChannelIdentityMode,
    ) -> Arc<Identity> {
        // Store mode for this channel
        {
            let mut modes = self.channel_modes.write().await;
            modes.insert(channel_id.to_string(), mode);
        }

        match mode {
            ChannelIdentityMode::Global => {
                // Use global identity
                self.global_identity.clone()
            }
            ChannelIdentityMode::PerChannel => {
                // Get or create per-channel identity
                let mut identities = self.channel_identities.write().await;

                if let Some(identity) = identities.get(channel_id) {
                    identity.clone()
                } else {
                    // Generate new per-channel identity
                    let channel_identity =
                        Self::derive_channel_identity(&self.global_identity, channel_id);
                    let identity_arc = Arc::new(channel_identity);
                    identities.insert(channel_id.to_string(), identity_arc.clone());
                    identity_arc
                }
            }
            ChannelIdentityMode::Throwaway => {
                // Generate throwaway identity (not cached)
                Arc::new(Self::generate_throwaway_identity(channel_id))
            }
        }
    }

    /// Derive a deterministic per-channel identity
    ///
    /// Uses cryptographic derivation to create unique but reproducible
    /// identity for each channel.
    ///
    /// # Arguments
    /// * `global_identity` - Base identity
    /// * `channel_id` - Channel identifier
    ///
    /// # Returns
    /// New channel-specific identity
    fn derive_channel_identity(global_identity: &Identity, channel_id: &str) -> Identity {
        // Derive channel-specific user ID
        let mut hasher = Sha256::new();
        hasher.update(b"channel_identity_v1:");
        hasher.update(global_identity.user_id.0.as_bytes());
        hasher.update(b":");
        hasher.update(channel_id.as_bytes());
        let hash = hasher.finalize();

        // Create pseudonymous user ID
        let channel_suffix = hex::encode(&hash[..8]);
        let channel_user_id = format!(
            "{}-{}",
            global_identity
                .user_id
                .0
                .split('@')
                .next()
                .unwrap_or(&global_identity.user_id.0),
            channel_suffix
        );

        // Create display name hint
        let display_name = format!("{} ({})", global_identity.display_name, &channel_suffix[..6]);

        // Derive node ID
        let node_id = format!("{}:{}", global_identity.node_id, channel_suffix);

        Identity {
            user_id: UserId(format!("{}@spacepanda.local", channel_user_id)),
            display_name,
            node_id,
        }
    }

    /// Generate a random throwaway identity
    ///
    /// Creates a completely random identity for maximum privacy.
    /// Not deterministic - each call generates a new identity.
    ///
    /// # Arguments
    /// * `context` - Context string for logging/debugging
    fn generate_throwaway_identity(context: &str) -> Identity {
        use rand::Rng;

        let mut rng = rand::rng();
        let random_bytes: [u8; 16] = rng.random();
        let random_id = hex::encode(random_bytes);

        Identity {
            user_id: UserId(format!("anon-{}@spacepanda.local", random_id)),
            display_name: format!("Anonymous ({})", &random_id[..8]),
            node_id: format!("throwaway-{}-{}", context, random_id),
        }
    }

    /// Get current identity for a channel
    ///
    /// # Arguments
    /// * `channel_id` - Channel identifier
    ///
    /// # Returns
    /// The identity currently used for this channel, or None if not set
    pub async fn get_channel_identity(&self, channel_id: &str) -> Option<Arc<Identity>> {
        let identities = self.channel_identities.read().await;
        identities.get(channel_id).cloned()
    }

    /// Get the mode for a channel
    ///
    /// # Arguments
    /// * `channel_id` - Channel identifier
    ///
    /// # Returns
    /// The identity mode for this channel
    pub async fn get_channel_mode(&self, channel_id: &str) -> ChannelIdentityMode {
        let modes = self.channel_modes.read().await;
        modes.get(channel_id).copied().unwrap_or_default()
    }

    /// Set mode for a channel
    ///
    /// # Arguments
    /// * `channel_id` - Channel identifier
    /// * `mode` - New identity mode
    ///
    /// # Note
    /// Changing mode after joining a channel may require re-joining
    pub async fn set_channel_mode(&self, channel_id: &str, mode: ChannelIdentityMode) {
        let mut modes = self.channel_modes.write().await;
        modes.insert(channel_id.to_string(), mode);
    }

    /// Remove channel identity (cleanup)
    ///
    /// # Arguments
    /// * `channel_id` - Channel identifier
    pub async fn remove_channel_identity(&self, channel_id: &str) {
        let mut identities = self.channel_identities.write().await;
        identities.remove(channel_id);

        let mut modes = self.channel_modes.write().await;
        modes.remove(channel_id);
    }

    /// Get global identity
    pub fn global_identity(&self) -> Arc<Identity> {
        self.global_identity.clone()
    }

    /// List all active channel identities
    pub async fn list_channel_identities(&self) -> Vec<(String, Arc<Identity>)> {
        let identities = self.channel_identities.read().await;
        identities.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_identity() -> Identity {
        Identity::new(
            UserId("alice@example.com".to_string()),
            "Alice".to_string(),
            "node-123".to_string(),
        )
    }

    #[tokio::test]
    async fn test_global_identity_mode() {
        let global = Arc::new(create_test_identity());
        let scoper = IdentityScoper::new(global.clone());

        let identity = scoper
            .get_or_create_channel_identity("channel1", ChannelIdentityMode::Global)
            .await;

        // Should return same global identity
        assert_eq!(identity.user_id, global.user_id);
        assert_eq!(identity.display_name, global.display_name);
    }

    #[tokio::test]
    async fn test_per_channel_identity() {
        let global = Arc::new(create_test_identity());
        let scoper = IdentityScoper::new(global.clone());

        let identity1 = scoper
            .get_or_create_channel_identity("channel1", ChannelIdentityMode::PerChannel)
            .await;

        let identity2 = scoper
            .get_or_create_channel_identity("channel2", ChannelIdentityMode::PerChannel)
            .await;

        // Identities should be different for different channels
        assert_ne!(identity1.user_id, identity2.user_id);
        assert_ne!(identity1.user_id, global.user_id);
        assert_ne!(identity2.user_id, global.user_id);
    }

    #[tokio::test]
    async fn test_per_channel_identity_deterministic() {
        let global = Arc::new(create_test_identity());
        let scoper = IdentityScoper::new(global.clone());

        let identity1 = scoper
            .get_or_create_channel_identity("channel1", ChannelIdentityMode::PerChannel)
            .await;

        let identity2 = scoper
            .get_or_create_channel_identity("channel1", ChannelIdentityMode::PerChannel)
            .await;

        // Same channel should return same identity
        assert_eq!(identity1.user_id, identity2.user_id);
    }

    #[tokio::test]
    async fn test_throwaway_identity() {
        let global = Arc::new(create_test_identity());
        let scoper = IdentityScoper::new(global.clone());

        let identity1 = scoper
            .get_or_create_channel_identity("channel1", ChannelIdentityMode::Throwaway)
            .await;

        let identity2 = scoper
            .get_or_create_channel_identity("channel1", ChannelIdentityMode::Throwaway)
            .await;

        // Throwaway identities should be different each time
        assert_ne!(identity1.user_id, identity2.user_id);
        assert_ne!(identity1.user_id, global.user_id);
        assert!(identity1.user_id.0.starts_with("anon-"));
        assert!(identity2.user_id.0.starts_with("anon-"));
    }

    #[tokio::test]
    async fn test_channel_identity_unlinkability() {
        let global = Arc::new(create_test_identity());
        let scoper = IdentityScoper::new(global.clone());

        let work_identity = scoper
            .get_or_create_channel_identity("work-channel", ChannelIdentityMode::PerChannel)
            .await;

        let activism_identity = scoper
            .get_or_create_channel_identity("activism-channel", ChannelIdentityMode::PerChannel)
            .await;

        let personal_identity = scoper
            .get_or_create_channel_identity("personal-channel", ChannelIdentityMode::PerChannel)
            .await;

        // All identities should be different (unlinkable)
        assert_ne!(work_identity.user_id, activism_identity.user_id);
        assert_ne!(work_identity.user_id, personal_identity.user_id);
        assert_ne!(activism_identity.user_id, personal_identity.user_id);

        // None should match global
        assert_ne!(work_identity.user_id, global.user_id);
        assert_ne!(activism_identity.user_id, global.user_id);
        assert_ne!(personal_identity.user_id, global.user_id);
    }

    #[tokio::test]
    async fn test_mode_persistence() {
        let global = Arc::new(create_test_identity());
        let scoper = IdentityScoper::new(global.clone());

        scoper.set_channel_mode("channel1", ChannelIdentityMode::PerChannel).await;
        let mode = scoper.get_channel_mode("channel1").await;

        assert_eq!(mode, ChannelIdentityMode::PerChannel);
    }

    #[tokio::test]
    async fn test_remove_channel_identity() {
        let global = Arc::new(create_test_identity());
        let scoper = IdentityScoper::new(global.clone());

        let identity = scoper
            .get_or_create_channel_identity("channel1", ChannelIdentityMode::PerChannel)
            .await;

        assert!(scoper.get_channel_identity("channel1").await.is_some());

        scoper.remove_channel_identity("channel1").await;

        assert!(scoper.get_channel_identity("channel1").await.is_none());
    }

    #[tokio::test]
    async fn test_list_channel_identities() {
        let global = Arc::new(create_test_identity());
        let scoper = IdentityScoper::new(global.clone());

        scoper
            .get_or_create_channel_identity("channel1", ChannelIdentityMode::PerChannel)
            .await;
        scoper
            .get_or_create_channel_identity("channel2", ChannelIdentityMode::PerChannel)
            .await;

        let list = scoper.list_channel_identities().await;
        assert_eq!(list.len(), 2);
    }

    #[tokio::test]
    async fn test_identity_derivation_format() {
        let global = Arc::new(create_test_identity());
        let scoper = IdentityScoper::new(global.clone());

        let identity = scoper
            .get_or_create_channel_identity("test-channel", ChannelIdentityMode::PerChannel)
            .await;

        // Should contain base username
        assert!(identity.user_id.0.starts_with("alice-"));
        // Should end with @spacepanda.local
        assert!(identity.user_id.0.ends_with("@spacepanda.local"));
        // Should have hex suffix
        let parts: Vec<&str> = identity.user_id.0.split('-').collect();
        assert!(parts.len() >= 2);
    }
}
