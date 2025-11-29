/*
    RouteTable - peer metadata and selection

    Local cache of known peers, their addresses, capabilities, and stats like last-seen, latency, reliability.
    Used for path selection (onion routing) and peer dialing and DHT lookups.

    Workflow:
    1. Insert peers discovered from bootstrap, RPC peer exchange, DHT responses
    2. Maintain health metrics (last seen, latency, failures)
    3. Provide helper: pick_diverse_relays(k) for onion path building (favour different ASNs, geos, low latency)

    Inputs:
      - RouteTableCommand::InsertPeer(peer_info)
      - RouteTableCommand::UpdatePeerStats(peer_id, stats)
      - RouteTableCommand::PickDiverseRelays(k)

    Outputs:
      - Queries like "get_best_route_for(peer_id)" or "pick_diverse_relays(k)"
    
    Notes:

    PeerInfo structure includes:
    - peer_id: PeerId
    - addresses: Vec<String> (simplified from Multiaddr for now)
    - capabilities: Vec<Capability>
    - overlay_pubkey: [u8; 32]
    - last_seen: SystemTime
    - latency: Option<Duration>
    - failure_count: u32
    - asn: Option<u32>
    - geo_location: Option<GeoLocation>
*/

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::{oneshot, Mutex};

use super::session_manager::PeerId;

/// Peer capabilities (e.g., relay, DHT node, storage)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Capability {
    Relay,
    DhtNode,
    Storage,
    LongLived, // Stable peer with good uptime
}

/// Geographic location for diversity
#[derive(Debug, Clone, PartialEq)]
pub struct GeoLocation {
    pub latitude: f64,
    pub longitude: f64,
    pub country_code: Option<String>,
}

/// Comprehensive peer information
#[derive(Debug, Clone)]
pub struct PeerInfo {
    pub peer_id: PeerId,
    pub addresses: Vec<String>,
    pub capabilities: Vec<Capability>,
    pub overlay_pubkey: [u8; 32],
    pub last_seen: SystemTime,
    pub latency: Option<Duration>,
    pub failure_count: u32,
    pub asn: Option<u32>,
    pub geo_location: Option<GeoLocation>,
}

impl PeerInfo {
    /// Create a new PeerInfo with minimal data
    pub fn new(peer_id: PeerId, addresses: Vec<String>) -> Self {
        PeerInfo {
            peer_id,
            addresses,
            capabilities: Vec::new(),
            overlay_pubkey: [0u8; 32],
            last_seen: SystemTime::now(),
            latency: None,
            failure_count: 0,
            asn: None,
            geo_location: None,
        }
    }

    /// Check if peer is healthy (low failure count, recently seen)
    pub fn is_healthy(&self, max_failures: u32, max_age: Duration) -> bool {
        if self.failure_count > max_failures {
            return false;
        }

        if let Ok(elapsed) = self.last_seen.elapsed() {
            elapsed < max_age
        } else {
            false
        }
    }

    /// Get a score for relay selection (lower is better)
    pub fn relay_score(&self) -> u64 {
        let mut score = 0u64;

        // Penalize failures
        score += (self.failure_count as u64) * 1000;

        // Penalize high latency
        if let Some(latency) = self.latency {
            score += latency.as_millis() as u64;
        } else {
            score += 500; // Unknown latency gets medium penalty
        }

        // Penalize old last_seen
        if let Ok(elapsed) = self.last_seen.elapsed() {
            score += elapsed.as_secs();
        }

        score
    }
}

/// Statistics update for a peer
#[derive(Debug, Clone)]
pub struct PeerStats {
    pub latency: Option<Duration>,
    pub failure_count_delta: i32, // Can be negative to reset
    pub last_seen: SystemTime,
}

/// Commands for RouteTable
#[derive(Debug)]
pub enum RouteTableCommand {
    /// Insert or update a peer
    InsertPeer(PeerInfo),
    /// Update peer statistics
    UpdatePeerStats {
        peer_id: PeerId,
        stats: PeerStats,
    },
    /// Pick k diverse relays for onion routing
    PickDiverseRelays {
        k: usize,
        response_tx: oneshot::Sender<Vec<PeerInfo>>,
    },
    /// Get a specific peer's info
    GetPeer {
        peer_id: PeerId,
        response_tx: oneshot::Sender<Option<PeerInfo>>,
    },
    /// List all peers with a specific capability
    ListPeersByCapability {
        capability: Capability,
        response_tx: oneshot::Sender<Vec<PeerInfo>>,
    },
    /// Remove a peer
    RemovePeer(PeerId),
    /// Get all known peers
    GetAllPeers {
        response_tx: oneshot::Sender<Vec<PeerInfo>>,
    },
}

/// RouteTable managing peer metadata
pub struct RouteTable {
    peers: Arc<Mutex<HashMap<PeerId, PeerInfo>>>,
}

impl RouteTable {
    /// Create a new RouteTable
    pub fn new() -> Self {
        RouteTable {
            peers: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Handle a command
    pub async fn handle_command(&self, command: RouteTableCommand) -> Result<(), String> {
        match command {
            RouteTableCommand::InsertPeer(peer_info) => {
                self.insert_peer(peer_info).await;
            }
            RouteTableCommand::UpdatePeerStats { peer_id, stats } => {
                self.update_peer_stats(peer_id, stats).await?;
            }
            RouteTableCommand::PickDiverseRelays { k, response_tx } => {
                let relays = self.pick_diverse_relays(k).await;
                let _ = response_tx.send(relays);
            }
            RouteTableCommand::GetPeer {
                peer_id,
                response_tx,
            } => {
                let peer = self.get_peer(&peer_id).await;
                let _ = response_tx.send(peer);
            }
            RouteTableCommand::ListPeersByCapability {
                capability,
                response_tx,
            } => {
                let peers = self.list_peers_by_capability(&capability).await;
                let _ = response_tx.send(peers);
            }
            RouteTableCommand::RemovePeer(peer_id) => {
                self.remove_peer(&peer_id).await;
            }
            RouteTableCommand::GetAllPeers { response_tx } => {
                let peers = self.get_all_peers().await;
                let _ = response_tx.send(peers);
            }
        }
        Ok(())
    }

    /// Insert or update a peer
    async fn insert_peer(&self, peer_info: PeerInfo) {
        let mut peers = self.peers.lock().await;
        peers.insert(peer_info.peer_id.clone(), peer_info);
    }

    /// Update peer statistics
    async fn update_peer_stats(&self, peer_id: PeerId, stats: PeerStats) -> Result<(), String> {
        let mut peers = self.peers.lock().await;
        
        if let Some(peer) = peers.get_mut(&peer_id) {
            if let Some(latency) = stats.latency {
                peer.latency = Some(latency);
            }
            
            // Update failure count (can go negative but floor at 0)
            let new_count = (peer.failure_count as i32) + stats.failure_count_delta;
            peer.failure_count = new_count.max(0) as u32;
            
            peer.last_seen = stats.last_seen;
            
            Ok(())
        } else {
            Err(format!("Peer {:?} not found", peer_id))
        }
    }

    /// Get a specific peer
    async fn get_peer(&self, peer_id: &PeerId) -> Option<PeerInfo> {
        let peers = self.peers.lock().await;
        peers.get(peer_id).cloned()
    }

    /// Remove a peer
    async fn remove_peer(&self, peer_id: &PeerId) {
        let mut peers = self.peers.lock().await;
        peers.remove(peer_id);
    }

    /// Get all peers
    async fn get_all_peers(&self) -> Vec<PeerInfo> {
        let peers = self.peers.lock().await;
        peers.values().cloned().collect()
    }

    /// List peers by capability
    async fn list_peers_by_capability(&self, capability: &Capability) -> Vec<PeerInfo> {
        let peers = self.peers.lock().await;
        peers
            .values()
            .filter(|p| p.capabilities.contains(capability))
            .cloned()
            .collect()
    }

    /// Pick k diverse relays for onion routing
    /// Prioritizes diversity in ASN, geo location, and low latency
    async fn pick_diverse_relays(&self, k: usize) -> Vec<PeerInfo> {
        let peers = self.peers.lock().await;
        
        // Filter to healthy relay-capable peers
        let mut candidates: Vec<PeerInfo> = peers
            .values()
            .filter(|p| {
                p.capabilities.contains(&Capability::Relay)
                    && p.is_healthy(3, Duration::from_secs(3600)) // max 3 failures, seen in last hour
            })
            .cloned()
            .collect();

        if candidates.is_empty() {
            return Vec::new();
        }

        // Sort by relay score (best first)
        candidates.sort_by_key(|p| p.relay_score());

        // Pick diverse relays
        let mut selected = Vec::new();
        let mut used_asns = Vec::new();
        
        for peer in candidates.iter() {
            if selected.len() >= k {
                break;
            }

            // Check ASN diversity
            let is_diverse = if let Some(asn) = peer.asn {
                !used_asns.contains(&asn)
            } else {
                true // Unknown ASN is considered diverse
            };

            if is_diverse {
                selected.push(peer.clone());
                if let Some(asn) = peer.asn {
                    used_asns.push(asn);
                }
            }
        }

        // If we don't have enough diverse peers, fill with best remaining
        if selected.len() < k {
            for peer in candidates.iter() {
                if selected.len() >= k {
                    break;
                }
                if !selected.iter().any(|p| p.peer_id == peer.peer_id) {
                    selected.push(peer.clone());
                }
            }
        }

        selected
    }

    /// Get the number of known peers
    pub async fn peer_count(&self) -> usize {
        let peers = self.peers.lock().await;
        peers.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_peer(id: u8, asn: Option<u32>, latency_ms: Option<u64>) -> PeerInfo {
        let mut peer = PeerInfo::new(
            PeerId::from_bytes(vec![id]),
            vec![format!("127.0.0.1:{}00", id)],
        );
        peer.capabilities.push(Capability::Relay);
        peer.asn = asn;
        peer.latency = latency_ms.map(Duration::from_millis);
        peer
    }

    #[tokio::test]
    async fn test_insert_and_get_peer() {
        let route_table = RouteTable::new();
        let peer = create_test_peer(1, Some(1234), Some(50));

        route_table.insert_peer(peer.clone()).await;

        let retrieved = route_table.get_peer(&peer.peer_id).await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().peer_id, peer.peer_id);
    }

    #[tokio::test]
    async fn test_update_peer_stats() {
        let route_table = RouteTable::new();
        let peer = create_test_peer(1, Some(1234), Some(50));
        
        route_table.insert_peer(peer.clone()).await;

        let stats = PeerStats {
            latency: Some(Duration::from_millis(100)),
            failure_count_delta: 2,
            last_seen: SystemTime::now(),
        };

        route_table
            .update_peer_stats(peer.peer_id.clone(), stats)
            .await
            .unwrap();

        let updated = route_table.get_peer(&peer.peer_id).await.unwrap();
        assert_eq!(updated.latency.unwrap().as_millis(), 100);
        assert_eq!(updated.failure_count, 2);
    }

    #[tokio::test]
    async fn test_remove_peer() {
        let route_table = RouteTable::new();
        let peer = create_test_peer(1, Some(1234), Some(50));

        route_table.insert_peer(peer.clone()).await;
        assert_eq!(route_table.peer_count().await, 1);

        route_table.remove_peer(&peer.peer_id).await;
        assert_eq!(route_table.peer_count().await, 0);
    }

    #[tokio::test]
    async fn test_list_peers_by_capability() {
        let route_table = RouteTable::new();
        
        let mut peer1 = create_test_peer(1, Some(1234), Some(50));
        peer1.capabilities.push(Capability::DhtNode);
        
        let peer2 = create_test_peer(2, Some(5678), Some(75));
        // peer2 only has Relay from create_test_peer
        
        let mut peer3 = create_test_peer(3, Some(9012), Some(100));
        peer3.capabilities.push(Capability::DhtNode);

        route_table.insert_peer(peer1).await;
        route_table.insert_peer(peer2).await;
        route_table.insert_peer(peer3).await;

        let (tx, rx) = oneshot::channel();
        route_table
            .handle_command(RouteTableCommand::ListPeersByCapability {
                capability: Capability::DhtNode,
                response_tx: tx,
            })
            .await
            .unwrap();

        let dht_peers = rx.await.unwrap();
        assert_eq!(dht_peers.len(), 2);
    }

    #[tokio::test]
    async fn test_pick_diverse_relays() {
        let route_table = RouteTable::new();

        // Add peers with different ASNs
        let peer1 = create_test_peer(1, Some(1111), Some(50));
        let peer2 = create_test_peer(2, Some(2222), Some(75));
        let peer3 = create_test_peer(3, Some(3333), Some(100));
        let peer4 = create_test_peer(4, Some(1111), Some(60)); // Same ASN as peer1

        route_table.insert_peer(peer1).await;
        route_table.insert_peer(peer2).await;
        route_table.insert_peer(peer3).await;
        route_table.insert_peer(peer4).await;

        let (tx, rx) = oneshot::channel();
        route_table
            .handle_command(RouteTableCommand::PickDiverseRelays {
                k: 3,
                response_tx: tx,
            })
            .await
            .unwrap();

        let relays = rx.await.unwrap();
        assert_eq!(relays.len(), 3);

        // Check ASN diversity - should pick peers with different ASNs first
        let asns: Vec<u32> = relays.iter().filter_map(|p| p.asn).collect();
        let unique_asns: std::collections::HashSet<_> = asns.iter().collect();
        assert_eq!(unique_asns.len(), 3); // All 3 should have different ASNs
    }

    #[tokio::test]
    async fn test_pick_diverse_relays_with_unhealthy_peers() {
        let route_table = RouteTable::new();

        let mut peer1 = create_test_peer(1, Some(1111), Some(50));
        peer1.failure_count = 5; // Unhealthy

        let peer2 = create_test_peer(2, Some(2222), Some(75));
        let peer3 = create_test_peer(3, Some(3333), Some(100));

        route_table.insert_peer(peer1).await;
        route_table.insert_peer(peer2).await;
        route_table.insert_peer(peer3).await;

        let (tx, rx) = oneshot::channel();
        route_table
            .handle_command(RouteTableCommand::PickDiverseRelays {
                k: 3,
                response_tx: tx,
            })
            .await
            .unwrap();

        let relays = rx.await.unwrap();
        // Should only pick 2 healthy peers
        assert_eq!(relays.len(), 2);
        assert!(relays.iter().all(|p| p.failure_count <= 3));
    }

    #[tokio::test]
    async fn test_peer_is_healthy() {
        let peer = create_test_peer(1, Some(1234), Some(50));
        assert!(peer.is_healthy(3, Duration::from_secs(3600)));

        let mut unhealthy = create_test_peer(2, Some(5678), Some(75));
        unhealthy.failure_count = 5;
        assert!(!unhealthy.is_healthy(3, Duration::from_secs(3600)));
    }

    #[tokio::test]
    async fn test_relay_score() {
        let peer1 = create_test_peer(1, Some(1111), Some(50));
        let mut peer2 = create_test_peer(2, Some(2222), Some(200));
        peer2.failure_count = 2;

        // peer1 should have better score (lower is better)
        assert!(peer1.relay_score() < peer2.relay_score());
    }

    #[tokio::test]
    async fn test_get_all_peers() {
        let route_table = RouteTable::new();

        route_table.insert_peer(create_test_peer(1, Some(1111), Some(50))).await;
        route_table.insert_peer(create_test_peer(2, Some(2222), Some(75))).await;
        route_table.insert_peer(create_test_peer(3, Some(3333), Some(100))).await;

        let (tx, rx) = oneshot::channel();
        route_table
            .handle_command(RouteTableCommand::GetAllPeers { response_tx: tx })
            .await
            .unwrap();

        let all_peers = rx.await.unwrap();
        assert_eq!(all_peers.len(), 3);
    }
}
