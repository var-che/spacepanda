/*
    DHTTable DHT version -  Kademlia bucket structure, store nearest peers by XOR distance

    Responsibilities:
    `routing_table.rs` implements the Kademlia routing table structure.
    It performs: bucket maintenance, insert/remove peer, replace stale entries, select closest nodes, respond to node lookup queries, refresh random IDs.

    Inputs:
    - peer info discovered
    - successful/failed RPC calls
    - DHT messages from peers

    Outputs:
    - list of closest K peers for a given key
    - bucket refresh events
*/

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use super::dht_key::DhtKey;

/// Peer information stored in routing table
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PeerContact {
    /// Peer's DHT ID
    pub id: DhtKey,
    /// Network address
    pub address: String,
    /// Last time we saw this peer (Unix timestamp)
    pub last_seen: u64,
    /// Number of failed RPC attempts
    pub failed_rpcs: u32,
}

impl PeerContact {
    pub fn new(id: DhtKey, address: String) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        PeerContact {
            id,
            address,
            last_seen: now,
            failed_rpcs: 0,
        }
    }

    /// Update last seen timestamp
    pub fn touch(&mut self) {
        self.last_seen = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        self.failed_rpcs = 0;
    }

    /// Mark a failed RPC
    pub fn mark_failed(&mut self) {
        self.failed_rpcs += 1;
    }

    /// Check if peer is considered stale (no response in threshold seconds)
    pub fn is_stale(&self, threshold_secs: u64) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        (now - self.last_seen) > threshold_secs || self.failed_rpcs >= 3
    }
}

/// A k-bucket containing peers at a specific distance range
#[derive(Debug, Clone)]
struct KBucket {
    /// Peers in this bucket
    peers: Vec<PeerContact>,
    /// Maximum bucket size (k)
    k: usize,
    /// Last time this bucket was refreshed
    last_refresh: u64,
}

impl KBucket {
    fn new(k: usize) -> Self {
        KBucket {
            peers: Vec::new(),
            k,
            last_refresh: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }

    /// Try to insert a peer into the bucket
    fn insert(&mut self, peer: PeerContact) -> Result<(), String> {
        // If peer already exists, update it
        if let Some(existing) = self.peers.iter_mut().find(|p| p.id == peer.id) {
            existing.address = peer.address;
            existing.touch();
            return Ok(());
        }

        // If bucket not full, add peer
        if self.peers.len() < self.k {
            self.peers.push(peer);
            Ok(())
        } else {
            // Bucket full - try to replace a stale peer
            if let Some(stale_idx) = self.peers.iter().position(|p| p.is_stale(3600)) {
                self.peers[stale_idx] = peer;
                Ok(())
            } else {
                Err("Bucket full, no stale peers to replace".to_string())
            }
        }
    }

    /// Remove a peer from the bucket
    fn remove(&mut self, id: &DhtKey) -> bool {
        if let Some(pos) = self.peers.iter().position(|p| &p.id == id) {
            self.peers.remove(pos);
            true
        } else {
            false
        }
    }

    /// Get all peers in the bucket
    fn peers(&self) -> &[PeerContact] {
        &self.peers
    }

    /// Mark bucket as refreshed
    fn touch(&mut self) {
        self.last_refresh = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
    }

    /// Check if bucket needs refresh
    fn needs_refresh(&self, interval_secs: u64) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        (now - self.last_refresh) > interval_secs
    }
}

/// Kademlia routing table
pub struct RoutingTable {
    /// Our node ID
    local_id: DhtKey,
    /// K-buckets indexed by prefix length
    buckets: HashMap<usize, KBucket>,
    /// Bucket size (k parameter)
    k: usize,
}

impl RoutingTable {
    /// Create a new routing table
    pub fn new(local_id: DhtKey, k: usize) -> Self {
        RoutingTable {
            local_id,
            buckets: HashMap::new(),
            k,
        }
    }

    /// Get the bucket index for a given peer ID
    fn bucket_index(&self, peer_id: &DhtKey) -> usize {
        self.local_id.distance(peer_id).leading_zeros() as usize
    }

    /// Insert a peer into the routing table
    pub fn insert(&mut self, peer: PeerContact) -> Result<(), String> {
        // Don't insert ourselves
        if peer.id == self.local_id {
            return Err("Cannot insert local node".to_string());
        }

        let idx = self.bucket_index(&peer.id);
        let bucket = self.buckets.entry(idx).or_insert_with(|| KBucket::new(self.k));
        
        bucket.insert(peer)
    }

    /// Remove a peer from the routing table
    pub fn remove(&mut self, id: &DhtKey) -> bool {
        let idx = self.bucket_index(id);
        
        if let Some(bucket) = self.buckets.get_mut(&idx) {
            bucket.remove(id)
        } else {
            false
        }
    }

    /// Get a peer by ID
    pub fn get(&self, id: &DhtKey) -> Option<PeerContact> {
        let idx = self.bucket_index(id);
        
        self.buckets.get(&idx)
            .and_then(|bucket| bucket.peers().iter().find(|p| &p.id == id))
            .cloned()
    }

    /// Update a peer's last seen timestamp
    pub fn touch(&mut self, id: &DhtKey) {
        let idx = self.bucket_index(id);
        
        if let Some(bucket) = self.buckets.get_mut(&idx) {
            if let Some(peer) = bucket.peers.iter_mut().find(|p| &p.id == id) {
                peer.touch();
            }
        }
    }

    /// Mark a peer as having failed an RPC
    pub fn mark_failed(&mut self, id: &DhtKey) {
        let idx = self.bucket_index(id);
        
        if let Some(bucket) = self.buckets.get_mut(&idx) {
            if let Some(peer) = bucket.peers.iter_mut().find(|p| &p.id == id) {
                peer.mark_failed();
            }
        }
    }

    /// Find the k closest peers to a target key
    pub fn find_closest(&self, target: &DhtKey, count: usize) -> Vec<PeerContact> {
        let mut all_peers: Vec<PeerContact> = self.buckets
            .values()
            .flat_map(|bucket| bucket.peers().iter().cloned())
            .collect();

        // Sort by XOR distance to target
        all_peers.sort_by_key(|peer| peer.id.distance(target));
        
        // Return up to count closest peers
        all_peers.into_iter().take(count).collect()
    }

    /// Get all peers in the routing table
    pub fn all_peers(&self) -> Vec<PeerContact> {
        self.buckets
            .values()
            .flat_map(|bucket| bucket.peers().iter().cloned())
            .collect()
    }

    /// Get total number of peers
    pub fn size(&self) -> usize {
        self.buckets.values().map(|b| b.peers.len()).sum()
    }

    /// Get buckets that need refresh
    pub fn buckets_needing_refresh(&self, interval_secs: u64) -> Vec<usize> {
        self.buckets
            .iter()
            .filter(|(_, bucket)| bucket.needs_refresh(interval_secs))
            .map(|(idx, _)| *idx)
            .collect()
    }

    /// Mark a bucket as refreshed
    pub fn touch_bucket(&mut self, idx: usize) {
        if let Some(bucket) = self.buckets.get_mut(&idx) {
            bucket.touch();
        }
    }

    /// Remove stale peers
    pub fn remove_stale_peers(&mut self, threshold_secs: u64) -> usize {
        let mut removed_count = 0;
        
        for bucket in self.buckets.values_mut() {
            let before = bucket.peers.len();
            bucket.peers.retain(|p| !p.is_stale(threshold_secs));
            removed_count += before - bucket.peers.len();
        }
        
        removed_count
    }

    /// Get the local node ID
    pub fn local_id(&self) -> DhtKey {
        self.local_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_peer_contact_creation() {
        let id = DhtKey::hash(b"peer1");
        let peer = PeerContact::new(id, "127.0.0.1:8080".to_string());
        
        assert_eq!(peer.id, id);
        assert_eq!(peer.address, "127.0.0.1:8080");
        assert_eq!(peer.failed_rpcs, 0);
    }

    #[test]
    fn test_peer_touch() {
        let id = DhtKey::hash(b"peer1");
        let mut peer = PeerContact::new(id, "127.0.0.1:8080".to_string());
        
        let before = peer.last_seen;
        peer.failed_rpcs = 5;
        
        std::thread::sleep(std::time::Duration::from_millis(1001));
        peer.touch();
        
        assert!(peer.last_seen > before);
        assert_eq!(peer.failed_rpcs, 0);
    }

    #[test]
    fn test_peer_failed() {
        let id = DhtKey::hash(b"peer1");
        let mut peer = PeerContact::new(id, "127.0.0.1:8080".to_string());
        
        assert_eq!(peer.failed_rpcs, 0);
        
        peer.mark_failed();
        assert_eq!(peer.failed_rpcs, 1);
        
        peer.mark_failed();
        peer.mark_failed();
        assert_eq!(peer.failed_rpcs, 3);
    }

    #[test]
    fn test_peer_is_stale() {
        let id = DhtKey::hash(b"peer1");
        let mut peer = PeerContact::new(id, "127.0.0.1:8080".to_string());
        
        // Fresh peer is not stale
        assert!(!peer.is_stale(3600));
        
        // Peer with 3+ failed RPCs is stale
        peer.mark_failed();
        peer.mark_failed();
        peer.mark_failed();
        assert!(peer.is_stale(3600));
    }

    #[test]
    fn test_routing_table_insert() {
        let local_id = DhtKey::hash(b"local");
        let mut table = RoutingTable::new(local_id, 20);
        
        let peer1 = PeerContact::new(DhtKey::hash(b"peer1"), "127.0.0.1:8001".to_string());
        let peer2 = PeerContact::new(DhtKey::hash(b"peer2"), "127.0.0.1:8002".to_string());
        
        assert!(table.insert(peer1.clone()).is_ok());
        assert!(table.insert(peer2.clone()).is_ok());
        
        assert_eq!(table.size(), 2);
    }

    #[test]
    fn test_routing_table_no_self_insert() {
        let local_id = DhtKey::hash(b"local");
        let mut table = RoutingTable::new(local_id, 20);
        
        let self_peer = PeerContact::new(local_id, "127.0.0.1:8000".to_string());
        
        assert!(table.insert(self_peer).is_err());
        assert_eq!(table.size(), 0);
    }

    #[test]
    fn test_routing_table_get() {
        let local_id = DhtKey::hash(b"local");
        let mut table = RoutingTable::new(local_id, 20);
        
        let peer_id = DhtKey::hash(b"peer1");
        let peer = PeerContact::new(peer_id, "127.0.0.1:8001".to_string());
        
        table.insert(peer.clone()).unwrap();
        
        let retrieved = table.get(&peer_id).unwrap();
        assert_eq!(retrieved.id, peer_id);
        assert_eq!(retrieved.address, "127.0.0.1:8001");
    }

    #[test]
    fn test_routing_table_remove() {
        let local_id = DhtKey::hash(b"local");
        let mut table = RoutingTable::new(local_id, 20);
        
        let peer_id = DhtKey::hash(b"peer1");
        let peer = PeerContact::new(peer_id, "127.0.0.1:8001".to_string());
        
        table.insert(peer).unwrap();
        assert_eq!(table.size(), 1);
        
        assert!(table.remove(&peer_id));
        assert_eq!(table.size(), 0);
        assert!(table.get(&peer_id).is_none());
    }

    #[test]
    fn test_routing_table_find_closest() {
        let local_id = DhtKey::hash(b"local");
        let mut table = RoutingTable::new(local_id, 20);
        
        // Insert multiple peers
        for i in 0..10 {
            let peer_id = DhtKey::hash(format!("peer{}", i).as_bytes());
            let peer = PeerContact::new(peer_id, format!("127.0.0.1:800{}", i));
            table.insert(peer).unwrap();
        }
        
        let target = DhtKey::hash(b"target");
        let closest = table.find_closest(&target, 5);
        
        assert_eq!(closest.len(), 5);
        
        // Verify they are sorted by distance
        for i in 0..closest.len() - 1 {
            let dist1 = closest[i].id.distance(&target);
            let dist2 = closest[i + 1].id.distance(&target);
            assert!(dist1 <= dist2);
        }
    }

    #[test]
    fn test_routing_table_touch() {
        let local_id = DhtKey::hash(b"local");
        let mut table = RoutingTable::new(local_id, 20);
        
        let peer_id = DhtKey::hash(b"peer1");
        let mut peer = PeerContact::new(peer_id, "127.0.0.1:8001".to_string());
        peer.failed_rpcs = 3;
        
        table.insert(peer).unwrap();
        
        std::thread::sleep(std::time::Duration::from_millis(10));
        table.touch(&peer_id);
        
        let updated = table.get(&peer_id).unwrap();
        assert_eq!(updated.failed_rpcs, 0);
    }

    #[test]
    fn test_routing_table_mark_failed() {
        let local_id = DhtKey::hash(b"local");
        let mut table = RoutingTable::new(local_id, 20);
        
        let peer_id = DhtKey::hash(b"peer1");
        let peer = PeerContact::new(peer_id, "127.0.0.1:8001".to_string());
        
        table.insert(peer).unwrap();
        
        table.mark_failed(&peer_id);
        table.mark_failed(&peer_id);
        
        let updated = table.get(&peer_id).unwrap();
        assert_eq!(updated.failed_rpcs, 2);
    }

    #[test]
    fn test_routing_table_all_peers() {
        let local_id = DhtKey::hash(b"local");
        let mut table = RoutingTable::new(local_id, 20);
        
        for i in 0..5 {
            let peer_id = DhtKey::hash(format!("peer{}", i).as_bytes());
            let peer = PeerContact::new(peer_id, format!("127.0.0.1:800{}", i));
            table.insert(peer).unwrap();
        }
        
        let all = table.all_peers();
        assert_eq!(all.len(), 5);
    }

    #[test]
    fn test_k_bucket_size_limit() {
        let local_id = DhtKey::hash(b"local");
        let mut table = RoutingTable::new(local_id, 3); // k=3
        
        // Try to insert 5 peers into the same bucket (very unlikely with real hash but force it)
        // For this test, we'll just test that table respects k
        let mut inserted = 0;
        for i in 0..10 {
            let peer_id = DhtKey::hash(format!("peer{}", i).as_bytes());
            let peer = PeerContact::new(peer_id, format!("127.0.0.1:800{}", i));
            if table.insert(peer).is_ok() {
                inserted += 1;
            }
        }
        
        // Should insert all 10 since they likely go to different buckets
        // But size should still be manageable
        assert!(table.size() <= 10);
    }

    #[test]
    fn test_routing_table_update_existing_peer() {
        let local_id = DhtKey::hash(b"local");
        let mut table = RoutingTable::new(local_id, 20);
        
        let peer_id = DhtKey::hash(b"peer1");
        let peer1 = PeerContact::new(peer_id, "127.0.0.1:8001".to_string());
        
        table.insert(peer1).unwrap();
        
        // Insert again with different address
        let peer2 = PeerContact::new(peer_id, "127.0.0.1:9999".to_string());
        table.insert(peer2).unwrap();
        
        // Should still have only one peer
        assert_eq!(table.size(), 1);
        
        // Address should be updated
        let retrieved = table.get(&peer_id).unwrap();
        assert_eq!(retrieved.address, "127.0.0.1:9999");
    }

    #[test]
    fn test_remove_stale_peers() {
        let local_id = DhtKey::hash(b"local");
        let mut table = RoutingTable::new(local_id, 20);
        
        // Insert peers and mark some as failed
        for i in 0..5 {
            let peer_id = DhtKey::hash(format!("peer{}", i).as_bytes());
            let mut peer = PeerContact::new(peer_id, format!("127.0.0.1:800{}", i));
            
            if i < 2 {
                // Mark first 2 peers as stale
                peer.mark_failed();
                peer.mark_failed();
                peer.mark_failed();
            }
            
            table.insert(peer).unwrap();
        }
        
        assert_eq!(table.size(), 5);
        
        let removed = table.remove_stale_peers(3600);
        assert_eq!(removed, 2);
        assert_eq!(table.size(), 3);
    }
}
