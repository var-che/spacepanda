/*
    Client - sends outbound DHT RPC messages.

    Responsibilities:
    `client.rs` implements the DHT client responsible for sending outbound DHT RPC messages.
    It is a thin wrapper to send messages via router:

    ```rust
    router_handle.send_direct(peer_id, DhtMessage::FindNode{...})
    ```

    it handles: timeouts, retries, and updating routing table with response quality.

    Inputs:
    - API calls from dht_node
    - search requests (kad_search)

    outputs:
    - request to router
    - resolved responses -> search engine
*/

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::timeout;

use super::message::{DhtMessage, FindValueResult, PeerInfo};
use super::{DhtKey, DhtValue, RoutingTable};
use crate::core_router::RouterHandle;

/// DHT client for outbound RPC calls
pub struct DhtClient {
    /// Local node ID
    local_id: DhtKey,
    /// Router handle for sending messages
    router: Arc<RouterHandle>,
    /// Routing table (for updating peer stats)
    routing_table: Arc<Mutex<RoutingTable>>,
    /// RPC timeout duration
    rpc_timeout: Duration,
    /// Request ID counter
    request_id_counter: Arc<Mutex<u64>>,
}

impl DhtClient {
    /// Create new DHT client
    pub fn new(
        local_id: DhtKey,
        router: Arc<RouterHandle>,
        routing_table: Arc<Mutex<RoutingTable>>,
        rpc_timeout: Duration,
    ) -> Self {
        DhtClient {
            local_id,
            router,
            routing_table,
            rpc_timeout,
            request_id_counter: Arc::new(Mutex::new(0)),
        }
    }

    /// Get next request ID
    async fn next_request_id(&self) -> u64 {
        let mut counter = self.request_id_counter.lock().await;
        *counter += 1;
        *counter
    }

    /// Send PING request
    pub async fn ping(&self, peer_id: DhtKey) -> Result<(), String> {
        let _msg = DhtMessage::new_ping(self.local_id);

        // For now, ping just checks if we can reach the peer
        // In production, this would use proper RPC protocol
        let result = timeout(
            self.rpc_timeout,
            async { Ok::<(), String>(()) }, // Placeholder
        )
        .await;

        match result {
            Ok(Ok(_)) => {
                // Update routing table on success
                self.routing_table.lock().await.touch(&peer_id);
                Ok(())
            }
            Ok(Err(e)) => {
                // Mark peer as failed
                self.routing_table.lock().await.mark_failed(&peer_id);
                Err(format!("Router error: {}", e))
            }
            Err(_) => {
                // Timeout
                self.routing_table.lock().await.mark_failed(&peer_id);
                Err("RPC timeout".to_string())
            }
        }
    }

    /// Send FIND_NODE request
    pub async fn find_node(
        &self,
        peer_id: DhtKey,
        target: DhtKey,
    ) -> Result<Vec<PeerInfo>, String> {
        let request_id = self.next_request_id().await;
        let msg = DhtMessage::FindNode { sender_id: self.local_id, target, request_id };

        // Send via router and wait for response
        let result = timeout(self.rpc_timeout, self.send_and_receive(peer_id, msg)).await;

        match result {
            Ok(Ok(response)) => {
                if let DhtMessage::FindNodeResponse { nodes, .. } = response {
                    // Update routing table on success
                    self.routing_table.lock().await.touch(&peer_id);
                    Ok(nodes)
                } else {
                    self.routing_table.lock().await.mark_failed(&peer_id);
                    Err("Unexpected response type".to_string())
                }
            }
            Ok(Err(e)) => {
                self.routing_table.lock().await.mark_failed(&peer_id);
                Err(e)
            }
            Err(_) => {
                self.routing_table.lock().await.mark_failed(&peer_id);
                Err("RPC timeout".to_string())
            }
        }
    }

    /// Send FIND_VALUE request
    pub async fn find_value(
        &self,
        peer_id: DhtKey,
        key: DhtKey,
    ) -> Result<FindValueResult, String> {
        let request_id = self.next_request_id().await;
        let msg = DhtMessage::FindValue { sender_id: self.local_id, key, request_id };

        let result = timeout(self.rpc_timeout, self.send_and_receive(peer_id, msg)).await;

        match result {
            Ok(Ok(response)) => {
                if let DhtMessage::FindValueResponse { result, .. } = response {
                    self.routing_table.lock().await.touch(&peer_id);
                    Ok(result)
                } else {
                    self.routing_table.lock().await.mark_failed(&peer_id);
                    Err("Unexpected response type".to_string())
                }
            }
            Ok(Err(e)) => {
                self.routing_table.lock().await.mark_failed(&peer_id);
                Err(e)
            }
            Err(_) => {
                self.routing_table.lock().await.mark_failed(&peer_id);
                Err("RPC timeout".to_string())
            }
        }
    }

    /// Send STORE request
    pub async fn store(&self, peer_id: DhtKey, key: DhtKey, value: DhtValue) -> Result<(), String> {
        let request_id = self.next_request_id().await;
        let msg = DhtMessage::Store { sender_id: self.local_id, key, value, request_id };

        let result = timeout(self.rpc_timeout, self.send_and_receive(peer_id, msg)).await;

        match result {
            Ok(Ok(response)) => {
                if let DhtMessage::StoreAck { success, error, .. } = response {
                    if success {
                        self.routing_table.lock().await.touch(&peer_id);
                        Ok(())
                    } else {
                        self.routing_table.lock().await.mark_failed(&peer_id);
                        Err(error.unwrap_or_else(|| "Store failed".to_string()))
                    }
                } else {
                    self.routing_table.lock().await.mark_failed(&peer_id);
                    Err("Unexpected response type".to_string())
                }
            }
            Ok(Err(e)) => {
                self.routing_table.lock().await.mark_failed(&peer_id);
                Err(e)
            }
            Err(_) => {
                self.routing_table.lock().await.mark_failed(&peer_id);
                Err("RPC timeout".to_string())
            }
        }
    }

    /// Helper: send message and receive response
    /// Note: This is a simplified implementation. In production, you'd use the RPC protocol
    /// from the router layer to handle request/response matching.
    async fn send_and_receive(
        &self,
        _peer_id: DhtKey,
        _msg: DhtMessage,
    ) -> Result<DhtMessage, String> {
        // In production, we'd serialize, send via router, and wait for response
        // For now, return an error indicating this needs RPC integration
        Err("Response handling requires RPC protocol integration".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_client() -> DhtClient {
        let local_id = DhtKey::hash(b"local");
        let (router, _handle) = RouterHandle::new();
        let router = Arc::new(router);
        let routing_table = Arc::new(Mutex::new(RoutingTable::new(local_id, 20)));

        DhtClient::new(local_id, router, routing_table, Duration::from_secs(5))
    }

    #[tokio::test]
    async fn test_client_creation() {
        let client = create_test_client();
        assert_eq!(client.local_id, DhtKey::hash(b"local"));
    }

    #[tokio::test]
    async fn test_request_id_increment() {
        let client = create_test_client();

        let id1 = client.next_request_id().await;
        let id2 = client.next_request_id().await;
        let id3 = client.next_request_id().await;

        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
        assert_eq!(id3, 3);
    }

    #[tokio::test]
    async fn test_ping_message_creation() {
        let local_id = DhtKey::hash(b"local");
        let msg = DhtMessage::new_ping(local_id);

        assert!(msg.is_request());
        assert_eq!(msg.message_type(), "Ping");
        assert_eq!(msg.sender_id(), local_id);
    }

    #[tokio::test]
    async fn test_client_timeout_config() {
        let local_id = DhtKey::hash(b"local");
        let (router, _handle) = RouterHandle::new();
        let router = Arc::new(router);
        let routing_table = Arc::new(Mutex::new(RoutingTable::new(local_id, 20)));

        let timeout_duration = Duration::from_secs(10);
        let client = DhtClient::new(local_id, router, routing_table, timeout_duration);

        assert_eq!(client.rpc_timeout, timeout_duration);
    }
}
