/*!
    Deterministic RNG helpers for reproducible tests

    Provides deterministic random number generators for integration tests
    and fuzz tests to ensure reproducibility across runs.

    Similar to benchmark reproducibility (benches/bench_config.rs), but
    for test harness usage.
*/

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

/// Default seed for deterministic tests
pub const DEFAULT_TEST_SEED: u64 = 42;

/// Create a deterministic RNG with the default seed
pub fn test_rng() -> StdRng {
    test_rng_with_seed(DEFAULT_TEST_SEED)
}

/// Create a deterministic RNG with a custom seed
pub fn test_rng_with_seed(seed: u64) -> StdRng {
    StdRng::seed_from_u64(seed)
}

/// Generate a deterministic vec of random bytes
pub fn deterministic_bytes(len: usize) -> Vec<u8> {
    let mut rng = test_rng();
    (0..len).map(|_| rng.random()).collect()
}

/// Generate a deterministic vec of random bytes with custom seed
pub fn deterministic_bytes_with_seed(len: usize, seed: u64) -> Vec<u8> {
    let mut rng = test_rng_with_seed(seed);
    (0..len).map(|_| rng.random()).collect()
}

/// Generate a deterministic u64 value
pub fn deterministic_u64() -> u64 {
    test_rng().random()
}

/// Generate a deterministic u64 value with custom seed
pub fn deterministic_u64_with_seed(seed: u64) -> u64 {
    test_rng_with_seed(seed).random()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rng_is_deterministic() {
        let mut rng1 = test_rng();
        let mut rng2 = test_rng();

        // Same seed produces same sequence
        for _ in 0..100 {
            assert_eq!(rng1.random::<u64>(), rng2.random::<u64>());
        }
    }

    #[test]
    fn test_rng_with_seed_is_deterministic() {
        let mut rng1 = test_rng_with_seed(12345);
        let mut rng2 = test_rng_with_seed(12345);

        for _ in 0..100 {
            assert_eq!(rng1.random::<u64>(), rng2.random::<u64>());
        }
    }

    #[test]
    fn test_different_seeds_produce_different_sequences() {
        let mut rng1 = test_rng_with_seed(1);
        let mut rng2 = test_rng_with_seed(2);

        // Different seeds should produce different values
        assert_ne!(rng1.random::<u64>(), rng2.random::<u64>());
    }

    #[test]
    fn test_deterministic_bytes_reproducible() {
        let bytes1 = deterministic_bytes(100);
        let bytes2 = deterministic_bytes(100);

        assert_eq!(bytes1, bytes2);
    }

    #[test]
    fn test_deterministic_bytes_with_seed_reproducible() {
        let bytes1 = deterministic_bytes_with_seed(100, 999);
        let bytes2 = deterministic_bytes_with_seed(100, 999);

        assert_eq!(bytes1, bytes2);
    }

    #[test]
    fn test_deterministic_u64_reproducible() {
        let val1 = deterministic_u64();
        let val2 = deterministic_u64();

        assert_eq!(val1, val2);
    }

    #[test]
    fn test_deterministic_u64_with_seed_reproducible() {
        let val1 = deterministic_u64_with_seed(777);
        let val2 = deterministic_u64_with_seed(777);

        assert_eq!(val1, val2);
    }
}
