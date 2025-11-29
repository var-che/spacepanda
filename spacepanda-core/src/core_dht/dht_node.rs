/*
    DhtNode - represents a full DHT participant node.
    Coordinates routing table, replication, searching, storing, messaging.

    Responsibilities:
    `dht_node.rs` is the brain of the DHT subsystem.

    A DhtNode instance manages:
    - owns the local node's DHT ID
    - holds the DHT routing table
    - exposes operations like: put(key, value), get(key), find_node(node_id), replicate()
    - communicates with the router layer to send RPC DHT messages
    - listens to incoming DHT RPC messages
    - updates routing table
    - ensures DHT config (replication factor, alpha concurrency, timeouts, etc) is respected
    - performs periodic maintenance tasks (replication, refreshing buckets, etc)

    Inputs:
    - request from the upper application: GetValue(key), PutValue(key, value), FindNode(node_id)
    - DHT messages from remote peers
    - timer events for refresh/replication
    - router events (peer discovered, peer disconnected, etc)

    Outputs:
    - RPC requests to remote peers via the router layer
    - updates in routing table
    - responses to application-level API calls
    - events (VALUE_FOUND, VALUE_STORED, SEARCH_FAILED, etc)
    - logs for monitoring and debugging
*/

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::{mpsc, oneshot, Mutex};
use tokio::time::{interval, Duration};

use super::dht_config::DhtConfig;
use super::dht_key::DhtKey;
use super::dht_value::DhtValue;

/// K-bucket entry representing a known peer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BucketEntry {
    pub node_id: DhtKey,
    pub last_seen: u64,
    pub rtt_ms: Option<u64>,
}

impl BucketEntry {
    fn new(node_id: DhtKey) -> Self {
        let now = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        BucketEntry {
            node_id,
            last_seen: now,
            rtt_ms: None,
        }
    }
    
    fn update_seen(&mut self) {
        self.last_seen = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
    }
}

/// Routing table using Kademlia k-buckets
#[derive(Debug)]
pub struct RoutingTable {
    local_id: DhtKey,
    buckets: Vec<Vec<BucketEntry>>,
    bucket_size: usize,
}

impl RoutingTable {
    fn new(local_id: DhtKey, bucket_size: usize, num_buckets: usize) -> Self {
        RoutingTable {
            local_id,
            buckets: vec![Vec::new(); num_buckets],
            bucket_size,
        }
    }
    
    /// Add or update a node in the routing table
    fn add_node(&mut self, node_id: DhtKey) -> bool {
        if node_id == self.local_id {
            return false; // Don't add self
        }
        
        let bucket_idx = node_id.bucket_index(&self.local_id);
        let bucket = &mut self.buckets[bucket_idx];
        
        // Check if already exists
        if let Some(entry) = bucket.iter_mut().find(|e| e.node_id == node_id) {
            entry.update_seen();
            return true;
        }
        
        // Add if bucket has space
        if bucket.len() < self.bucket_size {
            bucket.push(BucketEntry::new(node_id));
            return true;
        }
        
        false
    }
    
    /// Find the k closest nodes to a target key
    fn find_closest(&self, target: &DhtKey, k: usize) -> Vec<DhtKey> {
        let mut all_nodes: Vec<_> = self.buckets
            .iter()
            .flat_map(|bucket| bucket.iter())
            .map(|entry| entry.node_id)
            .collect();
        
        // Sort by distance to target
        all_nodes.sort_by(|a, b| {
            let dist_a = a.distance(target);
            let dist_b = b.distance(target);
            dist_a.cmp(&dist_b)
        });
        
        all_nodes.into_iter().take(k).collect()
    }
    
    /// Get all nodes
    fn all_nodes(&self) -> Vec<DhtKey> {
        self.buckets
            .iter()
            .flat_map(|bucket| bucket.iter())
            .map(|entry| entry.node_id)
            .collect()
    }
    
    /// Remove a node
    fn remove_node(&mut self, node_id: &DhtKey) -> bool {
        let bucket_idx = node_id.bucket_index(&self.local_id);
        let bucket = &mut self.buckets[bucket_idx];
        
        if let Some(pos) = bucket.iter().position(|e| &e.node_id == node_id) {
            bucket.remove(pos);
            return true;
        }
        
        false
    }
    
    /// Get total number of nodes
    fn node_count(&self) -> usize {
        self.buckets.iter().map(|b| b.len()).sum()
    }
}

/// DHT RPC messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DhtMessage {
    /// Ping request
    Ping { sender_id: DhtKey },
    /// Pong response
    Pong { sender_id: DhtKey },
    /// Find node request
    FindNode { sender_id: DhtKey, target: DhtKey },
    /// Find node response
    FindNodeResponse { nodes: Vec<DhtKey> },
    /// Store value request
    Store { sender_id: DhtKey, key: DhtKey, value: DhtValue },
    /// Store acknowledgment
    StoreAck { success: bool },
    /// Get value request
    GetValue { sender_id: DhtKey, key: DhtKey },
    /// Get value response
    GetValueResponse { value: Option<DhtValue>, closest_nodes: Vec<DhtKey> },
}

/// Commands for DhtNode
#[derive(Debug)]
pub enum DhtCommand {
    /// Put a value in the DHT
    Put {
        key: DhtKey,
        value: DhtValue,
        response_tx: oneshot::Sender<Result<(), String>>,
    },
    /// Get a value from the DHT
    Get {
        key: DhtKey,
        response_tx: oneshot::Sender<Result<Option<DhtValue>, String>>,
    },
    /// Find nodes closest to a key
    FindNode {
        target: DhtKey,
        response_tx: oneshot::Sender<Result<Vec<DhtKey>, String>>,
    },
    /// Handle incoming DHT message
    HandleMessage {
        from: DhtKey,
        message: DhtMessage,
    },
    /// Bootstrap with a known node
    Bootstrap {
        node_id: DhtKey,
    },
    /// Trigger maintenance tasks
    Maintenance,
    /// Shutdown
    Shutdown,
}

/// Events emitted by DhtNode
#[derive(Debug, Clone)]
pub enum DhtEvent {
    /// Value successfully stored
    ValueStored { key: DhtKey },
    /// Value found
    ValueFound { key: DhtKey, value: DhtValue },
    /// Search failed
    SearchFailed { key: DhtKey, reason: String },
    /// Node discovered
    NodeDiscovered { node_id: DhtKey },
    /// Node removed (timed out)
    NodeRemoved { node_id: DhtKey },
}

/// DHT Node
pub struct DhtNode {
    config: DhtConfig,
    local_id: DhtKey,
    routing_table: Arc<Mutex<RoutingTable>>,
    storage: Arc<Mutex<HashMap<DhtKey, DhtValue>>>,
    event_tx: mpsc::Sender<DhtEvent>,
}

impl DhtNode {
    /// Create a new DHT node
    pub fn new(
        local_id: DhtKey,
        config: DhtConfig,
        event_tx: mpsc::Sender<DhtEvent>,
    ) -> Result<Self, String> {
        config.validate()?;
        
        let routing_table = Arc::new(Mutex::new(RoutingTable::new(
            local_id,
            config.bucket_size,
            config.num_buckets,
        )));
        
        let storage = Arc::new(Mutex::new(HashMap::new()));
        
        Ok(DhtNode {
            config,
            local_id,
            routing_table,
            storage,
            event_tx,
        })
    }
    
    /// Get the local node ID
    pub fn local_id(&self) -> DhtKey {
        self.local_id
    }
    
    /// Start the DHT node event loop
    pub async fn run(
        self: Arc<Self>,
        mut command_rx: mpsc::Receiver<DhtCommand>,
    ) {
        // Spawn maintenance task
        let node = self.clone();
        tokio::spawn(async move {
            let mut tick = interval(node.config.bucket_refresh_interval);
            loop {
                tick.tick().await;
                node.perform_maintenance().await;
            }
        });
        
        loop {
            tokio::select! {
                Some(cmd) = command_rx.recv() => {
                    match cmd {
                        DhtCommand::Put { key, value, response_tx } => {
                            let result = self.handle_put(key, value).await;
                            let _ = response_tx.send(result);
                        }
                        DhtCommand::Get { key, response_tx } => {
                            let result = self.handle_get(key).await;
                            let _ = response_tx.send(result);
                        }
                        DhtCommand::FindNode { target, response_tx } => {
                            let result = self.handle_find_node(target).await;
                            let _ = response_tx.send(result);
                        }
                        DhtCommand::HandleMessage { from, message } => {
                            self.handle_message(from, message).await;
                        }
                        DhtCommand::Bootstrap { node_id } => {
                            self.handle_bootstrap(node_id).await;
                        }
                        DhtCommand::Maintenance => {
                            self.perform_maintenance().await;
                        }
                        DhtCommand::Shutdown => {
                            break;
                        }
                    }
                }
            }
        }
    }
    
    /// Handle PUT operation
    async fn handle_put(&self, key: DhtKey, value: DhtValue) -> Result<(), String> {
        // Validate value
        value.validate(self.config.max_value_size, self.config.require_signatures)?;
        
        // Find k closest nodes
        let _closest = self.routing_table
            .lock()
            .await
            .find_closest(&key, self.config.replication_factor);
        
        // Store locally (we're always one of the nodes responsible for storage)
        self.storage.lock().await.insert(key, value.clone());
        
        // TODO: Send STORE messages to closest nodes via router
        // For now, just emit success event
        let _ = self.event_tx.send(DhtEvent::ValueStored { key }).await;
        
        Ok(())
    }
    
    /// Handle GET operation
    async fn handle_get(&self, key: DhtKey) -> Result<Option<DhtValue>, String> {
        // Check local storage first
        if let Some(value) = self.storage.lock().await.get(&key) {
            if !value.is_expired() {
                let _ = self.event_tx
                    .send(DhtEvent::ValueFound {
                        key,
                        value: value.clone(),
                    })
                    .await;
                return Ok(Some(value.clone()));
            }
        }
        
        // TODO: Perform iterative lookup via router
        // For now, return not found
        let _ = self.event_tx
            .send(DhtEvent::SearchFailed {
                key,
                reason: "Value not found locally".to_string(),
            })
            .await;
        
        Ok(None)
    }
    
    /// Handle FIND_NODE operation
    async fn handle_find_node(&self, target: DhtKey) -> Result<Vec<DhtKey>, String> {
        Ok(self.routing_table
            .lock()
            .await
            .find_closest(&target, self.config.bucket_size))
    }
    
    /// Handle incoming DHT message
    async fn handle_message(&self, from: DhtKey, message: DhtMessage) {
        // Add sender to routing table
        self.routing_table.lock().await.add_node(from);
        
        match message {
            DhtMessage::Ping { .. } => {
                // TODO: Send Pong response via router
            }
            DhtMessage::Pong { .. } => {
                // Update routing table
            }
            DhtMessage::FindNode { target, .. } => {
                let _closest = self.routing_table
                    .lock()
                    .await
                    .find_closest(&target, self.config.bucket_size);
                // TODO: Send FindNodeResponse via router
            }
            DhtMessage::Store { key, value, .. } => {
                // Validate and store
                if value.validate(self.config.max_value_size, self.config.require_signatures).is_ok() {
                    self.storage.lock().await.insert(key, value);
                    // TODO: Send StoreAck via router
                }
            }
            DhtMessage::GetValue { key, .. } => {
                let _stored_value = self.storage.lock().await.get(&key).cloned();
                let _closest = self.routing_table
                    .lock()
                    .await
                    .find_closest(&key, self.config.bucket_size);
                // TODO: Send GetValueResponse via router
            }
            _ => {
                // Handle other message types
            }
        }
    }
    
    /// Handle bootstrap
    async fn handle_bootstrap(&self, node_id: DhtKey) {
        self.routing_table.lock().await.add_node(node_id);
        
        // TODO: Perform FIND_NODE for self.local_id to populate routing table
        let _ = self.event_tx
            .send(DhtEvent::NodeDiscovered { node_id })
            .await;
    }
    
    /// Perform periodic maintenance
    async fn perform_maintenance(&self) {
        // Remove expired values
        let mut storage = self.storage.lock().await;
        storage.retain(|_, value| !value.is_expired());
        
        // TODO: Refresh buckets, republish values, etc.
    }
    
    /// Get routing table statistics
    pub async fn routing_table_stats(&self) -> (usize, Vec<usize>) {
        let table = self.routing_table.lock().await;
        let total = table.node_count();
        let per_bucket: Vec<usize> = table.buckets.iter().map(|b| b.len()).collect();
        (total, per_bucket)
    }
    
    /// Get storage statistics
    pub async fn storage_stats(&self) -> (usize, usize) {
        let storage = self.storage.lock().await;
        let total = storage.len();
        let expired = storage.values().filter(|v| v.is_expired()).count();
        (total, expired)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bucket_entry_new() {
        let node_id = DhtKey::hash_string("test_node");
        let entry = BucketEntry::new(node_id);
        
        assert_eq!(entry.node_id, node_id);
        assert!(entry.last_seen > 0);
        assert!(entry.rtt_ms.is_none());
    }

    #[test]
    fn test_routing_table_new() {
        let local_id = DhtKey::hash_string("local");
        let table = RoutingTable::new(local_id, 20, 256);
        
        assert_eq!(table.local_id, local_id);
        assert_eq!(table.buckets.len(), 256);
        assert_eq!(table.bucket_size, 20);
    }

    #[test]
    fn test_routing_table_add_node() {
        let local_id = DhtKey::hash_string("local");
        let mut table = RoutingTable::new(local_id, 20, 256);
        
        let node1 = DhtKey::hash_string("node1");
        assert!(table.add_node(node1));
        assert_eq!(table.node_count(), 1);
        
        // Adding same node again should update
        assert!(table.add_node(node1));
        assert_eq!(table.node_count(), 1);
    }

    #[test]
    fn test_routing_table_add_self() {
        let local_id = DhtKey::hash_string("local");
        let mut table = RoutingTable::new(local_id, 20, 256);
        
        // Should not add self
        assert!(!table.add_node(local_id));
        assert_eq!(table.node_count(), 0);
    }

    #[test]
    fn test_routing_table_find_closest() {
        let local_id = DhtKey::hash_string("local");
        let mut table = RoutingTable::new(local_id, 20, 256);
        
        // Add some nodes
        for i in 0..10 {
            table.add_node(DhtKey::hash_string(&format!("node{}", i)));
        }
        
        let target = DhtKey::hash_string("target");
        let closest = table.find_closest(&target, 5);
        
        assert_eq!(closest.len(), 5);
    }

    #[test]
    fn test_routing_table_remove_node() {
        let local_id = DhtKey::hash_string("local");
        let mut table = RoutingTable::new(local_id, 20, 256);
        
        let node1 = DhtKey::hash_string("node1");
        table.add_node(node1);
        assert_eq!(table.node_count(), 1);
        
        assert!(table.remove_node(&node1));
        assert_eq!(table.node_count(), 0);
    }

    #[tokio::test]
    async fn test_dht_node_creation() {
        let local_id = DhtKey::hash_string("test_node");
        let config = DhtConfig::test_config();
        let (event_tx, _event_rx) = mpsc::channel(100);
        
        let node = DhtNode::new(local_id, config, event_tx);
        assert!(node.is_ok());
        
        let node = node.unwrap();
        assert_eq!(node.local_id(), local_id);
    }

    #[tokio::test]
    async fn test_dht_node_put_get() {
        let local_id = DhtKey::hash_string("test_node");
        let config = DhtConfig::test_config();
        let (event_tx, mut event_rx) = mpsc::channel(100);
        
        let node = Arc::new(DhtNode::new(local_id, config, event_tx).unwrap());
        let (cmd_tx, cmd_rx) = mpsc::channel(100);
        
        // Spawn node
        let node_clone = node.clone();
        tokio::spawn(async move {
            node_clone.run(cmd_rx).await;
        });
        
        // Bootstrap with a fake node to populate routing table
        cmd_tx.send(DhtCommand::Bootstrap {
            node_id: local_id, // Self for testing
        }).await.unwrap();
        
        // Wait for bootstrap event
        tokio::time::sleep(Duration::from_millis(10)).await;
        
        // Put a value
        let key = DhtKey::hash_string("test_key");
        let value = DhtValue::new(b"test_data".to_vec());
        
        let (response_tx, response_rx) = oneshot::channel();
        cmd_tx.send(DhtCommand::Put {
            key,
            value: value.clone(),
            response_tx,
        }).await.unwrap();
        
        let result = response_rx.await.unwrap();
        assert!(result.is_ok());
        
        // Should receive events (NodeDiscovered and ValueStored)
        // Skip NodeDiscovered event
        let _node_discovered = tokio::time::timeout(Duration::from_millis(100), event_rx.recv())
            .await
            .unwrap();
        
        // Get ValueStored event
        let event = tokio::time::timeout(Duration::from_millis(100), event_rx.recv())
            .await
            .unwrap()
            .unwrap();
        
        match event {
            DhtEvent::ValueStored { key: stored_key } => {
                assert_eq!(stored_key, key);
            }
            _ => panic!("Expected ValueStored event, got {:?}", event),
        }
        
        // Get the value
        let (response_tx, response_rx) = oneshot::channel();
        cmd_tx.send(DhtCommand::Get {
            key,
            response_tx,
        }).await.unwrap();
        
        let result = response_rx.await.unwrap();
        assert!(result.is_ok());
        
        let retrieved = result.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().data, value.data);
        
        // Shutdown
        cmd_tx.send(DhtCommand::Shutdown).await.unwrap();
    }

    #[tokio::test]
    async fn test_dht_node_find_node() {
        let local_id = DhtKey::hash_string("test_node");
        let config = DhtConfig::test_config();
        let (event_tx, _event_rx) = mpsc::channel(100);
        
        let node = Arc::new(DhtNode::new(local_id, config, event_tx).unwrap());
        let (cmd_tx, cmd_rx) = mpsc::channel(100);
        
        // Spawn node
        let node_clone = node.clone();
        tokio::spawn(async move {
            node_clone.run(cmd_rx).await;
        });
        
        // Bootstrap with some nodes
        for i in 0..5 {
            cmd_tx.send(DhtCommand::Bootstrap {
                node_id: DhtKey::hash_string(&format!("node{}", i)),
            }).await.unwrap();
        }
        
        tokio::time::sleep(Duration::from_millis(10)).await;
        
        // Find nodes
        let target = DhtKey::hash_string("target");
        let (response_tx, response_rx) = oneshot::channel();
        cmd_tx.send(DhtCommand::FindNode {
            target,
            response_tx,
        }).await.unwrap();
        
        let result = response_rx.await.unwrap();
        assert!(result.is_ok());
        
        let nodes = result.unwrap();
        assert!(!nodes.is_empty());
        
        // Shutdown
        cmd_tx.send(DhtCommand::Shutdown).await.unwrap();
    }

    #[tokio::test]
    async fn test_dht_node_routing_table_stats() {
        let local_id = DhtKey::hash_string("test_node");
        let config = DhtConfig::test_config();
        let (event_tx, _event_rx) = mpsc::channel(100);
        
        let node = Arc::new(DhtNode::new(local_id, config, event_tx).unwrap());
        
        // Add some nodes directly
        for i in 0..5 {
            node.routing_table.lock().await.add_node(
                DhtKey::hash_string(&format!("node{}", i))
            );
        }
        
        let (total, _per_bucket) = node.routing_table_stats().await;
        assert_eq!(total, 5);
    }

    #[tokio::test]
    async fn test_dht_node_storage_stats() {
        let local_id = DhtKey::hash_string("test_node");
        let config = DhtConfig::test_config();
        let (event_tx, _event_rx) = mpsc::channel(100);
        
        let node = Arc::new(DhtNode::new(local_id, config, event_tx).unwrap());
        
        // Add some values
        let key1 = DhtKey::hash_string("key1");
        let value1 = DhtValue::new(vec![1, 2, 3]);
        node.storage.lock().await.insert(key1, value1);
        
        let (total, _expired) = node.storage_stats().await;
        assert_eq!(total, 1);
    }

    #[test]
    fn test_dht_message_serialization() {
        let msg = DhtMessage::Ping {
            sender_id: DhtKey::hash_string("sender"),
        };
        
        let serialized = serde_json::to_string(&msg).unwrap();
        let deserialized: DhtMessage = serde_json::from_str(&serialized).unwrap();
        
        match deserialized {
            DhtMessage::Ping { sender_id } => {
                assert_eq!(sender_id, DhtKey::hash_string("sender"));
            }
            _ => panic!("Wrong message type"),
        }
    }
}
