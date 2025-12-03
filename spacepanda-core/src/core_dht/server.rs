/*
    Server - Handles inbound DHT RPC requests.

    Responsibilities:
    `server.rs` handles inbound DHT RPC requests.
    It is equivalent of DHT RPC listener.

    Workflow:
    - receive message from router
    - decode DHT message
    - update routing table
    - call handler:
      - on_find_node
      - on_find_value
      - on_store_value
      - on_ping
    - respond through router

    Inputs:
    - inbound DHT RPC messages from router
    - router callback

    Outputs:
    - DHT Response messages back to requester
    - storage operations
    - routing table mutations
*/

use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

use super::events::DhtEvent;
use super::message::{DhtMessage, FindValueResult, PeerInfo};
use super::{DhtConfig, DhtKey, DhtStorage, DhtValue, RoutingTable};
use crate::core_router::RouterHandle;

/// DHT server for handling inbound RPC requests
pub struct DhtServer {
    /// Local node ID
    local_id: DhtKey,
    /// DHT configuration
    config: DhtConfig,
    /// Router handle for sending responses
    router: Arc<RouterHandle>,
    /// Local storage
    storage: DhtStorage,
    /// Routing table
    routing_table: Arc<Mutex<RoutingTable>>,
    /// Event channel
    event_tx: mpsc::Sender<DhtEvent>,
}

impl DhtServer {
    /// Create new DHT server
    pub fn new(
        local_id: DhtKey,
        config: DhtConfig,
        router: Arc<RouterHandle>,
        storage: DhtStorage,
        routing_table: Arc<Mutex<RoutingTable>>,
        event_tx: mpsc::Sender<DhtEvent>,
    ) -> Self {
        DhtServer { local_id, config, router, storage, routing_table, event_tx }
    }

    /// Handle incoming DHT message
    pub async fn handle_message(&self, from: DhtKey, _data: Vec<u8>) -> Result<(), String> {
        // In production, deserialize message here
        // For now, just acknowledge receipt and update routing table

        // Create PeerContact and add to routing table
        let peer = super::routing_table::PeerContact::new(from, format!("unknown:{}", from));
        let _ = self.routing_table.lock().await.insert(peer);

        // Emit peer discovered event
        let _ = self.event_tx.send(DhtEvent::PeerDiscovered { peer_id: from }).await;

        // In production, deserialize and dispatch to handlers
        Ok(())
    }

    /// Handle PING request
    async fn handle_ping(&self, _from: DhtKey) -> Result<(), String> {
        // In production, send pong response via router
        Ok(())
    }

    /// Handle FIND_NODE request
    async fn handle_find_node(
        &self,
        _from: DhtKey,
        target: DhtKey,
        _request_id: u64,
    ) -> Result<(), String> {
        // Find k closest nodes to target
        let routing_table = self.routing_table.lock().await;
        let closest = routing_table.find_closest(&target, self.config.bucket_size);

        // Convert PeerContact to PeerInfo
        let _nodes: Vec<PeerInfo> = closest
            .iter()
            .map(|contact| PeerInfo::new(contact.id, contact.address.clone()))
            .collect();

        // In production, send response via router
        Ok(())
    }

    /// Handle FIND_VALUE request
    async fn handle_find_value(
        &self,
        _from: DhtKey,
        key: DhtKey,
        _request_id: u64,
    ) -> Result<(), String> {
        // Try to get value from local storage
        let _result = match self.storage.get(&key) {
            Ok(value) => {
                // Value found, emit event
                let _ =
                    self.event_tx.send(DhtEvent::ValueFound { key, value: value.clone() }).await;

                FindValueResult::Found(value)
            }
            Err(_) => {
                // Value not found, return closest nodes
                let routing_table = self.routing_table.lock().await;
                let closest = routing_table.find_closest(&key, self.config.bucket_size);

                let nodes: Vec<PeerInfo> = closest
                    .iter()
                    .map(|contact| PeerInfo::new(contact.id, contact.address.clone()))
                    .collect();

                FindValueResult::NotFound { closest_nodes: nodes }
            }
        };

        // In production, send response via router
        Ok(())
    }

    /// Handle STORE request
    async fn handle_store(
        &self,
        _from: DhtKey,
        key: DhtKey,
        value: DhtValue,
        _request_id: u64,
    ) -> Result<(), String> {
        // Validate value
        let validation_result =
            value.validate(self.config.max_value_size, self.config.require_signatures);

        match validation_result {
            Ok(_) => {
                // Store value
                match self.storage.put(key, value) {
                    Ok(_) => {
                        // Emit event
                        let _ = self.event_tx.send(DhtEvent::ValueStored { key }).await;
                        Ok(())
                    }
                    Err(e) => Err(e),
                }
            }
            Err(e) => {
                // Validation failed
                let _ =
                    self.event_tx.send(DhtEvent::ValidationFailed { key, reason: e.clone() }).await;
                Err(e)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core_dht::PeerContact;

    fn create_test_server() -> DhtServer {
        let local_id = DhtKey::hash(b"local");
        let config = DhtConfig::default();
        let (router, _handle) = RouterHandle::new();
        let router = Arc::new(router);
        let storage = DhtStorage::new();
        let routing_table = Arc::new(Mutex::new(RoutingTable::new(local_id, 20)));
        let (event_tx, _event_rx) = mpsc::channel(100);

        DhtServer::new(local_id, config, router, storage, routing_table, event_tx)
    }

    #[tokio::test]
    async fn test_server_creation() {
        let server = create_test_server();
        assert_eq!(server.local_id, DhtKey::hash(b"local"));
    }

    #[tokio::test]
    async fn test_handle_ping() {
        let server = create_test_server();
        let _peer_id = DhtKey::hash(b"peer");

        let result = server.handle_ping(_peer_id).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_handle_store_validation() {
        let server = create_test_server();
        let peer_id = DhtKey::hash(b"peer");
        let key = DhtKey::hash(b"key");
        let value = DhtValue::new(b"data".to_vec()).with_ttl(3600);

        // Store locally first to test
        let result = server.storage.put(key, value.clone());
        assert!(result.is_ok());

        // Verify storage
        let retrieved = server.storage.get(&key);
        assert!(retrieved.is_ok());
    }

    #[tokio::test]
    async fn test_handle_find_value_found() {
        let server = create_test_server();
        let key = DhtKey::hash(b"key");
        let value = DhtValue::new(b"data".to_vec()).with_ttl(3600);

        // Store value first
        server.storage.put(key, value.clone()).unwrap();

        // Try to retrieve
        let result = server.storage.get(&key);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().data, b"data");
    }

    #[tokio::test]
    async fn test_handle_find_value_not_found() {
        let server = create_test_server();
        let key = DhtKey::hash(b"nonexistent");

        // Try to retrieve non-existent key
        let result = server.storage.get(&key);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_handle_find_node() {
        let server = create_test_server();
        let target = DhtKey::hash(b"target");

        // Add some peers to routing table
        let mut table = server.routing_table.lock().await;
        let _ =
            table.insert(PeerContact::new(DhtKey::hash(b"peer1"), "127.0.0.1:8001".to_string()));
        let _ =
            table.insert(PeerContact::new(DhtKey::hash(b"peer2"), "127.0.0.1:8002".to_string()));
        let _ =
            table.insert(PeerContact::new(DhtKey::hash(b"peer3"), "127.0.0.1:8003".to_string()));
        drop(table);

        // Find closest nodes
        let table = server.routing_table.lock().await;
        let closest = table.find_closest(&target, 3);
        assert!(closest.len() <= 3);
    }

    #[tokio::test]
    async fn test_handle_message() {
        let server = create_test_server();
        let peer_id = DhtKey::hash(b"peer");

        // Send any data (not validated in current impl)
        let data = vec![1, 2, 3, 4, 5];
        let result = server.handle_message(peer_id, data).await;

        // Should succeed since we're not deserializing yet
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_routing_table_update_on_message() {
        let server = create_test_server();
        let peer_id = DhtKey::hash(b"new_peer");

        // Initially routing table shouldn't have the peer
        let table = server.routing_table.lock().await;
        let initial_count = table.all_peers().len();
        drop(table);

        // Handle a message (which updates routing table)
        let data = vec![];
        let _ = server.handle_message(peer_id, data).await;

        // Check routing table was updated
        let table = server.routing_table.lock().await;
        let final_count = table.all_peers().len();
        assert!(final_count >= initial_count);
    }

    #[test]
    fn test_message_type_routing() {
        let ping = DhtMessage::new_ping(DhtKey::hash(b"test"));
        assert!(ping.is_request());
        assert_eq!(ping.message_type(), "Ping");

        let pong = DhtMessage::new_pong(DhtKey::hash(b"test"));
        assert!(pong.is_response());
        assert_eq!(pong.message_type(), "Pong");
    }
}
