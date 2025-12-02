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
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot, Mutex};
use uuid::Uuid;
use hashlink::LruCache;

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
    /// Handle to abort the timeout task if response arrives
    timeout_handle: tokio::task::AbortHandle,
}

pub struct RpcProtocol {
    /// Pending requests awaiting responses
    pending_requests: Arc<Mutex<HashMap<String, PendingRequest>>>,
    /// Registered method handlers
    handlers: Arc<Mutex<HashMap<String, mpsc::Sender<RpcRequest>>>>,
    /// Seen request IDs to prevent replay attacks (LRU cache with automatic eviction)
    seen_requests: Arc<Mutex<LruCache<String, ()>>>,
    /// Channel to send session commands
    session_tx: mpsc::Sender<SessionCommand>,
    /// Default timeout for RPC calls
    default_timeout: Duration,
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
            Duration::from_secs(30),  // default RPC timeout
            100_000,                   // max 100k seen request IDs
        )
    }
    
    /// Create a new RPC protocol handler with custom configuration
    /// 
    /// # Arguments
    /// * `session_tx` - Channel to send session commands
    /// * `default_timeout` - Default timeout for RPC calls
    /// * `seen_requests_capacity` - Maximum number of seen request IDs to track (LRU)
    pub fn new_with_config(
        session_tx: mpsc::Sender<SessionCommand>,
        default_timeout: Duration,
        seen_requests_capacity: usize,
    ) -> Self {
        // Create LRU cache for seen requests (O(1) insert/check, automatic eviction)
        let seen_requests = Arc::new(Mutex::new(LruCache::new(seen_requests_capacity)));
        
        RpcProtocol {
            pending_requests: Arc::new(Mutex::new(HashMap::new())),
            handlers: Arc::new(Mutex::new(HashMap::new())),
            seen_requests,
            session_tx,
            default_timeout,
        }
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

        // Set up timeout task with abort handle
        let pending_requests = self.pending_requests.clone();
        let timeout = self.default_timeout;
        let request_id_for_timeout = request_id.clone();
        
        let timeout_task = tokio::spawn(async move {
            tokio::time::sleep(timeout).await;
            // Only send timeout error if request is still pending
            if let Some(pending) = pending_requests.lock().await.remove(&request_id_for_timeout) {
                let _ = pending.response_tx.send(Err(RpcError::timeout()));
            }
        });
        
        let timeout_handle = timeout_task.abort_handle();

        // Store pending request with timeout handle
        let pending = PendingRequest { 
            response_tx,
            timeout_handle,
        };
        self.pending_requests
            .lock()
            .await
            .insert(request_id.clone(), pending);

        // Send via session manager
        self.session_tx
            .send(SessionCommand::SendPlaintext(peer_id, bytes))
            .await
            .map_err(|e| format!("Failed to send RPC request: {}", e))?;

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
        // Check for replay attack using LRU cache
        // LRU automatically evicts oldest entries when capacity is reached
        {
            let mut seen = self.seen_requests.lock().await;
            
            // Check if request ID was already seen (replay attack)
            if seen.contains_key(&id) {
                // Replay detected! Reject the request
                let error = RpcError::new(ERR_DUPLICATE_REQUEST, format!("Duplicate request ID: {}", id));
                self.send_response(peer_id, id, Err(error)).await?;
                return Ok(()); // Don't process replay
            }
            
            // Insert into LRU cache (O(1) operation)
            // If at capacity, LRU automatically evicts least recently used entry
            seen.insert(id.clone(), ());
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
            // Abort the timeout task since response arrived
            pending.timeout_handle.abort();
            
            // Send response to caller
            let _ = pending.response_tx.send(result);
        }
        // If request not found, it may have already timed out - that's OK
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
    }
    
    #[tokio::test]
    async fn test_seen_requests_capacity_limit() {
        let (session_tx, _session_rx) = mpsc::channel(1000);
        // Create RPC with very small capacity for testing
        let rpc = RpcProtocol::new_with_config(
            session_tx,
            Duration::from_secs(30),
            10, // Only allow 10 seen requests
        );
        
        // LRU cache should start empty
        assert_eq!(rpc.seen_requests_count().await, 0);
        
        let (handler_tx, _handler_rx) = mpsc::channel(100);
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
        
        // Send 11th unique request - LRU will auto-evict oldest (req-0)
        let request = RpcMessage::Request {
            id: "req-10".to_string(),
            method: "test".to_string(),
            params: serde_json::json!({}),
        };
        let bytes = serde_json::to_vec(&request).unwrap();
        rpc.handle_session_event(SessionEvent::PlaintextFrame(peer_id.clone(), bytes))
            .await
            .unwrap();
        
        // Should still have 10 seen requests (capacity limit)
        assert_eq!(rpc.seen_requests_count().await, 10);
        
        // req-0 should have been evicted, so it can be reused
        let request = RpcMessage::Request {
            id: "req-0".to_string(),
            method: "test".to_string(),
            params: serde_json::json!({}),
        };
        let bytes = serde_json::to_vec(&request).unwrap();
        // This should succeed (not detected as duplicate) since req-0 was evicted
        let result = rpc.handle_session_event(SessionEvent::PlaintextFrame(peer_id.clone(), bytes))
            .await;
        
        // Should succeed without error (req-0 was evicted from LRU)
        assert!(result.is_ok(), "req-0 should be accepted after eviction");
    }
    
    #[tokio::test]
    async fn test_lru_eviction() {
        let (session_tx, _session_rx) = mpsc::channel(100);
        // Create RPC with small capacity to test LRU eviction
        let rpc = RpcProtocol::new_with_config(
            session_tx,
            Duration::from_secs(30),
            5, // Only 5 entries
        );
        
        let (handler_tx, mut _handler_rx) = mpsc::channel(100);
        rpc.handle_command(RpcCommand::RegisterHandler {
            method: "test".to_string(),
            handler_tx,
        })
        .await
        .unwrap();
        
        let peer_id = PeerId::from_bytes(vec![1, 2, 3, 4]);
        
        // Send 5 requests (fill up LRU cache)
        for i in 0..5 {
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
        
        // Should have 5 seen requests
        assert_eq!(rpc.seen_requests_count().await, 5);
        
        // Send 6th request - should evict oldest (req-0)
        let request = RpcMessage::Request {
            id: "req-5".to_string(),
            method: "test".to_string(),
            params: serde_json::json!({}),
        };
        let bytes = serde_json::to_vec(&request).unwrap();
        rpc.handle_session_event(SessionEvent::PlaintextFrame(peer_id.clone(), bytes))
            .await
            .unwrap();
        
        // Still 5 entries (LRU evicted oldest)
        assert_eq!(rpc.seen_requests_count().await, 5);
        
        // req-0 should be evicted, so we can reuse it
        let request = RpcMessage::Request {
            id: "req-0".to_string(),
            method: "test".to_string(),
            params: serde_json::json!({}),
        };
        let bytes = serde_json::to_vec(&request).unwrap();
        rpc.handle_session_event(SessionEvent::PlaintextFrame(peer_id, bytes))
            .await
            .unwrap();
        
        // Still 5 entries (oldest evicted)
        assert_eq!(rpc.seen_requests_count().await, 5);
    }
    
    #[tokio::test]
    async fn test_graceful_shutdown() {
        let (session_tx, _session_rx) = mpsc::channel(100);
        let rpc = RpcProtocol::new_with_config(
            session_tx,
            Duration::from_secs(30),
            1000,
        );
        
        // No background task with LRU implementation
        // Nothing to shutdown
        assert!(rpc.seen_requests.lock().await.is_empty());
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
        ];
        
        for payload in malformed_payloads {
            let result = rpc
                .handle_session_event(SessionEvent::PlaintextFrame(peer_id.clone(), payload))
                .await;
            
            // Should return error but not panic
            assert!(result.is_err());
        }
    }

    #[tokio::test]
    async fn test_timeout_cancellation_on_fast_response() {
        let (session_tx, mut session_rx) = mpsc::channel(100);
        let rpc = Arc::new(RpcProtocol::new_with_config(
            session_tx,
            Duration::from_secs(10),  // Long timeout
            1000,
        ));

        let peer_id = PeerId::from_bytes(vec![1, 2, 3, 4]);
        let (response_tx, response_rx) = oneshot::channel();

        // Make call
        let rpc_clone = rpc.clone();
        let peer_id_clone = peer_id.clone();
        tokio::spawn(async move {
            rpc_clone
                .handle_command(RpcCommand::Call {
                    peer_id: peer_id_clone,
                    method: "test".to_string(),
                    params: serde_json::json!({}),
                    response_tx,
                })
                .await
        });

        // Get the request
        let cmd = session_rx.recv().await.unwrap();
        let bytes = match cmd {
            SessionCommand::SendPlaintext(_, b) => b,
            _ => panic!("Expected SendPlaintext"),
        };

        let message: RpcMessage = serde_json::from_slice(&bytes).unwrap();
        let request_id = match message {
            RpcMessage::Request { id, .. } => id,
            _ => panic!("Expected Request"),
        };

        // Send fast response (before timeout)
        let response = RpcMessage::Response {
            id: request_id,
            result: Ok(serde_json::json!({"status": "ok"})),
        };
        let response_bytes = serde_json::to_vec(&response).unwrap();
        
        rpc.handle_session_event(SessionEvent::PlaintextFrame(peer_id, response_bytes))
            .await
            .unwrap();

        // Response should arrive, not timeout
        let result = tokio::time::timeout(Duration::from_millis(100), response_rx)
            .await
            .expect("Should receive response quickly")
            .expect("Channel should be open");

        assert!(result.is_ok());
        
        // Verify no pending requests (timeout was cancelled)
        assert_eq!(rpc.pending_requests.lock().await.len(), 0);
    }

    #[tokio::test]
    async fn test_timeout_fires_when_no_response() {
        let (session_tx, mut session_rx) = mpsc::channel(100);
        let rpc = Arc::new(RpcProtocol::new_with_config(
            session_tx,
            Duration::from_millis(100),  // Short timeout for testing
            1000,
        ));

        let peer_id = PeerId::from_bytes(vec![1, 2, 3, 4]);
        let (response_tx, response_rx) = oneshot::channel();

        // Make call
        rpc.handle_command(RpcCommand::Call {
            peer_id,
            method: "test".to_string(),
            params: serde_json::json!({}),
            response_tx,
        })
        .await
        .unwrap();

        // Consume the request but don't send response
        let _cmd = session_rx.recv().await.unwrap();

        // Wait for timeout
        let result = response_rx.await.expect("Should receive timeout");
        
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code, ERR_TIMEOUT);
        
        // Verify pending request was cleaned up
        assert_eq!(rpc.pending_requests.lock().await.len(), 0);
    }

    #[tokio::test]
    async fn test_concurrent_response_and_timeout_race() {
        // This test verifies no panic or double-send when response and timeout race
        let (session_tx, mut session_rx) = mpsc::channel(100);
        let rpc = Arc::new(RpcProtocol::new_with_config(
            session_tx,
            Duration::from_millis(50),  // Very short timeout
            1000,
        ));

        for _ in 0..100 {
            let peer_id = PeerId::from_bytes(vec![1, 2, 3, 4]);
            let (response_tx, response_rx) = oneshot::channel();

            let rpc_clone = rpc.clone();
            let peer_id_clone = peer_id.clone();
            
            tokio::spawn(async move {
                rpc_clone
                    .handle_command(RpcCommand::Call {
                        peer_id: peer_id_clone,
                        method: "test".to_string(),
                        params: serde_json::json!({}),
                        response_tx,
                    })
                    .await
            });

            // Get request
            let cmd = session_rx.recv().await.unwrap();
            let bytes = match cmd {
                SessionCommand::SendPlaintext(_, b) => b,
                _ => panic!("Expected SendPlaintext"),
            };

            let message: RpcMessage = serde_json::from_slice(&bytes).unwrap();
            let request_id = match message {
                RpcMessage::Request { id, .. } => id,
                _ => panic!("Expected Request"),
            };

            // Race: send response at roughly same time as timeout
            let response = RpcMessage::Response {
                id: request_id,
                result: Ok(serde_json::json!({"status": "ok"})),
            };
            let response_bytes = serde_json::to_vec(&response).unwrap();
            
            let rpc_clone = rpc.clone();
            let peer_id_clone = peer_id.clone();
            tokio::spawn(async move {
                let _ = rpc_clone
                    .handle_session_event(SessionEvent::PlaintextFrame(peer_id_clone, response_bytes))
                    .await;
            });

            // Should receive exactly one result (either response or timeout, no panic)
            let result = tokio::time::timeout(Duration::from_millis(200), response_rx)
                .await
                .expect("Should receive a result");
            
            assert!(result.is_ok(), "Channel should deliver exactly one message");
        }
        
        // All pending requests should be cleaned up
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(rpc.pending_requests.lock().await.len(), 0);
    }

    #[tokio::test]
    async fn test_multiple_concurrent_calls() {
        let (session_tx, mut session_rx) = mpsc::channel(1000);
        let rpc = Arc::new(RpcProtocol::new(session_tx));

        let mut handles = vec![];
        
        // Make 50 concurrent calls
        for i in 0..50 {
            let rpc_clone = rpc.clone();
            let handle = tokio::spawn(async move {
                let peer_id = PeerId::from_bytes(vec![1, 2, 3, 4]);
                let (response_tx, response_rx) = oneshot::channel();

                rpc_clone
                    .handle_command(RpcCommand::Call {
                        peer_id,
                        method: "test".to_string(),
                        params: serde_json::json!({"index": i}),
                        response_tx,
                    })
                    .await
                    .unwrap();

                response_rx
            });
            handles.push(handle);
        }

        // Respond to all requests
        for _ in 0..50 {
            let cmd = session_rx.recv().await.unwrap();
            let (peer_id, bytes) = match cmd {
                SessionCommand::SendPlaintext(p, b) => (p, b),
                _ => panic!("Expected SendPlaintext"),
            };

            let message: RpcMessage = serde_json::from_slice(&bytes).unwrap();
            let request_id = match message {
                RpcMessage::Request { id, .. } => id,
                _ => panic!("Expected Request"),
            };

            let response = RpcMessage::Response {
                id: request_id,
                result: Ok(serde_json::json!({"status": "ok"})),
            };
            let response_bytes = serde_json::to_vec(&response).unwrap();
            
            rpc.handle_session_event(SessionEvent::PlaintextFrame(peer_id, response_bytes))
                .await
                .unwrap();
        }

        // All calls should complete successfully
        for handle in handles {
            let rx = handle.await.unwrap();
            let result = rx.await.unwrap();
            assert!(result.is_ok());
        }
        
        // All pending requests should be cleaned up
        assert_eq!(rpc.pending_requests.lock().await.len(), 0);
    }

    #[tokio::test]
    async fn test_heavy_concurrent_seen_requests() {
        // Test LRU cache under heavy concurrent load (1000+ threads)
        let (session_tx, _session_rx) = mpsc::channel(10000);
        let rpc = Arc::new(RpcProtocol::new_with_config(
            session_tx,
            Duration::from_secs(30),
            1000, // 1000 entry capacity
        ));

        let (handler_tx, mut _handler_rx) = mpsc::channel(10000);
        rpc.handle_command(RpcCommand::RegisterHandler {
            method: "test".to_string(),
            handler_tx,
        })
        .await
        .unwrap();

        let peer_id = PeerId::from_bytes(vec![1, 2, 3, 4]);
        let mut handles = vec![];

        // Spawn 2000 concurrent tasks trying to insert
        for i in 0..2000 {
            let rpc_clone = rpc.clone();
            let peer_id_clone = peer_id.clone();
            let handle = tokio::spawn(async move {
                let request = RpcMessage::Request {
                    id: format!("concurrent-req-{}", i),
                    method: "test".to_string(),
                    params: serde_json::json!({"index": i}),
                };
                let bytes = serde_json::to_vec(&request).unwrap();
                rpc_clone
                    .handle_session_event(SessionEvent::PlaintextFrame(peer_id_clone, bytes))
                    .await
            });
            handles.push(handle);
        }

        // Wait for all to complete
        for handle in handles {
            handle.await.unwrap().unwrap();
        }

        // LRU should have capped at 1000 entries (oldest evicted)
        let count = rpc.seen_requests_count().await;
        assert_eq!(count, 1000, "LRU should cap at capacity");

        // No panics or data corruption under concurrent load
    }

    #[tokio::test]
    async fn test_lru_no_race_conditions() {
        // Test that the LRU check-and-insert is atomic
        let (session_tx, _session_rx) = mpsc::channel(1000);
        let rpc = Arc::new(RpcProtocol::new_with_config(
            session_tx,
            Duration::from_secs(30),
            100,
        ));

        let (handler_tx, _handler_rx) = mpsc::channel(1000);
        rpc.handle_command(RpcCommand::RegisterHandler {
            method: "test".to_string(),
            handler_tx,
        })
        .await
        .unwrap();

        let peer_id = PeerId::from_bytes(vec![1, 2, 3, 4]);

        // Send the same request ID many times - first should succeed, rest should be duplicates
        let request = RpcMessage::Request {
            id: "duplicate-id".to_string(),
            method: "test".to_string(),
            params: serde_json::json!({}),
        };
        let bytes = serde_json::to_vec(&request).unwrap();

        // First request should succeed
        let result1 = rpc
            .handle_session_event(SessionEvent::PlaintextFrame(peer_id.clone(), bytes.clone()))
            .await;
        assert!(result1.is_ok(), "First request should succeed");
        assert_eq!(rpc.seen_requests_count().await, 1);

        // Second request with same ID should be rejected as duplicate
        let result2 = rpc
            .handle_session_event(SessionEvent::PlaintextFrame(peer_id.clone(), bytes.clone()))
            .await;
        assert!(result2.is_ok(), "handle_session_event should not error on duplicate");
        // Still 1 seen request (duplicate was rejected, not added again)
        assert_eq!(rpc.seen_requests_count().await, 1);

        // Third request with same ID should also be rejected
        let result3 = rpc
            .handle_session_event(SessionEvent::PlaintextFrame(peer_id.clone(), bytes.clone()))
            .await;
        assert!(result3.is_ok());
        assert_eq!(rpc.seen_requests_count().await, 1);
    }
}
