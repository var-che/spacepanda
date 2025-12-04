//! Health check system for production readiness

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use std::time::{SystemTime, Duration};

/// Health status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

impl HealthStatus {
    /// Convert to HTTP status code
    pub fn to_http_status(&self) -> u16 {
        match self {
            HealthStatus::Healthy => 200,
            HealthStatus::Degraded => 200,
            HealthStatus::Unhealthy => 503,
        }
    }
}

/// Health check result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheck {
    pub status: HealthStatus,
    pub timestamp: SystemTime,
    pub components: Vec<ComponentHealth>,
    pub version: String,
    pub uptime_seconds: u64,
}

/// Component health status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentHealth {
    pub name: String,
    pub status: HealthStatus,
    pub message: Option<String>,
    pub last_check: SystemTime,
}

impl ComponentHealth {
    /// Create a healthy component
    pub fn healthy(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: HealthStatus::Healthy,
            message: None,
            last_check: SystemTime::now(),
        }
    }
    
    /// Create a degraded component
    pub fn degraded(name: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: HealthStatus::Degraded,
            message: Some(message.into()),
            last_check: SystemTime::now(),
        }
    }
    
    /// Create an unhealthy component
    pub fn unhealthy(name: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: HealthStatus::Unhealthy,
            message: Some(message.into()),
            last_check: SystemTime::now(),
        }
    }
}

/// Health checker service
pub struct HealthChecker {
    start_time: SystemTime,
    version: String,
    components: Arc<RwLock<Vec<ComponentHealth>>>,
}

impl HealthChecker {
    /// Create a new health checker
    pub fn new(version: impl Into<String>) -> Self {
        Self {
            start_time: SystemTime::now(),
            version: version.into(),
            components: Arc::new(RwLock::new(Vec::new())),
        }
    }
    
    /// Register a component for health checking
    pub async fn register_component(&self, name: impl Into<String>) {
        let mut components = self.components.write().await;
        components.push(ComponentHealth::healthy(name));
    }
    
    /// Update component health status
    pub async fn update_component(&self, name: &str, status: HealthStatus, message: Option<String>) {
        let mut components = self.components.write().await;
        
        if let Some(component) = components.iter_mut().find(|c| c.name == name) {
            component.status = status;
            component.message = message;
            component.last_check = SystemTime::now();
        }
    }
    
    /// Get current health status
    pub async fn check_health(&self) -> HealthCheck {
        let components = self.components.read().await.clone();
        
        // Determine overall status
        let status = if components.iter().any(|c| c.status == HealthStatus::Unhealthy) {
            HealthStatus::Unhealthy
        } else if components.iter().any(|c| c.status == HealthStatus::Degraded) {
            HealthStatus::Degraded
        } else {
            HealthStatus::Healthy
        };
        
        let uptime = self.start_time.elapsed()
            .unwrap_or(Duration::from_secs(0))
            .as_secs();
        
        HealthCheck {
            status,
            timestamp: SystemTime::now(),
            components,
            version: self.version.clone(),
            uptime_seconds: uptime,
        }
    }
    
    /// Perform readiness check (can accept traffic)
    pub async fn readiness_check(&self) -> bool {
        let health = self.check_health().await;
        health.status != HealthStatus::Unhealthy
    }
    
    /// Perform liveness check (process is running)
    pub async fn liveness_check(&self) -> bool {
        true // If this code runs, process is alive
    }
}

/// Built-in health checks
pub mod checks {
    use super::*;
    
    /// Check DHT health
    pub async fn check_dht(peer_count: usize, min_peers: usize) -> ComponentHealth {
        if peer_count == 0 {
            ComponentHealth::unhealthy("dht", "No connected peers")
        } else if peer_count < min_peers {
            ComponentHealth::degraded("dht", format!("Only {} peers connected (minimum: {})", peer_count, min_peers))
        } else {
            ComponentHealth::healthy("dht")
        }
    }
    
    /// Check store health
    pub async fn check_store(is_writable: bool, disk_usage_percent: f64) -> ComponentHealth {
        if !is_writable {
            ComponentHealth::unhealthy("store", "Store is not writable")
        } else if disk_usage_percent > 90.0 {
            ComponentHealth::degraded("store", format!("Disk usage at {:.1}%", disk_usage_percent))
        } else {
            ComponentHealth::healthy("store")
        }
    }
    
    /// Check memory health
    pub async fn check_memory(used_percent: f64) -> ComponentHealth {
        if used_percent > 95.0 {
            ComponentHealth::unhealthy("memory", format!("Memory usage at {:.1}%", used_percent))
        } else if used_percent > 85.0 {
            ComponentHealth::degraded("memory", format!("Memory usage at {:.1}%", used_percent))
        } else {
            ComponentHealth::healthy("memory")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_health_checker() {
        let checker = HealthChecker::new("1.0.0");
        
        checker.register_component("test_component").await;
        
        let health = checker.check_health().await;
        assert_eq!(health.status, HealthStatus::Healthy);
        assert_eq!(health.components.len(), 1);
    }
    
    #[tokio::test]
    async fn test_component_health() {
        let checker = HealthChecker::new("1.0.0");
        
        checker.register_component("component1").await;
        checker.update_component("component1", HealthStatus::Degraded, Some("Test degraded".to_string())).await;
        
        let health = checker.check_health().await;
        assert_eq!(health.status, HealthStatus::Degraded);
    }
    
    #[tokio::test]
    async fn test_readiness_check() {
        let checker = HealthChecker::new("1.0.0");
        assert!(checker.readiness_check().await);
        
        checker.register_component("test").await;
        checker.update_component("test", HealthStatus::Unhealthy, Some("Error".to_string())).await;
        
        assert!(!checker.readiness_check().await);
    }
}
