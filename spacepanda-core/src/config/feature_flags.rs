//! Feature flag management for runtime configuration

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Feature flags for enabling/disabling functionality at runtime
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureFlags {
    /// Enable experimental features
    pub experimental: bool,

    /// Enable DHT replication
    pub dht_replication: bool,

    /// Enable store snapshots
    pub store_snapshots: bool,

    /// Enable compression
    pub compression: bool,

    /// Enable rate limiting
    pub rate_limiting: bool,

    /// Enable circuit breaker
    pub circuit_breaker: bool,

    /// Custom feature flags (key-value pairs)
    pub custom: HashMap<String, bool>,
}

impl Default for FeatureFlags {
    fn default() -> Self {
        Self {
            experimental: false,
            dht_replication: true,
            store_snapshots: true,
            compression: true,
            rate_limiting: true,
            circuit_breaker: true,
            custom: HashMap::new(),
        }
    }
}

/// Thread-safe feature flag manager
#[derive(Debug, Clone)]
pub struct FeatureManager {
    flags: Arc<RwLock<FeatureFlags>>,
}

impl FeatureManager {
    /// Create a new feature manager with default flags
    pub fn new() -> Self {
        Self { flags: Arc::new(RwLock::new(FeatureFlags::default())) }
    }

    /// Create a new feature manager with custom flags
    pub fn with_flags(flags: FeatureFlags) -> Self {
        Self { flags: Arc::new(RwLock::new(flags)) }
    }

    /// Check if experimental features are enabled
    pub fn is_experimental_enabled(&self) -> bool {
        self.flags.read().unwrap().experimental
    }

    /// Check if DHT replication is enabled
    pub fn is_dht_replication_enabled(&self) -> bool {
        self.flags.read().unwrap().dht_replication
    }

    /// Check if store snapshots are enabled
    pub fn is_store_snapshots_enabled(&self) -> bool {
        self.flags.read().unwrap().store_snapshots
    }

    /// Check if compression is enabled
    pub fn is_compression_enabled(&self) -> bool {
        self.flags.read().unwrap().compression
    }

    /// Check if rate limiting is enabled
    pub fn is_rate_limiting_enabled(&self) -> bool {
        self.flags.read().unwrap().rate_limiting
    }

    /// Check if circuit breaker is enabled
    pub fn is_circuit_breaker_enabled(&self) -> bool {
        self.flags.read().unwrap().circuit_breaker
    }

    /// Check a custom feature flag
    pub fn is_enabled(&self, feature: &str) -> bool {
        self.flags.read().unwrap().custom.get(feature).copied().unwrap_or(false)
    }

    /// Enable a feature flag
    pub fn enable(&self, feature: &str) {
        let mut flags = self.flags.write().unwrap();
        match feature {
            "experimental" => flags.experimental = true,
            "dht_replication" => flags.dht_replication = true,
            "store_snapshots" => flags.store_snapshots = true,
            "compression" => flags.compression = true,
            "rate_limiting" => flags.rate_limiting = true,
            "circuit_breaker" => flags.circuit_breaker = true,
            _ => {
                flags.custom.insert(feature.to_string(), true);
            }
        }
    }

    /// Disable a feature flag
    pub fn disable(&self, feature: &str) {
        let mut flags = self.flags.write().unwrap();
        match feature {
            "experimental" => flags.experimental = false,
            "dht_replication" => flags.dht_replication = false,
            "store_snapshots" => flags.store_snapshots = false,
            "compression" => flags.compression = false,
            "rate_limiting" => flags.rate_limiting = false,
            "circuit_breaker" => flags.circuit_breaker = false,
            _ => {
                flags.custom.insert(feature.to_string(), false);
            }
        }
    }

    /// Get all current flags
    pub fn get_flags(&self) -> FeatureFlags {
        self.flags.read().unwrap().clone()
    }

    /// Update all flags
    pub fn update_flags(&self, new_flags: FeatureFlags) {
        *self.flags.write().unwrap() = new_flags;
    }
}

impl Default for FeatureManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_flags() {
        let flags = FeatureFlags::default();
        assert!(!flags.experimental);
        assert!(flags.dht_replication);
        assert!(flags.store_snapshots);
    }

    #[test]
    fn test_feature_manager() {
        let manager = FeatureManager::new();

        assert!(!manager.is_experimental_enabled());

        manager.enable("experimental");
        assert!(manager.is_experimental_enabled());

        manager.disable("experimental");
        assert!(!manager.is_experimental_enabled());
    }

    #[test]
    fn test_custom_flags() {
        let manager = FeatureManager::new();

        assert!(!manager.is_enabled("custom_feature"));

        manager.enable("custom_feature");
        assert!(manager.is_enabled("custom_feature"));

        manager.disable("custom_feature");
        assert!(!manager.is_enabled("custom_feature"));
    }
}
