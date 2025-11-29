/*
   RouterHandle

   Single entry point for the rest of the application to interact with the router.
   It exposes concise async methods for starting/stopping the router, sending/receiving messages,
   and managing connections.

   E.G: send_direct(peer_id, bytes) -> Result<()>
        send_anonymous(peer_id, bytes) -> Result<()>
        broadcast(topic, bytes) -> Result<()>
        subscribe(topic) -> Stream<InnerEnvelope>
        rpc_call(peer_id, method, params) -> Result<response_bytes>

    Workflow and where it sits?

    It is called by the APP (MLS layer), to send encrypted payloads to other peers.
    Internally it packages a RouterCommand and pushes it onto the router actor main channel.
    And it awaits responses for RPC (correlated by request ID).

    Example: `send_anonymous(peer_id, mls_ciphertext)` will create a
     `RouterCommand::OverlaySend { dest:peer_id, payload }`

    Architecture:

    ┌────────────────────────────────────────────────────┐
    │              Application Layer (MLS)               │
    │          (sends/receives encrypted messages)       │
    └─────────────────┬──────────────────────────────────┘
                      │
                      │ RouterHandle API
                      │ • send_direct(peer, data)
                      │ • rpc_call(peer, method, params)
                      │ • listen(addr)
                      │ • dial(addr)
                      │
    ┌─────────────────▼────────────────────────────────┐
    │              RouterHandle                        │
    │  (spawns and coordinates routing components)     │
    │                                                  │
    │  ┌───────────────────────────────────────────┐   │
    │  │        Router Event Loop                  │   │
    │  │  (processes events from all layers)       │   │
    │  └──┬───────────────────────────────────┬───-┘   │
    │     │                                   │        │
    │  ┌──▼────────┐  ┌──────────┐  ┌─────────▼───┐    │
    │  │ Transport │  │ Session  │  │ RPC Protocol│    │
    │  │ Manager   │─►│ Manager  │─►│             │    │
    │  │  (TCP)    │  │ (Noise)  │  │ (Req/Resp)  │    │
    │  └───────────┘  └──────────┘  └─────────────┘    │
    └──────────────────────────────────────────────────┘

*/

use serde_json::Value as JsonValue;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, Mutex};
use tokio::task::JoinHandle;

use super::onion_router::{OnionCommand, OnionConfig, OnionEvent, OnionRouter};
use super::route_table::RouteTable;
use super::rpc_protocol::{RpcCommand, RpcError, RpcProtocol};
use super::session_manager::{PeerId, SessionCommand, SessionEvent, SessionManager};
use super::transport_manager::{TransportCommand, TransportManager};

/// Commands sent to the router
#[derive(Debug)]
pub enum RouterCommand {
    /// Start listening on an address
    Listen(String),
    /// Dial a peer at an address
    Dial(String),
    /// Send data directly to a peer (encrypted)
    SendDirect(PeerId, Vec<u8>),
    /// Send data anonymously via onion routing
    SendAnonymous {
        destination: PeerId,
        payload: Vec<u8>,
        response_tx: oneshot::Sender<Result<(), String>>,
    },
    /// Make an RPC call to a peer
    RpcCall {
        peer_id: PeerId,
        method: String,
        params: JsonValue,
        response_tx: oneshot::Sender<Result<JsonValue, RpcError>>,
    },
    /// Register an RPC method handler
    RegisterRpcHandler {
        method: String,
        handler_tx: mpsc::Sender<super::rpc_protocol::RpcRequest>,
    },
    /// Shutdown the router
    Shutdown,
}

/// Events emitted by the router
#[derive(Debug, Clone)]
pub enum RouterEvent {
    /// Successfully listening on an address
    Listening(String),
    /// Connected to a peer
    PeerConnected(PeerId),
    /// Received data from a peer
    DataReceived(PeerId, Vec<u8>),
    /// Peer disconnected
    PeerDisconnected(PeerId),
}

/// Handle to interact with the router
#[derive(Clone)]
pub struct RouterHandle {
    command_tx: mpsc::Sender<RouterCommand>,
    event_rx: Arc<Mutex<mpsc::Receiver<RouterEvent>>>,
}

impl RouterHandle {
    /// Create a new router and spawn its event loop
    pub fn new() -> (Self, JoinHandle<()>) {
        let (command_tx, command_rx) = mpsc::channel(100);
        let (event_tx, event_rx) = mpsc::channel(100);

        let router = Router::new(command_rx, event_tx);
        let handle = router.spawn();

        (
            RouterHandle {
                command_tx,
                event_rx: Arc::new(Mutex::new(event_rx)),
            },
            handle,
        )
    }

    /// Start listening on an address
    pub async fn listen(&self, addr: String) -> Result<(), String> {
        self.command_tx
            .send(RouterCommand::Listen(addr))
            .await
            .map_err(|e| format!("Failed to send listen command: {}", e))
    }

    /// Dial a peer at an address
    pub async fn dial(&self, addr: String) -> Result<(), String> {
        self.command_tx
            .send(RouterCommand::Dial(addr))
            .await
            .map_err(|e| format!("Failed to send dial command: {}", e))
    }

    /// Send data directly to a peer
    pub async fn send_direct(&self, peer_id: PeerId, data: Vec<u8>) -> Result<(), String> {
        self.command_tx
            .send(RouterCommand::SendDirect(peer_id, data))
            .await
            .map_err(|e| format!("Failed to send data: {}", e))
    }

    /// Send data anonymously via onion routing
    pub async fn send_anonymous(&self, destination: PeerId, payload: Vec<u8>) -> Result<(), String> {
        let (response_tx, response_rx) = oneshot::channel();

        self.command_tx
            .send(RouterCommand::SendAnonymous {
                destination,
                payload,
                response_tx,
            })
            .await
            .map_err(|e| format!("Failed to send anonymous command: {}", e))?;

        response_rx
            .await
            .map_err(|e| format!("Anonymous send response channel closed: {}", e))?
    }

    /// Make an RPC call to a peer
    pub async fn rpc_call(
        &self,
        peer_id: PeerId,
        method: String,
        params: JsonValue,
    ) -> Result<JsonValue, RpcError> {
        let (response_tx, response_rx) = oneshot::channel();

        self.command_tx
            .send(RouterCommand::RpcCall {
                peer_id,
                method,
                params,
                response_tx,
            })
            .await
            .map_err(|e| RpcError::internal_error(&format!("Failed to send RPC command: {}", e)))?;

        response_rx
            .await
            .map_err(|e| RpcError::internal_error(&format!("RPC response channel closed: {}", e)))?
    }

    /// Register an RPC method handler
    pub async fn register_rpc_handler(
        &self,
        method: String,
        handler_tx: mpsc::Sender<super::rpc_protocol::RpcRequest>,
    ) -> Result<(), String> {
        self.command_tx
            .send(RouterCommand::RegisterRpcHandler {
                method,
                handler_tx,
            })
            .await
            .map_err(|e| format!("Failed to register RPC handler: {}", e))
    }

    /// Receive the next router event
    pub async fn next_event(&self) -> Option<RouterEvent> {
        self.event_rx.lock().await.recv().await
    }

    /// Shutdown the router
    pub async fn shutdown(&self) -> Result<(), String> {
        self.command_tx
            .send(RouterCommand::Shutdown)
            .await
            .map_err(|e| format!("Failed to send shutdown command: {}", e))
    }
}

/// Internal router orchestrating all components
struct Router {
    command_rx: mpsc::Receiver<RouterCommand>,
    event_tx: mpsc::Sender<RouterEvent>,
    rpc_protocol: RpcProtocol,
    transport_tx: mpsc::Sender<TransportCommand>,
    session_tx: mpsc::Sender<SessionCommand>,
    onion_tx: Option<mpsc::Sender<OnionCommand>>,
}

impl Router {
    fn new(command_rx: mpsc::Receiver<RouterCommand>, event_tx: mpsc::Sender<RouterEvent>) -> Self {
        let (transport_tx, _transport_rx) = mpsc::channel(100);
        let (session_tx, _session_rx) = mpsc::channel(100);

        // Create dummy event channels for now - we'll recreate managers in spawn()
        let rpc_protocol = RpcProtocol::new(session_tx.clone());

        Router {
            command_rx,
            event_tx,
            rpc_protocol,
            transport_tx,
            session_tx,
            onion_tx: None,
        }
    }

    fn spawn(mut self) -> JoinHandle<()> {
        tokio::spawn(async move {
            self.run().await;
        })
    }

    async fn run(&mut self) {
        let (transport_event_tx, mut transport_event_rx) = mpsc::channel(100);
        let (session_event_tx, mut session_event_rx) = mpsc::channel(100);

        // Create new transport manager with proper event channel
        let (transport_cmd_tx, mut transport_cmd_rx) = mpsc::channel(100);
        let transport_manager = TransportManager::new(transport_event_tx.clone());
        self.transport_tx = transport_cmd_tx;

        // Spawn transport manager task
        tokio::spawn(async move {
            while let Some(cmd) = transport_cmd_rx.recv().await {
                if let Err(e) = transport_manager.handle_command(cmd).await {
                    eprintln!("Transport manager error: {}", e);
                }
            }
        });

        // Create new session manager with proper event channel
        let (session_cmd_tx, mut session_cmd_rx) = mpsc::channel(100);
        let static_keypair = vec![0u8; 32]; // Placeholder keypair
        let session_manager = Arc::new(SessionManager::new(
            static_keypair,
            self.transport_tx.clone(),
            session_event_tx.clone(),
        ));
        self.session_tx = session_cmd_tx;

        // Spawn session manager task
        let session_mgr = session_manager.clone();
        tokio::spawn(async move {
            while let Some(cmd) = session_cmd_rx.recv().await {
                if let Err(e) = session_mgr.handle_command(cmd).await {
                    eprintln!("Session manager error: {}", e);
                }
            }
        });

        // Spawn session manager transport event handler
        let session_mgr_transport = session_manager.clone();
        tokio::spawn(async move {
            while let Some(event) = transport_event_rx.recv().await {
                if let Err(e) = session_mgr_transport.handle_transport_event(event).await {
                    eprintln!("Session manager transport event error: {}", e);
                }
            }
        });

        // Create and spawn onion router
        let route_table = Arc::new(RouteTable::new());
        let (onion_event_tx, mut onion_event_rx) = mpsc::channel(100);
        let onion_config = OnionConfig::default();
        let onion_router = Arc::new(OnionRouter::new(onion_config, route_table, onion_event_tx));
        
        let (onion_cmd_tx, onion_cmd_rx) = mpsc::channel(100);
        self.onion_tx = Some(onion_cmd_tx);
        
        let onion_router_clone = onion_router.clone();
        tokio::spawn(async move {
            onion_router_clone.run(onion_cmd_rx).await;
        });

        // Handle onion router events
        let session_tx_onion = self.session_tx.clone();
        tokio::spawn(async move {
            while let Some(event) = onion_event_rx.recv().await {
                match event {
                    OnionEvent::PacketForward { next_peer, blob } => {
                        // Forward the onion packet to the next hop
                        let _ = session_tx_onion
                            .send(SessionCommand::SendPlaintext(next_peer, blob))
                            .await;
                    }
                    OnionEvent::DeliverLocal { envelope } => {
                        // TODO: Deliver to application layer
                        eprintln!("Onion packet delivered locally: {:?}", envelope);
                    }
                    OnionEvent::CircuitBuilt { path_length } => {
                        eprintln!("Onion circuit built with {} hops", path_length);
                    }
                    OnionEvent::RelayError { error } => {
                        eprintln!("Onion relay error: {}", error);
                    }
                }
            }
        });

        loop {
            tokio::select! {
                // Handle router commands from application
                Some(cmd) = self.command_rx.recv() => {
                    if let Err(_) = self.handle_command(cmd).await {
                        break;
                    }
                }

                // Handle session events
                Some(event) = session_event_rx.recv() => {
                    if let Err(e) = self.handle_session_event(event).await {
                        eprintln!("Error handling session event: {}", e);
                    }
                }

                else => break,
            }
        }
    }

    async fn handle_command(&mut self, command: RouterCommand) -> Result<(), String> {
        match command {
            RouterCommand::Listen(addr) => {
                let addr_clone = addr.clone();
                self.transport_tx
                    .send(TransportCommand::Listen(addr))
                    .await
                    .map_err(|e| format!("Failed to send listen command: {}", e))?;
                // Note: We'll emit Listening event when we get Connected from transport
                let _ = self.event_tx.send(RouterEvent::Listening(addr_clone)).await;
            }
            RouterCommand::Dial(addr) => {
                self.transport_tx
                    .send(TransportCommand::Dial(addr))
                    .await
                    .map_err(|e| format!("Failed to send dial command: {}", e))?;
            }
            RouterCommand::SendDirect(peer_id, data) => {
                self.session_tx
                    .send(SessionCommand::SendPlaintext(peer_id, data))
                    .await
                    .map_err(|e| format!("Failed to send data via session: {}", e))?;
            }
            RouterCommand::SendAnonymous {
                destination,
                payload,
                response_tx,
            } => {
                if let Some(ref onion_tx) = self.onion_tx {
                    onion_tx
                        .send(OnionCommand::Send {
                            destination,
                            payload,
                            response_tx: Some(response_tx),
                        })
                        .await
                        .map_err(|e| format!("Failed to send to onion router: {}", e))?;
                } else {
                    let _ = response_tx.send(Err("Onion router not initialized".to_string()));
                }
            }
            RouterCommand::RpcCall {
                peer_id,
                method,
                params,
                response_tx,
            } => {
                self.rpc_protocol
                    .handle_command(RpcCommand::Call {
                        peer_id,
                        method,
                        params,
                        response_tx,
                    })
                    .await?;
            }
            RouterCommand::RegisterRpcHandler {
                method,
                handler_tx,
            } => {
                self.rpc_protocol
                    .handle_command(RpcCommand::RegisterHandler {
                        method,
                        handler_tx,
                    })
                    .await?;
            }
            RouterCommand::Shutdown => {
                return Err("Shutdown requested".to_string());
            }
        }
        Ok(())
    }



    async fn handle_session_event(&mut self, event: SessionEvent) -> Result<(), String> {
        match event.clone() {
            SessionEvent::Established(peer_id, _conn_id) => {
                let _ = self
                    .event_tx
                    .send(RouterEvent::PeerConnected(peer_id))
                    .await;
            }
            SessionEvent::PlaintextFrame(peer_id, data) => {
                // First try RPC protocol
                if let Err(_) = self.rpc_protocol.handle_session_event(event.clone()).await {
                    // If not an RPC message, emit as data received
                    let _ = self
                        .event_tx
                        .send(RouterEvent::DataReceived(peer_id, data))
                        .await;
                }
            }
            SessionEvent::Closed(peer_id) => {
                let _ = self
                    .event_tx
                    .send(RouterEvent::PeerDisconnected(peer_id))
                    .await;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::time::timeout;

    #[tokio::test]
    async fn test_router_handle_creation() {
        let (handle, router_task) = RouterHandle::new();

        // Verify we can send commands
        assert!(handle.listen("127.0.0.1:0".to_string()).await.is_ok());

        // Shutdown
        handle.shutdown().await.unwrap();

        // Wait for router to finish
        let _ = timeout(Duration::from_secs(1), router_task).await;
    }

    #[tokio::test]
    async fn test_router_listen_event() {
        let (handle, _router_task) = RouterHandle::new();

        // Start listening
        handle.listen("127.0.0.1:0".to_string()).await.unwrap();

        // Should receive a Listening event
        let event = timeout(Duration::from_millis(500), handle.next_event())
            .await
            .unwrap()
            .unwrap();

        match event {
            RouterEvent::Listening(addr) => {
                assert!(addr.starts_with("127.0.0.1"));
            }
            _ => panic!("Expected Listening event, got {:?}", event),
        }

        handle.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_router_send_direct() {
        let (handle, _router_task) = RouterHandle::new();

        let peer_id = PeerId::from_bytes(vec![1, 2, 3, 4]);
        let data = vec![5, 6, 7, 8];

        // This will fail since no actual peer exists, but we test the API
        let result = handle.send_direct(peer_id, data).await;
        // Should succeed in sending command (even if routing fails internally)
        assert!(result.is_ok());

        handle.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_router_rpc_call() {
        let (handle, _router_task) = RouterHandle::new();

        // Register an RPC handler
        let (handler_tx, _handler_rx) = mpsc::channel(10);
        handle
            .register_rpc_handler("test_method".to_string(), handler_tx)
            .await
            .unwrap();

        // Make an RPC call (will timeout since no peer exists)
        let peer_id = PeerId::from_bytes(vec![9, 10, 11, 12]);
        let call_future = handle.rpc_call(
            peer_id,
            "test_method".to_string(),
            serde_json::json!({"key": "value"}),
        );

        // The call should timeout since there's no actual peer to respond
        // We give it a short timeout to detect if it times out quickly
        let result = timeout(Duration::from_millis(50), call_future).await;
        
        // Either it times out (Err) or returns an error (Ok(Err))
        // Both cases are valid since there's no peer
        match result {
            Err(_) => {} // Timeout - expected
            Ok(Err(_)) => {} // RPC error - also expected (no session)
            Ok(Ok(_)) => panic!("Unexpected success - there's no peer to respond"),
        }

        // Don't call shutdown - the router may already be down
        let _ = handle.shutdown().await;
    }

    #[tokio::test]
    async fn test_router_register_handler() {
        let (handle, _router_task) = RouterHandle::new();

        let (handler_tx, _handler_rx) = mpsc::channel(10);

        // Should successfully register a handler
        let result = handle
            .register_rpc_handler("my_method".to_string(), handler_tx)
            .await;

        assert!(result.is_ok());

        handle.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_router_dial() {
        let (handle, _router_task) = RouterHandle::new();

        // Dial an address (will fail to connect, but API should work)
        let result = handle.dial("127.0.0.1:9999".to_string()).await;
        assert!(result.is_ok());

        handle.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_router_multiple_commands() {
        let (handle, _router_task) = RouterHandle::new();

        // Send multiple commands
        handle.listen("127.0.0.1:0".to_string()).await.unwrap();
        handle.dial("127.0.0.1:8888".to_string()).await.unwrap();

        let peer_id = PeerId::from_bytes(vec![13, 14, 15, 16]);
        handle
            .send_direct(peer_id, vec![1, 2, 3])
            .await
            .unwrap();

        // Should receive at least the listening event
        let event = timeout(Duration::from_millis(500), handle.next_event())
            .await
            .unwrap();
        assert!(event.is_some());

        handle.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_router_send_anonymous() {
        let (handle, _router_task) = RouterHandle::new();

        let destination = PeerId::from_bytes(vec![99, 99, 99, 99]);
        let payload = vec![10, 20, 30, 40];

        // This will fail since there are no relays in the route table
        // But we test that the API works correctly
        let result = handle.send_anonymous(destination, payload).await;
        
        // Should get an error about no relays being available
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.contains("No relays available") || e.contains("onion"));
        }

        handle.shutdown().await.unwrap();
    }
}
