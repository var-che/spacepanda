/*
    RpcProtocol - framed RPC and method dispatch

    Standardize control messages (peer exchange, DHT requests, ping, snapshot fetches)
    Provide request/response semantics with timeouts and retry.

    Workflow:

    1. Expose rpc_call(peer_id, method, params) which:
        - signs or attaches an auth signature
        - packages a `RpcRequest {id, method params}`
        - ask routing core to send (direct or anonymous)
        - waits on local response map keyed by id for a RpcResponse or timeout

    2. On receive:
        - parse frame to RpcRequest or RpcResponse
        - dispatch to appropriate handler (DHT handler, peer exchange, etc)
        - send RpcResponse back

    Inputs:
        - RpcCommand::RpcCall(peer_id, method, params)
        - Incoming PlaintextFrame(peer_id, bytes) from session_manager

    Outputs:
        - Synchronous results to callers (awaited futures)
        - Events to other modules (e.g. DHT handler)

    Notes:

    Message structure for example, using serde_json:
    ```json
    {
        "type": "request",
        "id": "unique_request_id",
        "method": "get_peer_info",
        "params": {
            "some_param": "<hex>"
        }
    }
    ```
*/

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, oneshot, Mutex};
use uuid::Uuid;

use super::session_manager::{PeerId, SessionCommand, SessionEvent};

/// RPC message types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum RpcMessage {
    #[serde(rename = "request")]
    Request {
        id: String,
        method: String,
        params: serde_json::Value,
    },
    #[serde(rename = "response")]
    Response {
        id: String,
        result: Result<serde_json::Value, RpcError>,
    },
}

/// RPC error types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcError {
    pub code: i32,
    pub message: String,
}

impl RpcError {
    pub fn new(code: i32, message: String) -> Self {
        RpcError { code, message }
    }

    pub fn method_not_found(method: &str) -> Self {
        RpcError::new(-32601, format!("Method not found: {}", method))
    }

    pub fn internal_error(msg: &str) -> Self {
        RpcError::new(-32603, format!("Internal error: {}", msg))
    }

    pub fn timeout() -> Self {
        RpcError::new(-32000, "Request timeout".to_string())
    }
}

/// Commands sent to RpcProtocol
#[derive(Debug)]
pub enum RpcCommand {
    /// Make an RPC call to a peer
    Call {
        peer_id: PeerId,
        method: String,
        params: serde_json::Value,
        response_tx: oneshot::Sender<Result<serde_json::Value, RpcError>>,
    },
    /// Register an RPC handler for a method
    RegisterHandler {
        method: String,
        handler_tx: mpsc::Sender<RpcRequest>,
    },
}

/// RPC request to be handled
#[derive(Debug)]
pub struct RpcRequest {
    pub peer_id: PeerId,
    pub id: String,
    pub method: String,
    pub params: serde_json::Value,
    pub response_tx: oneshot::Sender<Result<serde_json::Value, RpcError>>,
}

/// Pending RPC request awaiting response
struct PendingRequest {
    response_tx: oneshot::Sender<Result<serde_json::Value, RpcError>>,
}

/// Seen request ID for replay protection
struct SeenRequest {
    timestamp: Instant,
}

pub struct RpcProtocol {
    /// Pending requests awaiting responses
    pending_requests: Arc<Mutex<HashMap<String, PendingRequest>>>,
    /// Registered method handlers
    handlers: Arc<Mutex<HashMap<String, mpsc::Sender<RpcRequest>>>>,
    /// Seen request IDs to prevent replay attacks (request_id -> timestamp)
    seen_requests: Arc<Mutex<HashMap<String, SeenRequest>>>,
    /// Channel to send session commands
    session_tx: mpsc::Sender<SessionCommand>,
    /// Default timeout for RPC calls
    default_timeout: Duration,
    /// TTL for seen request IDs (for pruning)
    seen_requests_ttl: Duration,
}

impl RpcProtocol {
    /// Create a new RPC protocol handler
    pub fn new(session_tx: mpsc::Sender<SessionCommand>) -> Self {
        let rpc = RpcProtocol {
            pending_requests: Arc::new(Mutex::new(HashMap::new())),
            handlers: Arc::new(Mutex::new(HashMap::new())),
            seen_requests: Arc::new(Mutex::new(HashMap::new())),
            session_tx,
            default_timeout: Duration::from_secs(30),
            seen_requests_ttl: Duration::from_secs(300), // 5 minutes
        };
        
        // Start background task to prune old seen requests
        let seen_requests = rpc.seen_requests.clone();
        let ttl = rpc.seen_requests_ttl;
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));
            loop {
                interval.tick().await;
                let mut seen = seen_requests.lock().await;
                let now = Instant::now();
                seen.retain(|_, req| now.duration_since(req.timestamp) < ttl);
            }
        });
        
        rpc
    }

    /// Set the default timeout for RPC calls
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.default_timeout = timeout;
        self
    }
    
    /// Get the number of seen requests (for testing)
    #[cfg(test)]
    pub async fn seen_requests_count(&self) -> usize {
        self.seen_requests.lock().await.len()
    }

    /// Handle incoming session events (plaintext frames from peers)
    pub async fn handle_session_event(&self, event: SessionEvent) -> Result<(), String> {
        match event {
            SessionEvent::PlaintextFrame(peer_id, bytes) => {
                self.handle_frame(peer_id, bytes).await?;
            }
            SessionEvent::Established(_, _) | SessionEvent::Closed(_) => {
                // These events are informational for RPC layer
            }
        }
        Ok(())
    }

    /// Handle RPC commands
    pub async fn handle_command(&self, command: RpcCommand) -> Result<(), String> {
        match command {
            RpcCommand::Call {
                peer_id,
                method,
                params,
                response_tx,
            } => {
                self.make_call(peer_id, method, params, response_tx)
                    .await?;
            }
            RpcCommand::RegisterHandler {
                method,
                handler_tx,
            } => {
                self.handlers.lock().await.insert(method, handler_tx);
            }
        }
        Ok(())
    }

    /// Make an RPC call to a peer
    async fn make_call(
        &self,
        peer_id: PeerId,
        method: String,
        params: serde_json::Value,
        response_tx: oneshot::Sender<Result<serde_json::Value, RpcError>>,
    ) -> Result<(), String> {
        let request_id = Uuid::new_v4().to_string();

        let message = RpcMessage::Request {
            id: request_id.clone(),
            method,
            params,
        };

        // Serialize the message
        let bytes = serde_json::to_vec(&message)
            .map_err(|e| format!("Failed to serialize RPC request: {}", e))?;

        // Store pending request
        let pending = PendingRequest { response_tx };
        self.pending_requests
            .lock()
            .await
            .insert(request_id.clone(), pending);

        // Send via session manager
        self.session_tx
            .send(SessionCommand::SendPlaintext(peer_id, bytes))
            .await
            .map_err(|e| format!("Failed to send RPC request: {}", e))?;

        // Set up timeout
        let pending_requests = self.pending_requests.clone();
        let timeout = self.default_timeout;
        tokio::spawn(async move {
            tokio::time::sleep(timeout).await;
            if let Some(pending) = pending_requests.lock().await.remove(&request_id) {
                let _ = pending.response_tx.send(Err(RpcError::timeout()));
            }
        });

        Ok(())
    }

    /// Handle incoming frame from a peer
    async fn handle_frame(&self, peer_id: PeerId, bytes: Vec<u8>) -> Result<(), String> {
        let message: RpcMessage = serde_json::from_slice(&bytes)
            .map_err(|e| format!("Failed to deserialize RPC message: {}", e))?;

        match message {
            RpcMessage::Request { id, method, params } => {
                self.handle_request(peer_id, id, method, params).await?;
            }
            RpcMessage::Response { id, result } => {
                self.handle_response(id, result).await?;
            }
        }

        Ok(())
    }

    /// Handle incoming RPC request
    async fn handle_request(
        &self,
        peer_id: PeerId,
        id: String,
        method: String,
        params: serde_json::Value,
    ) -> Result<(), String> {
        // Check for replay attack
        {
            let mut seen = self.seen_requests.lock().await;
            if seen.contains_key(&id) {
                // Replay detected! Reject the request
                let error = RpcError::new(-32600, format!("Duplicate request ID: {}", id));
                self.send_response(peer_id, id, Err(error)).await?;
                return Ok(()); // Don't process replay
            }
            // Mark this request ID as seen
            seen.insert(id.clone(), SeenRequest {
                timestamp: Instant::now(),
            });
        }
        
        let handlers = self.handlers.lock().await;
        let handler_tx = handlers.get(&method).cloned();
        drop(handlers);

        if let Some(handler_tx) = handler_tx {
            // Create response channel
            let (response_tx, response_rx) = oneshot::channel();

            let request = RpcRequest {
                peer_id: peer_id.clone(),
                id: id.clone(),
                method,
                params,
                response_tx,
            };

            // Send to handler
            if handler_tx.send(request).await.is_err() {
                self.send_error_response(peer_id, id, RpcError::internal_error("Handler crashed"))
                    .await?;
                return Ok(());
            }

            // Wait for handler response
            let peer_id_clone = peer_id.clone();
            let session_tx = self.session_tx.clone();
            tokio::spawn(async move {
                match response_rx.await {
                    Ok(result) => {
                        let message = RpcMessage::Response { id, result };
                        if let Ok(bytes) = serde_json::to_vec(&message) {
                            let _ = session_tx
                                .send(SessionCommand::SendPlaintext(peer_id_clone, bytes))
                                .await;
                        }
                    }
                    Err(_) => {
                        // Handler dropped without responding
                    }
                }
            });
        } else {
            // Method not found
            self.send_error_response(peer_id, id, RpcError::method_not_found(&method))
                .await?;
        }

        Ok(())
    }

    /// Handle incoming RPC response
    async fn handle_response(
        &self,
        id: String,
        result: Result<serde_json::Value, RpcError>,
    ) -> Result<(), String> {
        if let Some(pending) = self.pending_requests.lock().await.remove(&id) {
            let _ = pending.response_tx.send(result);
        }
        Ok(())
    }

    /// Send an error response to a peer
    async fn send_error_response(
        &self,
        peer_id: PeerId,
        id: String,
        error: RpcError,
    ) -> Result<(), String> {
        self.send_response(peer_id, id, Err(error)).await
    }
    
    /// Send a response (success or error) to a peer
    async fn send_response(
        &self,
        peer_id: PeerId,
        id: String,
        result: Result<serde_json::Value, RpcError>,
    ) -> Result<(), String> {
        let message = RpcMessage::Response { id, result };

        let bytes = serde_json::to_vec(&message)
            .map_err(|e| format!("Failed to serialize response: {}", e))?;

        self.session_tx
            .send(SessionCommand::SendPlaintext(peer_id, bytes))
            .await
            .map_err(|e| format!("Failed to send response: {}", e))?;

        Ok(())
    }

    /// Get statistics about pending requests
    pub async fn pending_count(&self) -> usize {
        self.pending_requests.lock().await.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{sleep, timeout};

    #[tokio::test]
    async fn test_rpc_request_response() {
        let (session_tx, mut session_rx) = mpsc::channel(100);
        let rpc = RpcProtocol::new(session_tx);

        // Register a handler for "ping" method
        let (handler_tx, mut _handler_rx) = mpsc::channel(100);
        rpc.handle_command(RpcCommand::RegisterHandler {
            method: "ping".to_string(),
            handler_tx,
        })
        .await
        .unwrap();

        // Make an RPC call
        let peer_id = PeerId::from_bytes(vec![1, 2, 3, 4]);
        let (response_tx, _response_rx) = oneshot::channel();

        rpc.handle_command(RpcCommand::Call {
            peer_id: peer_id.clone(),
            method: "ping".to_string(),
            params: serde_json::json!({"data": "hello"}),
            response_tx,
        })
        .await
        .unwrap();

        // Verify session command was sent
        let cmd = session_rx.recv().await.unwrap();
        match cmd {
            SessionCommand::SendPlaintext(pid, bytes) => {
                assert_eq!(pid, peer_id);
                let msg: RpcMessage = serde_json::from_slice(&bytes).unwrap();
                if let RpcMessage::Request { method, .. } = msg {
                    assert_eq!(method, "ping");
                } else {
                    panic!("Expected request");
                }
            }
            _ => panic!("Expected SendPlaintext"),
        }

        // Verify we have 1 pending request
        assert_eq!(rpc.pending_count().await, 1);

        // Note: Full request/response cycle would require simulating the peer's response
        // For now, we verify the request was sent correctly
    }

    #[tokio::test]
    async fn test_rpc_handle_incoming_request() {
        let (session_tx, mut session_rx) = mpsc::channel(100);
        let rpc = RpcProtocol::new(session_tx);

        // Register a handler
        let (handler_tx, mut handler_rx) = mpsc::channel(100);
        rpc.handle_command(RpcCommand::RegisterHandler {
            method: "echo".to_string(),
            handler_tx,
        })
        .await
        .unwrap();

        // Simulate incoming request from peer
        let peer_id = PeerId::from_bytes(vec![5, 6, 7, 8]);
        let request = RpcMessage::Request {
            id: "test-123".to_string(),
            method: "echo".to_string(),
            params: serde_json::json!({"msg": "hello"}),
        };
        let bytes = serde_json::to_vec(&request).unwrap();

        // Handle the incoming frame
        rpc.handle_session_event(SessionEvent::PlaintextFrame(peer_id.clone(), bytes))
            .await
            .unwrap();

        // Handler should receive the request
        let req = timeout(Duration::from_secs(1), handler_rx.recv())
            .await
            .unwrap()
            .unwrap();

        assert_eq!(req.method, "echo");
        assert_eq!(req.id, "test-123");
        assert_eq!(req.params["msg"], "hello");

        // Send response from handler
        req.response_tx
            .send(Ok(serde_json::json!({"echo": "hello"})))
            .unwrap();

        // Verify response was sent via session
        let cmd = timeout(Duration::from_secs(1), session_rx.recv())
            .await
            .unwrap()
            .unwrap();

        match cmd {
            SessionCommand::SendPlaintext(pid, bytes) => {
                assert_eq!(pid, peer_id);
                let msg: RpcMessage = serde_json::from_slice(&bytes).unwrap();
                if let RpcMessage::Response { id, result } = msg {
                    assert_eq!(id, "test-123");
                    assert!(result.is_ok());
                } else {
                    panic!("Expected response");
                }
            }
            _ => panic!("Expected SendPlaintext"),
        }
    }

    #[tokio::test]
    async fn test_rpc_method_not_found() {
        let (session_tx, mut session_rx) = mpsc::channel(100);
        let rpc = RpcProtocol::new(session_tx);

        // No handler registered
        let peer_id = PeerId::from_bytes(vec![9, 10, 11, 12]);
        let request = RpcMessage::Request {
            id: "test-404".to_string(),
            method: "nonexistent".to_string(),
            params: serde_json::json!({}),
        };
        let bytes = serde_json::to_vec(&request).unwrap();

        rpc.handle_session_event(SessionEvent::PlaintextFrame(peer_id.clone(), bytes))
            .await
            .unwrap();

        // Should receive error response
        let cmd = timeout(Duration::from_secs(1), session_rx.recv())
            .await
            .unwrap()
            .unwrap();

        match cmd {
            SessionCommand::SendPlaintext(pid, bytes) => {
                assert_eq!(pid, peer_id);
                let msg: RpcMessage = serde_json::from_slice(&bytes).unwrap();
                if let RpcMessage::Response { id, result } = msg {
                    assert_eq!(id, "test-404");
                    assert!(result.is_err());
                    let err = result.unwrap_err();
                    assert_eq!(err.code, -32601); // Method not found
                } else {
                    panic!("Expected response");
                }
            }
            _ => panic!("Expected SendPlaintext"),
        }
    }

    #[tokio::test]
    async fn test_rpc_timeout() {
        let (session_tx, _session_rx) = mpsc::channel(100);
        let rpc = RpcProtocol::new(session_tx).with_timeout(Duration::from_millis(100));

        let peer_id = PeerId::from_bytes(vec![13, 14, 15, 16]);
        let (response_tx, response_rx) = oneshot::channel();

        rpc.handle_command(RpcCommand::Call {
            peer_id,
            method: "slow_method".to_string(),
            params: serde_json::json!({}),
            response_tx,
        })
        .await
        .unwrap();

        // Wait for timeout
        let result = timeout(Duration::from_secs(1), response_rx)
            .await
            .unwrap()
            .unwrap();

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code, -32000); // Timeout error

        // Pending request should be removed
        sleep(Duration::from_millis(150)).await;
        assert_eq!(rpc.pending_count().await, 0);
    }
}
