/*
    identity_meta.rs - Identity Metadata
    
    Per-device identity metadata for MLS integration.
    
    This tracks cryptographic identities used for:
    - MLS group membership
    - Message signing
    - Key packages
    - Credential management
    
    Each user can have multiple identities:
    - One global identity per device
    - Per-channel pseudonymous identities (future)
    - Throwaway identities for privacy
*/

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Identity metadata for a user on this device
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IdentityMeta {
    /// Unique identity ID
    pub identity_id: String,
    
    /// Public key for this identity (Ed25519)
    pub public_key: Vec<u8>,
    
    /// MLS credential type
    pub credential_type: CredentialType,
    
    /// MLS leaf indices per channel
    pub leaf_indices: HashMap<String, u32>,
    
    /// When this identity was created
    pub created_at: u64,
    
    /// When this identity was last used
    pub last_used: u64,
    
    /// Whether this is the primary identity for the device
    pub is_primary: bool,
    
    /// Optional display name
    pub display_name: Option<String>,
    
    /// Identity scope (global, per-channel, throwaway)
    pub scope: IdentityScope,
}

/// Type of MLS credential
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CredentialType {
    /// Basic credential (public key only)
    Basic,
    
    /// X.509 certificate
    X509,
    
    /// Custom credential type
    Custom,
}

impl Default for CredentialType {
    fn default() -> Self {
        CredentialType::Basic
    }
}

/// Scope of an identity
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IdentityScope {
    /// Global identity used across all channels
    Global,
    
    /// Per-channel pseudonymous identity
    PerChannel,
    
    /// Temporary throwaway identity
    Throwaway,
}

impl Default for IdentityScope {
    fn default() -> Self {
        IdentityScope::Global
    }
}

/// Key package for MLS group joining
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyPackage {
    /// MLS version
    pub version: u16,
    
    /// Cipher suite
    pub cipher_suite: u16,
    
    /// Init key (HPKE public key)
    pub init_key: Vec<u8>,
    
    /// Leaf node
    pub leaf_node: Vec<u8>,
    
    /// Extensions
    pub extensions: Vec<Extension>,
    
    /// Signature over the package
    pub signature: Vec<u8>,
}

/// MLS extension
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Extension {
    /// Extension type
    pub extension_type: u16,
    
    /// Extension data
    pub extension_data: Vec<u8>,
}

impl IdentityMeta {
    /// Create a new identity
    pub fn new(
        identity_id: String,
        public_key: Vec<u8>,
        scope: IdentityScope,
    ) -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};
        
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        IdentityMeta {
            identity_id,
            public_key,
            credential_type: CredentialType::default(),
            leaf_indices: HashMap::new(),
            created_at: now,
            last_used: now,
            is_primary: false,
            display_name: None,
            scope,
        }
    }
    
    /// Create a global identity
    pub fn new_global(identity_id: String, public_key: Vec<u8>) -> Self {
        let mut identity = Self::new(identity_id, public_key, IdentityScope::Global);
        identity.is_primary = true;
        identity
    }
    
    /// Create a per-channel identity
    pub fn new_per_channel(identity_id: String, public_key: Vec<u8>) -> Self {
        Self::new(identity_id, public_key, IdentityScope::PerChannel)
    }
    
    /// Create a throwaway identity
    pub fn new_throwaway(identity_id: String, public_key: Vec<u8>) -> Self {
        Self::new(identity_id, public_key, IdentityScope::Throwaway)
    }
    
    /// Set display name
    pub fn set_display_name(&mut self, name: String) {
        self.display_name = Some(name);
    }
    
    /// Record leaf index for a channel
    pub fn set_leaf_index(&mut self, channel_id: String, leaf_index: u32) {
        self.leaf_indices.insert(channel_id, leaf_index);
    }
    
    /// Get leaf index for a channel
    pub fn get_leaf_index(&self, channel_id: &str) -> Option<u32> {
        self.leaf_indices.get(channel_id).copied()
    }
    
    /// Remove leaf index for a channel (left the group)
    pub fn remove_leaf_index(&mut self, channel_id: &str) -> Option<u32> {
        self.leaf_indices.remove(channel_id)
    }
    
    /// Update last used timestamp
    pub fn touch(&mut self) {
        use std::time::{SystemTime, UNIX_EPOCH};
        
        self.last_used = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
    }
    
    /// Check if this identity is in a specific channel
    pub fn is_in_channel(&self, channel_id: &str) -> bool {
        self.leaf_indices.contains_key(channel_id)
    }
    
    /// Get number of channels this identity is in
    pub fn channel_count(&self) -> usize {
        self.leaf_indices.len()
    }
    
    /// Check if this identity should be automatically cleaned up
    pub fn is_expired(&self, max_age_seconds: u64) -> bool {
        use std::time::{SystemTime, UNIX_EPOCH};
        
        if self.scope != IdentityScope::Throwaway {
            return false;
        }
        
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        now - self.last_used > max_age_seconds
    }
}

/// Identity manager for tracking multiple identities
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IdentityManager {
    /// All identities indexed by ID
    identities: HashMap<String, IdentityMeta>,
    
    /// Primary identity ID
    primary_identity: Option<String>,
}

impl IdentityManager {
    /// Create a new identity manager
    pub fn new() -> Self {
        IdentityManager {
            identities: HashMap::new(),
            primary_identity: None,
        }
    }
    
    /// Add an identity
    pub fn add_identity(&mut self, identity: IdentityMeta) {
        if identity.is_primary {
            self.primary_identity = Some(identity.identity_id.clone());
        }
        self.identities.insert(identity.identity_id.clone(), identity);
    }
    
    /// Remove an identity
    pub fn remove_identity(&mut self, identity_id: &str) -> Option<IdentityMeta> {
        if self.primary_identity.as_deref() == Some(identity_id) {
            self.primary_identity = None;
        }
        self.identities.remove(identity_id)
    }
    
    /// Get an identity
    pub fn get_identity(&self, identity_id: &str) -> Option<&IdentityMeta> {
        self.identities.get(identity_id)
    }
    
    /// Get mutable identity
    pub fn get_identity_mut(&mut self, identity_id: &str) -> Option<&mut IdentityMeta> {
        self.identities.get_mut(identity_id)
    }
    
    /// Get the primary identity
    pub fn get_primary(&self) -> Option<&IdentityMeta> {
        self.primary_identity.as_ref()
            .and_then(|id| self.identities.get(id))
    }
    
    /// Get identity for a specific channel
    pub fn get_identity_for_channel(&self, channel_id: &str) -> Option<&IdentityMeta> {
        // First try to find a per-channel identity
        for identity in self.identities.values() {
            if identity.scope == IdentityScope::PerChannel && identity.is_in_channel(channel_id) {
                return Some(identity);
            }
        }
        
        // Fall back to primary identity
        self.get_primary()
    }
    
    /// List all identities
    pub fn list_identities(&self) -> Vec<&IdentityMeta> {
        self.identities.values().collect()
    }
    
    /// Clean up expired throwaway identities
    pub fn cleanup_expired(&mut self, max_age_seconds: u64) -> usize {
        let expired: Vec<_> = self.identities.iter()
            .filter(|(_, identity)| identity.is_expired(max_age_seconds))
            .map(|(id, _)| id.clone())
            .collect();
        
        let count = expired.len();
        for id in expired {
            self.identities.remove(&id);
        }
        
        count
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_identity_creation() {
        let identity = IdentityMeta::new_global(
            "id123".to_string(),
            vec![1, 2, 3],
        );
        
        assert_eq!(identity.identity_id, "id123");
        assert!(identity.is_primary);
        assert_eq!(identity.scope, IdentityScope::Global);
    }
    
    #[test]
    fn test_leaf_index_management() {
        let mut identity = IdentityMeta::new_global(
            "id123".to_string(),
            vec![1, 2, 3],
        );
        
        identity.set_leaf_index("channel1".to_string(), 5);
        assert_eq!(identity.get_leaf_index("channel1"), Some(5));
        assert!(identity.is_in_channel("channel1"));
        assert_eq!(identity.channel_count(), 1);
        
        identity.remove_leaf_index("channel1");
        assert!(!identity.is_in_channel("channel1"));
    }
    
    #[test]
    fn test_identity_manager() {
        let mut manager = IdentityManager::new();
        
        let identity1 = IdentityMeta::new_global(
            "id1".to_string(),
            vec![1, 2, 3],
        );
        
        let identity2 = IdentityMeta::new_per_channel(
            "id2".to_string(),
            vec![4, 5, 6],
        );
        
        manager.add_identity(identity1);
        manager.add_identity(identity2);
        
        assert_eq!(manager.list_identities().len(), 2);
        assert!(manager.get_primary().is_some());
        assert_eq!(manager.get_primary().unwrap().identity_id, "id1");
    }
    
    #[test]
    fn test_throwaway_identity_expiration() {
        let mut identity = IdentityMeta::new_throwaway(
            "throwaway".to_string(),
            vec![1, 2, 3],
        );
        
        // Set last_used to 2 hours ago
        identity.last_used = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() - 7200;
        
        assert!(identity.is_expired(3600)); // 1 hour threshold
        assert!(!identity.is_expired(10000)); // 3 hour threshold
    }
    
    #[test]
    fn test_cleanup_expired() {
        let mut manager = IdentityManager::new();
        
        let mut throwaway = IdentityMeta::new_throwaway(
            "throwaway".to_string(),
            vec![1, 2, 3],
        );
        throwaway.last_used = 0; // Very old
        
        let global = IdentityMeta::new_global(
            "global".to_string(),
            vec![4, 5, 6],
        );
        
        manager.add_identity(throwaway);
        manager.add_identity(global);
        
        let cleaned = manager.cleanup_expired(60);
        assert_eq!(cleaned, 1);
        assert_eq!(manager.list_identities().len(), 1);
    }
}
