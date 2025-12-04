//! Metrics collection and export for observability

use metrics::{counter, gauge, histogram, describe_counter, describe_gauge, describe_histogram};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

mod collector;
mod exporter;

pub use collector::MetricsCollector;
pub use exporter::{PrometheusExporter, MetricsExporter};

/// Initialize metrics with descriptions
pub fn init_metrics() {
    // CRDT metrics
    describe_counter!("crdt.or_set.add", "Number of ORSet add operations");
    describe_counter!("crdt.or_set.remove", "Number of ORSet remove operations");
    describe_counter!("crdt.or_set.merge", "Number of ORSet merge operations");
    describe_histogram!("crdt.or_set.merge.duration_ms", "ORSet merge operation duration in milliseconds");
    describe_gauge!("crdt.or_set.size", "Current number of elements in ORSet");
    
    // DHT metrics
    describe_counter!("dht.requests.total", "Total DHT requests");
    describe_counter!("dht.requests.success", "Successful DHT requests");
    describe_counter!("dht.requests.failed", "Failed DHT requests");
    describe_histogram!("dht.request.duration_ms", "DHT request duration in milliseconds");
    describe_gauge!("dht.peers.active", "Number of active DHT peers");
    describe_gauge!("dht.peers.total", "Total number of known DHT peers");
    describe_gauge!("dht.bucket.entries", "Number of entries in DHT buckets");
    
    // Store metrics
    describe_counter!("store.operations.total", "Total store operations");
    describe_counter!("store.operations.read", "Store read operations");
    describe_counter!("store.operations.write", "Store write operations");
    describe_counter!("store.operations.delete", "Store delete operations");
    describe_histogram!("store.operation.duration_ms", "Store operation duration in milliseconds");
    describe_gauge!("store.size.bytes", "Store size in bytes");
    describe_gauge!("store.tombstones.count", "Number of tombstones in store");
    
    // Network metrics
    describe_counter!("network.messages.sent", "Number of network messages sent");
    describe_counter!("network.messages.received", "Number of network messages received");
    describe_counter!("network.bytes.sent", "Number of bytes sent over network");
    describe_counter!("network.bytes.received", "Number of bytes received over network");
    describe_histogram!("network.latency_ms", "Network latency in milliseconds");
    
    // MLS metrics
    describe_counter!("mls.proposals.created", "Number of MLS proposals created");
    describe_counter!("mls.commits.created", "Number of MLS commits created");
    describe_counter!("mls.messages.encrypted", "Number of MLS messages encrypted");
    describe_counter!("mls.messages.decrypted", "Number of MLS messages decrypted");
    describe_histogram!("mls.encryption.duration_ms", "MLS encryption duration in milliseconds");
    describe_histogram!("mls.decryption.duration_ms", "MLS decryption duration in milliseconds");
    
    // System metrics
    describe_gauge!("system.memory.used_bytes", "System memory used in bytes");
    describe_gauge!("system.cpu.usage_percent", "CPU usage percentage");
    describe_gauge!("system.threads.count", "Number of active threads");
    describe_gauge!("system.uptime_seconds", "System uptime in seconds");
}

/// Record a counter metric
pub fn record_counter(name: &'static str, value: u64) {
    counter!(name).increment(value);
}

/// Record a gauge metric
pub fn record_gauge(name: &'static str, value: f64) {
    gauge!(name).set(value);
}

/// Record a histogram metric
pub fn record_histogram(name: &'static str, value: f64) {
    histogram!(name).record(value);
}

/// Timer for measuring operation duration
pub struct Timer {
    name: String,
    start: Instant,
}

impl Timer {
    /// Create a new timer
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            start: Instant::now(),
        }
    }
    
    /// Stop the timer and record the duration
    pub fn stop(self) {
        let duration = self.start.elapsed();
        histogram!(self.name).record(duration.as_secs_f64() * 1000.0);
    }
}

/// Metrics snapshot for reporting
#[derive(Debug, Clone)]
pub struct MetricsSnapshot {
    pub timestamp: std::time::SystemTime,
    pub crdt_operations: u64,
    pub dht_requests: u64,
    pub store_operations: u64,
    pub network_messages_sent: u64,
    pub network_messages_received: u64,
    pub active_peers: usize,
    pub store_size_bytes: u64,
}

/// Metrics service for background collection
pub struct MetricsService {
    collector: Arc<RwLock<MetricsCollector>>,
    collection_interval: Duration,
}

impl MetricsService {
    /// Create a new metrics service
    pub fn new(collection_interval: Duration) -> Self {
        Self {
            collector: Arc::new(RwLock::new(MetricsCollector::new())),
            collection_interval,
        }
    }
    
    /// Start the metrics collection service
    pub async fn run(self: Arc<Self>) {
        let mut interval = tokio::time::interval(self.collection_interval);
        
        loop {
            interval.tick().await;
            
            // Collect system metrics
            self.collect_system_metrics().await;
        }
    }
    
    /// Collect system metrics
    async fn collect_system_metrics(&self) {
        // Record system metrics
        // In production, use sysinfo or similar crate
        gauge!("system.threads.count").set(num_cpus::get() as f64);
    }
    
    /// Get current metrics snapshot
    pub async fn snapshot(&self) -> MetricsSnapshot {
        let collector = self.collector.read().await;
        collector.snapshot()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_metrics_init() {
        init_metrics();
        // Metrics are initialized globally, just ensure it doesn't panic
    }
    
    #[test]
    fn test_timer() {
        let timer = Timer::new("test.operation");
        std::thread::sleep(std::time::Duration::from_millis(10));
        timer.stop();
    }
    
    #[tokio::test]
    async fn test_metrics_service() {
        let service = Arc::new(MetricsService::new(Duration::from_millis(100)));
        let snapshot = service.snapshot().await;
        assert!(snapshot.timestamp <= std::time::SystemTime::now());
    }
}
