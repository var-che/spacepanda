# SpacePanda Benchmark Suite

This document describes the benchmark suite for SpacePanda Core performance testing.

## Overview

The benchmark suite uses [Criterion.rs](https://github.com/bheisler/criterion.rs) to provide statistical analysis of performance characteristics across critical code paths.

## Running Benchmarks

### Run All Benchmarks

```bash
cargo bench
```

### Run Specific Benchmark Suite

```bash
cargo bench --bench rpc_protocol
cargo bench --bench dht_operations
cargo bench --bench crdt_operations
cargo bench --bench crypto_operations
```

### View HTML Reports

After running benchmarks, open `target/criterion/report/index.html` in your browser to view detailed performance reports with charts and statistical analysis.

## Benchmark Suites

### 1. RPC Protocol Benchmarks (`rpc_protocol.rs`)

**Status**: ✅ Working

Tests performance of the RPC protocol layer including:

#### `rpc_protocol_init`

- **default_config**: ~2.0 µs per initialization with default settings
- **custom_config**: ~1.99 µs per initialization with custom configuration

#### `rpc_protocol_lifecycle`

Tests protocol creation and shutdown with varying capacity limits:

- **1,000 capacity**: ~11.5 µs, ~86 Melem/s throughput
- **10,000 capacity**: ~11.5 µs, ~872 Melem/s throughput
- **50,000 capacity**: ~11.7 µs, ~4.3 Gelem/s throughput
- **100,000 capacity**: ~11.5 µs, ~8.7 Gelem/s throughput

#### `rpc_json_serialization`

Benchmarks JSON request serialization with varying payload sizes (100B - 50KB)

#### `rpc_json_deserialization`

Benchmarks JSON response deserialization with varying payload sizes

#### `rpc_frame_validation`

Tests frame size validation checks (1KB - 128KB)

#### `rpc_protocol_shutdown`

- **shutdown**: ~11.5 µs for graceful protocol shutdown

**Key Findings**:

- Protocol initialization is very fast (~2 µs)
- Lifecycle operations scale well with capacity
- Shutdown is clean and performant

### 2. DHT Operations Benchmarks (`dht_operations.rs`)

**Status**: ✅ Working

Tests performance of the Distributed Hash Table layer including:

#### `dht_key_generation`
- **single_key**: ~197 ns per DhtKey hash generation
- **batch/10**: ~1.84 µs, ~5.4 Melem/s throughput
- **batch/100**: ~18.9 µs, ~5.3 Melem/s throughput  
- **batch/500**: ~95.0 µs, ~5.3 Melem/s throughput
- **batch/1000**: ~191 µs, ~5.2 Melem/s throughput

#### `dht_key_hashing`
Tests Blake2b hashing performance with varying data sizes:
- **32 bytes**: ~206 ns, ~148 MiB/s
- **256 bytes**: ~380 ns, ~643 MiB/s
- **1 KiB**: ~1.38 µs, ~705 MiB/s
- **4 KiB**: ~5.44 µs, ~717 MiB/s
- **16 KiB**: ~21.6 µs, ~723 MiB/s

#### `dht_key_distance`
XOR distance calculation performance (critical for routing):
- **10 comparisons**: TBD
- **100 comparisons**: TBD
- **1,000 comparisons**: TBD
- **10,000 comparisons**: TBD

#### `dht_routing_table_init`
Routing table initialization with varying k-values (8, 16, 20, 32)

#### `dht_routing_insert`
Peer insertion into routing table (10, 100, 500, 1000 peers)

#### `dht_routing_lookup`
Find closest peers lookup performance (varying table sizes)

#### `dht_value_serialization`
DhtValue serialization with bincode (100B - 50KB)

#### `dht_peer_contact`
PeerContact creation and management

#### `dht_bucket_distribution`
Bucket distribution analysis with large peer sets (100 - 5000 peers)

**Key Findings**:
- DhtKey hash generation is extremely fast (~197 ns)
- Blake2b hashing scales well: ~700+ MiB/s for larger payloads
- Consistent ~5.2-5.4 Melem/s throughput for batch operations
- XOR distance calculation is a core primitive for routing efficiency

### 3. CRDT Operations Benchmarks (`crdt_operations.rs`)

**Status**: ⚠️ Partial (simplified to core operations)

Current working benchmarks:

- **JSON operations**: Serialization performance for CRDT values (10B - 10KB)
- **Timestamp generation**: SystemTime-based timestamp creation (single and batch)
- **HashMap operations**: Insert performance at various sizes (100 - 100K elements)

**Note**: Full LWWRegister benchmarks pending API signature resolution.

### 4. Crypto Operations Benchmarks (`crypto_operations.rs`)

**Status**: ⚠️ Partial (needs compatibility fixes)

Planned tests:

- Ed25519 keypair generation (single and batch)
- Ed25519 signing with varying message sizes
- Ed25519 verification performance
- X25519 key exchange (Diffie-Hellman)
- Noise protocol handshake (XX pattern)
- Noise transport mode encryption/decryption
- HKDF key derivation
- SHA256 hashing at various data sizes
- Concurrent cryptographic operations

**Note**: Currently blocked on trait compatibility between rand crate versions.

## Performance Baseline

### RPC Protocol (Current)

| Operation            | Time    | Throughput  |
| -------------------- | ------- | ----------- |
| Init (default)       | 2.0 µs  | N/A         |
| Init (custom)        | 2.0 µs  | N/A         |
| Lifecycle (100K cap) | 11.5 µs | 8.7 Gelem/s |
| Shutdown             | 11.5 µs | N/A         |

### DHT Operations (Current)

| Operation           | Size/Count | Time    | Throughput  |
| ------------------- | ---------- | ------- | ----------- |
| Key generation      | Single     | 197 ns  | N/A         |
| Key generation      | Batch 1000 | 191 µs  | 5.2 Melem/s |
| Blake2b hashing     | 1 KiB      | 1.38 µs | 705 MiB/s   |
| Blake2b hashing     | 16 KiB     | 21.6 µs | 723 MiB/s   |

### CRDT Operations (Current)

| Operation      | Size   | Time   |
| -------------- | ------ | ------ |
| JSON serialize | 10KB   | TBD    |
| Timestamp gen  | Single | ~100ns |
| HashMap insert | 10K    | TBD    |

## Future Work

### Priority 1: Fix Compatibility Issues

- [x] Update DHT benchmarks to match current `core_dht` API
- [ ] Resolve CRDT `LWWRegister::set` signature (4-arg vs 3-arg)
- [ ] Fix crypto rand crate compatibility (RngCore trait bounds)

### Priority 2: Expand Coverage

- [ ] Add network protocol benchmarks (Noise handshake, transport)
- [ ] Add storage layer benchmarks (LocalStore, snapshots)
- [ ] Add query/search index benchmarks
- [ ] Add synchronization protocol benchmarks

### Priority 3: Performance Targets

Once benchmarks are working, establish performance budgets:

- RPC request handling: < 10 µs
- DHT lookup: < 100 µs
- CRDT merge: < 1 ms
- Crypto sign/verify: < 50 µs
- Noise handshake: < 1 ms

## Development Guidelines

### Adding New Benchmarks

1. Create a new file in `benches/` or add to existing suite
2. Use `criterion_group!` and `criterion_main!` macros
3. Follow naming convention: `bench_<component>_<operation>`
4. Include throughput measurements where applicable
5. Test with varying input sizes to identify scaling behavior
6. Document expected performance characteristics

### Benchmark Best Practices

- Use `black_box()` to prevent compiler optimizations
- Include warm-up iterations for accurate measurement
- Test across realistic input ranges
- Isolate benchmarks to measure specific operations
- Use `Throughput::Bytes` or `Throughput::Elements` for rate measurements
- Run benchmarks on dedicated hardware when possible

## CI/CD Integration

**Note**: Not yet implemented. Planned features:

- Automatic benchmark runs on PR
- Performance regression detection
- Historical trend tracking
- Comparison with baseline/main branch

## Troubleshooting

### "Method not found" errors

- Check that methods are `pub` and not `#[cfg(test)]` only
- Verify module paths match current project structure

### Rand trait bound errors

- Ensure `rand` dependency versions match across `ed25519-dalek`, `x25519-dalek`
- May need to re-export or wrap RNG types

### Timeout during benchmarks

- Increase measurement time: `group.measurement_time(Duration::from_secs(30))`
- Reduce iteration count for expensive operations
- Consider using `--bench <name>` to run subsets

## References

- [Criterion.rs User Guide](https://bheisler.github.io/criterion.rs/book/)
- [Rust Performance Book](https://nnethercote.github.io/perf-book/)
- [Flamegraph profiling](https://github.com/flamegraph-rs/flamegraph)
