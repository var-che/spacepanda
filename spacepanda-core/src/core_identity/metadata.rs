//! Metadata module
//!
//! User and device metadata replicated via CRDT.

use crate::core_identity::device_id::DeviceId;
use crate::core_identity::user_id::UserId;
use crate::core_store::crdt::{LWWRegister, ORMap, AddId, VectorClock, Crdt};
use crate::core_store::model::types::Timestamp;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Hash type for content addressing
pub type Hash = Vec<u8>;

/// Device metadata tracked per device
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceMetadata {
    /// Device identifier
    pub device_id: DeviceId,
    /// Human-readable device name
    pub device_name: LWWRegister<String>,
    /// Last seen timestamp
    pub last_seen: LWWRegister<Timestamp>,
    /// Reference to key package stored in DHT
    pub key_package_ref: LWWRegister<Option<Hash>>,
    /// Device capabilities (protocol version, features, etc.)
    pub capabilities: LWWRegister<HashMap<String, String>>,
}

impl DeviceMetadata {
    /// Create new device metadata
    pub fn new(device_id: DeviceId, device_name: String, node_id: &str) -> Self {
        let now = Timestamp::now();
        let mut vc = VectorClock::new();
        vc.increment(node_id);

        DeviceMetadata {
            device_id,
            device_name: LWWRegister::with_value(device_name, node_id.to_string()),
            last_seen: LWWRegister::with_value(now, node_id.to_string()),
            key_package_ref: LWWRegister::with_value(None, node_id.to_string()),
            capabilities: LWWRegister::with_value(HashMap::new(), node_id.to_string()),
        }
    }

    /// Update last seen timestamp
    pub fn update_last_seen(&mut self, ts: Timestamp, node_id: &str) {
        let mut vc = VectorClock::new();
        vc.increment(node_id);
        self.last_seen.set(ts, ts.as_millis(), node_id.to_string(), vc);
    }

    /// Set key package reference
    pub fn set_key_package_ref(&mut self, hash: Option<Hash>, ts: Timestamp, node_id: &str) {
        let mut vc = VectorClock::new();
        vc.increment(node_id);
        self.key_package_ref.set(hash, ts.as_millis(), node_id.to_string(), vc);
    }
}

/// User metadata - replicated across all peers via CRDT
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserMetadata {
    /// User identifier
    pub user_id: UserId,
    /// Display name
    pub display_name: LWWRegister<String>,
    /// Avatar content hash
    pub avatar_hash: LWWRegister<Option<Hash>>,
    /// Map of devices owned by this user
    pub devices: ORMap<DeviceId, DeviceMetadata>,
}

impl UserMetadata {
    /// Create new user metadata
    pub fn new(user_id: UserId) -> Self {
        UserMetadata {
            user_id,
            display_name: LWWRegister::with_value(String::new(), "local".to_string()),
            avatar_hash: LWWRegister::with_value(None, "local".to_string()),
            devices: ORMap::new(),
        }
    }

    /// Set display name
    pub fn set_display_name(&mut self, name: String, ts: Timestamp, node_id: &str) {
        let mut vc = VectorClock::new();
        vc.increment(node_id);
        self.display_name.set(name, ts.as_millis(), node_id.to_string(), vc);
    }

    /// Set avatar hash
    pub fn set_avatar_hash(&mut self, hash: Option<Hash>, ts: Timestamp, node_id: &str) {
        let mut vc = VectorClock::new();
        vc.increment(node_id);
        self.avatar_hash.set(hash, ts.as_millis(), node_id.to_string(), vc);
    }

    /// Add a device
    pub fn add_device(&mut self, meta: DeviceMetadata, add_id: AddId, vc: VectorClock) {
        self.devices.put(meta.device_id.clone(), meta, add_id, vc);
    }

    /// Remove a device
    pub fn remove_device(&mut self, device_id: &DeviceId, vc: VectorClock) {
        self.devices.remove(device_id, vc);
    }

    /// Get a device
    pub fn get_device(&self, device_id: &DeviceId) -> Option<&DeviceMetadata> {
        self.devices.get(device_id)
    }

    /// Merge with remote metadata
    pub fn merge(&mut self, other: &UserMetadata) {
        self.display_name.merge(&other.display_name);
        self.avatar_hash.merge(&other.avatar_hash);
        // Merge devices using CRDT merge
        let _ = self.devices.merge(&other.devices);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_metadata_creation() {
        let device_id = DeviceId::generate();
        let meta = DeviceMetadata::new(device_id.clone(), "My Device".to_string(), "node1");
        assert_eq!(meta.device_id, device_id);
        assert_eq!(meta.device_name.get(), Some(&"My Device".to_string()));
    }

    #[test]
    fn test_user_metadata_creation() {
        let pubkey = vec![1, 2, 3, 4, 5];
        let user_id = UserId::from_public_key(&pubkey);
        let meta = UserMetadata::new(user_id.clone());
        assert_eq!(meta.user_id, user_id);
    }

    #[test]
    fn test_add_and_remove_device() {
        let pubkey = vec![1, 2, 3, 4, 5];
        let user_id = UserId::from_public_key(&pubkey);
        let mut meta = UserMetadata::new(user_id);

        let device_id = DeviceId::generate();
        let device_meta = DeviceMetadata::new(device_id.clone(), "Device 1".to_string(), "node1");

        let add_id = AddId::new("node1".to_string(), Timestamp::now().as_millis());
        let mut vc = VectorClock::new();
        vc.increment("node1");

        meta.add_device(device_meta, add_id, vc.clone());
        assert!(meta.get_device(&device_id).is_some());

        meta.remove_device(&device_id, vc);
        assert!(meta.get_device(&device_id).is_none());
    }

    #[test]
    fn test_metadata_merge() {
        let pubkey = vec![1, 2, 3, 4, 5];
        let user_id = UserId::from_public_key(&pubkey);
        
        let mut meta1 = UserMetadata::new(user_id.clone());
        let mut meta2 = UserMetadata::new(user_id);

        meta1.set_display_name("Alice".to_string(), Timestamp::now(), "node1");
        meta2.set_display_name("Bob".to_string(), Timestamp::now(), "node2");

        meta1.merge(&meta2);
        // LWW register will have one of the names
        assert!(meta1.display_name.get().is_some());
    }
}
