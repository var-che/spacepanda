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
        RpcError::new(ERR_METHOD_NOT_FOUND, format!("Method not found: {}", method))
    }

    pub fn internal_error(msg: &str) -> Self {
        RpcError::new(ERR_INTERNAL_ERROR, format!("Internal error: {}", msg))
    }

    pub fn timeout() -> Self {
        RpcError::new(ERR_TIMEOUT, "Request timeout".to_string())
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
    /// Maximum capacity for seen_requests map to prevent memory exhaustion
    seen_requests_max_capacity: usize,
    /// Shutdown signal for background pruning task
    prune_shutdown_tx: Option<oneshot::Sender<()>>,
    /// Handle to background pruning task
    prune_task_handle: Option<tokio::task::JoinHandle<()>>,
}

/// Maximum frame size to prevent memory exhaustion DoS (64 KiB)
const MAX_FRAME_SIZE: usize = 64 * 1024;

/// Error codes for RPC errors
const ERR_METHOD_NOT_FOUND: i32 = -32601;
const ERR_INTERNAL_ERROR: i32 = -32603;
const ERR_TIMEOUT: i32 = -32000;
const ERR_DUPLICATE_REQUEST: i32 = -32600;

impl RpcProtocol {
    /// Create a new RPC protocol handler with default settings
    pub fn new(session_tx: mpsc::Sender<SessionCommand>) -> Self {
        Self::new_with_config(
            session_tx,
            Duration::from_secs(30),        // default RPC timeout
            Duration::from_secs(300),        // 5 minute TTL for seen requests
            Duration::from_secs(60),         // prune every 60 seconds
            100_000,                         // max 100k seen request IDs
        )
    }
    
    /// Create a new RPC protocol handler with custom configuration
    /// 
    /// # Arguments
    /// * `session_tx` - Channel to send session commands
    /// * `default_timeout` - Default timeout for RPC calls
    /// * `seen_requests_ttl` - TTL for seen request IDs (replay protection window)
    /// * `prune_interval` - How often to prune expired seen requests
    /// * `seen_requests_max_capacity` - Maximum number of seen request IDs to track
    pub fn new_with_config(
        session_tx: mpsc::Sender<SessionCommand>,
        default_timeout: Duration,
        seen_requests_ttl: Duration,
        prune_interval: Duration,
        seen_requests_max_capacity: usize,
    ) -> Self {
        let seen_requests = Arc::new(Mutex::new(HashMap::<String, SeenRequest>::new()));
        let (shutdown_tx, mut shutdown_rx) = oneshot::channel();
        
        // Start background task to prune old seen requests
        let seen_requests_clone = seen_requests.clone();
        let ttl = seen_requests_ttl;
        let max_capacity = seen_requests_max_capacity;
        let prune_handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(prune_interval);
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        let mut seen = seen_requests_clone.lock().await;
                        let now = Instant::now();
                        // Prune expired entries
                        seen.retain(|_, req| now.duration_since(req.timestamp) < ttl);
                        // Enforce capacity limit by removing oldest entries if over limit
                        if seen.len() > max_capacity {
                            let to_remove = seen.len() - max_capacity;
                            let mut entries: Vec<_> = seen.iter()
                                .map(|(id, req)| (id.clone(), req.timestamp))
                                .collect();
                            entries.sort_by_key(|(_, ts)| *ts);
                            for (id, _) in entries.iter().take(to_remove) {
                                seen.remove(id);
                            }
                        }
                    }
                    _ = &mut shutdown_rx => {
                        break;
                    }
                }
            }
        });
        
        RpcProtocol {
            pending_requests: Arc::new(Mutex::new(HashMap::new())),
            handlers: Arc::new(Mutex::new(HashMap::new())),
            seen_requests,
            session_tx,
            default_timeout,
            seen_requests_ttl,
            seen_requests_max_capacity,
            prune_shutdown_tx: Some(shutdown_tx),
            prune_task_handle: Some(prune_handle),
        }
    }

    /// Set the default timeout for RPC calls
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.default_timeout = timeout;
        self
    }
    
    /// Gracefully shutdown the RPC protocol, stopping background tasks
    pub async fn shutdown(&mut self) {
        if let Some(tx) = self.prune_shutdown_tx.take() {
            let _ = tx.send(());
        }
        if let Some(handle) = self.prune_task_handle.take() {
            let _ = handle.await;
        }
    }
    
    /// Get the number of seen requests (for testing)
    #[cfg(test)]
    pub async fn seen_requests_count(&self) -> usize {
        self.seen_requests.lock().await.len()
    }
    
    /// Get the maximum capacity for seen requests (for testing)
    #[cfg(test)]
    pub fn seen_requests_max_capacity(&self) -> usize {
        self.seen_requests_max_capacity
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
        // Reject oversized frames to prevent memory exhaustion DoS
        if bytes.len() > MAX_FRAME_SIZE {
            return Err(format!("Frame too large: {} bytes (max {})", bytes.len(), MAX_FRAME_SIZE));
        }
        
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
        // Check for replay attack and enforce capacity limit
        // NOTE: This is safe from race conditions because we hold the mutex
        // during both the contains_key check and insert operation.
        {
            let mut seen = self.seen_requests.lock().await;
            if seen.contains_key(&id) {
                // Replay detected! Reject the request
                let error = RpcError::new(ERR_DUPLICATE_REQUEST, format!("Duplicate request ID: {}", id));
                self.send_response(peer_id, id, Err(error)).await?;
                return Ok(()); // Don't process replay
            }
            
            // Enforce capacity limit - reject new requests if at capacity
            if seen.len() >= self.seen_requests_max_capacity {
                let error = RpcError::new(ERR_INTERNAL_ERROR, "Too many pending requests".to_string());
                self.send_response(peer_id, id, Err(error)).await?;
                return Ok(());
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
    
    #[tokio::test]
    async fn test_oversized_frame_rejection() {
        let (session_tx, _session_rx) = mpsc::channel(100);
        let mut rpc = RpcProtocol::new(session_tx);
        
        let peer_id = PeerId::from_bytes(vec![1, 2, 3, 4]);
        
        // Create a frame larger than MAX_FRAME_SIZE (64 KiB)
        let oversized_payload = vec![0u8; MAX_FRAME_SIZE + 1];
        
        // Should reject with clear error
        let result = rpc.handle_session_event(SessionEvent::PlaintextFrame(
            peer_id,
            oversized_payload,
        )).await;
        
        assert!(result.is_err(), "Should reject oversized frame");
        let err_msg = result.unwrap_err();
        assert!(err_msg.contains("Frame too large"), "Error should mention frame size: {}", err_msg);
        // The error message contains "65536" (64 * 1024) not just "64"
        assert!(err_msg.contains("65536"), "Error should mention limit: {}", err_msg);
        
        // Cleanup
        rpc.shutdown().await;
    }
    
    #[tokio::test]
    async fn test_seen_requests_capacity_limit() {
        let (session_tx, mut session_rx) = mpsc::channel(1000);
        // Create RPC with very small capacity for testing
        let rpc = RpcProtocol::new_with_config(
            session_tx,
            Duration::from_secs(30),
            Duration::from_secs(300),
            Duration::from_secs(60),
            10, // Only allow 10 seen requests
        );
        
        assert_eq!(rpc.seen_requests_max_capacity(), 10);
        
        let (handler_tx, mut _handler_rx) = mpsc::channel(100);
        rpc.handle_command(RpcCommand::RegisterHandler {
            method: "test".to_string(),
            handler_tx,
        })
        .await
        .unwrap();
        
        let peer_id = PeerId::from_bytes(vec![1, 2, 3, 4]);
        
        // Send 10 unique requests (should all be accepted)
        for i in 0..10 {
            let request = RpcMessage::Request {
                id: format!("req-{}", i),
                method: "test".to_string(),
                params: serde_json::json!({}),
            };
            let bytes = serde_json::to_vec(&request).unwrap();
            rpc.handle_session_event(SessionEvent::PlaintextFrame(peer_id.clone(), bytes))
                .await
                .unwrap();
        }
        
        // Should have 10 seen requests
        assert_eq!(rpc.seen_requests_count().await, 10);
        
        // Send 11th unique request - should be rejected (at capacity)
        let request = RpcMessage::Request {
            id: "req-11".to_string(),
            method: "test".to_string(),
            params: serde_json::json!({}),
        };
        let bytes = serde_json::to_vec(&request).unwrap();
        rpc.handle_session_event(SessionEvent::PlaintextFrame(peer_id.clone(), bytes))
            .await
            .unwrap();
        
        // Should receive error response for capacity limit
        let cmd = timeout(Duration::from_millis(100), session_rx.recv())
            .await
            .unwrap()
            .unwrap();
        
        match cmd {
            SessionCommand::SendPlaintext(_, bytes) => {
                let msg: RpcMessage = serde_json::from_slice(&bytes).unwrap();
                if let RpcMessage::Response { result, .. } = msg {
                    let err = result.unwrap_err();
                    assert!(err.message.contains("Too many"), "Should reject at capacity: {}", err.message);
                }
            }
            _ => panic!("Expected error response"),
        }
    }
    
    #[tokio::test]
    async fn test_seen_requests_ttl_pruning() {
        let (session_tx, _session_rx) = mpsc::channel(100);
        // Create RPC with very short TTL and prune interval for testing
        let mut rpc = RpcProtocol::new_with_config(
            session_tx,
            Duration::from_secs(30),
            Duration::from_millis(100), // 100ms TTL
            Duration::from_millis(50),  // prune every 50ms
            1000,
        );
        
        let (handler_tx, mut _handler_rx) = mpsc::channel(100);
        rpc.handle_command(RpcCommand::RegisterHandler {
            method: "test".to_string(),
            handler_tx,
        })
        .await
        .unwrap();
        
        let peer_id = PeerId::from_bytes(vec![1, 2, 3, 4]);
        
        // Send a request
        let request = RpcMessage::Request {
            id: "expire-test".to_string(),
            method: "test".to_string(),
            params: serde_json::json!({}),
        };
        let bytes = serde_json::to_vec(&request).unwrap();
        rpc.handle_session_event(SessionEvent::PlaintextFrame(peer_id.clone(), bytes.clone()))
            .await
            .unwrap();
        
        // Should have 1 seen request
        assert_eq!(rpc.seen_requests_count().await, 1);
        
        // Wait for TTL + prune interval
        sleep(Duration::from_millis(200)).await;
        
        // Should be pruned now
        assert_eq!(rpc.seen_requests_count().await, 0, "Old request should be pruned");
        
        // Should be able to reuse same ID after expiry
        rpc.handle_session_event(SessionEvent::PlaintextFrame(peer_id, bytes))
            .await
            .unwrap();
        
        assert_eq!(rpc.seen_requests_count().await, 1, "Should accept previously expired ID");
        
        // Cleanup
        rpc.shutdown().await;
    }
    
    #[tokio::test]
    async fn test_graceful_shutdown() {
        let (session_tx, _session_rx) = mpsc::channel(100);
        let mut rpc = RpcProtocol::new_with_config(
            session_tx,
            Duration::from_secs(30),
            Duration::from_millis(100),
            Duration::from_millis(50),
            1000,
        );
        
        // Should have background task running
        assert!(rpc.prune_task_handle.is_some());
        
        // Shutdown
        rpc.shutdown().await;
        
        // Background task should be stopped
        assert!(rpc.prune_task_handle.is_none());
        assert!(rpc.prune_shutdown_tx.is_none());
    }
    
    #[tokio::test]
    async fn test_malformed_frames_dont_panic() {
        let (session_tx, _session_rx) = mpsc::channel(100);
        let rpc = RpcProtocol::new(session_tx);
        
        let peer_id = PeerId::from_bytes(vec![1, 2, 3, 4]);
        
        // Test various malformed payloads
        let malformed_payloads = vec![
            vec![],                           // empty
            vec![0xFF, 0xFF, 0xFF],          // invalid UTF-8/JSON
            vec![b'{'; 100],                 // incomplete JSON
            b"not json at all".to_vec(),     // plain text
            b"{\"type\":\"unknown\"}".to_vec(), // unknown message type
        ];
        
        for payload in malformed_payloads {
            let result = rpc.handle_session_event(SessionEvent::PlaintextFrame(
                peer_id.clone(),
                payload.clone(),
            )).await;
            
            // Should return error, not panic
            assert!(result.is_err(), "Malformed payload should be rejected: {:?}", payload);
        }
    }
}
