# Timing Attack Mitigations

This document describes the timing attack mitigations implemented in SpacePanda and how to verify them.

## Overview

Timing attacks exploit variations in execution time to extract secret information. Attackers measure how long operations take with different inputs and use statistical analysis to infer secrets like cryptographic keys or plaintext content.

## What We Protect Against

### 1. **Cryptographic Timing Leaks**

All cryptographic operations use constant-time implementations:

- **ChaCha20-Poly1305** (AEAD encryption): Constant-time regardless of plaintext/ciphertext content
- **Ed25519** (digital signatures): Constant-time signature verification (doesn't leak validity)
- **HKDF-SHA256** (key derivation): Constant-time regardless of input key material
- **Metadata encryption**: Maintains constant-time properties through wrapper layer

### 2. **Validation Timing Leaks**

Input validation and error handling use constant-time comparisons where appropriate to avoid leaking information through early-exit patterns.

## What We DON'T Protect Against

### 1. **Hardware-Level Timing Channels**

- CPU cache timing attacks (e.g., Spectre, Meltdown)
- Speculative execution side-channels
- Power analysis attacks

These require hardware-level countermeasures beyond software scope.

### 2. **Network-Level Timing Attacks**

- Packet size correlation (ciphertext length is public)
- Round-trip time analysis (network latency is observable)

These require application-level padding/traffic shaping if needed.

### 3. **Algorithm Complexity Differences**

- Different-sized messages naturally take different time
- This is NOT a timing leak (message size is public metadata)
- Only constant-time for equal-sized inputs

## Implementation Details

### Cryptographic Libraries

We use industry-standard, audited cryptographic libraries with constant-time guarantees:

```toml
[dependencies]
chacha20poly1305 = "0.10"  # Constant-time AEAD
ed25519-dalek = "2.1"      # Constant-time Ed25519
hkdf = "0.12"              # Constant-time HKDF
sha2 = "0.10"              # Constant-time SHA-256
```

### Metadata Encryption

Our metadata encryption wrapper (`MetadataEncryption`) maintains constant-time properties:

```rust
// Key derivation uses HKDF (constant-time)
let hkdf = Hkdf::<Sha256>::new(Some(SALT), &APPLICATION_MASTER_KEY);
hkdf.expand(&group_id, &mut key_material)?;

// Encryption uses ChaCha20-Poly1305 (constant-time)
cipher.encrypt(nonce, plaintext)?;
```

See [`core_mls/storage/metadata_encryption.rs`](../spacepanda-core/src/core_mls/storage/metadata_encryption.rs) for implementation details.

### String Comparison

For comparing secret strings (e.g., passphrases, keys), use constant-time comparison:

```rust
use subtle::ConstantTimeEq;

if key1.ct_eq(&key2).into() {
    // Keys match
}
```

**Never use `==` for secret comparison** - it may short-circuit on first byte difference.

## Testing

### Running Timing Attack Tests

Timing tests are **highly sensitive** to system load and must be run in isolation:

```bash
# Run timing tests (single-threaded, no other tests)
nix develop --command cargo test --lib core_mls::security::timing_tests -- --test-threads=1 --ignored

# Or without nix:
cargo test --lib core_mls::security::timing_tests -- --test-threads=1 --ignored
```

**Important:**

- Close all unnecessary applications
- Minimize background processes
- Do NOT run in parallel with other tests
- Tests may be flaky in shared CI/CD environments

### Test Coverage

We have 7 timing attack resistance tests:

1. **`test_chacha20poly1305_encryption_timing`**

   - Verifies encryption time doesn't vary based on plaintext content
   - Uses coefficient of variation (CV) < 0.3 (30%)

2. **`test_chacha20poly1305_decryption_timing`**

   - Verifies decryption time doesn't vary based on ciphertext content
   - Uses CV < 0.3 threshold

3. **`test_ed25519_signature_verification_timing`**

   - Verifies signature verification doesn't leak validity via timing
   - Compares valid vs invalid signature verification timing

4. **`test_hkdf_key_derivation_timing`**

   - Verifies HKDF timing independent of input key material
   - Tests with all-zeros, all-ones, and sequential bytes

5. **`test_metadata_encryption_timing`**

   - Verifies metadata encryption wrapper maintains constant-time properties
   - Tests encryption of different content (same length)

6. **`test_metadata_decryption_timing`**

   - Verifies metadata decryption timing doesn't leak plaintext content
   - Tests decryption of equal-length ciphertexts

7. **`test_metadata_encryption_key_derivation_timing`**
   - Verifies key derivation timing independent of group ID
   - Tests HKDF with different group identifiers

### Statistical Analysis

Tests use **coefficient of variation (CV)** to measure timing consistency:

```
CV = std_dev / mean
```

- **CV < 0.1 (10%)**: Excellent constant-time behavior
- **CV < 0.3 (30%)**: Good (tolerates OS/hardware variance)
- **CV > 0.5 (50%)**: Potential timing leak

We use **CV < 0.3** as our threshold to balance:

- Detection of obvious timing leaks (e.g., early-exit patterns)
- Tolerance for normal OS scheduling and CPU frequency scaling
- Achievability with constant-time crypto primitives

## CI/CD Integration

### GitHub Actions Example

```yaml
- name: Run Timing Attack Tests
  run: |
    # Only run on dedicated runner with minimal load
    cargo test --lib core_mls::security::timing_tests -- \
      --test-threads=1 \
      --ignored \
      --nocapture
  # Mark as allowed to fail in shared CI
  continue-on-error: true
```

### Recommended Approach

1. **Local Development**: Run timing tests manually before security-critical PRs
2. **CI/CD**: Make timing tests opt-in or run on dedicated low-load runners
3. **Production**: Focus on using audited crypto libraries (library tests are sufficient)

## Threat Model Assumptions

Our timing attack mitigations assume:

1. **Attacker Model**: Network-level attacker measuring operation timing
2. **Protected Secrets**: Cryptographic keys, plaintext content, signature validity
3. **Public Information**: Message size, ciphertext length, operation type
4. **Environment**: General-purpose OS with unpredictable scheduling

We do NOT assume:

- Hardware-level attackers (cache, power, EM)
- Attacker with physical access to device
- Protection against OS/kernel timing leaks

## Best Practices for Developers

### ✅ DO

- Use constant-time crypto libraries (ChaCha20-Poly1305, Ed25519, HKDF)
- Use `subtle::ConstantTimeEq` for secret comparisons
- Validate timing test results before security-critical releases
- Document any intentional timing differences (e.g., different message sizes)

### ❌ DON'T

- Use `==` for comparing secrets (keys, passphrases, MACs)
- Use early-exit patterns in crypto validation code
- Implement custom cryptographic primitives
- Assume timing tests catch all timing leaks (they test libraries, not hardware)

## References

- [Timing Attacks on Implementations of Diffie-Hellman, RSA, DSS, and Other Systems](https://www.paulkocher.com/doc/TimingAttacks.pdf) - Paul Kocher (1996)
- [RustCrypto: Constant-Time Cryptography](https://github.com/RustCrypto/crypto-traits)
- [Subtle: Pure Rust constant-time comparison](https://docs.rs/subtle/latest/subtle/)
- [OWASP: Timing Attack](https://owasp.org/www-community/attacks/Timing_attack)

## Related Documentation

- [Threat Model](./threat-model.md) - STRIDE analysis and attack trees
- [Privacy Audit](./privacy-audit.md) - Privacy data flow analysis
- [Security Quick Reference](./security-quick-reference.md) - Developer security checklist
