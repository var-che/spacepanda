/*
    anti_entropy.rs - Anti-Entropy Sync Protocol
    
    Periodic sync loop to detect and fetch missing operations.
    
    Responsibilities:
    - Periodically compare vector clocks with peers
    - Detect missing operations
    - Request missing deltas from DHT or peers
    - Resolve divergence through gossip
    
    Anti-Entropy ensures eventual consistency even when:
    - Direct messages are lost
    - Nodes are temporarily offline
    - Network partitions heal
*/

use crate::core_store::crdt::VectorClock;
use crate::core_store::store::errors::StoreResult;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Configuration for anti-entropy sync
#[derive(Debug, Clone)]
pub struct AntiEntropyConfig {
    /// How often to run anti-entropy (in seconds)
    pub sync_interval: Duration,
    
    /// Maximum number of peers to sync with per round
    pub max_peers_per_round: usize,
    
    /// Maximum age of a delta before requesting (in seconds)
    pub max_delta_age: u64,
    
    /// Whether to use DHT as fallback for missing data
    pub use_dht_fallback: bool,
    
    /// Batch size for delta requests
    pub batch_size: usize,
}

impl Default for AntiEntropyConfig {
    fn default() -> Self {
        AntiEntropyConfig {
            sync_interval: Duration::from_secs(30),
            max_peers_per_round: 3,
            max_delta_age: 3600, // 1 hour
            use_dht_fallback: true,
            batch_size: 10,
        }
    }
}

/// Represents a peer's sync state
#[derive(Debug, Clone)]
pub struct PeerSyncState {
    /// Peer node ID
    pub peer_id: String,
    
    /// Peer's vector clock
    pub vector_clock: VectorClock,
    
    /// Last successful sync timestamp
    pub last_sync: u64,
    
    /// Number of consecutive failures
    pub failure_count: u32,
}

/// Sync request to send to a peer
#[derive(Debug, Clone)]
pub struct SyncRequest {
    /// Target channel or space
    pub target_id: String,
    
    /// Our current vector clock
    pub our_clock: VectorClock,
    
    /// Request ID for correlation
    pub request_id: String,
}

/// Sync response from a peer
#[derive(Debug, Clone)]
pub struct SyncResponse {
    /// Peer's vector clock
    pub peer_clock: VectorClock,
    
    /// Delta IDs we're missing
    pub missing_deltas: Vec<String>,
    
    /// Request ID this responds to
    pub request_id: String,
}

/// Anti-entropy manager
pub struct AntiEntropyManager {
    /// Configuration
    config: AntiEntropyConfig,
    
    /// Known peers and their sync state
    peers: HashMap<String, PeerSyncState>,
    
    /// Targets we're tracking (channels, spaces)
    targets: HashSet<String>,
    
    /// Our current vector clocks per target
    our_clocks: HashMap<String, VectorClock>,
    
    /// Last sync round timestamp
    last_sync_round: u64,
    
    /// Pending sync requests
    pending_requests: HashMap<String, SyncRequest>,
}

impl AntiEntropyManager {
    /// Create a new anti-entropy manager
    pub fn new(config: AntiEntropyConfig) -> Self {
        AntiEntropyManager {
            config,
            peers: HashMap::new(),
            targets: HashSet::new(),
            our_clocks: HashMap::new(),
            last_sync_round: 0,
            pending_requests: HashMap::new(),
        }
    }
    
    /// Add a target to track
    pub fn add_target(&mut self, target_id: String, clock: VectorClock) {
        self.targets.insert(target_id.clone());
        self.our_clocks.insert(target_id, clock);
    }
    
    /// Remove a target
    pub fn remove_target(&mut self, target_id: &str) {
        self.targets.remove(target_id);
        self.our_clocks.remove(target_id);
    }
    
    /// Update our vector clock for a target
    pub fn update_clock(&mut self, target_id: &str, clock: VectorClock) {
        if self.targets.contains(target_id) {
            self.our_clocks.insert(target_id.to_string(), clock);
        }
    }
    
    /// Register a peer
    pub fn add_peer(&mut self, peer_id: String, vector_clock: VectorClock) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("System clock is before UNIX epoch")
            .as_secs();
        
        self.peers.insert(peer_id.clone(), PeerSyncState {
            peer_id,
            vector_clock,
            last_sync: now,
            failure_count: 0,
        });
    }
    
    /// Remove a peer
    pub fn remove_peer(&mut self, peer_id: &str) {
        self.peers.remove(peer_id);
    }
    
    /// Update peer's vector clock
    pub fn update_peer_clock(&mut self, peer_id: &str, clock: VectorClock) {
        if let Some(peer) = self.peers.get_mut(peer_id) {
            peer.vector_clock = clock;
            peer.last_sync = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("System clock is before UNIX epoch")
                .as_secs();
            peer.failure_count = 0;
        }
    }
    
    /// Check if we should run a sync round
    pub fn should_sync(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("System clock is before UNIX epoch")
            .as_secs();
        
        now - self.last_sync_round >= self.config.sync_interval.as_secs()
    }
    
    /// Select peers for this sync round
    pub fn select_peers_for_sync(&self) -> Vec<String> {
        let mut peers: Vec<_> = self.peers.keys().cloned().collect();
        
        // Prioritize peers we haven't synced with recently
        peers.sort_by_key(|peer_id| {
            self.peers.get(peer_id).map(|p| p.last_sync).unwrap_or(0)
        });
        
        peers.truncate(self.config.max_peers_per_round);
        peers
    }
    
    /// Detect missing operations by comparing clocks
    pub fn detect_missing(&self, our_clock: &VectorClock, peer_clock: &VectorClock) -> bool {
        // If peer has operations we don't have, we're missing data
        // This is simplified - actual implementation needs VectorClock comparison
        peer_clock.is_concurrent(our_clock) || peer_clock.happened_before(our_clock)
    }
    
    /// Create sync requests for all targets
    pub fn create_sync_requests(&mut self, peer_ids: &[String]) -> Vec<SyncRequest> {
        let mut requests = Vec::new();
        
        for target_id in &self.targets {
            for peer_id in peer_ids {
                if let Some(our_clock) = self.our_clocks.get(target_id) {
                    let request_id = format!("{}:{}:{}", 
                        target_id, 
                        peer_id, 
                        SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .expect("System clock is before UNIX epoch")
                            .as_millis()
                    );
                    
                    let request = SyncRequest {
                        target_id: target_id.clone(),
                        our_clock: our_clock.clone(),
                        request_id: request_id.clone(),
                    };
                    
                    self.pending_requests.insert(request_id, request.clone());
                    requests.push(request);
                }
            }
        }
        
        requests
    }
    
    /// Process a sync response
    pub fn process_sync_response(&mut self, response: SyncResponse) -> StoreResult<Vec<String>> {
        // Remove from pending
        self.pending_requests.remove(&response.request_id);
        
        // Return the list of deltas we need to fetch
        Ok(response.missing_deltas)
    }
    
    /// Mark sync round as complete
    pub fn mark_sync_complete(&mut self) {
        self.last_sync_round = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("System clock is before UNIX epoch")
            .as_secs();
    }
    
    /// Record a peer sync failure
    pub fn record_failure(&mut self, peer_id: &str) {
        if let Some(peer) = self.peers.get_mut(peer_id) {
            peer.failure_count += 1;
            
            // Remove peer if too many failures
            if peer.failure_count > 5 {
                self.peers.remove(peer_id);
            }
        }
    }
    
    /// Get statistics
    pub fn stats(&self) -> AntiEntropyStats {
        AntiEntropyStats {
            peer_count: self.peers.len(),
            target_count: self.targets.len(),
            pending_requests: self.pending_requests.len(),
            last_sync: self.last_sync_round,
        }
    }
}

/// Anti-entropy statistics
#[derive(Debug, Clone)]
pub struct AntiEntropyStats {
    pub peer_count: usize,
    pub target_count: usize,
    pub pending_requests: usize,
    pub last_sync: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_anti_entropy_manager_creation() {
        let config = AntiEntropyConfig::default();
        let manager = AntiEntropyManager::new(config);
        
        let stats = manager.stats();
        assert_eq!(stats.peer_count, 0);
        assert_eq!(stats.target_count, 0);
    }
    
    #[test]
    fn test_add_target() {
        let config = AntiEntropyConfig::default();
        let mut manager = AntiEntropyManager::new(config);
        
        let vc = VectorClock::new();
        manager.add_target("channel_123".to_string(), vc);
        
        let stats = manager.stats();
        assert_eq!(stats.target_count, 1);
    }
    
    #[test]
    fn test_add_peer() {
        let config = AntiEntropyConfig::default();
        let mut manager = AntiEntropyManager::new(config);
        
        let vc = VectorClock::new();
        manager.add_peer("peer1".to_string(), vc);
        
        let stats = manager.stats();
        assert_eq!(stats.peer_count, 1);
    }
    
    #[test]
    fn test_select_peers_for_sync() {
        let config = AntiEntropyConfig {
            max_peers_per_round: 2,
            ..Default::default()
        };
        let mut manager = AntiEntropyManager::new(config);
        
        let vc = VectorClock::new();
        manager.add_peer("peer1".to_string(), vc.clone());
        manager.add_peer("peer2".to_string(), vc.clone());
        manager.add_peer("peer3".to_string(), vc);
        
        let selected = manager.select_peers_for_sync();
        assert_eq!(selected.len(), 2);
    }
    
    #[test]
    fn test_create_sync_requests() {
        let config = AntiEntropyConfig::default();
        let mut manager = AntiEntropyManager::new(config);
        
        let vc = VectorClock::new();
        manager.add_target("channel_123".to_string(), vc.clone());
        manager.add_peer("peer1".to_string(), vc);
        
        let requests = manager.create_sync_requests(&["peer1".to_string()]);
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].target_id, "channel_123");
    }
    
    #[test]
    fn test_record_failure() {
        let config = AntiEntropyConfig::default();
        let mut manager = AntiEntropyManager::new(config);
        
        let vc = VectorClock::new();
        manager.add_peer("peer1".to_string(), vc);
        
        // Record multiple failures
        for _ in 0..6 {
            manager.record_failure("peer1");
        }
        
        // Peer should be removed after too many failures
        let stats = manager.stats();
        assert_eq!(stats.peer_count, 0);
    }
}
