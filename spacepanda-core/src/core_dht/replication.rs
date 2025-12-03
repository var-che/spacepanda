/*
    Replication - periodic replication and republishing of DHT key-value pairs

    Responsibilities:
    `replication.rs` implements the periodic replication and republishing of DHT key-value pairs.
    It handles:
    - publishes original values you PUT
    - refresh keys
    - pushes replicas to nearest nodes
    - ensures redundancy level K
    - garbage collection of expired keys

    Inputs:
    - timer events
    - storage list of stored keys

    outputs:
    - PUT RPC messages
    - storage updates
    - replication logs

*/

use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;
use tokio::time::interval;

use super::dht_config::DhtConfig;
use super::dht_key::DhtKey;
use super::dht_storage::DhtStorage;
use super::dht_value::DhtValue;
use super::routing_table::{PeerContact, RoutingTable};

/// Replication event
#[derive(Debug, Clone)]
pub enum ReplicationEvent {
    /// Need to replicate a value to peers
    ReplicateValue { key: DhtKey, value: DhtValue, peers: Vec<PeerContact> },
    /// Need to refresh a key
    RefreshKey { key: DhtKey },
    /// Expired keys garbage collected
    GarbageCollected { count: usize },
}

/// Tracks replication state for a key
#[derive(Debug, Clone)]
struct ReplicationState {
    /// When this key was last replicated
    last_replicated: u64,
    /// Peers that have this replica
    replica_peers: HashSet<DhtKey>,
    /// Whether this is an original value we published
    is_original: bool,
}

impl ReplicationState {
    fn new(is_original: bool) -> Self {
        ReplicationState {
            last_replicated: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            replica_peers: HashSet::new(),
            is_original,
        }
    }

    fn needs_replication(&self, interval_secs: u64) -> bool {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);

        (now - self.last_replicated) > interval_secs
    }

    fn mark_replicated(&mut self) {
        self.last_replicated =
            SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    }

    fn add_replica_peer(&mut self, peer: DhtKey) {
        self.replica_peers.insert(peer);
    }
}

/// Manages replication and republishing
pub struct ReplicationManager {
    /// DHT configuration
    config: DhtConfig,
    /// Local storage
    storage: DhtStorage,
    /// Routing table (for finding replica nodes)
    routing_table: Arc<Mutex<RoutingTable>>,
    /// Replication state per key
    replication_state: Arc<Mutex<std::collections::HashMap<DhtKey, ReplicationState>>>,
    /// Event channel
    event_tx: tokio::sync::mpsc::Sender<ReplicationEvent>,
}

impl ReplicationManager {
    /// Create a new replication manager
    pub fn new(
        config: DhtConfig,
        storage: DhtStorage,
        routing_table: Arc<Mutex<RoutingTable>>,
        event_tx: tokio::sync::mpsc::Sender<ReplicationEvent>,
    ) -> Self {
        ReplicationManager {
            config,
            storage,
            routing_table,
            replication_state: Arc::new(Mutex::new(std::collections::HashMap::new())),
            event_tx,
        }
    }

    /// Start the replication loop
    pub async fn run(self: Arc<Self>) {
        let mut replication_tick = interval(self.config.republish_interval);
        let mut gc_tick = interval(Duration::from_secs(300)); // GC every 5 minutes

        loop {
            tokio::select! {
                _ = replication_tick.tick() => {
                    if let Err(e) = self.do_replication().await {
                        eprintln!("Replication error: {}", e);
                    }
                }
                _ = gc_tick.tick() => {
                    if let Err(e) = self.do_garbage_collection().await {
                        eprintln!("Garbage collection error: {}", e);
                    }
                }
            }
        }
    }

    /// Perform replication round
    async fn do_replication(&self) -> Result<(), String> {
        let keys = self.storage.active_keys()?;

        for key in keys {
            if let Ok(value) = self.storage.get(&key) {
                // Check if this key needs replication
                let mut states = self.replication_state.lock().await;
                let state = states.entry(key).or_insert_with(|| ReplicationState::new(false));

                let interval = if state.is_original {
                    self.config.republish_interval.as_secs()
                } else {
                    // For cached values, use half the republish interval
                    self.config.republish_interval.as_secs() / 2
                };

                if state.needs_replication(interval) {
                    // Find k closest nodes to this key
                    let routing_table = self.routing_table.lock().await;
                    let closest_peers = routing_table.find_closest(&key, self.config.bucket_size);
                    drop(routing_table);

                    // Filter out peers that already have the replica
                    let peers_to_replicate: Vec<PeerContact> = closest_peers
                        .into_iter()
                        .filter(|p| !state.replica_peers.contains(&p.id))
                        .collect();

                    if !peers_to_replicate.is_empty() {
                        // Emit replication event
                        let _ = self
                            .event_tx
                            .send(ReplicationEvent::ReplicateValue {
                                key,
                                value: value.clone(),
                                peers: peers_to_replicate.clone(),
                            })
                            .await;

                        // Mark peers as having replica
                        for peer in peers_to_replicate {
                            state.add_replica_peer(peer.id);
                        }
                    }

                    state.mark_replicated();
                }
            }
        }

        Ok(())
    }

    /// Perform garbage collection
    async fn do_garbage_collection(&self) -> Result<(), String> {
        let removed = self.storage.cleanup_expired()?;

        if removed > 0 {
            // Clean up replication state for removed keys
            let keys = self.storage.keys()?;
            let mut states = self.replication_state.lock().await;
            states.retain(|k, _| keys.contains(k));

            let _ = self.event_tx.send(ReplicationEvent::GarbageCollected { count: removed }).await;
        }

        Ok(())
    }

    /// Mark a key as original (we published it)
    pub async fn mark_original(&self, key: DhtKey) {
        let mut states = self.replication_state.lock().await;
        let state = states.entry(key).or_insert_with(|| ReplicationState::new(true));
        state.is_original = true;
    }

    /// Mark that a peer now has a replica of a key
    pub async fn mark_peer_has_replica(&self, key: &DhtKey, peer_id: DhtKey) {
        let mut states = self.replication_state.lock().await;
        let state = states.entry(*key).or_insert_with(|| ReplicationState::new(false));
        state.add_replica_peer(peer_id);

        // Also update storage
        let _ = self.storage.add_replica(key, peer_id);
    }

    /// Get replication statistics
    pub async fn stats(&self) -> ReplicationStats {
        let states = self.replication_state.lock().await;

        let total_keys = states.len();
        let original_keys = states.values().filter(|s| s.is_original).count();
        let cached_keys = total_keys - original_keys;
        let total_replicas: usize = states.values().map(|s| s.replica_peers.len()).sum();

        ReplicationStats { total_keys, original_keys, cached_keys, total_replicas }
    }
}

/// Replication statistics
#[derive(Debug, Clone)]
pub struct ReplicationStats {
    /// Total number of keys
    pub total_keys: usize,
    /// Keys we originally published
    pub original_keys: usize,
    /// Keys we're caching for others
    pub cached_keys: usize,
    /// Total replica peers across all keys
    pub total_replicas: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_routing_table() -> Arc<Mutex<RoutingTable>> {
        let local_id = DhtKey::hash(b"local");
        let table = RoutingTable::new(local_id, 20);
        Arc::new(Mutex::new(table))
    }

    #[tokio::test]
    async fn test_replication_manager_creation() {
        let config = DhtConfig::default();
        let storage = DhtStorage::new();
        let routing_table = create_test_routing_table();
        let (event_tx, _event_rx) = tokio::sync::mpsc::channel(100);

        let _manager = ReplicationManager::new(config, storage, routing_table, event_tx);
    }

    #[tokio::test]
    async fn test_mark_original() {
        let config = DhtConfig::default();
        let storage = DhtStorage::new();
        let routing_table = create_test_routing_table();
        let (event_tx, _event_rx) = tokio::sync::mpsc::channel(100);

        let manager = ReplicationManager::new(config, storage, routing_table, event_tx);

        let key = DhtKey::hash(b"test_key");
        manager.mark_original(key).await;

        let states = manager.replication_state.lock().await;
        assert!(states.get(&key).unwrap().is_original);
    }

    #[tokio::test]
    async fn test_mark_peer_has_replica() {
        let config = DhtConfig::default();
        let storage = DhtStorage::new();
        let routing_table = create_test_routing_table();
        let (event_tx, _event_rx) = tokio::sync::mpsc::channel(100);

        let manager = ReplicationManager::new(config, storage.clone(), routing_table, event_tx);

        let key = DhtKey::hash(b"test_key");
        let value = DhtValue::new(b"test_data".to_vec()).with_ttl(3600);
        storage.put(key, value).unwrap();

        let peer_id = DhtKey::hash(b"peer1");
        manager.mark_peer_has_replica(&key, peer_id).await;

        let states = manager.replication_state.lock().await;
        assert!(states.get(&key).unwrap().replica_peers.contains(&peer_id));
    }

    #[tokio::test]
    async fn test_replication_stats() {
        let config = DhtConfig::default();
        let storage = DhtStorage::new();
        let routing_table = create_test_routing_table();
        let (event_tx, _event_rx) = tokio::sync::mpsc::channel(100);

        let manager = ReplicationManager::new(config, storage.clone(), routing_table, event_tx);

        // Add some keys
        let key1 = DhtKey::hash(b"key1");
        let key2 = DhtKey::hash(b"key2");

        manager.mark_original(key1).await;
        manager.mark_peer_has_replica(&key1, DhtKey::hash(b"peer1")).await;
        manager.mark_peer_has_replica(&key1, DhtKey::hash(b"peer2")).await;

        let peer_id = DhtKey::hash(b"peer3");
        manager.mark_peer_has_replica(&key2, peer_id).await;

        let stats = manager.stats().await;
        assert_eq!(stats.total_keys, 2);
        assert_eq!(stats.original_keys, 1);
        assert_eq!(stats.cached_keys, 1);
    }

    #[tokio::test]
    async fn test_garbage_collection() {
        let config = DhtConfig::default();
        let storage = DhtStorage::new();
        let routing_table = create_test_routing_table();
        let (event_tx, mut event_rx) = tokio::sync::mpsc::channel(100);

        let manager =
            Arc::new(ReplicationManager::new(config, storage.clone(), routing_table, event_tx));

        // Add expired keys
        storage
            .put(DhtKey::hash(b"key1"), DhtValue::new(b"data1".to_vec()).with_ttl(0))
            .unwrap();
        storage
            .put(DhtKey::hash(b"key2"), DhtValue::new(b"data2".to_vec()).with_ttl(0))
            .unwrap();

        // Run GC
        manager.do_garbage_collection().await.unwrap();

        // Should receive GC event
        if let Some(event) = event_rx.try_recv().ok() {
            match event {
                ReplicationEvent::GarbageCollected { count } => {
                    assert_eq!(count, 2);
                }
                _ => panic!("Expected GarbageCollected event"),
            }
        }
    }

    #[tokio::test]
    async fn test_replication_round() {
        let config = DhtConfig::default();
        let storage = DhtStorage::new();
        let routing_table = create_test_routing_table();
        let (event_tx, mut event_rx) = tokio::sync::mpsc::channel(100);

        // Add some peers to routing table
        {
            let mut table = routing_table.lock().await;
            for i in 1..=5 {
                let peer = PeerContact::new(
                    DhtKey::hash(format!("peer{}", i).as_bytes()),
                    format!("127.0.0.1:800{}", i),
                );
                let _ = table.insert(peer);
            }
        }

        let manager =
            Arc::new(ReplicationManager::new(config, storage.clone(), routing_table, event_tx));

        // Add a key to storage and mark as original
        let key = DhtKey::hash(b"test_key");
        let value = DhtValue::new(b"test_data".to_vec()).with_ttl(3600);
        storage.put(key, value.clone()).unwrap();
        manager.mark_original(key).await;

        // Force immediate replication by setting last_replicated to 0
        {
            let mut states = manager.replication_state.lock().await;
            states.get_mut(&key).unwrap().last_replicated = 0;
        }

        // Run replication
        manager.do_replication().await.unwrap();

        // Should receive replication event
        if let Some(event) = event_rx.try_recv().ok() {
            match event {
                ReplicationEvent::ReplicateValue { key: k, value: v, peers } => {
                    assert_eq!(k, key);
                    assert_eq!(v.data, value.data);
                    assert!(!peers.is_empty());
                }
                _ => panic!("Expected ReplicateValue event"),
            }
        }
    }

    #[test]
    fn test_replication_state_needs_replication() {
        let mut state = ReplicationState::new(false);

        // Fresh state doesn't need replication
        assert!(!state.needs_replication(60));

        // Old state needs replication
        state.last_replicated = 0;
        assert!(state.needs_replication(60));
    }

    #[test]
    fn test_replication_state_replica_peers() {
        let mut state = ReplicationState::new(false);

        let peer1 = DhtKey::hash(b"peer1");
        let peer2 = DhtKey::hash(b"peer2");

        state.add_replica_peer(peer1);
        state.add_replica_peer(peer2);

        assert_eq!(state.replica_peers.len(), 2);
        assert!(state.replica_peers.contains(&peer1));
        assert!(state.replica_peers.contains(&peer2));
    }

    #[test]
    fn test_replication_state_mark_replicated() {
        let mut state = ReplicationState::new(false);
        let before = state.last_replicated;

        std::thread::sleep(std::time::Duration::from_millis(1001));
        state.mark_replicated();

        assert!(state.last_replicated > before);
    }
}
