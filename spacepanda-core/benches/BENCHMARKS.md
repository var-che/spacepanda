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

**Status**: ✅ Working

Benchmarks LWWRegister CRDT performance including:

#### `crdt_lww_creation`

- **new_empty**: Empty register creation
- **with_value**: Initialize with value (~147 ns)
- **batch/10**: ~3.1 µs, ~3.2 Melem/s throughput
- **batch/100**: ~35 µs, ~2.9 Melem/s throughput
- **batch/1000**: ~372 µs, ~2.7 Melem/s throughput

#### `crdt_lww_set`

- **single_set**: Update register with new value and timestamp
- **sequential_updates**: Multiple updates (10, 100, 1000)

#### `crdt_lww_conflicts`

- **timestamp_wins**: Conflict resolution by timestamp
- **node_id_tiebreaker**: Conflict resolution by node ID when timestamps equal
- **conflicts**: Batch conflict resolution (10, 100, 1000 conflicts)

#### `crdt_vector_clock`

- **new**: VectorClock creation
- **increment**: Single node increment
- **merge**: Merging vector clocks (2, 5, 10, 20 nodes)

#### `crdt_lww_get`

- **get_value**: Read current register value

**Key Findings**:

- LWWRegister creation is very fast (~147 ns)
- Batch creation scales linearly with consistent ~2.7-3.2 Melem/s throughput
- Conflict resolution is deterministic and performant

### 4. Crypto Operations Benchmarks (`crypto_operations.rs`)

**Status**: ✅ Working

Tests cryptographic primitive performance including:

#### `crypto_ed25519_keygen`

- **generate_keypair**: Single Ed25519 keypair (~22 µs)
- **batch_generation/10**: ~220 µs, ~45K elem/s
- **batch_generation/50**: ~1.15 ms, ~43K elem/s
- **batch_generation/100**: ~2.26 ms, ~44K elem/s
- **batch_generation/500**: ~11.3 ms, ~44K elem/s

#### `crypto_ed25519_signing`

Message signing with varying sizes (32B - 16KB)

#### `crypto_ed25519_verification`

Signature verification with varying message sizes

#### `crypto_x25519_key_exchange`

- **dh_exchange**: Full Diffie-Hellman key exchange
- **batch_exchanges**: Batch DH operations (10, 50, 100, 500)

#### `crypto_noise_handshake`

- **full_handshake**: Complete Noise XX pattern handshake

#### `crypto_noise_transport`

- **encrypt_size**: ChaCha20Poly1305 encryption (64B - 16KB)

#### `crypto_hkdf_derivation`

- **derive_key**: HKDF-SHA256 key derivation
- **batch_derivation**: Batch key derivation (10, 50, 100, 500)

#### `crypto_sha256_hash`

SHA256 hashing with varying data sizes (32B - 64KB)

#### `crypto_concurrent_ops`

- **concurrent_signing**: Concurrent Ed25519 signing (10, 50, 100, 500 tasks)

**Key Findings**:

- Ed25519 keypair generation: ~22 µs per key, ~44K keys/sec batch
- Consistent throughput across batch sizes
- Crypto operations are suitable for high-performance scenarios

## Performance Baseline

### RPC Protocol (Current)

| Operation            | Time    | Throughput  |
| -------------------- | ------- | ----------- |
| Init (default)       | 2.0 µs  | N/A         |
| Init (custom)        | 2.0 µs  | N/A         |
| Lifecycle (100K cap) | 11.5 µs | 8.7 Gelem/s |
| Shutdown             | 11.5 µs | N/A         |

### DHT Operations (Current)

| Operation       | Size/Count | Time    | Throughput  |
| --------------- | ---------- | ------- | ----------- |
| Key generation  | Single     | 197 ns  | N/A         |
| Key generation  | Batch 1000 | 191 µs  | 5.2 Melem/s |
| Blake2b hashing | 1 KiB      | 1.38 µs | 705 MiB/s   |
| Blake2b hashing | 16 KiB     | 21.6 µs | 723 MiB/s   |

### CRDT Operations (Current)

| Operation              | Size/Count | Time    | Throughput  |
| ---------------------- | ---------- | ------- | ----------- |
| LWW creation           | Single     | 147 ns  | N/A         |
| LWW creation (batch)   | 1000       | 372 µs  | 2.7 Melem/s |
| Vector clock merge     | 10 nodes   | TBD     | N/A         |
| Conflict resolution    | 1000       | TBD     | N/A         |

### Crypto Operations (Current)

| Operation         | Size/Count | Time     | Throughput |
| ----------------- | ---------- | -------- | ---------- |
| Ed25519 keygen    | Single     | ~22 µs   | N/A        |
| Ed25519 keygen    | Batch 500  | ~11.3 ms | 44K keys/s |
| X25519 DH         | Single     | TBD      | N/A        |
| ChaCha20Poly1305  | 1 KiB      | TBD      | N/A        |
| SHA256 hash       | 16 KiB     | TBD      | N/A        |

## Future Work

### Priority 1: Fix Compatibility Issues ✅ COMPLETE

- [x] Update DHT benchmarks to match current `core_dht` API
- [x] Resolve CRDT `LWWRegister::set` signature (implemented full LWW benchmarks)
- [x] Fix crypto rand crate compatibility (resolved with rand 0.9 Rng trait)

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
