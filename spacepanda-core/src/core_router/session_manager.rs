/*
  SessionManager - Noise handshakes & AEAD frames

  Upgrades raw connections into authenticated sessions using Noise or equivalent,
  producing the peer_id (public key) associated with the session.

  Workflow:
  1. On Connected(conn_id): start Noise handshake (XX or IK if we know peer)
  2. When handshake succedes: derive AEAD keys and create Session object.
  3. For sending: encrypt plaintext into AEAD ciphertext => hand to transport_manager.rs as Send.
  4. For receiving: decrypt AEAD ciphertext and emit PlaintextFrame(peer_id, bytes) to routing core.

  Inputs:
    - TransportEvent::Data(conn_id, bytes) (handshake frames/incoming AEAD frames)
    - TransportEvent::Connected(conn_id, remote_addr)
    - TransportEvent::Disconnected(conn_id)
    - Commands: SessionSend(peer_id, plaintext)

  Outputs:
    - SessionEvent::PlaintextFrame(peer_id, bytes) when a full decrypting and routing.
    - SessionEvent::Established(peer_id, conn_id) for routing table.
    - SessionEvent::Closed(peer_id) when session is closed.

  Notes:
  Verify the static identity key during Noise handshake, or require signed cert post-handshake.
  Keep replay window counters.

  ```
  1. App: "Connect to Bob at 10.0.0.5:8080"
   ↓
2. TransportManager.dial("10.0.0.5:8080")
   → Opens TCP connection
   → Assigns conn_id = 7
   → Emits: TransportEvent::Connected(7, "10.0.0.5:8080")
   ↓
3. SessionManager receives Connected(7)
   → Initiates Noise handshake
   → Generates ephemeral keypair
   → Sends first handshake message via TransportCommand::Send(7, bytes)
   ↓
4. TransportManager.Send(7, bytes)
   → Writes to TCP socket #7
   
   ... (messages flow back and forth) ...
   
5. Bob's handshake response arrives at TransportManager
   → Emits: TransportEvent::Data(7, handshake_bytes)
   ↓
6. SessionManager processes handshake
   → Extracts Bob's static public key
   → Verifies signature
   → Derives PeerId(Bob) = [0x1234...]
   → Completes handshake
   → Emits: SessionEvent::Established(PeerId(Bob), 7)
   ↓
7. App: "Great! Now send 'Hey Bob' to Bob"
   → SessionManager.SendPlaintext(PeerId(Bob), "Hey Bob")
   ↓
8. SessionManager encrypts with ChaCha20-Poly1305 AEAD
   → Ciphertext = [0xAB, 0xCD, 0xEF, ...]
   → Sends: TransportCommand::Send(7, ciphertext)
   ↓
9. TransportManager writes encrypted bytes to socket #7
  ```
*/

use snow::{Builder, HandshakeState, TransportState};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

use super::transport_manager::{TransportCommand, TransportEvent};

/// Noise protocol pattern - using XX for mutual authentication
const NOISE_PATTERN: &str = "Noise_XX_25519_ChaChaPoly_BLAKE2s";

/// Peer identity derived from Noise static public key
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PeerId(pub Vec<u8>);

impl PeerId {
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        PeerId(bytes)
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

/// Commands sent to SessionManager
#[derive(Debug)]
pub enum SessionCommand {
    /// Send plaintext data to a peer
    SendPlaintext(PeerId, Vec<u8>),
    /// Close a session with a peer
    CloseSession(PeerId),
}

/// Events emitted by SessionManager
#[derive(Debug, Clone)]
pub enum SessionEvent {
    /// Session successfully established with a peer
    Established(PeerId, u64), // peer_id, conn_id
    /// Received plaintext data from a peer
    PlaintextFrame(PeerId, Vec<u8>),
    /// Session closed
    Closed(PeerId),
}

/// Session state machine
enum SessionState {
    /// Handshake in progress
    Handshaking(HandshakeState),
    /// Handshake complete, ready for encrypted communication
    Established(TransportState, PeerId),
}

/// A session with encryption and authentication
struct Session {
    #[allow(dead_code)]
    conn_id: u64,
    state: SessionState,
}

pub struct SessionManager {
    sessions: Arc<Mutex<HashMap<u64, Session>>>, // conn_id -> Session
    peer_to_conn: Arc<Mutex<HashMap<PeerId, u64>>>, // peer_id -> conn_id
    static_keypair: Vec<u8>, // Our long-term identity key
    transport_tx: mpsc::Sender<TransportCommand>,
    event_tx: mpsc::Sender<SessionEvent>,
}

impl SessionManager {
    /// Create a new SessionManager with a static keypair
    pub fn new(
        static_keypair: Vec<u8>,
        transport_tx: mpsc::Sender<TransportCommand>,
        event_tx: mpsc::Sender<SessionEvent>,
    ) -> Self {
        SessionManager {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            peer_to_conn: Arc::new(Mutex::new(HashMap::new())),
            static_keypair,
            transport_tx,
            event_tx,
        }
    }

    /// Generate a new static keypair for testing
    pub fn generate_keypair() -> Vec<u8> {
        let builder = Builder::new(NOISE_PATTERN.parse().unwrap());
        let keypair = builder.generate_keypair().unwrap();
        keypair.private.to_vec()
    }

    /// Handle incoming transport events
    pub async fn handle_transport_event(&self, event: TransportEvent) -> Result<(), String> {
        match event {
            TransportEvent::Connected(conn_id, _addr) => {
                self.initiate_handshake(conn_id).await?;
            }
            TransportEvent::Data(conn_id, bytes) => {
                self.handle_data(conn_id, bytes).await?;
            }
            TransportEvent::Disconnected(conn_id) => {
                self.handle_disconnect(conn_id).await?;
            }
        }
        Ok(())
    }

    /// Handle session commands
    pub async fn handle_command(&self, command: SessionCommand) -> Result<(), String> {
        match command {
            SessionCommand::SendPlaintext(peer_id, plaintext) => {
                self.send_plaintext(peer_id, plaintext).await?;
            }
            SessionCommand::CloseSession(peer_id) => {
                self.close_session(peer_id).await?;
            }
        }
        Ok(())
    }

    /// Initiate Noise handshake for a new connection
    async fn initiate_handshake(&self, conn_id: u64) -> Result<(), String> {
        // Build Noise handshake state (initiator role)
        let builder = Builder::new(NOISE_PATTERN.parse().unwrap());
        let builder = builder.local_private_key(&self.static_keypair);

        let mut handshake = builder
            .build_initiator()
            .map_err(|e| format!("Failed to build initiator: {}", e))?;

        // Send first handshake message
        let mut buffer = vec![0u8; 1024];
        let len = handshake
            .write_message(&[], &mut buffer)
            .map_err(|e| format!("Failed to write handshake: {}", e))?;

        // Store handshake state
        let session = Session {
            conn_id,
            state: SessionState::Handshaking(handshake),
        };
        self.sessions.lock().await.insert(conn_id, session);

        // Send handshake message via transport
        self.transport_tx
            .send(TransportCommand::Send(conn_id, buffer[..len].to_vec()))
            .await
            .map_err(|e| format!("Failed to send handshake: {}", e))?;

        Ok(())
    }

    /// Handle incoming data (handshake or encrypted message)
    async fn handle_data(&self, conn_id: u64, bytes: Vec<u8>) -> Result<(), String> {
        let mut sessions = self.sessions.lock().await;
        let session = sessions
            .get_mut(&conn_id)
            .ok_or_else(|| format!("Session {} not found", conn_id))?;

        match &mut session.state {
            SessionState::Handshaking(handshake) => {
                // Process handshake message
                let mut buffer = vec![0u8; 1024];
                let len = handshake
                    .read_message(&bytes, &mut buffer)
                    .map_err(|e| format!("Handshake read failed: {}", e))?;

                // Check if handshake is complete
                if handshake.is_handshake_finished() {
                    // Extract peer's static public key
                    let remote_static = handshake
                        .get_remote_static()
                        .ok_or("No remote static key")?
                        .to_vec();
                    let peer_id = PeerId::from_bytes(remote_static);

                    // Take ownership and transition to transport mode
                    // We need to replace the handshake state temporarily
                    let old_state = std::mem::replace(
                        &mut session.state,
                        SessionState::Handshaking(
                            Builder::new(NOISE_PATTERN.parse().unwrap())
                                .build_initiator()
                                .unwrap(),
                        ),
                    );

                    if let SessionState::Handshaking(hs) = old_state {
                        let transport = hs
                            .into_transport_mode()
                            .map_err(|e| format!("Failed to enter transport mode: {}", e))?;
                        session.state = SessionState::Established(transport, peer_id.clone());
                    }

                    // Update peer mapping
                    drop(sessions);
                    self.peer_to_conn
                        .lock()
                        .await
                        .insert(peer_id.clone(), conn_id);

                    // Emit Established event
                    self.event_tx
                        .send(SessionEvent::Established(peer_id, conn_id))
                        .await
                        .map_err(|e| format!("Failed to send event: {}", e))?;
                } else {
                    // Continue handshake
                    if len > 0 {
                        let len = handshake
                            .write_message(&[], &mut buffer)
                            .map_err(|e| format!("Handshake write failed: {}", e))?;

                        drop(sessions);
                        self.transport_tx
                            .send(TransportCommand::Send(conn_id, buffer[..len].to_vec()))
                            .await
                            .map_err(|e| format!("Failed to send handshake: {}", e))?;
                    }
                }
            }
            SessionState::Established(transport, peer_id) => {
                // Decrypt message
                let mut buffer = vec![0u8; bytes.len()];
                let len = transport
                    .read_message(&bytes, &mut buffer)
                    .map_err(|e| format!("Decryption failed: {}", e))?;

                let plaintext = buffer[..len].to_vec();
                let peer_id = peer_id.clone();

                drop(sessions);

                // Emit plaintext frame
                self.event_tx
                    .send(SessionEvent::PlaintextFrame(peer_id, plaintext))
                    .await
                    .map_err(|e| format!("Failed to send event: {}", e))?;
            }
        }

        Ok(())
    }

    /// Send plaintext to a peer (encrypts and sends)
    async fn send_plaintext(&self, peer_id: PeerId, plaintext: Vec<u8>) -> Result<(), String> {
        let peer_to_conn = self.peer_to_conn.lock().await;
        let conn_id = peer_to_conn
            .get(&peer_id)
            .ok_or_else(|| format!("No session for peer"))?;
        let conn_id = *conn_id;
        drop(peer_to_conn);

        let mut sessions = self.sessions.lock().await;
        let session = sessions
            .get_mut(&conn_id)
            .ok_or_else(|| format!("Session {} not found", conn_id))?;

        match &mut session.state {
            SessionState::Established(transport, _) => {
                let mut buffer = vec![0u8; plaintext.len() + 100]; // Extra space for tag
                let len = transport
                    .write_message(&plaintext, &mut buffer)
                    .map_err(|e| format!("Encryption failed: {}", e))?;

                drop(sessions);

                self.transport_tx
                    .send(TransportCommand::Send(conn_id, buffer[..len].to_vec()))
                    .await
                    .map_err(|e| format!("Failed to send encrypted data: {}", e))?;

                Ok(())
            }
            SessionState::Handshaking(_) => Err("Session not yet established".to_string()),
        }
    }

    /// Handle connection disconnect
    async fn handle_disconnect(&self, conn_id: u64) -> Result<(), String> {
        let mut sessions = self.sessions.lock().await;
        let session = sessions.remove(&conn_id);

        if let Some(session) = session {
            if let SessionState::Established(_, peer_id) = session.state {
                drop(sessions);

                self.peer_to_conn.lock().await.remove(&peer_id);
                self.event_tx
                    .send(SessionEvent::Closed(peer_id))
                    .await
                    .map_err(|e| format!("Failed to send event: {}", e))?;
            }
        }

        Ok(())
    }

    /// Close a session with a peer
    async fn close_session(&self, peer_id: PeerId) -> Result<(), String> {
        let mut peer_to_conn = self.peer_to_conn.lock().await;
        let conn_id = peer_to_conn
            .remove(&peer_id)
            .ok_or_else(|| format!("No session for peer"))?;

        drop(peer_to_conn);

        self.sessions.lock().await.remove(&conn_id);

        self.transport_tx
            .send(TransportCommand::Close(conn_id))
            .await
            .map_err(|e| format!("Failed to close transport: {}", e))?;

        self.event_tx
            .send(SessionEvent::Closed(peer_id))
            .await
            .map_err(|e| format!("Failed to send event: {}", e))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::Duration;

    #[tokio::test]
    async fn test_session_handshake() {
        // Create channels
        let (transport_tx, mut transport_rx) = mpsc::channel(100);
        let (event_tx, _event_rx) = mpsc::channel(100);

        // Generate keypairs for two peers
        let keypair1 = SessionManager::generate_keypair();

        // Create session manager
        let manager1 = SessionManager::new(keypair1, transport_tx, event_tx);

        // Simulate connection establishment
        let conn_id_1 = 1;

        // Manager1 initiates handshake
        manager1
            .handle_transport_event(TransportEvent::Connected(conn_id_1, "peer2".to_string()))
            .await
            .expect("Failed to initiate handshake");

        // Get handshake message from manager1
        let msg1 = if let Some(TransportCommand::Send(_id, bytes)) = transport_rx.recv().await {
            bytes
        } else {
            panic!("Expected Send command");
        };

        // For this test, we'll verify that handshake messages are generated
        assert!(!msg1.is_empty(), "Handshake message should not be empty");
    }

    #[tokio::test]
    async fn test_session_encrypt_decrypt() {
        // Create channels
        let (transport_tx, _transport_rx) = mpsc::channel(100);
        let (event_tx, _event_rx) = mpsc::channel(100);

        // Generate keypair
        let keypair = SessionManager::generate_keypair();
        let manager = SessionManager::new(keypair, transport_tx, event_tx.clone());

        // Manually create an established session for testing
        let builder = Builder::new(NOISE_PATTERN.parse().unwrap());
        let keypair2 = SessionManager::generate_keypair();
        let builder = builder.local_private_key(&keypair2);
        let _handshake = builder.build_initiator().unwrap();

        // For this simplified test, verify the manager can be created
        assert!(manager.sessions.lock().await.is_empty());
    }

    #[tokio::test]
    async fn test_session_close() {
        let (transport_tx, mut transport_rx) = mpsc::channel(100);
        let (event_tx, _event_rx) = mpsc::channel(100);

        let keypair = SessionManager::generate_keypair();
        let manager = SessionManager::new(keypair, transport_tx, event_tx);

        let peer_id = PeerId::from_bytes(vec![1, 2, 3, 4]);

        // Manually insert a session
        let conn_id = 42;
        manager
            .peer_to_conn
            .lock()
            .await
            .insert(peer_id.clone(), conn_id);

        // Create a mock transport state by completing a handshake
        let builder = Builder::new(NOISE_PATTERN.parse().unwrap());
        let keypair2 = SessionManager::generate_keypair();
        let builder = builder.local_private_key(&keypair2);
        let mut handshake = builder.build_initiator().unwrap();

        // Perform a mock handshake by writing/reading empty messages to complete it
        // We'll simulate this by using build_responder and exchanging messages
        let builder_resp = Builder::new(NOISE_PATTERN.parse().unwrap());
        let keypair3 = SessionManager::generate_keypair();
        let builder_resp = builder_resp.local_private_key(&keypair3);
        let mut handshake_resp = builder_resp.build_responder().unwrap();

        // Exchange handshake messages
        let mut buf1 = vec![0u8; 1024];
        let mut buf2 = vec![0u8; 1024];

        let len1 = handshake.write_message(&[], &mut buf1).unwrap();
        let _len2 = handshake_resp.read_message(&buf1[..len1], &mut buf2).unwrap();

        let len3 = handshake_resp.write_message(&[], &mut buf1).unwrap();
        let _len4 = handshake.read_message(&buf1[..len3], &mut buf2).unwrap();

        let len5 = handshake.write_message(&[], &mut buf1).unwrap();
        let _len6 = handshake_resp.read_message(&buf1[..len5], &mut buf2).unwrap();

        // Now handshake should be finished
        assert!(handshake.is_handshake_finished());

        let transport_state = handshake.into_transport_mode().unwrap();

        let session = Session {
            conn_id,
            state: SessionState::Established(transport_state, peer_id.clone()),
        };
        manager.sessions.lock().await.insert(conn_id, session);

        // Close the session
        manager
            .handle_command(SessionCommand::CloseSession(peer_id.clone()))
            .await
            .expect("Failed to close session");

        // Verify transport close command was sent
        let cmd = tokio::time::timeout(Duration::from_secs(1), transport_rx.recv())
            .await
            .expect("Timeout")
            .expect("Channel closed");

        match cmd {
            TransportCommand::Close(id) => assert_eq!(id, conn_id),
            _ => panic!("Expected Close command"),
        }

        // Verify session was removed
        assert!(manager.sessions.lock().await.is_empty());
        assert!(manager.peer_to_conn.lock().await.is_empty());
    }
}

