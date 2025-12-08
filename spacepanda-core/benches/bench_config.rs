//! Benchmark configuration for reproducibility
//!
//! This module provides deterministic configuration for benchmarks to ensure
//! reproducible results across runs and CI environments.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Benchmark configuration for reproducibility
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchConfig {
    /// Random seed for deterministic RNG
    pub seed: u64,

    /// Rust version used for benchmarks
    pub rust_version: String,

    /// CPU model for hardware reference
    pub cpu_model: String,

    /// Number of CPU cores
    pub cpu_cores: usize,

    /// RAM in GB
    pub ram_gb: usize,

    /// OS version
    pub os_version: String,

    /// Benchmark-specific parameters
    pub parameters: HashMap<String, String>,
}

impl Default for BenchConfig {
    fn default() -> Self {
        Self {
            seed: 42, // Deterministic default seed
            rust_version: "1.91.1".to_string(),
            cpu_model: "Unknown".to_string(),
            cpu_cores: num_cpus::get(),
            ram_gb: 16,
            os_version: std::env::consts::OS.to_string(),
            parameters: HashMap::new(),
        }
    }
}

impl BenchConfig {
    /// Create new config with specific seed
    pub fn with_seed(seed: u64) -> Self {
        Self { seed, ..Default::default() }
    }

    /// Load config from file, or create default
    pub fn load_or_default<P: AsRef<Path>>(path: P) -> Self {
        if let Ok(contents) = fs::read_to_string(path) {
            serde_json::from_str(&contents).unwrap_or_default()
        } else {
            Default::default()
        }
    }

    /// Save config to file
    pub fn save<P: AsRef<Path>>(&self, path: P) -> std::io::Result<()> {
        let contents = serde_json::to_string_pretty(self)?;
        fs::write(path, contents)
    }

    /// Set parameter
    pub fn set_param(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.parameters.insert(key.into(), value.into());
    }

    /// Get parameter
    pub fn get_param(&self, key: &str) -> Option<&str> {
        self.parameters.get(key).map(|s| s.as_str())
    }
}

/// Benchmark results for tracking performance over time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchResult {
    /// Benchmark name
    pub name: String,

    /// Timestamp of benchmark run
    pub timestamp: String,

    /// Configuration used
    pub config: BenchConfig,

    /// Mean execution time in nanoseconds
    pub mean_ns: f64,

    /// Standard deviation in nanoseconds
    pub std_dev_ns: f64,

    /// Median (p50) in nanoseconds
    pub p50_ns: f64,

    /// 95th percentile in nanoseconds
    pub p95_ns: f64,

    /// 99th percentile in nanoseconds
    pub p99_ns: f64,

    /// Throughput (operations per second)
    pub throughput_ops: Option<f64>,
}

impl BenchResult {
    /// Create new benchmark result
    pub fn new(name: impl Into<String>, config: BenchConfig) -> Self {
        Self {
            name: name.into(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            config,
            mean_ns: 0.0,
            std_dev_ns: 0.0,
            p50_ns: 0.0,
            p95_ns: 0.0,
            p99_ns: 0.0,
            throughput_ops: None,
        }
    }

    /// Save result to file
    pub fn save<P: AsRef<Path>>(&self, path: P) -> std::io::Result<()> {
        let contents = serde_json::to_string_pretty(self)?;
        fs::write(path, contents)
    }

    /// Load results from file
    pub fn load<P: AsRef<Path>>(path: P) -> std::io::Result<Self> {
        let contents = fs::read_to_string(path)?;
        serde_json::from_str(&contents)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }
}

/// Helper to create deterministic RNG from config
pub fn create_rng(config: &BenchConfig) -> rand::rngs::StdRng {
    use rand::SeedableRng;
    rand::rngs::StdRng::seed_from_u64(config.seed)
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_bench_config_default() {
        let config = BenchConfig::default();
        assert_eq!(config.seed, 42);
        assert!(config.cpu_cores > 0);
    }

    #[test]
    fn test_bench_config_with_seed() {
        let config = BenchConfig::with_seed(12345);
        assert_eq!(config.seed, 12345);
    }

    #[test]
    fn test_bench_config_save_load() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("bench_config.json");

        let mut config = BenchConfig::with_seed(999);
        config.set_param("test_param", "test_value");

        config.save(&path).unwrap();

        let loaded = BenchConfig::load_or_default(&path);
        assert_eq!(loaded.seed, 999);
        assert_eq!(loaded.get_param("test_param"), Some("test_value"));
    }

    #[test]
    fn test_bench_result_save_load() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("result.json");

        let config = BenchConfig::default();
        let mut result = BenchResult::new("test_bench", config);
        result.mean_ns = 1000.0;
        result.p50_ns = 950.0;
        result.p95_ns = 1500.0;
        result.p99_ns = 2000.0;

        result.save(&path).unwrap();

        let loaded = BenchResult::load(&path).unwrap();
        assert_eq!(loaded.name, "test_bench");
        assert_eq!(loaded.mean_ns, 1000.0);
    }

    #[test]
    fn test_deterministic_rng() {
        let config = BenchConfig::with_seed(42);

        let mut rng1 = create_rng(&config);
        let mut rng2 = create_rng(&config);

        use rand::Rng;
        let val1: u64 = rng1.random();
        let val2: u64 = rng2.random();

        assert_eq!(val1, val2, "Same seed should produce same random values");
    }
}
