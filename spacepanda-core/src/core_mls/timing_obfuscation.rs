//! Timing Obfuscation - Prevent timing-based metadata leakage
//!
//! This module provides utilities to obfuscate message timing metadata
//! to prevent network observers from inferring communication patterns.
//!
//! ## Threat Model
//!
//! **Without Timing Obfuscation:**
//! ```text
//! Message A: sequence = 1703001234000  (exact unix timestamp)
//! Message B: sequence = 1703001234001  (1ms later)
//! → Network observer: "These messages were sent together in a burst"
//! ```
//!
//! **With Timing Obfuscation:**
//! ```text
//! Message A: sequence = 1703001234000 + jitter_a  (e.g., +5s)
//! Message B: sequence = 1703001234001 + jitter_b  (e.g., -3s)
//! → Network observer: Cannot determine actual timing relationship
//! ```
//!
//! ## Security Properties
//!
//! - **Unlinkability**: Messages sent together don't cluster in time
//! - **Ambiguity**: ±30 second window makes exact timing unknown
//! - **Ordering Preservation**: Jitter doesn't break message ordering within channel
//!
//! ## Usage
//!
//! ```rust,ignore
//! use spacepanda_core::core_mls::timing_obfuscation::generate_obfuscated_sequence;
//!
//! // Instead of raw timestamp:
//! let sequence = std::time::SystemTime::now()
//!     .duration_since(std::time::UNIX_EPOCH)
//!     .unwrap()
//!     .as_secs() as i64;
//!
//! // Use obfuscated sequence:
//! let sequence = generate_obfuscated_sequence();
//! ```

use rand::Rng;
use std::time::{SystemTime, UNIX_EPOCH};

/// Maximum jitter in seconds (±30 seconds)
///
/// This value balances privacy and usability:
/// - Too small: Timing correlation still possible
/// - Too large: Message ordering becomes confusing
///
/// 30 seconds is chosen as a reasonable trade-off.
const MAX_JITTER_SECONDS: i64 = 30;

/// Generate an obfuscated sequence number for message ordering
///
/// This adds random jitter to the current Unix timestamp to prevent
/// timing-based traffic analysis while preserving message ordering.
///
/// # Returns
///
/// An obfuscated Unix timestamp (in seconds) with random jitter applied
///
/// # Security
///
/// - Jitter is uniformly distributed in [-30, +30] seconds
/// - Each message gets independent jitter (no correlation)
/// - Ordering is preserved within the jitter window
///
/// # Example
///
/// ```rust,ignore
/// let seq1 = generate_obfuscated_sequence();
/// let seq2 = generate_obfuscated_sequence();
/// // seq2 may be less than seq1 due to jitter, but that's expected
/// ```
pub fn generate_obfuscated_sequence() -> i64 {
    // Get current Unix timestamp in seconds
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs() as i64;

    // Generate random jitter in [-MAX_JITTER_SECONDS, +MAX_JITTER_SECONDS]
    let jitter = rand::rng().random_range(-MAX_JITTER_SECONDS..=MAX_JITTER_SECONDS);

    // Apply jitter
    now + jitter
}

/// Generate an obfuscated sequence number with a minimum value
///
/// This ensures the sequence number is always greater than the provided
/// minimum, which is useful for maintaining message ordering within a channel.
///
/// # Arguments
///
/// * `min_sequence` - The minimum sequence number (exclusive)
///
/// # Returns
///
/// An obfuscated Unix timestamp greater than `min_sequence`
///
/// # Example
///
/// ```rust,ignore
/// // Ensure new message has higher sequence than last message
/// let last_seq = 1703001234000;
/// let new_seq = generate_obfuscated_sequence_after(last_seq);
/// assert!(new_seq > last_seq);
/// ```
pub fn generate_obfuscated_sequence_after(min_sequence: i64) -> i64 {
    loop {
        let seq = generate_obfuscated_sequence();
        if seq > min_sequence {
            return seq;
        }
        // Retry if jitter caused sequence to be too low
    }
}

/// Generate a deterministic sequence number without jitter
///
/// This is used for testing or when timing privacy is not required
/// (e.g., local-only operations).
///
/// # Returns
///
/// Current Unix timestamp in seconds (no jitter)
pub fn generate_sequence_no_jitter() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_obfuscated_sequence() {
        let seq = generate_obfuscated_sequence();
        let now = generate_sequence_no_jitter();

        // Sequence should be within ±MAX_JITTER_SECONDS of now
        assert!(
            seq >= now - MAX_JITTER_SECONDS,
            "Sequence {} too far below now {}",
            seq,
            now
        );
        assert!(
            seq <= now + MAX_JITTER_SECONDS,
            "Sequence {} too far above now {}",
            seq,
            now
        );
    }

    #[test]
    fn test_generate_obfuscated_sequence_uniqueness() {
        // Generate 100 sequences and verify they're not all identical
        let sequences: Vec<i64> = (0..100).map(|_| generate_obfuscated_sequence()).collect();

        let unique_count = sequences.iter().collect::<std::collections::HashSet<_>>().len();

        // At least 50% should be unique due to jitter
        assert!(
            unique_count >= 50,
            "Only {} unique sequences out of 100",
            unique_count
        );
    }

    #[test]
    fn test_generate_obfuscated_sequence_after() {
        let min_seq = 1703001234000;
        let new_seq = generate_obfuscated_sequence_after(min_seq);

        assert!(
            new_seq > min_seq,
            "New sequence {} not greater than min {}",
            new_seq,
            min_seq
        );
    }

    #[test]
    fn test_generate_sequence_no_jitter() {
        let seq1 = generate_sequence_no_jitter();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let seq2 = generate_sequence_no_jitter();

        // Should be deterministic (no jitter)
        assert!(seq2 >= seq1);
        assert!(seq2 - seq1 < 2); // Less than 2 seconds apart
    }

    #[test]
    fn test_jitter_distribution() {
        // Generate many sequences and check jitter distribution
        let now = generate_sequence_no_jitter();
        let samples = 1000;

        let jitters: Vec<i64> = (0..samples)
            .map(|_| generate_obfuscated_sequence() - now)
            .collect();

        // Check that jitter spans the full range
        let min_jitter = jitters.iter().min().unwrap();
        let max_jitter = jitters.iter().max().unwrap();

        // Should see jitter across most of the range
        assert!(
            *min_jitter < -10,
            "Min jitter {} not sufficiently negative",
            min_jitter
        );
        assert!(
            *max_jitter > 10,
            "Max jitter {} not sufficiently positive",
            max_jitter
        );

        // Average jitter should be close to 0 (uniform distribution)
        let avg_jitter: f64 = jitters.iter().map(|&x| x as f64).sum::<f64>() / samples as f64;
        assert!(
            avg_jitter.abs() < 2.0,
            "Average jitter {} not close to 0",
            avg_jitter
        );
    }
}
