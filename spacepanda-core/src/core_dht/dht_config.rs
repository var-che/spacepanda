/*
    DhtConfig - config params for DHT behavior

    Responsibilities:
    `dht_config.rs` defines the configuration structure for the DHT subsystem.
    It defines: keyspace size, bucket size(k), parallelism(alpha), republish interval, expiration times, replication strategy (push/pull), etc.
    Makes the DHT behavior customizable.

    Inputs:
    - load configuration from file or defaults
    - update configuration at runtime (if needed)

    Outputs:
    - static values to the rest of the DHT subsystem
*/

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Replication strategy for DHT values
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReplicationStrategy {
    /// Push values to k closest nodes
    Push,
    /// Pull values when queried
    Pull,
    /// Hybrid: push initially, pull on demand
    Hybrid,
}

/// Configuration for DHT behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DhtConfig {
    /// Bucket size (k parameter) - max nodes per bucket
    pub bucket_size: usize,

    /// Parallelism factor (alpha) - concurrent lookups
    pub alpha: usize,

    /// Replication factor - number of nodes storing each value
    pub replication_factor: usize,

    /// Republish interval - how often to republish values
    pub republish_interval: Duration,

    /// Value expiration time - how long values live without republish
    pub value_expiration: Duration,

    /// Bucket refresh interval - how often to refresh buckets
    pub bucket_refresh_interval: Duration,

    /// Replication strategy
    pub replication_strategy: ReplicationStrategy,

    /// Timeout for RPC requests
    pub rpc_timeout: Duration,

    /// Maximum number of hops for iterative lookups
    pub max_lookup_hops: usize,

    /// Enable signature verification for values
    pub require_signatures: bool,

    /// Maximum value size in bytes
    pub max_value_size: usize,

    /// Number of buckets (typically 256 for 256-bit keyspace)
    pub num_buckets: usize,
}

impl Default for DhtConfig {
    fn default() -> Self {
        DhtConfig {
            bucket_size: 20,                                    // Standard k=20
            alpha: 3,                                           // Standard alpha=3
            replication_factor: 20,                             // Store on k nodes
            republish_interval: Duration::from_secs(3600),      // 1 hour
            value_expiration: Duration::from_secs(86400),       // 24 hours
            bucket_refresh_interval: Duration::from_secs(3600), // 1 hour
            replication_strategy: ReplicationStrategy::Hybrid,
            rpc_timeout: Duration::from_secs(5),
            max_lookup_hops: 8,
            require_signatures: false,
            max_value_size: 1024 * 1024, // 1 MB
            num_buckets: 256,            // 256-bit keyspace
        }
    }
}

impl DhtConfig {
    /// Create a new DhtConfig with custom values
    pub fn new() -> Self {
        Self::default()
    }

    /// Builder: set bucket size
    pub fn with_bucket_size(mut self, size: usize) -> Self {
        self.bucket_size = size;
        self
    }

    /// Builder: set alpha (parallelism)
    pub fn with_alpha(mut self, alpha: usize) -> Self {
        self.alpha = alpha;
        self
    }

    /// Builder: set replication factor
    pub fn with_replication_factor(mut self, factor: usize) -> Self {
        self.replication_factor = factor;
        self
    }

    /// Builder: set republish interval
    pub fn with_republish_interval(mut self, interval: Duration) -> Self {
        self.republish_interval = interval;
        self
    }

    /// Builder: set value expiration
    pub fn with_value_expiration(mut self, expiration: Duration) -> Self {
        self.value_expiration = expiration;
        self
    }

    /// Builder: set replication strategy
    pub fn with_replication_strategy(mut self, strategy: ReplicationStrategy) -> Self {
        self.replication_strategy = strategy;
        self
    }

    /// Builder: set RPC timeout
    pub fn with_rpc_timeout(mut self, timeout: Duration) -> Self {
        self.rpc_timeout = timeout;
        self
    }

    /// Builder: enable signature verification
    pub fn with_signatures(mut self, enabled: bool) -> Self {
        self.require_signatures = enabled;
        self
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.bucket_size == 0 {
            return Err("Bucket size must be greater than 0".to_string());
        }

        if self.alpha == 0 {
            return Err("Alpha must be greater than 0".to_string());
        }

        if self.alpha > self.bucket_size {
            return Err("Alpha should not exceed bucket size".to_string());
        }

        if self.replication_factor == 0 {
            return Err("Replication factor must be greater than 0".to_string());
        }

        if self.max_value_size == 0 {
            return Err("Max value size must be greater than 0".to_string());
        }

        if self.num_buckets != 256 {
            return Err("Number of buckets must be 256 for 256-bit keyspace".to_string());
        }

        Ok(())
    }

    /// Create a test configuration (faster timeouts, smaller sizes)
    #[cfg(test)]
    pub fn test_config() -> Self {
        DhtConfig {
            bucket_size: 5,
            alpha: 2,
            replication_factor: 3,
            republish_interval: Duration::from_secs(10),
            value_expiration: Duration::from_secs(30),
            bucket_refresh_interval: Duration::from_secs(10),
            replication_strategy: ReplicationStrategy::Push,
            rpc_timeout: Duration::from_millis(100),
            max_lookup_hops: 4,
            require_signatures: false,
            max_value_size: 1024,
            num_buckets: 256,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dht_config_default() {
        let config = DhtConfig::default();

        assert_eq!(config.bucket_size, 20);
        assert_eq!(config.alpha, 3);
        assert_eq!(config.replication_factor, 20);
        assert_eq!(config.num_buckets, 256);
    }

    #[test]
    fn test_dht_config_builder() {
        let config =
            DhtConfig::new().with_bucket_size(10).with_alpha(5).with_replication_factor(15);

        assert_eq!(config.bucket_size, 10);
        assert_eq!(config.alpha, 5);
        assert_eq!(config.replication_factor, 15);
    }

    #[test]
    fn test_dht_config_validate_valid() {
        let config = DhtConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_dht_config_validate_zero_bucket_size() {
        let mut config = DhtConfig::default();
        config.bucket_size = 0;

        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Bucket size"));
    }

    #[test]
    fn test_dht_config_validate_zero_alpha() {
        let mut config = DhtConfig::default();
        config.alpha = 0;

        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Alpha"));
    }

    #[test]
    fn test_dht_config_validate_alpha_exceeds_bucket_size() {
        let mut config = DhtConfig::default();
        config.bucket_size = 5;
        config.alpha = 10;

        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Alpha should not exceed"));
    }

    #[test]
    fn test_dht_config_validate_zero_replication() {
        let mut config = DhtConfig::default();
        config.replication_factor = 0;

        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Replication factor"));
    }

    #[test]
    fn test_dht_config_validate_invalid_num_buckets() {
        let mut config = DhtConfig::default();
        config.num_buckets = 128;

        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("256"));
    }

    #[test]
    fn test_dht_config_test_config() {
        let config = DhtConfig::test_config();

        assert_eq!(config.bucket_size, 5);
        assert_eq!(config.alpha, 2);
        assert!(config.rpc_timeout.as_millis() < 1000);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_replication_strategy_equality() {
        assert_eq!(ReplicationStrategy::Push, ReplicationStrategy::Push);
        assert_ne!(ReplicationStrategy::Push, ReplicationStrategy::Pull);
        assert_ne!(ReplicationStrategy::Pull, ReplicationStrategy::Hybrid);
    }

    #[test]
    fn test_dht_config_serialization() {
        let config = DhtConfig::default();

        let serialized = serde_json::to_string(&config).unwrap();
        let deserialized: DhtConfig = serde_json::from_str(&serialized).unwrap();

        assert_eq!(config.bucket_size, deserialized.bucket_size);
        assert_eq!(config.alpha, deserialized.alpha);
    }

    #[test]
    fn test_dht_config_with_signatures() {
        let config = DhtConfig::new().with_signatures(true);
        assert!(config.require_signatures);

        let config2 = DhtConfig::new().with_signatures(false);
        assert!(!config2.require_signatures);
    }
}
