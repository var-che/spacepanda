/*
    OverlayDiscovery - maintain overlay candidates

    Keep the pool of available relays fresh. Periodic process that does peer exchange and checks liveness.

    Workflow:
    1. Periodic tick:
      - call some peers with "peer_exchange" RPC to get new candidates
      - validate returned peer descriptors 
      - ping the ones we don't have currently
      - if under capacity, try bootstrap list
    2. Feed new peers into route_table.rs and notify onion_router.rs for path selection updates

    Inputs:
      - Config: desired relay pool size N,
      - Events:
        -  PeerDiscovered(peer_info)
        -  PeerLivenessResult(peer_id, is_alive)
    Outputs:
      - Events:
        -  RelayPoolUpdated(new_relay_list)

    Architecture:

    ┌────────────────────────────────────────────────────┐
    │         OverlayDiscovery (Periodic Task)           │
    │                                                    │
    │  Timer Tick ──► Peer Exchange RPC ──► Validation  │
    │                      ↓                             │
    │                 Liveness Check (ping)              │
    │                      ↓                             │
    │              Update RouteTable                     │
    │                      ↓                             │
    │          Emit RelayPoolUpdated Event               │
    └────────────────────────────────────────────────────┘
                           │
                           ▼
                    RouterHandle (RPC)
                           │
                           ▼
                      RouteTable
*/

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::{mpsc, Mutex};
use tokio::time::interval;

use super::route_table::{Capability, PeerInfo, RouteTable, RouteTableCommand};
use super::session_manager::PeerId;

/// Configuration for overlay discovery
#[derive(Debug, Clone)]
pub struct DiscoveryConfig {
    /// Desired number of relay peers in the pool
    pub target_relay_count: usize,
    /// How often to run discovery
    pub discovery_interval: Duration,
    /// Bootstrap peer addresses
    pub bootstrap_peers: Vec<String>,
    /// Maximum number of peers to request in peer exchange
    pub peer_exchange_count: usize,
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        DiscoveryConfig {
            target_relay_count: 20,
            discovery_interval: Duration::from_secs(60),
            bootstrap_peers: Vec::new(),
            peer_exchange_count: 10,
        }
    }
}

/// Peer exchange request/response for RPC
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerExchangeRequest {
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerExchangeResponse {
    pub peers: Vec<PeerDescriptor>,
}

/// Lightweight peer descriptor for exchange
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerDescriptor {
    pub peer_id_bytes: Vec<u8>,
    pub addresses: Vec<String>,
    pub capabilities: Vec<String>,
    pub asn: Option<u32>,
}

impl PeerDescriptor {
    /// Convert to PeerInfo
    pub fn to_peer_info(&self) -> PeerInfo {
        let mut peer_info = PeerInfo::new(
            PeerId::from_bytes(self.peer_id_bytes.clone()),
            self.addresses.clone(),
        );

        // Parse capabilities
        for cap_str in &self.capabilities {
            match cap_str.as_str() {
                "relay" => peer_info.capabilities.push(Capability::Relay),
                "dht" => peer_info.capabilities.push(Capability::DhtNode),
                "storage" => peer_info.capabilities.push(Capability::Storage),
                "longlived" => peer_info.capabilities.push(Capability::LongLived),
                _ => {}
            }
        }

        peer_info.asn = self.asn;
        peer_info
    }

    /// Create from PeerInfo
    pub fn from_peer_info(info: &PeerInfo) -> Self {
        let capabilities = info
            .capabilities
            .iter()
            .map(|c| match c {
                Capability::Relay => "relay",
                Capability::DhtNode => "dht",
                Capability::Storage => "storage",
                Capability::LongLived => "longlived",
            })
            .map(String::from)
            .collect();

        PeerDescriptor {
            peer_id_bytes: info.peer_id.0.clone(),
            addresses: info.addresses.clone(),
            capabilities,
            asn: info.asn,
        }
    }
}

/// Commands for OverlayDiscovery
#[derive(Debug)]
pub enum DiscoveryCommand {
    /// Manually trigger discovery
    TriggerDiscovery,
    /// Add a discovered peer
    PeerDiscovered(PeerInfo),
    /// Report liveness check result
    PeerLivenessResult { peer_id: PeerId, is_alive: bool },
    /// Shutdown the discovery process
    Shutdown,
}

/// Events emitted by OverlayDiscovery
#[derive(Debug, Clone)]
pub enum DiscoveryEvent {
    /// Relay pool has been updated
    RelayPoolUpdated { relay_count: usize },
    /// New peer discovered
    NewPeerFound { peer_id: PeerId },
    /// Peer failed liveness check
    PeerUnreachable { peer_id: PeerId },
}

/// OverlayDiscovery manages the relay pool
pub struct OverlayDiscovery {
    config: DiscoveryConfig,
    route_table: Arc<RouteTable>,
    discovered_peers: Arc<Mutex<HashSet<PeerId>>>,
    event_tx: mpsc::Sender<DiscoveryEvent>,
}

impl OverlayDiscovery {
    /// Create a new OverlayDiscovery
    pub fn new(
        config: DiscoveryConfig,
        route_table: Arc<RouteTable>,
        event_tx: mpsc::Sender<DiscoveryEvent>,
    ) -> Self {
        OverlayDiscovery {
            config,
            route_table,
            discovered_peers: Arc::new(Mutex::new(HashSet::new())),
            event_tx,
        }
    }

    /// Start the discovery loop
    pub async fn run(
        self: Arc<Self>,
        mut command_rx: mpsc::Receiver<DiscoveryCommand>,
    ) {
        let mut tick_interval = interval(self.config.discovery_interval);

        loop {
            tokio::select! {
                _ = tick_interval.tick() => {
                    if let Err(e) = self.periodic_discovery().await {
                        eprintln!("Discovery error: {}", e);
                    }
                }

                Some(cmd) = command_rx.recv() => {
                    match cmd {
                        DiscoveryCommand::TriggerDiscovery => {
                            if let Err(e) = self.periodic_discovery().await {
                                eprintln!("Manual discovery error: {}", e);
                            }
                        }
                        DiscoveryCommand::PeerDiscovered(peer_info) => {
                            self.handle_peer_discovered(peer_info).await;
                        }
                        DiscoveryCommand::PeerLivenessResult { peer_id, is_alive } => {
                            self.handle_liveness_result(peer_id, is_alive).await;
                        }
                        DiscoveryCommand::Shutdown => {
                            break;
                        }
                    }
                }
            }
        }
    }

    /// Periodic discovery process
    async fn periodic_discovery(&self) -> Result<(), String> {
        let current_relay_count = self.count_relays().await;

        // If we're under capacity, try to discover more
        if current_relay_count < self.config.target_relay_count {
            // Try peer exchange first
            self.peer_exchange().await?;

            // If still under capacity, try bootstrap peers
            let after_exchange = self.count_relays().await;
            if after_exchange < self.config.target_relay_count {
                self.try_bootstrap_peers().await?;
            }
        }

        // Emit relay pool update event
        let relay_count = self.count_relays().await;
        let _ = self
            .event_tx
            .send(DiscoveryEvent::RelayPoolUpdated { relay_count })
            .await;

        Ok(())
    }

    /// Perform peer exchange with known peers
    async fn peer_exchange(&self) -> Result<(), String> {
        // Get current peers from route table
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.route_table
            .handle_command(RouteTableCommand::GetAllPeers { response_tx: tx })
            .await?;

        let all_peers = rx.await.map_err(|e| format!("Failed to get peers: {}", e))?;

        // Pick a few random peers to ask for more peers
        let candidates: Vec<&PeerInfo> = all_peers
            .iter()
            .filter(|p| p.capabilities.contains(&Capability::Relay))
            .take(3)
            .collect();

        for peer in candidates {
            // In a real implementation, we would make RPC calls here
            // For now, we simulate by just logging
            // rpc_call(peer.peer_id, "peer_exchange", PeerExchangeRequest { count: self.config.peer_exchange_count })
            
            // Simulate discovering the peer itself if not already known
            let mut discovered = self.discovered_peers.lock().await;
            if !discovered.contains(&peer.peer_id) {
                discovered.insert(peer.peer_id.clone());
                let _ = self
                    .event_tx
                    .send(DiscoveryEvent::NewPeerFound {
                        peer_id: peer.peer_id.clone(),
                    })
                    .await;
            }
        }

        Ok(())
    }

    /// Try connecting to bootstrap peers
    async fn try_bootstrap_peers(&self) -> Result<(), String> {
        for addr in &self.config.bootstrap_peers {
            // In a real implementation, we would:
            // 1. Dial the bootstrap peer
            // 2. Perform handshake
            // 3. Get peer info
            // 4. Add to route table
            
            // For now, create a simulated bootstrap peer
            let peer_info = self.create_bootstrap_peer(addr);
            
            self.route_table
                .handle_command(RouteTableCommand::InsertPeer(peer_info.clone()))
                .await?;

            let mut discovered = self.discovered_peers.lock().await;
            discovered.insert(peer_info.peer_id.clone());
        }

        Ok(())
    }

    /// Create a simulated bootstrap peer (in real implementation, this comes from handshake)
    fn create_bootstrap_peer(&self, addr: &str) -> PeerInfo {
        // Hash the address to create a deterministic peer ID for testing
        let peer_id_bytes = addr.bytes().take(16).collect::<Vec<u8>>();
        let mut peer_info = PeerInfo::new(
            PeerId::from_bytes(peer_id_bytes),
            vec![addr.to_string()],
        );
        peer_info.capabilities.push(Capability::Relay);
        peer_info.capabilities.push(Capability::LongLived);
        peer_info
    }

    /// Handle a discovered peer
    async fn handle_peer_discovered(&self, peer_info: PeerInfo) {
        // Add to route table
        if let Err(e) = self
            .route_table
            .handle_command(RouteTableCommand::InsertPeer(peer_info.clone()))
            .await
        {
            eprintln!("Failed to insert peer: {}", e);
            return;
        }

        // Track as discovered
        let mut discovered = self.discovered_peers.lock().await;
        if !discovered.contains(&peer_info.peer_id) {
            discovered.insert(peer_info.peer_id.clone());
            let _ = self
                .event_tx
                .send(DiscoveryEvent::NewPeerFound {
                    peer_id: peer_info.peer_id,
                })
                .await;
        }
    }

    /// Handle liveness check result
    async fn handle_liveness_result(&self, peer_id: PeerId, is_alive: bool) {
        if is_alive {
            // Update last_seen in route table
            let _ = self
                .route_table
                .handle_command(RouteTableCommand::UpdatePeerStats {
                    peer_id: peer_id.clone(),
                    stats: super::route_table::PeerStats {
                        latency: None,
                        failure_count_delta: -1, // Successful check reduces failure count
                        last_seen: SystemTime::now(),
                    },
                })
                .await;
        } else {
            // Mark as unreachable
            let _ = self
                .event_tx
                .send(DiscoveryEvent::PeerUnreachable {
                    peer_id: peer_id.clone(),
                })
                .await;

            // Increment failure count
            let _ = self
                .route_table
                .handle_command(RouteTableCommand::UpdatePeerStats {
                    peer_id,
                    stats: super::route_table::PeerStats {
                        latency: None,
                        failure_count_delta: 1,
                        last_seen: SystemTime::now(),
                    },
                })
                .await;
        }
    }

    /// Count current relay peers
    async fn count_relays(&self) -> usize {
        let (tx, rx) = tokio::sync::oneshot::channel();
        if let Err(e) = self
            .route_table
            .handle_command(RouteTableCommand::ListPeersByCapability {
                capability: Capability::Relay,
                response_tx: tx,
            })
            .await
        {
            eprintln!("Failed to list relays: {}", e);
            return 0;
        }

        rx.await.map(|peers| peers.len()).unwrap_or(0)
    }

    /// Get the number of discovered peers
    pub async fn discovered_count(&self) -> usize {
        self.discovered_peers.lock().await.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_discovery_config_default() {
        let config = DiscoveryConfig::default();
        assert_eq!(config.target_relay_count, 20);
        assert_eq!(config.peer_exchange_count, 10);
    }

    #[tokio::test]
    async fn test_peer_descriptor_conversion() {
        let mut peer_info = PeerInfo::new(
            PeerId::from_bytes(vec![1, 2, 3, 4]),
            vec!["127.0.0.1:8080".to_string()],
        );
        peer_info.capabilities.push(Capability::Relay);
        peer_info.capabilities.push(Capability::DhtNode);
        peer_info.asn = Some(1234);

        let descriptor = PeerDescriptor::from_peer_info(&peer_info);
        assert_eq!(descriptor.peer_id_bytes, vec![1, 2, 3, 4]);
        assert_eq!(descriptor.addresses.len(), 1);
        assert_eq!(descriptor.capabilities.len(), 2);
        assert!(descriptor.capabilities.contains(&"relay".to_string()));
        assert_eq!(descriptor.asn, Some(1234));

        // Convert back
        let converted = descriptor.to_peer_info();
        assert_eq!(converted.peer_id.0, vec![1, 2, 3, 4]);
        assert!(converted.capabilities.contains(&Capability::Relay));
        assert!(converted.capabilities.contains(&Capability::DhtNode));
    }

    #[tokio::test]
    async fn test_overlay_discovery_creation() {
        let config = DiscoveryConfig::default();
        let route_table = Arc::new(RouteTable::new());
        let (event_tx, _event_rx) = mpsc::channel(100);

        let discovery = OverlayDiscovery::new(config, route_table, event_tx);
        assert_eq!(discovery.discovered_count().await, 0);
    }

    #[tokio::test]
    async fn test_handle_peer_discovered() {
        let config = DiscoveryConfig::default();
        let route_table = Arc::new(RouteTable::new());
        let (event_tx, mut event_rx) = mpsc::channel(100);

        let discovery = Arc::new(OverlayDiscovery::new(config, route_table.clone(), event_tx));

        let mut peer_info = PeerInfo::new(
            PeerId::from_bytes(vec![5, 6, 7, 8]),
            vec!["192.168.1.1:8080".to_string()],
        );
        peer_info.capabilities.push(Capability::Relay);

        discovery.handle_peer_discovered(peer_info.clone()).await;

        // Check that peer was added to route table
        assert_eq!(route_table.peer_count().await, 1);

        // Check that peer is in discovered set
        assert_eq!(discovery.discovered_count().await, 1);

        // Check event was emitted
        let event = tokio::time::timeout(Duration::from_millis(100), event_rx.recv())
            .await
            .unwrap()
            .unwrap();

        match event {
            DiscoveryEvent::NewPeerFound { peer_id } => {
                assert_eq!(peer_id.0, vec![5, 6, 7, 8]);
            }
            _ => panic!("Expected NewPeerFound event"),
        }
    }

    #[tokio::test]
    async fn test_handle_liveness_result_alive() {
        let config = DiscoveryConfig::default();
        let route_table = Arc::new(RouteTable::new());
        let (event_tx, _event_rx) = mpsc::channel(100);

        let discovery = Arc::new(OverlayDiscovery::new(config, route_table.clone(), event_tx));

        // Add a peer first
        let mut peer_info = PeerInfo::new(
            PeerId::from_bytes(vec![9, 10, 11, 12]),
            vec!["10.0.0.1:8080".to_string()],
        );
        peer_info.capabilities.push(Capability::Relay);
        peer_info.failure_count = 2;

        route_table
            .handle_command(RouteTableCommand::InsertPeer(peer_info.clone()))
            .await
            .unwrap();

        // Report as alive
        discovery
            .handle_liveness_result(peer_info.peer_id.clone(), true)
            .await;

        // Check that failure count was reduced
        let (tx, rx) = tokio::sync::oneshot::channel();
        route_table
            .handle_command(RouteTableCommand::GetPeer {
                peer_id: peer_info.peer_id,
                response_tx: tx,
            })
            .await
            .unwrap();

        let updated_peer = rx.await.unwrap().unwrap();
        assert_eq!(updated_peer.failure_count, 1); // Reduced from 2 to 1
    }

    #[tokio::test]
    async fn test_handle_liveness_result_dead() {
        let config = DiscoveryConfig::default();
        let route_table = Arc::new(RouteTable::new());
        let (event_tx, mut event_rx) = mpsc::channel(100);

        let discovery = Arc::new(OverlayDiscovery::new(config, route_table.clone(), event_tx));

        // Add a peer first
        let mut peer_info = PeerInfo::new(
            PeerId::from_bytes(vec![13, 14, 15, 16]),
            vec!["172.16.0.1:8080".to_string()],
        );
        peer_info.capabilities.push(Capability::Relay);

        route_table
            .handle_command(RouteTableCommand::InsertPeer(peer_info.clone()))
            .await
            .unwrap();

        // Report as dead
        discovery
            .handle_liveness_result(peer_info.peer_id.clone(), false)
            .await;

        // Check event was emitted
        let event = tokio::time::timeout(Duration::from_millis(100), event_rx.recv())
            .await
            .unwrap()
            .unwrap();

        match event {
            DiscoveryEvent::PeerUnreachable { peer_id } => {
                assert_eq!(peer_id.0, vec![13, 14, 15, 16]);
            }
            _ => panic!("Expected PeerUnreachable event"),
        }

        // Check that failure count was incremented
        let (tx, rx) = tokio::sync::oneshot::channel();
        route_table
            .handle_command(RouteTableCommand::GetPeer {
                peer_id: peer_info.peer_id,
                response_tx: tx,
            })
            .await
            .unwrap();

        let updated_peer = rx.await.unwrap().unwrap();
        assert_eq!(updated_peer.failure_count, 1);
    }

    #[tokio::test]
    async fn test_count_relays() {
        let config = DiscoveryConfig::default();
        let route_table = Arc::new(RouteTable::new());
        let (event_tx, _event_rx) = mpsc::channel(100);

        let discovery = Arc::new(OverlayDiscovery::new(config, route_table.clone(), event_tx));

        // Add some relay peers
        for i in 1..=5 {
            let mut peer = PeerInfo::new(
                PeerId::from_bytes(vec![i]),
                vec![format!("10.0.0.{}:8080", i)],
            );
            peer.capabilities.push(Capability::Relay);
            route_table
                .handle_command(RouteTableCommand::InsertPeer(peer))
                .await
                .unwrap();
        }

        // Add a non-relay peer
        let non_relay = PeerInfo::new(
            PeerId::from_bytes(vec![99]),
            vec!["10.0.0.99:8080".to_string()],
        );
        route_table
            .handle_command(RouteTableCommand::InsertPeer(non_relay))
            .await
            .unwrap();

        assert_eq!(discovery.count_relays().await, 5);
    }

    #[tokio::test]
    async fn test_bootstrap_peers() {
        let mut config = DiscoveryConfig::default();
        config.bootstrap_peers = vec![
            "bootstrap1.example.com:8080".to_string(),
            "bootstrap2.example.com:8080".to_string(),
        ];

        let route_table = Arc::new(RouteTable::new());
        let (event_tx, _event_rx) = mpsc::channel(100);

        let discovery = Arc::new(OverlayDiscovery::new(config, route_table.clone(), event_tx));

        discovery.try_bootstrap_peers().await.unwrap();

        // Check that bootstrap peers were added
        assert_eq!(route_table.peer_count().await, 2);
        assert_eq!(discovery.discovered_count().await, 2);
    }

    #[tokio::test]
    async fn test_periodic_discovery_under_capacity() {
        let mut config = DiscoveryConfig::default();
        config.target_relay_count = 5;
        config.bootstrap_peers = vec![
            "bootstrap.example.com:8080".to_string(),
        ];

        let route_table = Arc::new(RouteTable::new());
        let (event_tx, mut event_rx) = mpsc::channel(100);

        let discovery = Arc::new(OverlayDiscovery::new(config, route_table.clone(), event_tx));

        // Run periodic discovery
        discovery.periodic_discovery().await.unwrap();

        // Should have tried bootstrap since we're under capacity
        assert!(route_table.peer_count().await > 0);

        // Should emit RelayPoolUpdated event
        let event = tokio::time::timeout(Duration::from_millis(100), event_rx.recv())
            .await
            .unwrap()
            .unwrap();

        match event {
            DiscoveryEvent::RelayPoolUpdated { relay_count } => {
                assert!(relay_count > 0);
            }
            _ => panic!("Expected RelayPoolUpdated event"),
        }
    }

    #[tokio::test]
    async fn test_discovery_command_trigger() {
        let config = DiscoveryConfig::default();
        let route_table = Arc::new(RouteTable::new());
        let (event_tx, mut event_rx) = mpsc::channel(100);
        let (cmd_tx, cmd_rx) = mpsc::channel(100);

        let discovery = Arc::new(OverlayDiscovery::new(
            config,
            route_table.clone(),
            event_tx,
        ));

        // Spawn discovery task
        let discovery_clone = discovery.clone();
        let task = tokio::spawn(async move {
            discovery_clone.run(cmd_rx).await;
        });

        // Trigger manual discovery
        cmd_tx
            .send(DiscoveryCommand::TriggerDiscovery)
            .await
            .unwrap();

        // Should emit event
        let event = tokio::time::timeout(Duration::from_millis(200), event_rx.recv())
            .await
            .unwrap()
            .unwrap();

        match event {
            DiscoveryEvent::RelayPoolUpdated { .. } => {
                // Success
            }
            _ => panic!("Expected RelayPoolUpdated event"),
        }

        // Shutdown
        cmd_tx.send(DiscoveryCommand::Shutdown).await.unwrap();
        let _ = tokio::time::timeout(Duration::from_millis(100), task).await;
    }
}
