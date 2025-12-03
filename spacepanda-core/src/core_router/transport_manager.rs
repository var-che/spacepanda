/*
  TransportManager - raw transport (TCP/QUIC/WS)

  Abstracts the OS sockets and handles dialing/listening and reconnects. Provides stream of
  raw frames for the session_manager.rs to handle.

  Workflow:
  Listens on configured addresses (TCP/UDP/QUIC) and spawns tasks to accept incoming connections.
  When dialing, it establishes a socket and returns conn_id.


  Handles NAT traversal helpers (STUN/TURN) if configured.

  Inputs:
  are the following commands:
    - Dial(addr) -> attempts to connect to addr, emits Connected event on success
    - Listen(addr) -> starts listening on addr for incoming connections
    - Send(conn_id, bytes) -> sends bytes on the specified connection
    - Close(conn_id) -> closes the specified connection

  Outputs:
    Emits `TransportEvent::Connected(conn_id, remote_addr)` when a new connection is established.
    Emits `TransportEvent::Data(conn_id, bytes)` when data is received on a connection.
    Emits `TransportEvent::Disconnected(conn_id)` when a connection is closed.

  Important:
  Always perform basic framing (prefix length) on the bytes sent/received to avoid message boundary issues.
  Keep this module ignorant of identities; its only bytes and addresses.

┌─────────────────────────────────────────────────────────┐
│                   TransportManager                       │
├─────────────────────────────────────────────────────────┤
│                                                           │
│  Commands In (from your app):                            │
│    • Listen("0.0.0.0:8080") ──► Spawn Listener Task     │
│    • Dial("peer.com:8080")  ──► Spawn Dial + Reader     │
│    • Send(conn_id, bytes)   ──► Write to socket         │
│    • Close(conn_id)         ──► Shutdown socket         │
│                                                           │
│  Events Out (to your app):                               │
│    • Connected(conn_id, addr) ◄── New connection         │
│    • Data(conn_id, bytes)     ◄── Received data          │
│    • Disconnected(conn_id)    ◄── Connection closed      │
│                                                           │
└─────────────────────────────────────────────────────────┘

Internal Tasks:
┌──────────────┐       ┌──────────────┐       ┌──────────────┐
│   Listener   │       │   Conn #1    │       │   Conn #2    │
│    Task      │──────►│ Reader Task  │       │ Reader Task  │
│ (accept loop)│       │              │       │              │
└──────────────┘       └──────────────┘       └──────────────┘
                               │                       │
                               └───────┬───────────────┘
                                       │
                                  Events sent here

*/
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, Mutex};

#[derive(Debug)]
pub enum TransportCommand {
    Dial(String),
    Listen(String),
    Send(u64, Vec<u8>),
    Close(u64),
}

#[derive(Debug, Clone)]
pub enum TransportEvent {
    Connected(u64, String),
    Data(u64, Vec<u8>),
    Disconnected(u64),
}

pub struct TransportManager {
    connections: Arc<Mutex<HashMap<u64, TcpStream>>>,
    next_conn_id: Arc<AtomicU64>,
    event_tx: mpsc::Sender<TransportEvent>,
}

impl TransportManager {
    pub fn new(event_tx: mpsc::Sender<TransportEvent>) -> Self {
        TransportManager {
            connections: Arc::new(Mutex::new(HashMap::new())),
            next_conn_id: Arc::new(AtomicU64::new(1)),
            event_tx,
        }
    }

    pub async fn handle_command(&self, command: TransportCommand) -> Result<(), String> {
        match command {
            TransportCommand::Dial(addr) => {
                self.handle_dial(addr).await?;
            }
            TransportCommand::Listen(addr) => {
                self.handle_listen(addr).await?;
            }
            TransportCommand::Send(conn_id, bytes) => {
                self.handle_send(conn_id, bytes).await?;
            }
            TransportCommand::Close(conn_id) => {
                self.handle_close(conn_id).await?;
            }
        }
        Ok(())
    }

    async fn handle_listen(&self, addr: String) -> Result<(), String> {
        let listener = TcpListener::bind(&addr)
            .await
            .map_err(|e| format!("Failed to bind to {}: {}", addr, e))?;

        let connections = self.connections.clone();
        let next_conn_id = self.next_conn_id.clone();
        let event_tx = self.event_tx.clone();

        tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((socket, peer_addr)) => {
                        let conn_id = next_conn_id.fetch_add(1, Ordering::SeqCst);

                        // Store the connection
                        connections.lock().await.insert(conn_id, socket);

                        // Emit Connected event
                        let addr_str = peer_addr.to_string();
                        if let Err(e) =
                            event_tx.send(TransportEvent::Connected(conn_id, addr_str)).await
                        {
                            eprintln!("Failed to send Connected event: {}", e);
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to accept connection: {}", e);
                    }
                }
            }
        });

        Ok(())
    }

    async fn handle_dial(&self, addr: String) -> Result<(), String> {
        let socket = TcpStream::connect(&addr)
            .await
            .map_err(|e| format!("Failed to connect to {}: {}", addr, e))?;

        let conn_id = self.next_conn_id.fetch_add(1, Ordering::SeqCst);

        self.connections.lock().await.insert(conn_id, socket);

        // Emit Connected event
        if let Err(e) = self.event_tx.send(TransportEvent::Connected(conn_id, addr)).await {
            eprintln!("Failed to send Connected event: {}", e);
        }

        Ok(())
    }

    async fn handle_send(&self, conn_id: u64, bytes: Vec<u8>) -> Result<(), String> {
        let mut connections = self.connections.lock().await;
        let socket = connections
            .get_mut(&conn_id)
            .ok_or_else(|| format!("Connection {} not found", conn_id))?;

        // Frame the message: 4-byte length prefix + data
        let len = bytes.len() as u32;
        socket
            .write_all(&len.to_be_bytes())
            .await
            .map_err(|e| format!("Failed to write length: {}", e))?;
        socket
            .write_all(&bytes)
            .await
            .map_err(|e| format!("Failed to write data: {}", e))?;

        Ok(())
    }

    async fn handle_close(&self, conn_id: u64) -> Result<(), String> {
        let mut connections = self.connections.lock().await;
        connections
            .remove(&conn_id)
            .ok_or_else(|| format!("Connection {} not found", conn_id))?;

        // Emit Disconnected event
        if let Err(e) = self.event_tx.send(TransportEvent::Disconnected(conn_id)).await {
            eprintln!("Failed to send Disconnected event: {}", e);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_listen_accepts_connections() {
        // Create event channel
        let (event_tx, mut event_rx) = mpsc::channel(100);

        // Create TransportManager with a fixed port
        let test_port = 18080;
        let listen_addr = format!("127.0.0.1:{}", test_port);

        let manager = TransportManager::new(event_tx);
        manager
            .handle_command(TransportCommand::Listen(listen_addr.clone()))
            .await
            .expect("Failed to start listening");

        sleep(Duration::from_millis(50)).await;

        // Connect a client to the listener
        let _client =
            TcpStream::connect(&listen_addr).await.expect("Failed to connect to listener");

        // Wait for the Connected event
        let event = tokio::time::timeout(Duration::from_secs(2), event_rx.recv())
            .await
            .expect("Timeout waiting for event")
            .expect("Event channel closed");

        // Verify we got a Connected event
        match event {
            TransportEvent::Connected(conn_id, addr) => {
                assert!(conn_id > 0, "Connection ID should be positive");
                assert!(addr.starts_with("127.0.0.1:"), "Address should be localhost");
            }
            _ => panic!("Expected Connected event, got {:?}", event),
        }
    }

    #[tokio::test]
    async fn test_listen_multiple_connections() {
        let (event_tx, mut event_rx) = mpsc::channel(100);
        let manager = TransportManager::new(event_tx);

        let test_port = 18081;
        let listen_addr = format!("127.0.0.1:{}", test_port);

        manager
            .handle_command(TransportCommand::Listen(listen_addr.clone()))
            .await
            .expect("Failed to start listening");

        sleep(Duration::from_millis(50)).await;

        // Connect multiple clients
        let client1 = TcpStream::connect(&listen_addr).await.unwrap();
        let client2 = TcpStream::connect(&listen_addr).await.unwrap();
        let client3 = TcpStream::connect(&listen_addr).await.unwrap();

        // Collect events
        let mut conn_ids = Vec::new();
        for _ in 0..3 {
            let event = tokio::time::timeout(Duration::from_secs(2), event_rx.recv())
                .await
                .expect("Timeout")
                .expect("Channel closed");

            if let TransportEvent::Connected(conn_id, _) = event {
                conn_ids.push(conn_id);
            } else {
                panic!("Expected Connected event");
            }
        }

        // Verify all connection IDs are unique
        assert_eq!(conn_ids.len(), 3);
        assert_ne!(conn_ids[0], conn_ids[1]);
        assert_ne!(conn_ids[1], conn_ids[2]);
        assert_ne!(conn_ids[0], conn_ids[2]);

        // Clean up
        drop(client1);
        drop(client2);
        drop(client3);
    }

    #[tokio::test]
    async fn test_listen_invalid_address() {
        let (event_tx, _event_rx) = mpsc::channel(100);
        let manager = TransportManager::new(event_tx);

        // Try to listen on an invalid address
        let result = manager
            .handle_command(TransportCommand::Listen("invalid:address".to_string()))
            .await;

        assert!(result.is_err(), "Should fail with invalid address");
    }
}
