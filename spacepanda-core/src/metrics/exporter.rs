//! Metrics export functionality

use std::sync::Arc;
use tokio::sync::RwLock;

/// Trait for metrics exporters
pub trait MetricsExporter: Send + Sync {
    /// Export metrics in the exporter's format
    fn export(&self) -> String;
}

/// Prometheus metrics exporter
pub struct PrometheusExporter {
    registry: Arc<RwLock<Vec<(String, f64)>>>,
}

impl PrometheusExporter {
    /// Create a new Prometheus exporter
    pub fn new() -> Self {
        Self { registry: Arc::new(RwLock::new(Vec::new())) }
    }

    /// Record a metric value
    pub async fn record(&self, name: String, value: f64) {
        let mut registry = self.registry.write().await;
        registry.push((name, value));
    }

    /// Export metrics in Prometheus text format
    pub async fn export_prometheus(&self) -> String {
        let registry = self.registry.read().await;
        let mut output = String::new();

        for (name, value) in registry.iter() {
            output.push_str(&format!("{} {}\n", name, value));
        }

        output
    }
}

impl Default for PrometheusExporter {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricsExporter for PrometheusExporter {
    fn export(&self) -> String {
        // This is a blocking call, for async use export_prometheus()
        // In production, use prometheus crate with proper registry
        String::from("# Metrics export requires async context\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_prometheus_exporter() {
        let exporter = PrometheusExporter::new();

        exporter.record("test_metric".to_string(), 42.0).await;
        exporter.record("another_metric".to_string(), 100.0).await;

        let output = exporter.export_prometheus().await;
        assert!(output.contains("test_metric 42"));
        assert!(output.contains("another_metric 100"));
    }
}
