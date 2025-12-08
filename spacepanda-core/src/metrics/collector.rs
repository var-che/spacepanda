//! Metrics collector implementation

use super::MetricsSnapshot;
use std::sync::atomic::{AtomicU64, Ordering};

/// Metrics collector for aggregating metrics data
#[derive(Debug)]
pub struct MetricsCollector {
    crdt_operations: AtomicU64,
    dht_requests: AtomicU64,
    store_operations: AtomicU64,
    network_messages_sent: AtomicU64,
    network_messages_received: AtomicU64,
}

impl MetricsCollector {
    /// Create a new metrics collector
    pub fn new() -> Self {
        Self {
            crdt_operations: AtomicU64::new(0),
            dht_requests: AtomicU64::new(0),
            store_operations: AtomicU64::new(0),
            network_messages_sent: AtomicU64::new(0),
            network_messages_received: AtomicU64::new(0),
        }
    }

    /// Increment CRDT operations counter
    pub fn inc_crdt_operations(&self) {
        self.crdt_operations.fetch_add(1, Ordering::Relaxed);
    }

    /// Increment DHT requests counter
    pub fn inc_dht_requests(&self) {
        self.dht_requests.fetch_add(1, Ordering::Relaxed);
    }

    /// Increment store operations counter
    pub fn inc_store_operations(&self) {
        self.store_operations.fetch_add(1, Ordering::Relaxed);
    }

    /// Increment network messages sent counter
    pub fn inc_network_sent(&self) {
        self.network_messages_sent.fetch_add(1, Ordering::Relaxed);
    }

    /// Increment network messages received counter
    pub fn inc_network_received(&self) {
        self.network_messages_received.fetch_add(1, Ordering::Relaxed);
    }

    /// Get a snapshot of current metrics
    pub fn snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            timestamp: std::time::SystemTime::now(),
            crdt_operations: self.crdt_operations.load(Ordering::Relaxed),
            dht_requests: self.dht_requests.load(Ordering::Relaxed),
            store_operations: self.store_operations.load(Ordering::Relaxed),
            network_messages_sent: self.network_messages_sent.load(Ordering::Relaxed),
            network_messages_received: self.network_messages_received.load(Ordering::Relaxed),
            active_peers: 0,     // Will be populated from DHT
            store_size_bytes: 0, // Will be populated from store
        }
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}
