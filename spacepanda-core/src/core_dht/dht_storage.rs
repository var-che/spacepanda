/*
    DHTStorage - local storage engine for DHT key-value pairs

    Responsibilities:
    `dht_storage.rs` implements a local storage engine for DHT key-value pairs.
    This is not a database and its very simple key-value store.
    It handles:
    - persistent map or sled/rocksdb store
    - returns stored value
    - maintains expiration
    - handles conflict resolution
    - used by replication and GET handlers

    The storage must also keep which peers store replicas.

    Inputs:
    - requests: store(key, value), get(key), delete(key)
    - load value(key)
    - refresh key (extend expiration)

    Outputs:
    - stored values
    - deletion notifications
    - expiration scans
*/

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

/// Get current Unix timestamp in seconds
/// Returns 0 if system clock is before UNIX epoch (should never happen on modern systems)
fn current_timestamp() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

use super::dht_key::DhtKey;
use super::dht_value::DhtValue;

/// Entry in the storage with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
struct StorageEntry {
    /// The DHT value
    value: DhtValue,
    /// When this entry was stored locally (Unix timestamp)
    stored_at: u64,
    /// Peer IDs that store replicas of this value
    replica_peers: Vec<DhtKey>,
}

impl StorageEntry {
    fn new(value: DhtValue) -> Self {
        let now = current_timestamp();

        StorageEntry { value, stored_at: now, replica_peers: Vec::new() }
    }

    /// Check if this entry has expired
    fn is_expired(&self) -> bool {
        self.value.is_expired()
    }

    /// Add a replica peer
    fn add_replica(&mut self, peer: DhtKey) {
        if !self.replica_peers.contains(&peer) {
            self.replica_peers.push(peer);
        }
    }

    /// Remove a replica peer
    fn remove_replica(&mut self, peer: &DhtKey) {
        self.replica_peers.retain(|p| p != peer);
    }
}

/// Simple in-memory DHT storage
#[derive(Clone)]
pub struct DhtStorage {
    /// Storage map: key -> entry
    store: Arc<RwLock<HashMap<DhtKey, StorageEntry>>>,
}

impl DhtStorage {
    /// Create a new DHT storage
    pub fn new() -> Self {
        DhtStorage { store: Arc::new(RwLock::new(HashMap::new())) }
    }

    /// Store a value
    pub fn put(&self, key: DhtKey, value: DhtValue) -> Result<(), String> {
        let mut store =
            self.store.write().map_err(|e| format!("Failed to acquire write lock: {}", e))?;

        // Check if we already have this key
        if let Some(existing) = store.get(&key) {
            // Only update if new value has higher sequence number
            if value.sequence <= existing.value.sequence {
                return Err(format!(
                    "Stale value: existing sequence {} >= new sequence {}",
                    existing.value.sequence, value.sequence
                ));
            }
        }

        store.insert(key, StorageEntry::new(value));
        Ok(())
    }

    /// Get a value
    pub fn get(&self, key: &DhtKey) -> Result<DhtValue, String> {
        let store = self.store.read().map_err(|e| format!("Failed to acquire read lock: {}", e))?;

        match store.get(key) {
            Some(entry) => {
                if entry.is_expired() {
                    Err("Value has expired".to_string())
                } else {
                    Ok(entry.value.clone())
                }
            }
            None => Err("Key not found".to_string()),
        }
    }

    /// Delete a value
    pub fn delete(&self, key: &DhtKey) -> Result<(), String> {
        let mut store =
            self.store.write().map_err(|e| format!("Failed to acquire write lock: {}", e))?;

        if store.remove(key).is_some() {
            Ok(())
        } else {
            Err("Key not found".to_string())
        }
    }

    /// Refresh a key (extend its TTL)
    pub fn refresh(&self, key: &DhtKey, new_ttl: u64) -> Result<(), String> {
        let mut store =
            self.store.write().map_err(|e| format!("Failed to acquire write lock: {}", e))?;

        match store.get_mut(key) {
            Some(entry) => {
                entry.value.ttl = new_ttl;
                entry.value.timestamp = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("System clock is before UNIX epoch")
                    .as_secs();
                Ok(())
            }
            None => Err("Key not found".to_string()),
        }
    }

    /// Get all keys in storage
    pub fn keys(&self) -> Result<Vec<DhtKey>, String> {
        let store = self.store.read().map_err(|e| format!("Failed to acquire read lock: {}", e))?;
        Ok(store.keys().copied().collect())
    }

    /// Get all non-expired keys
    pub fn active_keys(&self) -> Result<Vec<DhtKey>, String> {
        let store = self.store.read().map_err(|e| format!("Failed to acquire read lock: {}", e))?;
        Ok(store
            .iter()
            .filter(|(_, entry)| !entry.is_expired())
            .map(|(key, _)| *key)
            .collect())
    }

    /// Remove all expired entries
    pub fn cleanup_expired(&self) -> Result<usize, String> {
        let mut store =
            self.store.write().map_err(|e| format!("Failed to acquire write lock: {}", e))?;
        let before_count = store.len();

        store.retain(|_, entry| !entry.is_expired());

        Ok(before_count - store.len())
    }

    /// Add a replica peer for a key
    pub fn add_replica(&self, key: &DhtKey, peer: DhtKey) -> Result<(), String> {
        let mut store =
            self.store.write().map_err(|e| format!("Failed to acquire write lock: {}", e))?;

        match store.get_mut(key) {
            Some(entry) => {
                entry.add_replica(peer);
                Ok(())
            }
            None => Err("Key not found".to_string()),
        }
    }

    /// Remove a replica peer for a key
    pub fn remove_replica(&self, key: &DhtKey, peer: &DhtKey) -> Result<(), String> {
        let mut store =
            self.store.write().map_err(|e| format!("Failed to acquire write lock: {}", e))?;

        match store.get_mut(key) {
            Some(entry) => {
                entry.remove_replica(peer);
                Ok(())
            }
            None => Err("Key not found".to_string()),
        }
    }

    /// Get replica peers for a key
    pub fn get_replicas(&self, key: &DhtKey) -> Result<Vec<DhtKey>, String> {
        let store = self.store.read().map_err(|e| format!("Failed to acquire read lock: {}", e))?;

        match store.get(key) {
            Some(entry) => Ok(entry.replica_peers.clone()),
            None => Err("Key not found".to_string()),
        }
    }

    /// Get total number of stored entries
    pub fn size(&self) -> Result<usize, String> {
        Ok(self
            .store
            .read()
            .map_err(|e| format!("Failed to acquire read lock: {}", e))?
            .len())
    }

    /// Clear all entries
    pub fn clear(&self) -> Result<(), String> {
        self.store
            .write()
            .map_err(|e| format!("Failed to acquire write lock: {}", e))?
            .clear();
        Ok(())
    }

    /// Get all entries (for debugging/testing)
    pub fn entries(&self) -> Result<Vec<(DhtKey, DhtValue)>, String> {
        let store = self.store.read().map_err(|e| format!("Failed to acquire read lock: {}", e))?;
        Ok(store.iter().map(|(key, entry)| (*key, entry.value.clone())).collect())
    }
}

impl Default for DhtStorage {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_storage_put_get() {
        let storage = DhtStorage::new();
        let key = DhtKey::hash(b"test_key");
        let value = DhtValue::new(b"test_data".to_vec()).with_ttl(3600);

        // Store value
        assert!(storage.put(key, value.clone()).is_ok());

        // Retrieve value
        let retrieved = storage.get(&key).unwrap();
        assert_eq!(retrieved.data, value.data);
    }

    #[test]
    fn test_storage_delete() {
        let storage = DhtStorage::new();
        let key = DhtKey::hash(b"test_key");
        let value = DhtValue::new(b"test_data".to_vec()).with_ttl(3600);

        storage.put(key, value).unwrap();
        assert!(storage.get(&key).is_ok());

        // Delete value
        assert!(storage.delete(&key).is_ok());
        assert!(storage.get(&key).is_err());

        // Delete non-existent key
        assert!(storage.delete(&key).is_err());
    }

    #[test]
    fn test_storage_sequence_conflict() {
        let storage = DhtStorage::new();
        let key = DhtKey::hash(b"test_key");

        let mut value1 = DhtValue::new(b"data1".to_vec()).with_ttl(3600);
        value1.sequence = 10;

        let mut value2 = DhtValue::new(b"data2".to_vec()).with_ttl(3600);
        value2.sequence = 5;

        // Store first value
        storage.put(key, value1.clone()).unwrap();

        // Try to store older value - should fail
        assert!(storage.put(key, value2).is_err());

        // Value should still be value1
        let retrieved = storage.get(&key).unwrap();
        assert_eq!(retrieved.data, b"data1");
        assert_eq!(retrieved.sequence, 10);
    }

    #[test]
    fn test_storage_sequence_update() {
        let storage = DhtStorage::new();
        let key = DhtKey::hash(b"test_key");

        let mut value1 = DhtValue::new(b"data1".to_vec()).with_ttl(3600);
        value1.sequence = 10;

        let mut value2 = DhtValue::new(b"data2".to_vec()).with_ttl(3600);
        value2.sequence = 20;

        // Store first value
        storage.put(key, value1).unwrap();

        // Store newer value - should succeed
        storage.put(key, value2.clone()).unwrap();

        // Value should be updated to value2
        let retrieved = storage.get(&key).unwrap();
        assert_eq!(retrieved.data, b"data2");
        assert_eq!(retrieved.sequence, 20);
    }

    #[test]
    fn test_storage_expired_values() {
        let storage = DhtStorage::new();
        let key = DhtKey::hash(b"test_key");

        // Create value with 0 TTL (already expired)
        let value = DhtValue::new(b"test_data".to_vec()).with_ttl(0);

        storage.put(key, value).unwrap();

        // Should fail to retrieve expired value
        assert!(storage.get(&key).is_err());
    }

    #[test]
    fn test_storage_cleanup_expired() {
        let storage = DhtStorage::new();

        let key1 = DhtKey::hash(b"key1");
        let key2 = DhtKey::hash(b"key2");
        let key3 = DhtKey::hash(b"key3");

        // Store with long TTL
        storage.put(key1, DhtValue::new(b"data1".to_vec()).with_ttl(3600)).unwrap();

        // Store with 0 TTL (expired)
        storage.put(key2, DhtValue::new(b"data2".to_vec()).with_ttl(0)).unwrap();
        storage.put(key3, DhtValue::new(b"data3".to_vec()).with_ttl(0)).unwrap();

        assert_eq!(storage.size().unwrap(), 3);

        // Cleanup expired
        let removed = storage.cleanup_expired().unwrap();
        assert_eq!(removed, 2);
        assert_eq!(storage.size().unwrap(), 1);

        // Only key1 should remain
        assert!(storage.get(&key1).is_ok());
        assert!(storage.get(&key2).is_err());
        assert!(storage.get(&key3).is_err());
    }

    #[test]
    fn test_storage_refresh() {
        let storage = DhtStorage::new();
        let key = DhtKey::hash(b"test_key");
        let value = DhtValue::new(b"test_data".to_vec()).with_ttl(100);

        storage.put(key, value.clone()).unwrap();

        let before = storage.get(&key).unwrap();
        let before_timestamp = before.timestamp;

        // Wait a moment
        std::thread::sleep(std::time::Duration::from_millis(10));

        // Refresh with new TTL
        storage.refresh(&key, 200).unwrap();

        let after = storage.get(&key).unwrap();
        assert_eq!(after.ttl, 200);
        assert!(after.timestamp >= before_timestamp);
    }

    #[test]
    fn test_storage_replicas() {
        let storage = DhtStorage::new();
        let key = DhtKey::hash(b"test_key");
        let value = DhtValue::new(b"test_data".to_vec()).with_ttl(3600);

        storage.put(key, value).unwrap();

        let peer1 = DhtKey::hash(b"peer1");
        let peer2 = DhtKey::hash(b"peer2");
        let peer3 = DhtKey::hash(b"peer3");

        // Add replicas
        storage.add_replica(&key, peer1).unwrap();
        storage.add_replica(&key, peer2).unwrap();
        storage.add_replica(&key, peer3).unwrap();

        let replicas = storage.get_replicas(&key).unwrap();
        assert_eq!(replicas.len(), 3);
        assert!(replicas.contains(&peer1));
        assert!(replicas.contains(&peer2));
        assert!(replicas.contains(&peer3));

        // Remove a replica
        storage.remove_replica(&key, &peer2).unwrap();

        let replicas = storage.get_replicas(&key).unwrap();
        assert_eq!(replicas.len(), 2);
        assert!(replicas.contains(&peer1));
        assert!(!replicas.contains(&peer2));
        assert!(replicas.contains(&peer3));
    }

    #[test]
    fn test_storage_keys() {
        let storage = DhtStorage::new();

        let key1 = DhtKey::hash(b"key1");
        let key2 = DhtKey::hash(b"key2");
        let key3 = DhtKey::hash(b"key3");

        storage.put(key1, DhtValue::new(b"data1".to_vec()).with_ttl(3600)).unwrap();
        storage.put(key2, DhtValue::new(b"data2".to_vec()).with_ttl(3600)).unwrap();
        storage.put(key3, DhtValue::new(b"data3".to_vec()).with_ttl(0)).unwrap();

        let all_keys = storage.keys().unwrap();
        assert_eq!(all_keys.len(), 3);

        let active_keys = storage.active_keys().unwrap();
        assert_eq!(active_keys.len(), 2);
        assert!(active_keys.contains(&key1));
        assert!(active_keys.contains(&key2));
        assert!(!active_keys.contains(&key3));
    }

    #[test]
    fn test_storage_clear() {
        let storage = DhtStorage::new();

        storage
            .put(DhtKey::hash(b"key1"), DhtValue::new(b"data1".to_vec()).with_ttl(3600))
            .unwrap();
        storage
            .put(DhtKey::hash(b"key2"), DhtValue::new(b"data2".to_vec()).with_ttl(3600))
            .unwrap();

        assert_eq!(storage.size().unwrap(), 2);

        storage.clear().unwrap();
        assert_eq!(storage.size().unwrap(), 0);
    }

    #[test]
    fn test_storage_concurrent_access() {
        use std::sync::Arc;
        use std::thread;

        let storage = Arc::new(DhtStorage::new());
        let mut handles = vec![];

        // Spawn multiple threads writing different keys
        for i in 0..10 {
            let storage_clone = storage.clone();
            let handle = thread::spawn(move || {
                let key = DhtKey::hash(format!("key{}", i).as_bytes());
                let value = DhtValue::new(format!("data{}", i).as_bytes().to_vec()).with_ttl(3600);
                storage_clone.put(key, value).unwrap();
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(storage.size().unwrap(), 10);
    }
}
