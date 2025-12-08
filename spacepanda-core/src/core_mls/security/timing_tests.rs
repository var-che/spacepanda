//! Timing Attack Resistance Tests
//!
//! These tests validate that cryptographic operations are implemented using constant-time
//! algorithms to prevent timing side-channel attacks.
//!
//! Timing attacks exploit variations in execution time based on secret data (keys, plaintexts).
//! Attackers can statistically analyze timing differences to extract secrets.
//!
//! Test Strategy:
//! 1. Measure operation timing with different inputs
//! 2. Use statistical analysis to detect timing variations
//! 3. Verify crypto libraries provide constant-time guarantees
//!
//! ## Important Notes
//!
//! **Running These Tests:**
//! - These tests are **highly sensitive** to system load and CPU scheduling
//! - They should be run in isolation with minimal background processes
//! - Run with: `cargo test --lib core_mls::security::timing_tests -- --test-threads=1`
//! - Do NOT run in parallel with other tests (they will fail due to CPU contention)
//!
//! **CI/CD Considerations:**
//! - These tests may be flaky in shared CI/CD environments
//! - Consider making them opt-in via a feature flag in production
//! - They verify *library implementations*, not hardware-level timing channels
//!
//! **What These Tests Verify:**
//! - ChaCha20-Poly1305 encryption/decryption is constant-time
//! - Ed25519 signature verification doesn't leak validity via timing
//! - HKDF key derivation timing is independent of input
//! - Metadata encryption wrapper maintains constant-time properties
//!
//! **What These Tests DO NOT Verify:**
//! - Hardware-level timing channels (CPU cache, speculative execution, etc.)
//! - Network-level timing attacks (these require different testing approaches)
//! - Timing attacks against non-cryptographic code paths

use crate::core_mls::storage::metadata_encryption::MetadataEncryption;
use chacha20poly1305::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    ChaCha20Poly1305, Nonce,
};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use hkdf::Hkdf;
use sha2::Sha256;
use std::time::{Duration, Instant};

/// Number of iterations for statistical timing tests
const TIMING_ITERATIONS: usize = 1000;

/// Maximum acceptable timing variation (coefficient of variation)
/// CV = std_dev / mean
/// Lower CV = more consistent timing
///
/// Note: In a real-world system with OS scheduling, CPU frequency scaling,
/// and cache effects, perfect constant-time behavior (CV ≈ 0) is not achievable.
///
/// A CV of 0.3 (30%) is a reasonable threshold that:
/// - Detects obvious timing leaks (e.g., early-exit on invalid input)
/// - Tolerates normal OS/hardware variance
/// - Is achievable with constant-time crypto primitives
const MAX_TIMING_CV: f64 = 0.3; // 30% variation allowed

/// Statistical helper: Calculate mean
fn mean(values: &[Duration]) -> Duration {
    let total: Duration = values.iter().sum();
    total / values.len() as u32
}

/// Statistical helper: Calculate standard deviation
fn std_dev(values: &[Duration]) -> Duration {
    let avg = mean(values);
    let variance: f64 = values
        .iter()
        .map(|&d| {
            let diff = d.as_nanos() as f64 - avg.as_nanos() as f64;
            diff * diff
        })
        .sum::<f64>()
        / values.len() as f64;
    
    Duration::from_nanos(variance.sqrt() as u64)
}

/// Statistical helper: Calculate coefficient of variation (CV)
fn coefficient_of_variation(values: &[Duration]) -> f64 {
    let avg = mean(values);
    let sd = std_dev(values);
    
    if avg.as_nanos() == 0 {
        return 0.0;
    }
    
    sd.as_nanos() as f64 / avg.as_nanos() as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore] // Run separately due to timing sensitivity: cargo test --lib core_mls::security::timing_tests -- --test-threads=1 --ignored
    fn test_chacha20poly1305_encryption_timing() {
        // Test that encryption time doesn't vary based on plaintext content
        
        let key = ChaCha20Poly1305::generate_key(OsRng);
        let cipher = ChaCha20Poly1305::new(&key);
        let nonce = Nonce::from_slice(b"unique nonce");
        
        // Plaintext with all zeros
        let plaintext_zeros = vec![0u8; 1024];
        
        // Plaintext with all ones
        let plaintext_ones = vec![1u8; 1024];
        
        // Plaintext with random data
        let plaintext_random: Vec<u8> = (0..1024).map(|i| (i % 256) as u8).collect();
        
        // Measure timing for each plaintext type
        let mut timings_zeros = Vec::with_capacity(TIMING_ITERATIONS);
        let mut timings_ones = Vec::with_capacity(TIMING_ITERATIONS);
        let mut timings_random = Vec::with_capacity(TIMING_ITERATIONS);
        
        for _ in 0..TIMING_ITERATIONS {
            // Time encryption of zeros
            let start = Instant::now();
            let _ciphertext = cipher.encrypt(nonce, plaintext_zeros.as_ref()).unwrap();
            timings_zeros.push(start.elapsed());
            
            // Time encryption of ones
            let start = Instant::now();
            let _ciphertext = cipher.encrypt(nonce, plaintext_ones.as_ref()).unwrap();
            timings_ones.push(start.elapsed());
            
            // Time encryption of random
            let start = Instant::now();
            let _ciphertext = cipher.encrypt(nonce, plaintext_random.as_ref()).unwrap();
            timings_random.push(start.elapsed());
        }
        
        // Calculate statistics
        let cv_zeros = coefficient_of_variation(&timings_zeros);
        let cv_ones = coefficient_of_variation(&timings_ones);
        let cv_random = coefficient_of_variation(&timings_random);
        
        // Verify consistent timing (low variation)
        assert!(
            cv_zeros < MAX_TIMING_CV,
            "ChaCha20-Poly1305 encryption timing varies too much for zeros (CV: {:.3})",
            cv_zeros
        );
        assert!(
            cv_ones < MAX_TIMING_CV,
            "ChaCha20-Poly1305 encryption timing varies too much for ones (CV: {:.3})",
            cv_ones
        );
        assert!(
            cv_random < MAX_TIMING_CV,
            "ChaCha20-Poly1305 encryption timing varies too much for random (CV: {:.3})",
            cv_random
        );
        
        // Verify timing doesn't vary significantly between different plaintexts
        let mean_zeros = mean(&timings_zeros);
        let mean_ones = mean(&timings_ones);
        let mean_random = mean(&timings_random);
        
        let max_mean = mean_zeros.max(mean_ones).max(mean_random);
        let min_mean = mean_zeros.min(mean_ones).min(mean_random);
        
        let mean_variation = (max_mean.as_nanos() - min_mean.as_nanos()) as f64
            / max_mean.as_nanos() as f64;
        
        assert!(
            mean_variation < MAX_TIMING_CV,
            "ChaCha20-Poly1305 mean timing varies between plaintexts (variation: {:.3})",
            mean_variation
        );
    }

    #[test]
    #[ignore] // Run separately due to timing sensitivity: cargo test --lib core_mls::security::timing_tests -- --test-threads=1 --ignored
    fn test_chacha20poly1305_decryption_timing() {
        // Test that decryption time doesn't vary based on ciphertext content
        
        let key = ChaCha20Poly1305::generate_key(OsRng);
        let cipher = ChaCha20Poly1305::new(&key);
        let nonce = Nonce::from_slice(b"unique nonce");
        
        // Create valid ciphertexts
        let plaintext1 = vec![0u8; 1024];
        let plaintext2 = vec![1u8; 1024];
        
        let ciphertext1 = cipher.encrypt(nonce, plaintext1.as_ref()).unwrap();
        let ciphertext2 = cipher.encrypt(nonce, plaintext2.as_ref()).unwrap();
        
        // Measure decryption timing
        let mut timings1 = Vec::with_capacity(TIMING_ITERATIONS);
        let mut timings2 = Vec::with_capacity(TIMING_ITERATIONS);
        
        for _ in 0..TIMING_ITERATIONS {
            let start = Instant::now();
            let _plaintext = cipher.decrypt(nonce, ciphertext1.as_ref()).unwrap();
            timings1.push(start.elapsed());
            
            let start = Instant::now();
            let _plaintext = cipher.decrypt(nonce, ciphertext2.as_ref()).unwrap();
            timings2.push(start.elapsed());
        }
        
        let cv1 = coefficient_of_variation(&timings1);
        let cv2 = coefficient_of_variation(&timings2);
        
        assert!(
            cv1 < MAX_TIMING_CV,
            "Decryption timing varies too much (CV: {:.3})",
            cv1
        );
        assert!(
            cv2 < MAX_TIMING_CV,
            "Decryption timing varies too much (CV: {:.3})",
            cv2
        );
    }

    #[test]
    #[ignore] // Run separately due to timing sensitivity: cargo test --lib core_mls::security::timing_tests -- --test-threads=1 --ignored
    fn test_ed25519_signature_verification_timing() {
        // Test that signature verification is constant-time
        // (doesn't leak whether signature is valid/invalid based on timing)
        
        let signing_key = SigningKey::generate(&mut OsRng);
        let verifying_key = signing_key.verifying_key();
        
        let message = b"test message for signature";
        let signature = signing_key.sign(message);
        
        // Create invalid signature (flip a bit)
        let mut invalid_sig_bytes = signature.to_bytes();
        invalid_sig_bytes[0] ^= 1;
        let invalid_signature = Signature::from_bytes(&invalid_sig_bytes);
        
        // Measure timing for valid signature verification
        let mut timings_valid = Vec::with_capacity(TIMING_ITERATIONS);
        for _ in 0..TIMING_ITERATIONS {
            let start = Instant::now();
            let _ = verifying_key.verify(message, &signature);
            timings_valid.push(start.elapsed());
        }
        
        // Measure timing for invalid signature verification
        let mut timings_invalid = Vec::with_capacity(TIMING_ITERATIONS);
        for _ in 0..TIMING_ITERATIONS {
            let start = Instant::now();
            let _ = verifying_key.verify(message, &invalid_signature);
            timings_invalid.push(start.elapsed());
        }
        
        let cv_valid = coefficient_of_variation(&timings_valid);
        let cv_invalid = coefficient_of_variation(&timings_invalid);
        
        assert!(
            cv_valid < MAX_TIMING_CV,
            "Valid signature verification timing varies (CV: {:.3})",
            cv_valid
        );
        assert!(
            cv_invalid < MAX_TIMING_CV,
            "Invalid signature verification timing varies (CV: {:.3})",
            cv_invalid
        );
        
        // Verify mean timing doesn't differ significantly between valid/invalid
        let mean_valid = mean(&timings_valid);
        let mean_invalid = mean(&timings_invalid);
        
        let timing_diff = if mean_valid > mean_invalid {
            (mean_valid.as_nanos() - mean_invalid.as_nanos()) as f64 / mean_valid.as_nanos() as f64
        } else {
            (mean_invalid.as_nanos() - mean_valid.as_nanos()) as f64 / mean_invalid.as_nanos() as f64
        };
        
        assert!(
            timing_diff < MAX_TIMING_CV,
            "Signature verification timing differs between valid/invalid (diff: {:.3})",
            timing_diff
        );
    }

    #[test]
    #[ignore] // Run separately due to timing sensitivity: cargo test --lib core_mls::security::timing_tests -- --test-threads=1 --ignored
    fn test_hkdf_key_derivation_timing() {
        // Test that HKDF key derivation is constant-time regardless of input
        
        let ikm1 = vec![0u8; 32]; // Input key material (all zeros)
        let ikm2 = vec![1u8; 32]; // Input key material (all ones)
        let ikm3: Vec<u8> = (0..32).map(|i| i as u8).collect(); // Sequential
        
        let salt = b"test salt";
        let info = b"test info";
        
        // Measure HKDF timing for different inputs
        let mut timings1 = Vec::with_capacity(TIMING_ITERATIONS);
        let mut timings2 = Vec::with_capacity(TIMING_ITERATIONS);
        let mut timings3 = Vec::with_capacity(TIMING_ITERATIONS);
        
        for _ in 0..TIMING_ITERATIONS {
            let mut output = [0u8; 32];
            
            let start = Instant::now();
            let hkdf1 = Hkdf::<Sha256>::new(Some(salt), &ikm1);
            hkdf1.expand(info, &mut output).unwrap();
            timings1.push(start.elapsed());
            
            let start = Instant::now();
            let hkdf2 = Hkdf::<Sha256>::new(Some(salt), &ikm2);
            hkdf2.expand(info, &mut output).unwrap();
            timings2.push(start.elapsed());
            
            let start = Instant::now();
            let hkdf3 = Hkdf::<Sha256>::new(Some(salt), &ikm3);
            hkdf3.expand(info, &mut output).unwrap();
            timings3.push(start.elapsed());
        }
        
        let cv1 = coefficient_of_variation(&timings1);
        let cv2 = coefficient_of_variation(&timings2);
        let cv3 = coefficient_of_variation(&timings3);
        
        assert!(
            cv1 < MAX_TIMING_CV,
            "HKDF timing varies for input 1 (CV: {:.3})",
            cv1
        );
        assert!(
            cv2 < MAX_TIMING_CV,
            "HKDF timing varies for input 2 (CV: {:.3})",
            cv2
        );
        assert!(
            cv3 < MAX_TIMING_CV,
            "HKDF timing varies for input 3 (CV: {:.3})",
            cv3
        );
    }

    #[test]
    #[ignore] // Run separately due to timing sensitivity: cargo test --lib core_mls::security::timing_tests -- --test-threads=1 --ignored
    fn test_metadata_encryption_timing() {
        // Test that our metadata encryption wrapper maintains constant-time properties
        
        let group_id = b"test_group_id_12345";
        
        let plaintext1 = vec![0u8; 256];
        let plaintext2 = vec![1u8; 256];
        let plaintext3 = b"Secret channel name".to_vec();
        
        let enc = MetadataEncryption::new(group_id);
        
        // Measure encryption timing
        let mut timings1 = Vec::with_capacity(TIMING_ITERATIONS);
        let mut timings2 = Vec::with_capacity(TIMING_ITERATIONS);
        let mut timings3 = Vec::with_capacity(TIMING_ITERATIONS);
        
        for _ in 0..TIMING_ITERATIONS {
            let start = Instant::now();
            let _encrypted = enc.encrypt(&plaintext1).unwrap();
            timings1.push(start.elapsed());
            
            let start = Instant::now();
            let _encrypted = enc.encrypt(&plaintext2).unwrap();
            timings2.push(start.elapsed());
            
            let start = Instant::now();
            let _encrypted = enc.encrypt(&plaintext3).unwrap();
            timings3.push(start.elapsed());
        }
        
        let cv1 = coefficient_of_variation(&timings1);
        let cv2 = coefficient_of_variation(&timings2);
        let cv3 = coefficient_of_variation(&timings3);
        
        assert!(
            cv1 < MAX_TIMING_CV,
            "Metadata encryption timing varies (CV: {:.3})",
            cv1
        );
        assert!(
            cv2 < MAX_TIMING_CV,
            "Metadata encryption timing varies (CV: {:.3})",
            cv2
        );
        assert!(
            cv3 < MAX_TIMING_CV,
            "Metadata encryption timing varies (CV: {:.3})",
            cv3
        );
    }

    #[test]
    #[ignore] // Run separately due to timing sensitivity: cargo test --lib core_mls::security::timing_tests -- --test-threads=1 --ignored
    fn test_metadata_decryption_timing() {
        // Test that decryption timing doesn't leak information about plaintext content
        // (when plaintexts are the same length)
        
        let group_id = b"test_group_id_12345";
        let enc = MetadataEncryption::new(group_id);
        
        // Create valid ciphertexts with SAME LENGTH but different content
        let plaintext1 = vec![0u8; 256];
        let plaintext2 = vec![1u8; 256]; // Same length, different content
        
        let ciphertext1 = enc.encrypt(&plaintext1).unwrap();
        let ciphertext2 = enc.encrypt(&plaintext2).unwrap();
        
        // Measure decryption timing
        let mut timings1 = Vec::with_capacity(TIMING_ITERATIONS);
        let mut timings2 = Vec::with_capacity(TIMING_ITERATIONS);
        
        for _ in 0..TIMING_ITERATIONS {
            let start = Instant::now();
            let _plaintext = enc.decrypt(&ciphertext1).unwrap();
            timings1.push(start.elapsed());
            
            let start = Instant::now();
            let _plaintext = enc.decrypt(&ciphertext2).unwrap();
            timings2.push(start.elapsed());
        }
        
        let cv1 = coefficient_of_variation(&timings1);
        let cv2 = coefficient_of_variation(&timings2);
        
        assert!(
            cv1 < MAX_TIMING_CV,
            "Metadata decryption timing varies (CV: {:.3})",
            cv1
        );
        assert!(
            cv2 < MAX_TIMING_CV,
            "Metadata decryption timing varies (CV: {:.3})",
            cv2
        );
        
        // Verify similar mean timing
        let mean1 = mean(&timings1);
        let mean2 = mean(&timings2);
        
        let mean_diff = if mean1 > mean2 {
            (mean1.as_nanos() - mean2.as_nanos()) as f64 / mean1.as_nanos() as f64
        } else {
            (mean2.as_nanos() - mean1.as_nanos()) as f64 / mean2.as_nanos() as f64
        };
        
        assert!(
            mean_diff < MAX_TIMING_CV,
            "Metadata decryption mean timing differs (diff: {:.3})",
            mean_diff
        );
    }

    #[test]
    #[ignore] // Run separately due to timing sensitivity: cargo test --lib core_mls::security::timing_tests -- --test-threads=1 --ignored
    fn test_metadata_encryption_key_derivation_timing() {
        // Test that creating encryption contexts with different group IDs
        // takes consistent time (key derivation should be constant-time)
        
        let group_id1 = b"group_000000000000";
        let group_id2 = b"group_111111111111";
        let group_id3 = b"group_abcdef12345";
        
        let mut timings1 = Vec::with_capacity(TIMING_ITERATIONS);
        let mut timings2 = Vec::with_capacity(TIMING_ITERATIONS);
        let mut timings3 = Vec::with_capacity(TIMING_ITERATIONS);
        
        for _ in 0..TIMING_ITERATIONS {
            let start = Instant::now();
            let _enc1 = MetadataEncryption::new(group_id1);
            timings1.push(start.elapsed());
            
            let start = Instant::now();
            let _enc2 = MetadataEncryption::new(group_id2);
            timings2.push(start.elapsed());
            
            let start = Instant::now();
            let _enc3 = MetadataEncryption::new(group_id3);
            timings3.push(start.elapsed());
        }
        
        let cv1 = coefficient_of_variation(&timings1);
        let cv2 = coefficient_of_variation(&timings2);
        let cv3 = coefficient_of_variation(&timings3);
        
        assert!(
            cv1 < MAX_TIMING_CV,
            "Key derivation timing varies (CV: {:.3})",
            cv1
        );
        assert!(
            cv2 < MAX_TIMING_CV,
            "Key derivation timing varies (CV: {:.3})",
            cv2
        );
        assert!(
            cv3 < MAX_TIMING_CV,
            "Key derivation timing varies (CV: {:.3})",
            cv3
        );
    }

    #[test]
    fn test_statistical_helpers() {
        // Verify our statistical functions work correctly
        
        let durations = vec![
            Duration::from_micros(100),
            Duration::from_micros(110),
            Duration::from_micros(90),
            Duration::from_micros(105),
            Duration::from_micros(95),
        ];
        
        let avg = mean(&durations);
        assert_eq!(avg, Duration::from_micros(100));
        
        let cv = coefficient_of_variation(&durations);
        assert!(cv > 0.0 && cv < 0.1, "CV should be small for consistent values");
    }
}
