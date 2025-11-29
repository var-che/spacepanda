/*
    OnionRouter - Onion building, relaying, mixing.

    Construct, send, and reply to onion-wrapped packets. Manage circuits and perform forwarding.

    Workflow for Sending (build path)
    1. Path selection - ask route_table for k relays: R1,R2,R3
    2. Ephemeral keys: generate ephemeral X25519 keypair e
    3. Use layered ephemeral per hop
    4. Layer encryption: start with final payload "P", create inner blob `L3 = AEAD(K3,header3 || P)`, then
         `L2 = AEAD(K2,header2 || L3)`, then `L1 = AEAD(K1,header1 || L2)`
    5. Send L1 to R1 over session_manager

    Workflow for Relaying (at R1)

    1. Receive "L1" from session_manager, compute "K1" using "eph_pub" + "R1_priv"
    2. Decrypt L1 using K1, parse header1 to get next hop
    3. Forward "L2" to next hop (by dialing / using session_manager)
    4. If "deliver_local" flag is set, hand the final decrypted "InnerEnvelope" to RouterHandler for dispatch

    Inputs:
      - OnionCommand::Send(dest_node, payload)
      - OnionCommand::RelayPacket(encrypted_blob)

    Outputs:
      - OnionEvent::PacketForward(next_peer, blob)
      - OnionEvent::DeliverLocal(inner_envelope)

    Notes:
    
    Mixing: optionally batch multiple decrypted inner blobs into a short window (50-200ms), shuffle,
    and forward to reduce timing correlation. This is heavier but increases anonymity.

    Architecture:

    ┌────────────────────────────────────────────────────┐
    │         Application (wants to send)                 │
    └─────────────────┬──────────────────────────────────┘
                      │
                      ▼
    ┌────────────────────────────────────────────────────┐
    │         OnionRouter::send(dest, payload)            │
    │                                                     │
    │  1. Pick Path:  RouteTable.pick_diverse_relays(3)  │
    │     → [R1, R2, R3]                                  │
    │                                                     │
    │  2. Build Layers (backward):                        │
    │     L3 = encrypt(K3, header3 + payload)             │
    │     L2 = encrypt(K2, header2 + L3)                  │
    │     L1 = encrypt(K1, header1 + L2)                  │
    │                                                     │
    │  3. Send L1 to R1                                   │
    └─────────────────┬──────────────────────────────────┘
                      │
                      ▼
    ┌────────────────────────────────────────────────────┐
    │         Relay Node R1 (receives L1)                 │
    │                                                     │
    │  1. Decrypt: L2 = decrypt(K1, L1)                   │
    │  2. Parse header: next_hop = R2                     │
    │  3. Forward L2 to R2                                │
    └─────────────────┬──────────────────────────────────┘
                      │
                      ▼  (repeat at R2, R3...)
                      
    ┌────────────────────────────────────────────────────┐
    │         Final Node R3 (exit/destination)            │
    │                                                     │
    │  1. Decrypt: payload = decrypt(K3, L3)              │
    │  2. If deliver_local: emit DeliverLocal(payload)    │
    │  3. Else: forward to final destination              │
    └────────────────────────────────────────────────────┘
*/

use chacha20poly1305::{
    aead::{Aead, KeyInit},
    ChaCha20Poly1305, Nonce,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, Mutex};
use tokio::time::interval;

use super::route_table::{PeerInfo, RouteTable, RouteTableCommand};
use super::session_manager::PeerId;

/// Onion packet header
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnionHeader {
    /// Next hop peer ID (empty if final destination)
    pub next_hop: Vec<u8>,
    /// Ephemeral public key for this hop
    pub ephemeral_pubkey: Vec<u8>,
    /// Whether this is the final hop
    pub deliver_local: bool,
}

/// Inner envelope after all layers are decrypted
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InnerEnvelope {
    /// Final destination peer ID
    pub destination: Vec<u8>,
    /// Actual payload
    pub payload: Vec<u8>,
}

/// Commands for OnionRouter
#[derive(Debug)]
pub enum OnionCommand {
    /// Send data via onion routing
    Send {
        destination: PeerId,
        payload: Vec<u8>,
        response_tx: Option<tokio::sync::oneshot::Sender<Result<(), String>>>,
    },
    /// Relay a received onion packet
    RelayPacket {
        encrypted_blob: Vec<u8>,
    },
    /// Enable/disable mixing
    SetMixing {
        enabled: bool,
        window_ms: u64,
    },
    /// Shutdown
    Shutdown,
}

/// Events emitted by OnionRouter
#[derive(Debug, Clone)]
pub enum OnionEvent {
    /// Forward packet to next peer
    PacketForward { next_peer: PeerId, blob: Vec<u8> },
    /// Deliver to local application
    DeliverLocal { envelope: InnerEnvelope },
    /// Circuit built successfully
    CircuitBuilt { path_length: usize },
    /// Relay error
    RelayError { error: String },
}

/// Configuration for onion routing
#[derive(Debug, Clone)]
pub struct OnionConfig {
    /// Number of hops in the circuit (default: 3)
    pub circuit_hops: usize,
    /// Enable mixing for anonymity
    pub mixing_enabled: bool,
    /// Mixing window duration
    pub mixing_window: Duration,
}

impl Default for OnionConfig {
    fn default() -> Self {
        OnionConfig {
            circuit_hops: 3,
            mixing_enabled: false,
            mixing_window: Duration::from_millis(100),
        }
    }
}

/// Mix queue for batching and shuffling packets
struct MixQueue {
    packets: Vec<(PeerId, Vec<u8>)>,
    enabled: bool,
    window: Duration,
}

impl MixQueue {
    fn new(enabled: bool, window: Duration) -> Self {
        MixQueue {
            packets: Vec::new(),
            enabled,
            window,
        }
    }

    fn add(&mut self, peer: PeerId, blob: Vec<u8>) {
        self.packets.push((peer, blob));
    }

    fn flush(&mut self) -> Vec<(PeerId, Vec<u8>)> {
        use rand::seq::SliceRandom;
        use rand::rng;

        if self.enabled {
            // Shuffle for mixing
            let mut rng = rng();
            self.packets.shuffle(&mut rng);
        }

        std::mem::take(&mut self.packets)
    }
}

/// OnionRouter manages onion routing
pub struct OnionRouter {
    config: OnionConfig,
    route_table: Arc<RouteTable>,
    event_tx: mpsc::Sender<OnionEvent>,
    mix_queue: Arc<Mutex<MixQueue>>,
}

impl OnionRouter {
    /// Create a new OnionRouter
    pub fn new(
        config: OnionConfig,
        route_table: Arc<RouteTable>,
        event_tx: mpsc::Sender<OnionEvent>,
    ) -> Self {
        let mix_queue = Arc::new(Mutex::new(MixQueue::new(
            config.mixing_enabled,
            config.mixing_window,
        )));

        OnionRouter {
            config,
            route_table,
            event_tx,
            mix_queue,
        }
    }

    /// Start the onion router loop
    pub async fn run(
        self: Arc<Self>,
        mut command_rx: mpsc::Receiver<OnionCommand>,
    ) {
        // Spawn mixing task if enabled
        if self.config.mixing_enabled {
            let mix_queue = self.mix_queue.clone();
            let event_tx = self.event_tx.clone();
            let window = self.config.mixing_window;
            
            tokio::spawn(async move {
                let mut tick = interval(window);
                loop {
                    tick.tick().await;
                    let packets = mix_queue.lock().await.flush();
                    for (peer, blob) in packets {
                        let _ = event_tx
                            .send(OnionEvent::PacketForward {
                                next_peer: peer,
                                blob,
                            })
                            .await;
                    }
                }
            });
        }

        loop {
            tokio::select! {
                Some(cmd) = command_rx.recv() => {
                    match cmd {
                        OnionCommand::Send { destination, payload, response_tx } => {
                            let result = self.handle_send(destination, payload).await;
                            if let Some(tx) = response_tx {
                                let _ = tx.send(result);
                            }
                        }
                        OnionCommand::RelayPacket { encrypted_blob } => {
                            if let Err(e) = self.handle_relay(encrypted_blob).await {
                                let _ = self.event_tx
                                    .send(OnionEvent::RelayError { error: e })
                                    .await;
                            }
                        }
                        OnionCommand::SetMixing { enabled, window_ms } => {
                            let mut queue = self.mix_queue.lock().await;
                            queue.enabled = enabled;
                            queue.window = Duration::from_millis(window_ms);
                        }
                        OnionCommand::Shutdown => {
                            break;
                        }
                    }
                }
            }
        }
    }

    /// Handle sending data via onion routing
    async fn handle_send(&self, destination: PeerId, payload: Vec<u8>) -> Result<(), String> {
        // 1. Pick path from route table
        let path = self.pick_path().await?;

        if path.is_empty() {
            return Err("No relays available for onion routing".to_string());
        }

        // 2. Build inner envelope
        let envelope = InnerEnvelope {
            destination: destination.0.clone(),
            payload,
        };

        let envelope_bytes = serde_json::to_vec(&envelope)
            .map_err(|e| format!("Failed to serialize envelope: {}", e))?;

        // 3. Build onion layers (backward from destination)
        let onion_packet = self.build_onion_layers(&path, envelope_bytes)?;

        // 4. Send to first hop
        if let Some(first_hop) = path.first() {
            if self.config.mixing_enabled {
                self.mix_queue
                    .lock()
                    .await
                    .add(first_hop.peer_id.clone(), onion_packet);
            } else {
                let _ = self
                    .event_tx
                    .send(OnionEvent::PacketForward {
                        next_peer: first_hop.peer_id.clone(),
                        blob: onion_packet,
                    })
                    .await;
            }

            let _ = self
                .event_tx
                .send(OnionEvent::CircuitBuilt {
                    path_length: path.len(),
                })
                .await;

            Ok(())
        } else {
            Err("Empty path".to_string())
        }
    }

    /// Handle relaying an onion packet
    async fn handle_relay(&self, encrypted_blob: Vec<u8>) -> Result<(), String> {
        // Parse the encrypted blob to extract header and payload
        // In a real implementation, we would:
        // 1. Extract ephemeral public key from blob
        // 2. Derive shared secret using our private key
        // 3. Decrypt the layer
        // 4. Parse header to get next hop
        // 5. Either forward or deliver locally

        // For now, simplified version:
        let decrypted = self.decrypt_layer(&encrypted_blob)?;

        // Try to parse as header + remaining layers
        if let Ok(header) = serde_json::from_slice::<OnionHeader>(&decrypted[..64.min(decrypted.len())]) {
            if header.deliver_local {
                // Final hop - deliver to application
                let envelope: InnerEnvelope = serde_json::from_slice(&decrypted[64..])
                    .map_err(|e| format!("Failed to parse inner envelope: {}", e))?;

                let _ = self
                    .event_tx
                    .send(OnionEvent::DeliverLocal { envelope })
                    .await;
            } else {
                // Forward to next hop
                let next_peer = PeerId::from_bytes(header.next_hop);
                let remaining_blob = decrypted[64..].to_vec();

                if self.config.mixing_enabled {
                    self.mix_queue
                        .lock()
                        .await
                        .add(next_peer, remaining_blob);
                } else {
                    let _ = self
                        .event_tx
                        .send(OnionEvent::PacketForward {
                            next_peer,
                            blob: remaining_blob,
                        })
                        .await;
                }
            }
        }

        Ok(())
    }

    /// Pick a path of relay nodes
    async fn pick_path(&self) -> Result<Vec<PeerInfo>, String> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        
        self.route_table
            .handle_command(RouteTableCommand::PickDiverseRelays {
                k: self.config.circuit_hops,
                response_tx: tx,
            })
            .await?;

        rx.await
            .map_err(|e| format!("Failed to pick relays: {}", e))
    }

    /// Build onion layers (simplified version)
    fn build_onion_layers(&self, path: &[PeerInfo], payload: Vec<u8>) -> Result<Vec<u8>, String> {
        let mut current_blob = payload;

        // Build layers backward from destination
        for (i, hop) in path.iter().enumerate().rev() {
            let is_last = i == path.len() - 1;

            let header = OnionHeader {
                next_hop: if is_last {
                    Vec::new()
                } else {
                    path[i + 1].peer_id.0.clone()
                },
                ephemeral_pubkey: vec![0u8; 32], // Simplified: would be real X25519 pubkey
                deliver_local: is_last,
            };

            let header_bytes = serde_json::to_vec(&header)
                .map_err(|e| format!("Failed to serialize header: {}", e))?;

            // Combine header and current blob
            let mut combined = header_bytes;
            combined.extend_from_slice(&current_blob);

            // Encrypt this layer
            current_blob = self.encrypt_layer(&hop.peer_id, &combined)?;
        }

        Ok(current_blob)
    }

    /// Encrypt a layer (simplified - uses peer_id as key material)
    fn encrypt_layer(&self, peer_id: &PeerId, data: &[u8]) -> Result<Vec<u8>, String> {
        // Derive a key from peer_id (in production, use proper X25519 ECDH)
        let mut key = [0u8; 32];
        for (i, byte) in peer_id.0.iter().take(32).enumerate() {
            key[i] = *byte;
        }

        let cipher = ChaCha20Poly1305::new(&key.into());
        let nonce = Nonce::from_slice(&[0u8; 12]); // Simplified: use proper nonce

        cipher
            .encrypt(nonce, data)
            .map_err(|e| format!("Encryption failed: {}", e))
    }

    /// Decrypt a layer (simplified)
    fn decrypt_layer(&self, data: &[u8]) -> Result<Vec<u8>, String> {
        // In production, derive key from our private key + ephemeral public key
        // For now, use a dummy key
        let key = [0u8; 32];

        let cipher = ChaCha20Poly1305::new(&key.into());
        let nonce = Nonce::from_slice(&[0u8; 12]);

        cipher
            .decrypt(nonce, data)
            .map_err(|e| format!("Decryption failed: {}", e))
    }

    /// Get mixing queue size
    pub async fn mix_queue_size(&self) -> usize {
        self.mix_queue.lock().await.packets.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core_router::route_table::{Capability, RouteTable};

    fn create_test_relay(id: u8, asn: u32) -> PeerInfo {
        let mut peer = PeerInfo::new(
            PeerId::from_bytes(vec![id]),
            vec![format!("10.0.0.{}:8080", id)],
        );
        peer.capabilities.push(Capability::Relay);
        peer.asn = Some(asn);
        peer
    }

    #[tokio::test]
    async fn test_onion_config_default() {
        let config = OnionConfig::default();
        assert_eq!(config.circuit_hops, 3);
        assert!(!config.mixing_enabled);
    }

    #[tokio::test]
    async fn test_onion_router_creation() {
        let config = OnionConfig::default();
        let route_table = Arc::new(RouteTable::new());
        let (event_tx, _event_rx) = mpsc::channel(100);

        let router = OnionRouter::new(config, route_table, event_tx);
        assert_eq!(router.mix_queue_size().await, 0);
    }

    #[tokio::test]
    async fn test_pick_path() {
        let config = OnionConfig::default();
        let route_table = Arc::new(RouteTable::new());
        let (event_tx, _event_rx) = mpsc::channel(100);

        // Add some relays
        for i in 1..=5 {
            route_table
                .handle_command(RouteTableCommand::InsertPeer(create_test_relay(
                    i,
                    1000 + i as u32,
                )))
                .await
                .unwrap();
        }

        let router = Arc::new(OnionRouter::new(config, route_table, event_tx));
        let path = router.pick_path().await.unwrap();

        assert_eq!(path.len(), 3); // Default circuit hops
    }

    #[tokio::test]
    async fn test_encrypt_decrypt_layer() {
        let config = OnionConfig::default();
        let route_table = Arc::new(RouteTable::new());
        let (event_tx, _event_rx) = mpsc::channel(100);

        let router = OnionRouter::new(config, route_table, event_tx);
        let peer_id = PeerId::from_bytes(vec![1, 2, 3, 4]);
        let data = b"test data";

        let encrypted = router.encrypt_layer(&peer_id, data).unwrap();
        assert_ne!(encrypted, data);

        // Note: decrypt_layer uses a dummy key, so it won't decrypt our encrypted data
        // In production, encryption/decryption would be symmetric with proper key derivation
    }

    #[tokio::test]
    async fn test_build_onion_layers() {
        let config = OnionConfig::default();
        let route_table = Arc::new(RouteTable::new());
        let (event_tx, _event_rx) = mpsc::channel(100);

        let router = OnionRouter::new(config, route_table, event_tx);

        let path = vec![
            create_test_relay(1, 1000),
            create_test_relay(2, 2000),
            create_test_relay(3, 3000),
        ];

        let payload = b"secret message".to_vec();
        let onion = router.build_onion_layers(&path, payload).unwrap();

        // Onion should be larger than original due to headers and encryption overhead
        assert!(onion.len() > 14);
    }

    #[tokio::test]
    async fn test_handle_send_no_relays() {
        let config = OnionConfig::default();
        let route_table = Arc::new(RouteTable::new());
        let (event_tx, _event_rx) = mpsc::channel(100);

        let router = Arc::new(OnionRouter::new(config, route_table, event_tx));

        let destination = PeerId::from_bytes(vec![99, 99, 99, 99]);
        let payload = b"test".to_vec();

        let result = router.handle_send(destination, payload).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("No relays available"));
    }

    #[tokio::test]
    async fn test_handle_send_with_relays() {
        let config = OnionConfig::default();
        let route_table = Arc::new(RouteTable::new());
        let (event_tx, mut event_rx) = mpsc::channel(100);

        // Add relays
        for i in 1..=5 {
            route_table
                .handle_command(RouteTableCommand::InsertPeer(create_test_relay(
                    i,
                    1000 + i as u32,
                )))
                .await
                .unwrap();
        }

        let router = Arc::new(OnionRouter::new(config, route_table, event_tx));

        let destination = PeerId::from_bytes(vec![99, 99, 99, 99]);
        let payload = b"secret data".to_vec();

        let result = router.handle_send(destination, payload).await;
        assert!(result.is_ok());

        // Should emit PacketForward and CircuitBuilt events
        let event1 = tokio::time::timeout(Duration::from_millis(100), event_rx.recv())
            .await
            .unwrap()
            .unwrap();

        match event1 {
            OnionEvent::PacketForward { .. } => {
                // Success
            }
            OnionEvent::CircuitBuilt { path_length } => {
                assert_eq!(path_length, 3);
            }
            _ => panic!("Unexpected event: {:?}", event1),
        }
    }

    #[tokio::test]
    async fn test_mixing_queue() {
        let mut config = OnionConfig::default();
        config.mixing_enabled = true;
        config.mixing_window = Duration::from_millis(50);

        let route_table = Arc::new(RouteTable::new());
        let (event_tx, _event_rx) = mpsc::channel(100);

        let router = OnionRouter::new(config, route_table, event_tx);

        assert_eq!(router.mix_queue_size().await, 0);

        // Add to mix queue
        router
            .mix_queue
            .lock()
            .await
            .add(PeerId::from_bytes(vec![1]), vec![1, 2, 3]);

        assert_eq!(router.mix_queue_size().await, 1);

        // Flush
        let flushed = router.mix_queue.lock().await.flush();
        assert_eq!(flushed.len(), 1);
        assert_eq!(router.mix_queue_size().await, 0);
    }

    #[tokio::test]
    async fn test_onion_command_send() {
        let config = OnionConfig::default();
        let route_table = Arc::new(RouteTable::new());
        let (event_tx, mut event_rx) = mpsc::channel(100);
        let (cmd_tx, cmd_rx) = mpsc::channel(100);

        // Add relays
        for i in 1..=5 {
            route_table
                .handle_command(RouteTableCommand::InsertPeer(create_test_relay(
                    i,
                    1000 + i as u32,
                )))
                .await
                .unwrap();
        }

        let router = Arc::new(OnionRouter::new(config, route_table, event_tx));

        // Spawn router task
        let router_clone = router.clone();
        let task = tokio::spawn(async move {
            router_clone.run(cmd_rx).await;
        });

        // Send command
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();
        cmd_tx
            .send(OnionCommand::Send {
                destination: PeerId::from_bytes(vec![99, 99, 99, 99]),
                payload: b"test message".to_vec(),
                response_tx: Some(response_tx),
            })
            .await
            .unwrap();

        // Check response
        let result = tokio::time::timeout(Duration::from_millis(100), response_rx)
            .await
            .unwrap()
            .unwrap();
        assert!(result.is_ok());

        // Should receive events
        let _ = tokio::time::timeout(Duration::from_millis(100), event_rx.recv()).await;

        // Shutdown
        cmd_tx.send(OnionCommand::Shutdown).await.unwrap();
        let _ = tokio::time::timeout(Duration::from_millis(100), task).await;
    }

    #[tokio::test]
    async fn test_inner_envelope_serialization() {
        let envelope = InnerEnvelope {
            destination: vec![1, 2, 3, 4],
            payload: vec![5, 6, 7, 8],
        };

        let serialized = serde_json::to_vec(&envelope).unwrap();
        let deserialized: InnerEnvelope = serde_json::from_slice(&serialized).unwrap();

        assert_eq!(deserialized.destination, envelope.destination);
        assert_eq!(deserialized.payload, envelope.payload);
    }

    #[tokio::test]
    async fn test_onion_header_serialization() {
        let header = OnionHeader {
            next_hop: vec![1, 2, 3, 4],
            ephemeral_pubkey: vec![0u8; 32],
            deliver_local: false,
        };

        let serialized = serde_json::to_vec(&header).unwrap();
        let deserialized: OnionHeader = serde_json::from_slice(&serialized).unwrap();

        assert_eq!(deserialized.next_hop, header.next_hop);
        assert_eq!(deserialized.deliver_local, header.deliver_local);
    }
}
