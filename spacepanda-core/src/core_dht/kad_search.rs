/*
    KadSearch - implements interactive Kademlia search operations.

    Responsibilities:
    `kad_search.rs` implements the Kademlia search procedures for finding nodes and values in the DHT.
    Its behaviors include: iterative FIND_NODE, iterative GET_VALUE, alpha parallel lookups, termination criteria, integrate results from peers,
    detect stalled peers, ranking peers by closeness.

    Inputs:
    - search request (key or node id)
    - routing table initial candidates
    - RPC responses from peers

    Outputs:
    - final value or final closest nodes
    - search logs
    - errors
*/

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::Duration;

use super::dht_key::DhtKey;
use super::dht_value::DhtValue;
use super::routing_table::PeerContact;

/// Search state for a single peer query
#[derive(Debug, Clone, PartialEq)]
enum PeerState {
    /// Not yet queried
    Pending,
    /// Query in progress
    Querying,
    /// Successfully responded
    Responded,
    /// Failed to respond
    Failed,
}

/// Result of a search operation
#[derive(Debug, Clone)]
pub enum SearchResult {
    /// Found the value
    Value(DhtValue),
    /// Found closest nodes but no value
    Nodes(Vec<PeerContact>),
    /// Search failed
    Failed(String),
}

/// Search type
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SearchType {
    /// Find a value
    FindValue,
    /// Find nodes
    FindNode,
}

/// Manages a single Kademlia search operation
pub struct KadSearch {
    /// Target key we're searching for
    target: DhtKey,
    /// Type of search
    search_type: SearchType,
    /// Alpha parameter (parallel queries)
    alpha: usize,
    /// K parameter (bucket size / result count)
    k: usize,
    /// Peers we know about, sorted by distance to target
    peers: Vec<(PeerContact, PeerState)>,
    /// Peers we've already seen (by ID)
    seen: HashSet<DhtKey>,
    /// Found value (if any)
    found_value: Option<DhtValue>,
    /// Whether search is complete
    complete: bool,
    /// RPC timeout
    rpc_timeout: Duration,
}

impl KadSearch {
    /// Create a new search
    pub fn new(
        target: DhtKey,
        search_type: SearchType,
        initial_peers: Vec<PeerContact>,
        alpha: usize,
        k: usize,
    ) -> Self {
        let mut search = KadSearch {
            target,
            search_type,
            alpha,
            k,
            peers: Vec::new(),
            seen: HashSet::new(),
            found_value: None,
            complete: false,
            rpc_timeout: Duration::from_secs(5),
        };

        // Add initial peers
        for peer in initial_peers {
            search.add_peer(peer);
        }

        search
    }

    /// Add a peer to the search
    fn add_peer(&mut self, peer: PeerContact) {
        // Don't add duplicates
        if self.seen.contains(&peer.id) {
            return;
        }

        self.seen.insert(peer.id);
        self.peers.push((peer.clone(), PeerState::Pending));

        // Sort by distance to target
        self.peers.sort_by_key(|(p, _)| p.id.distance(&self.target));

        // Keep only k closest
        if self.peers.len() > self.k {
            self.peers.truncate(self.k);
        }
    }

    /// Add multiple peers
    pub fn add_peers(&mut self, peers: Vec<PeerContact>) {
        for peer in peers {
            self.add_peer(peer);
        }
    }

    /// Get the next peers to query (up to alpha)
    pub fn get_next_queries(&mut self) -> Vec<PeerContact> {
        let mut queries = Vec::new();

        for (peer, state) in &mut self.peers {
            if *state == PeerState::Pending && queries.len() < self.alpha {
                *state = PeerState::Querying;
                queries.push(peer.clone());
            }
        }

        queries
    }

    /// Mark a peer as responded with new peers
    pub fn mark_responded(&mut self, peer_id: &DhtKey, new_peers: Vec<PeerContact>) {
        // Update state
        for (peer, state) in &mut self.peers {
            if &peer.id == peer_id {
                *state = PeerState::Responded;
                break;
            }
        }

        // Add new peers
        self.add_peers(new_peers);
    }

    /// Mark a peer as responded with a value
    pub fn mark_value_found(&mut self, peer_id: &DhtKey, value: DhtValue) {
        // Update state
        for (peer, state) in &mut self.peers {
            if &peer.id == peer_id {
                *state = PeerState::Responded;
                break;
            }
        }

        self.found_value = Some(value);
        self.complete = true;
    }

    /// Mark a peer as failed
    pub fn mark_failed(&mut self, peer_id: &DhtKey) {
        for (peer, state) in &mut self.peers {
            if &peer.id == peer_id {
                *state = PeerState::Failed;
                break;
            }
        }
    }

    /// Check if search is complete
    pub fn is_complete(&self) -> bool {
        if self.complete {
            return true;
        }

        // Check if we've queried all peers
        let all_done = self
            .peers
            .iter()
            .all(|(_, state)| *state == PeerState::Responded || *state == PeerState::Failed);

        all_done
    }

    /// Get the search result
    pub fn result(&self) -> SearchResult {
        if let Some(ref value) = self.found_value {
            SearchResult::Value(value.clone())
        } else if !self.peers.is_empty() {
            // Return k closest peers that responded
            let closest: Vec<PeerContact> = self
                .peers
                .iter()
                .filter(|(_, state)| *state == PeerState::Responded)
                .take(self.k)
                .map(|(peer, _)| peer.clone())
                .collect();

            if closest.is_empty() {
                SearchResult::Failed("No peers responded".to_string())
            } else {
                SearchResult::Nodes(closest)
            }
        } else {
            SearchResult::Failed("No peers available".to_string())
        }
    }

    /// Get count of pending/querying peers
    pub fn active_count(&self) -> usize {
        self.peers
            .iter()
            .filter(|(_, state)| *state == PeerState::Pending || *state == PeerState::Querying)
            .count()
    }

    /// Get the target key
    pub fn target(&self) -> DhtKey {
        self.target
    }

    /// Get search type
    pub fn search_type(&self) -> SearchType {
        self.search_type
    }
}

/// Search manager coordinating multiple searches
pub struct SearchManager {
    /// Active searches by search ID
    searches: Arc<Mutex<HashMap<u64, KadSearch>>>,
    /// Next search ID
    next_id: Arc<Mutex<u64>>,
}

impl SearchManager {
    pub fn new() -> Self {
        SearchManager {
            searches: Arc::new(Mutex::new(HashMap::new())),
            next_id: Arc::new(Mutex::new(0)),
        }
    }

    /// Start a new search
    pub async fn start_search(
        &self,
        target: DhtKey,
        search_type: SearchType,
        initial_peers: Vec<PeerContact>,
        alpha: usize,
        k: usize,
    ) -> u64 {
        let mut next_id = self.next_id.lock().await;
        let search_id = *next_id;
        *next_id += 1;

        let search = KadSearch::new(target, search_type, initial_peers, alpha, k);
        self.searches.lock().await.insert(search_id, search);

        search_id
    }

    /// Get next queries for a search
    pub async fn get_next_queries(&self, search_id: u64) -> Option<Vec<PeerContact>> {
        let mut searches = self.searches.lock().await;
        searches.get_mut(&search_id).map(|s| s.get_next_queries())
    }

    /// Add peers to a search
    pub async fn add_peers(&self, search_id: u64, peers: Vec<PeerContact>) {
        let mut searches = self.searches.lock().await;
        if let Some(search) = searches.get_mut(&search_id) {
            search.add_peers(peers);
        }
    }

    /// Mark peer as responded
    pub async fn mark_responded(
        &self,
        search_id: u64,
        peer_id: &DhtKey,
        new_peers: Vec<PeerContact>,
    ) {
        let mut searches = self.searches.lock().await;
        if let Some(search) = searches.get_mut(&search_id) {
            search.mark_responded(peer_id, new_peers);
        }
    }

    /// Mark value found
    pub async fn mark_value_found(&self, search_id: u64, peer_id: &DhtKey, value: DhtValue) {
        let mut searches = self.searches.lock().await;
        if let Some(search) = searches.get_mut(&search_id) {
            search.mark_value_found(peer_id, value);
        }
    }

    /// Mark peer as failed
    pub async fn mark_failed(&self, search_id: u64, peer_id: &DhtKey) {
        let mut searches = self.searches.lock().await;
        if let Some(search) = searches.get_mut(&search_id) {
            search.mark_failed(peer_id);
        }
    }

    /// Check if search is complete
    pub async fn is_complete(&self, search_id: u64) -> bool {
        let searches = self.searches.lock().await;
        searches.get(&search_id).map(|s| s.is_complete()).unwrap_or(true)
    }

    /// Get search result
    pub async fn get_result(&self, search_id: u64) -> Option<SearchResult> {
        let searches = self.searches.lock().await;
        searches.get(&search_id).map(|s| s.result())
    }

    /// Remove a completed search
    pub async fn remove_search(&self, search_id: u64) {
        self.searches.lock().await.remove(&search_id);
    }
}

impl Default for SearchManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_creation() {
        let target = DhtKey::hash(b"target");
        let peer1 = PeerContact::new(DhtKey::hash(b"peer1"), "127.0.0.1:8001".to_string());
        let peer2 = PeerContact::new(DhtKey::hash(b"peer2"), "127.0.0.1:8002".to_string());

        let search = KadSearch::new(target, SearchType::FindValue, vec![peer1, peer2], 3, 20);

        assert_eq!(search.target(), target);
        assert_eq!(search.search_type(), SearchType::FindValue);
        assert_eq!(search.active_count(), 2);
    }

    #[test]
    fn test_search_get_next_queries() {
        let target = DhtKey::hash(b"target");
        let peer1 = PeerContact::new(DhtKey::hash(b"peer1"), "127.0.0.1:8001".to_string());
        let peer2 = PeerContact::new(DhtKey::hash(b"peer2"), "127.0.0.1:8002".to_string());
        let peer3 = PeerContact::new(DhtKey::hash(b"peer3"), "127.0.0.1:8003".to_string());

        let mut search = KadSearch::new(
            target,
            SearchType::FindNode,
            vec![peer1, peer2, peer3],
            2, // alpha=2
            20,
        );

        let queries = search.get_next_queries();
        assert_eq!(queries.len(), 2); // Should return alpha peers

        let queries2 = search.get_next_queries();
        assert_eq!(queries2.len(), 1); // Only 1 remaining peer
    }

    #[test]
    fn test_search_mark_responded() {
        let target = DhtKey::hash(b"target");
        let peer1 = PeerContact::new(DhtKey::hash(b"peer1"), "127.0.0.1:8001".to_string());
        let peer2 = PeerContact::new(DhtKey::hash(b"peer2"), "127.0.0.1:8002".to_string());
        let peer3 = PeerContact::new(DhtKey::hash(b"peer3"), "127.0.0.1:8003".to_string());

        let mut search = KadSearch::new(target, SearchType::FindNode, vec![peer1.clone()], 3, 20);

        // Mark as responded with new peers
        search.mark_responded(&peer1.id, vec![peer2.clone(), peer3.clone()]);

        // Should have added new peers
        assert_eq!(search.peers.len(), 3);
    }

    #[test]
    fn test_search_mark_value_found() {
        let target = DhtKey::hash(b"target");
        let peer1 = PeerContact::new(DhtKey::hash(b"peer1"), "127.0.0.1:8001".to_string());

        let mut search = KadSearch::new(target, SearchType::FindValue, vec![peer1.clone()], 3, 20);

        let value = DhtValue::new(b"test_data".to_vec()).with_ttl(3600);
        search.mark_value_found(&peer1.id, value.clone());

        assert!(search.is_complete());

        match search.result() {
            SearchResult::Value(v) => assert_eq!(v.data, value.data),
            _ => panic!("Expected value result"),
        }
    }

    #[test]
    fn test_search_mark_failed() {
        let target = DhtKey::hash(b"target");
        let peer1 = PeerContact::new(DhtKey::hash(b"peer1"), "127.0.0.1:8001".to_string());

        let mut search = KadSearch::new(target, SearchType::FindNode, vec![peer1.clone()], 3, 20);

        search.get_next_queries(); // Start query
        search.mark_failed(&peer1.id);

        assert!(search.is_complete());
    }

    #[test]
    fn test_search_completion() {
        let target = DhtKey::hash(b"target");
        let peer1 = PeerContact::new(DhtKey::hash(b"peer1"), "127.0.0.1:8001".to_string());
        let peer2 = PeerContact::new(DhtKey::hash(b"peer2"), "127.0.0.1:8002".to_string());

        let mut search =
            KadSearch::new(target, SearchType::FindNode, vec![peer1.clone(), peer2.clone()], 3, 20);

        assert!(!search.is_complete());

        // Query both
        search.get_next_queries();

        // Mark both as responded
        search.mark_responded(&peer1.id, vec![]);
        search.mark_responded(&peer2.id, vec![]);

        assert!(search.is_complete());
    }

    #[test]
    fn test_search_result_nodes() {
        let target = DhtKey::hash(b"target");
        let peer1 = PeerContact::new(DhtKey::hash(b"peer1"), "127.0.0.1:8001".to_string());

        let mut search = KadSearch::new(target, SearchType::FindNode, vec![peer1.clone()], 3, 20);

        search.get_next_queries();
        search.mark_responded(&peer1.id, vec![]);

        match search.result() {
            SearchResult::Nodes(nodes) => {
                assert_eq!(nodes.len(), 1);
                assert_eq!(nodes[0].id, peer1.id);
            }
            _ => panic!("Expected nodes result"),
        }
    }

    #[test]
    fn test_search_no_duplicate_peers() {
        let target = DhtKey::hash(b"target");
        let peer1 = PeerContact::new(DhtKey::hash(b"peer1"), "127.0.0.1:8001".to_string());

        let mut search = KadSearch::new(target, SearchType::FindNode, vec![peer1.clone()], 3, 20);

        // Try to add same peer again
        search.add_peer(peer1.clone());

        assert_eq!(search.peers.len(), 1);
    }

    #[test]
    fn test_search_closest_k_peers() {
        let target = DhtKey::hash(b"target");
        let mut peers = vec![];

        // Create many peers
        for i in 0..50 {
            let peer = PeerContact::new(
                DhtKey::hash(format!("peer{}", i).as_bytes()),
                format!("127.0.0.1:80{:02}", i),
            );
            peers.push(peer);
        }

        let search = KadSearch::new(target, SearchType::FindNode, peers, 3, 20); // k=20

        // Should keep only 20 closest peers
        assert_eq!(search.peers.len(), 20);

        // Verify they are sorted by distance
        for i in 0..search.peers.len() - 1 {
            let dist1 = search.peers[i].0.id.distance(&target);
            let dist2 = search.peers[i + 1].0.id.distance(&target);
            assert!(dist1 <= dist2);
        }
    }

    #[tokio::test]
    async fn test_search_manager_creation() {
        let manager = SearchManager::new();
        let target = DhtKey::hash(b"target");
        let peer1 = PeerContact::new(DhtKey::hash(b"peer1"), "127.0.0.1:8001".to_string());

        let search_id =
            manager.start_search(target, SearchType::FindValue, vec![peer1], 3, 20).await;

        assert_eq!(search_id, 0);
    }

    #[tokio::test]
    async fn test_search_manager_multiple_searches() {
        let manager = SearchManager::new();
        let target1 = DhtKey::hash(b"target1");
        let target2 = DhtKey::hash(b"target2");

        let peer1 = PeerContact::new(DhtKey::hash(b"peer1"), "127.0.0.1:8001".to_string());

        let id1 = manager
            .start_search(target1, SearchType::FindValue, vec![peer1.clone()], 3, 20)
            .await;

        let id2 = manager.start_search(target2, SearchType::FindNode, vec![peer1], 3, 20).await;

        assert_ne!(id1, id2);
    }

    #[tokio::test]
    async fn test_search_manager_lifecycle() {
        let manager = SearchManager::new();
        let target = DhtKey::hash(b"target");
        let peer1 = PeerContact::new(DhtKey::hash(b"peer1"), "127.0.0.1:8001".to_string());

        let search_id = manager
            .start_search(target, SearchType::FindValue, vec![peer1.clone()], 3, 20)
            .await;

        // Get next queries
        let queries = manager.get_next_queries(search_id).await.unwrap();
        assert_eq!(queries.len(), 1);

        // Mark as responded
        manager.mark_responded(search_id, &peer1.id, vec![]).await;

        // Should be complete
        assert!(manager.is_complete(search_id).await);

        // Get result
        let result = manager.get_result(search_id).await.unwrap();
        match result {
            SearchResult::Nodes(_) => {} // Expected
            _ => panic!("Expected nodes result"),
        }

        // Remove search
        manager.remove_search(search_id).await;
    }

    #[tokio::test]
    async fn test_search_manager_value_found() {
        let manager = SearchManager::new();
        let target = DhtKey::hash(b"target");
        let peer1 = PeerContact::new(DhtKey::hash(b"peer1"), "127.0.0.1:8001".to_string());

        let search_id = manager
            .start_search(target, SearchType::FindValue, vec![peer1.clone()], 3, 20)
            .await;

        let value = DhtValue::new(b"found!".to_vec()).with_ttl(3600);
        manager.mark_value_found(search_id, &peer1.id, value.clone()).await;

        assert!(manager.is_complete(search_id).await);

        let result = manager.get_result(search_id).await.unwrap();
        match result {
            SearchResult::Value(v) => assert_eq!(v.data, b"found!"),
            _ => panic!("Expected value result"),
        }
    }
}
