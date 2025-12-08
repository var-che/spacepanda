# Fuzz Testing Guide

This document describes the fuzz testing infrastructure for SpacePanda and how to run fuzzing campaigns.

## Overview

Fuzz testing (fuzzing) automatically generates random inputs to test software robustness against malformed, unexpected, or malicious data. SpacePanda uses [cargo-fuzz](https://github.com/rust-fuzz/cargo-fuzz) with [libFuzzer](https://llvm.org/docs/LibFuzzer.html) for coverage-guided fuzzing.

## Fuzz Targets

We have **6 fuzz targets** covering critical parsing and cryptographic code paths:

### 1. `fuzz_mls_message_parsing`

**Location**: `spacepanda-core/fuzz/fuzz_targets/fuzz_mls_message_parsing.rs`

**Purpose**: Tests resilience of message parsing against malformed inputs

**What it tests**:

- `EncryptedEnvelope::from_bytes()` - Bincode deserialization
- `MlsEnvelope::from_bytes()` - Transport envelope parsing
- `MlsEnvelope::from_json()` - JSON deserialization
- `SenderData::from_bytes()` - Fixed-length sender metadata parsing

**Why it matters**: Message parsing is exposed to untrusted network input. Malformed messages could cause crashes, panics, or memory corruption.

### 2. `fuzz_snapshot_parsing`

**Location**: `spacepanda-core/fuzz/fuzz_targets/fuzz_snapshot_parsing.rs`

**Purpose**: Tests resilience of group snapshot deserialization

**What it tests**:

- `GroupSnapshot::from_bytes()` - Bincode deserialization
- `serde_json::from_str::<GroupSnapshot>()` - JSON deserialization

**Why it matters**: Snapshots persist group state. Corrupted snapshots could cause data loss or security breaches.

### 3. `fuzz_group_blob_parsing`

**Location**: `spacepanda-core/fuzz/fuzz_targets/fuzz_group_blob_parsing.rs`

**Purpose**: Tests resilience of encrypted persistence format

**What it tests**:

- `EncryptedGroupBlob::from_bytes()` - Custom binary format parsing
- `decrypt_group_state()` - AEAD decryption of malformed data

**Why it matters**: Persistent data could be tampered with on disk. Robust parsing prevents privilege escalation or DoS.

### 4. `fuzz_metadata_encryption` ⭐ NEW

**Location**: `spacepanda-core/fuzz/fuzz_targets/fuzz_metadata_encryption.rs`

**Purpose**: Tests metadata encryption/decryption robustness

**What it tests**:

- `MetadataEncryption::new()` - HKDF key derivation with arbitrary group IDs
- `encrypt()` - ChaCha20-Poly1305 encryption with edge case plaintexts
- `decrypt()` - Resilience against malformed/tampered ciphertexts
- Large plaintext handling (DoS resistance up to 1MB)

**Why it matters**: Metadata encryption protects sensitive channel names/topics. Fuzzing ensures no leaks or crashes with unusual inputs.

### 5. `fuzz_sealed_sender` ⭐ NEW

**Location**: `spacepanda-core/fuzz/fuzz_targets/fuzz_sealed_sender.rs`

**Purpose**: Tests sealed sender cryptography robustness

**What it tests**:

- `derive_sender_key()` - HKDF key derivation with arbitrary key material
- `seal_sender()` - ChaCha20-Poly1305 encryption of sender identities
- `unseal_sender()` - Decryption with wrong keys/epochs (negative testing)
- Large sender identity handling (DoS resistance up to 10KB)

**Why it matters**: Sealed sender provides privacy. Fuzzing ensures no identity leaks or crashes with malicious inputs.

### 6. `fuzz_target_1`

**Location**: `spacepanda-core/fuzz/fuzz_targets/fuzz_target_1.rs`

**Purpose**: Generic fuzz target (placeholder for future use)

**Status**: Not currently used

## Prerequisites

### 1. Install Nightly Rust

Fuzzing requires Rust nightly for sanitizer support:

```bash
rustup install nightly
rustup default nightly
```

Or use nightly only for fuzzing:

```bash
rustup toolchain install nightly
```

### 2. Install cargo-fuzz

```bash
cargo install cargo-fuzz
```

### 3. Verify Installation

```bash
cargo fuzz --version
# Should print: cargo-fuzz 0.12.x
```

## Running Fuzz Tests

### List Available Targets

```bash
cd spacepanda-core
cargo fuzz list
```

Output:

```
fuzz_group_blob_parsing
fuzz_metadata_encryption
fuzz_mls_message_parsing
fuzz_sealed_sender
fuzz_snapshot_parsing
fuzz_target_1
```

### Run a Single Fuzz Target

Basic fuzzing (runs indefinitely until crash or Ctrl+C):

```bash
cd spacepanda-core
cargo fuzz run fuzz_mls_message_parsing
```

### Run with Time Limit

Run for 60 seconds:

```bash
cargo fuzz run fuzz_mls_message_parsing -- -max_total_time=60
```

### Run with Iteration Limit

Run for 1 million iterations:

```bash
cargo fuzz run fuzz_metadata_encryption -- -runs=1000000
```

### Run All Targets (Recommended)

Fuzz each target for 5 minutes:

```bash
#!/bin/bash
TARGETS=(
  "fuzz_mls_message_parsing"
  "fuzz_snapshot_parsing"
  "fuzz_group_blob_parsing"
  "fuzz_metadata_encryption"
  "fuzz_sealed_sender"
)

for target in "${TARGETS[@]}"; do
  echo "=== Fuzzing $target for 5 minutes ==="
  cargo fuzz run "$target" -- -max_total_time=300 -jobs=4
done
```

### Parallel Fuzzing

Use multiple CPU cores (4 jobs):

```bash
cargo fuzz run fuzz_metadata_encryption -- -jobs=4
```

## Analyzing Results

### Crash Artifacts

When fuzzer finds a crash, it saves the input to:

```
spacepanda-core/fuzz/artifacts/fuzz_<target>/
```

Example:

```
fuzz/artifacts/fuzz_mls_message_parsing/crash-abc123...
```

### Reproduce a Crash

```bash
cargo fuzz run fuzz_mls_message_parsing fuzz/artifacts/fuzz_mls_message_parsing/crash-abc123...
```

### Debug a Crash

```bash
# Build in debug mode
cargo fuzz run --dev fuzz_mls_message_parsing fuzz/artifacts/fuzz_mls_message_parsing/crash-abc123...

# Or use gdb/lldb
gdb --args target/x86_64-unknown-linux-gnu/release/fuzz_mls_message_parsing fuzz/artifacts/.../crash-abc123...
```

### Coverage Reports

Generate HTML coverage report:

```bash
cargo fuzz coverage fuzz_metadata_encryption
```

View coverage:

```bash
# Install coverage tools
cargo install cargo-binutils
rustup component add llvm-tools-preview

# Generate report
cargo fuzz coverage fuzz_metadata_encryption
llvm-cov show target/x86_64-unknown-linux-gnu/release/fuzz_metadata_encryption \
    --format=html \
    --instr-profile=fuzz/coverage/fuzz_metadata_encryption/coverage.profdata \
    > coverage.html
```

## CI/CD Integration

### GitHub Actions Example

```yaml
name: Fuzz Testing

on:
  schedule:
    # Run nightly
    - cron: "0 2 * * *"
  workflow_dispatch:

jobs:
  fuzz:
    runs-on: ubuntu-latest
    strategy:
      matrix:
        target:
          - fuzz_mls_message_parsing
          - fuzz_snapshot_parsing
          - fuzz_group_blob_parsing
          - fuzz_metadata_encryption
          - fuzz_sealed_sender

    steps:
      - uses: actions/checkout@v3

      - name: Install Rust Nightly
        uses: actions-rs/toolchain@v1
        with:
          toolchain: nightly
          override: true

      - name: Install cargo-fuzz
        run: cargo install cargo-fuzz

      - name: Run Fuzzer (10 minutes)
        run: |
          cd spacepanda-core
          cargo fuzz run ${{ matrix.target }} -- -max_total_time=600

      - name: Upload Artifacts
        if: failure()
        uses: actions/upload-artifact@v3
        with:
          name: fuzz-artifacts-${{ matrix.target }}
          path: spacepanda-core/fuzz/artifacts/
```

## Interpreting Fuzzer Output

### Example Output

```
INFO: Running with entropic power schedule (0xFF, 100).
INFO: Seed: 1234567890
INFO: Loaded 1 modules   (123456 inline 8-bit counters): 123456 [0x..., 0x...)
INFO: Loaded 1 PC tables (123456 PCs): 123456 [0x...,0x...)
INFO: -max_total_time=60 seconds
#0      READ units: 1
#1000   NEW    cov: 456 ft: 789 corp: 12 exec/s: 100 rss: 128Mb
#2000   NEW    cov: 478 ft: 812 corp: 15 exec/s: 120 rss: 132Mb
...
Done 50000 runs in 60 second(s)
```

**Key Metrics**:

- `cov`: Code coverage (higher is better)
- `ft`: Features covered (higher is better)
- `corp`: Corpus size (number of interesting inputs)
- `exec/s`: Executions per second (higher is faster)
- `rss`: Memory usage

### What to Look For

✅ **Good Signs**:

- No crashes after millions of iterations
- Coverage plateaus (no new code paths found)
- Fast execution (>1000 exec/s)

⚠️ **Warning Signs**:

- Crashes or hangs
- Slow execution (<100 exec/s) - suggests performance issues
- Coverage stuck at low % - suggests dead code or unreachable paths

❌ **Critical Issues**:

- Panics or crashes
- Memory leaks (rss steadily increasing)
- Integer overflows
- Buffer overruns

## Best Practices

### 1. Start with Short Runs

Don't run indefinitely on first try:

```bash
# 60 second smoke test
cargo fuzz run fuzz_metadata_encryption -- -max_total_time=60
```

### 2. Use Corpus Seeds

Provide seed inputs for better coverage:

```bash
mkdir -p fuzz/corpus/fuzz_metadata_encryption
echo "test input" > fuzz/corpus/fuzz_metadata_encryption/seed1
cargo fuzz run fuzz_metadata_encryption
```

### 3. Minimize Crashes

Reduce crash inputs to minimal size:

```bash
cargo fuzz tmin fuzz_metadata_encryption fuzz/artifacts/fuzz_metadata_encryption/crash-abc123...
```

### 4. Deduplicate Crashes

Fuzzer may find many crashes from same bug. Minimize and group them:

```bash
cargo fuzz cmin fuzz_metadata_encryption
```

### 5. Long-Running Campaigns

For thorough testing, run for hours/days:

```bash
# 24 hours
cargo fuzz run fuzz_mls_message_parsing -- -max_total_time=86400 -jobs=4
```

## Limitations

### What Fuzzing Tests

✅ **DOES TEST**:

- Parsing robustness (malformed inputs)
- Unexpected data types or values
- Buffer overflows
- Integer overflows
- Panics and crashes
- Memory leaks (with AddressSanitizer)

❌ **DOES NOT TEST**:

- Logical correctness (use unit tests)
- Cryptographic soundness (use security audits)
- Performance (use benchmarks)
- Concurrency bugs (use Loom or ThreadSanitizer)
- Semantic errors (requires formal verification)

### Coverage Limitations

Fuzzing struggles with:

- Checksums (hard to guess valid checksums randomly)
- Complex state machines (needs guided inputs)
- Cryptographic MACs (authentication prevents fuzzing)

For these, use:

- Structured fuzzing (deserialize to valid struct, then mutate)
- Property-based testing (QuickCheck, Proptest)
- Manual security review

## Troubleshooting

### "Error: Could not compile"

Ensure you're using nightly Rust:

```bash
rustup default nightly
# Or
cargo +nightly fuzz run ...
```

### "Out of Memory"

Reduce memory limit:

```bash
cargo fuzz run fuzz_metadata_encryption -- -rss_limit_mb=2048
```

### Slow Fuzzing (<100 exec/s)

Optimize fuzz target:

- Avoid expensive operations (I/O, allocation)
- Return early on invalid input
- Use `#[inline]` for hot paths

### Nix Environment Issues

If using Nix, fuzzing might conflict with Nix's Rust setup:

```bash
# Exit nix develop shell
exit

# Run fuzzer outside nix
rustup default nightly
cd spacepanda-core
cargo fuzz run fuzz_metadata_encryption
```

## Security Policy

### Reporting Fuzz Crashes

If fuzzing finds a security-sensitive crash:

1. **DO NOT** file a public GitHub issue
2. **DO** report via GitHub Security Advisory
3. Include:
   - Fuzz target name
   - Crash artifact file
   - Stack trace
   - Severity assessment

### Triaging Crashes

**Critical** (fix immediately):

- Remote code execution
- Authentication bypass
- Encryption key leaks

**High** (fix in next release):

- Denial of service (crash)
- Data corruption
- Privacy leaks

**Medium** (fix when possible):

- Panics on malformed local data
- Performance degradation

**Low** (document as known issue):

- Panics on impossible inputs (e.g., > RAM size)

## References

- [cargo-fuzz Documentation](https://rust-fuzz.github.io/book/cargo-fuzz.html)
- [libFuzzer Tutorial](https://llvm.org/docs/LibFuzzer.html)
- [Fuzzing in Rust](https://rust-fuzz.github.io/book/)
- [American Fuzzy Lop (AFL)](https://github.com/rust-fuzz/afl.rs)

## Related Documentation

- [Timing Attack Mitigations](./timing-attack-mitigations.md) - Constant-time cryptography
- [Threat Model](./threat-model.md) - STRIDE analysis
- [Privacy Audit](./privacy-audit.md) - Privacy data flows
- [Security Quick Reference](./security-quick-reference.md) - Security checklist
