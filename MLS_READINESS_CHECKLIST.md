# MLS Readiness Checklist

**Status**: 🔴 NOT READY - Critical security items must be addressed  
**Last Updated**: 2025-12-02  
**Target**: Address all MUST-FIX items before MLS integration

---

## Critical Issues (MUST-FIX) - Blocking MLS Integration

### 1. Device Proof-of-Possession ⚠️ CRITICAL

**Status**: ✅ COMPLETE  
**Priority**: P0 (Blocking)  
**Effort**: 1-2 days (COMPLETED)

**Problem**: Master can sign device binding, but nothing prevents attacker from binding a public key they don't control.

**Solution**:

```rust
// During device registration:
1. Master generates fresh challenge (random nonce + timestamp)
2. Device must sign challenge with private key
3. Master validates proof-of-possession before accepting binding
4. Challenge expires after 5 minutes
```

**Implementation**:

- ✅ `DeviceChallenge` struct with nonce, timestamp, device_id
- ✅ `ProofOfPossession` struct with challenge, signature, public key
- ✅ `DeviceKey::register_with_proof_of_possession()` - secure 3-step protocol
- ✅ `DeviceKey::validate_proof_of_possession()` - signature + expiry validation
- ✅ `DeviceKey::create_proof_of_possession()` - device-side proof creation
- ✅ `Keypair::from_public_key()` - verification-only keypairs
- ✅ Deprecated insecure `DeviceKey::generate()` (test-only)

**Files Modified**:

- ✅ `spacepanda-core/src/core_identity/device_key.rs`
- ✅ `spacepanda-core/src/core_identity/keypair.rs`

**Test Cases**:

- ✅ Valid proof-of-possession accepted (`test_proof_of_possession_valid`)
- ✅ Forged signature rejected (`test_proof_of_possession_forged_signature`)
- ✅ Expired challenge rejected (`test_proof_of_possession_expired_challenge`)
- ✅ Wrong device key detected (`test_proof_of_possession_wrong_device_key`)
- ✅ Cannot forge proof for others' keys (`test_proof_of_possession_cannot_forge_for_others_key`)
- ✅ Challenge message format validated (`test_challenge_message_format`)

**Test Results**: All 6 tests passing ✅

---

### 2. Handshake Replay & Partial-Handshake Timeouts ⚠️ CRITICAL

**Status**: ✅ COMPLETE  
**Priority**: P0 (Blocking)  
**Effort**: 0.5-1 day (COMPLETED)

**Problem**: Handshake replay and stalled partial handshakes can cause DoS.

**Solution**:

```rust
// Add to handshake processing:
1. Nonce/timestamp in handshake frames
2. Nonce window for replay detection
3. Per-connection handshake timeout (abort if stalled)
4. Tests for replayed handshake frames
```

**Implementation**:

- ✅ `HandshakeMetadata` struct with nonce, timestamp, seen_nonces
- ✅ Random nonce generation per handshake (64-bit)
- ✅ Nonce window with automatic cleanup (max 100 nonces)
- ✅ Handshake timeout (30 seconds) with automatic cleanup
- ✅ Replay detection via nonce tracking in HashSet
- ✅ Expired handshake rejection on data processing
- ✅ Timeout task spawned per handshake to cleanup stalled sessions

**Files Modified**:

- ✅ `spacepanda-core/src/core_router/session_manager.rs`
  - Added `HandshakeMetadata` with nonce window
  - Modified `SessionState::Handshaking` to include metadata
  - Added timeout spawn in `initiate_handshake`
  - Added replay detection in `handle_data`
  - Added expiration checks

**Test Cases**:

- ✅ Replayed handshake frame is rejected (`test_handshake_replay_detection`)
- ✅ Partial handshake times out and cleans up state (`test_handshake_timeout`)
- ✅ Expired handshake rejected (`test_expired_handshake_rejected`)
- ✅ Concurrent handshake attempts are handled safely (`test_concurrent_handshake_attempts`)
- ✅ Nonce window cleanup works correctly (`test_nonce_window_cleanup`)

**Test Results**: All 8 session_manager tests passing ✅

---

### 3. Keystore Integrity (AEAD + Encryption at Rest) ⚠️ CRITICAL

**Status**: ❌ Not Implemented  
**Priority**: P0 (Blocking)  
**Effort**: 1-2 days

**Problem**: Keystore exports/imports lack integrity protection and encryption at rest.

**Solution**:

```rust
// Wrap serialized keystore with:
1. AEAD (XChaCha20-Poly1305 or AES-256-GCM)
2. KDF for deriving keystore encryption key from passphrase
3. Version + magic header + AEAD tag
4. HMAC/checksum for integrity
```

**Files to Modify**:

- `spacepanda-core/src/core_identity/keystore/file_keystore.rs`
- `spacepanda-core/src/core_identity/keystore/mod.rs`

**Test Cases**:

- [ ] Corrupted AEAD tag causes import failure
- [ ] Corrupted ciphertext causes import failure
- [ ] Wrong passphrase causes import failure
- [ ] Migration from unencrypted keystore works
- [ ] Truncated keystore file detected

**Dependencies to Add**:

```toml
chacha20poly1305 = "0.10"  # Already present
argon2 = "0.5"             # KDF for passphrase
```

---

### 4. RPC Timeout Cancellation ⚠️ CRITICAL

**Status**: ❌ Not Implemented  
**Priority**: P0 (Blocking)  
**Effort**: 0.5-1 day

**Problem**: `make_call` spawns timeout task; if response arrives, both timeout and response race to remove pending entry.

**Solution**:

```rust
// Use tokio::select! or AbortHandle:
let timeout_handle = tokio::spawn(timeout_task);
// On response: timeout_handle.abort();
// Or: tokio::select! { response => ..., timeout => ... }
```

**Files to Modify**:

- `spacepanda-core/src/core_router/rpc_protocol.rs`

**Test Cases**:

- [ ] Fast response cancels timeout (no double-send)
- [ ] Timeout fires when no response (proper cleanup)
- [ ] Concurrent response+timeout handled safely
- [ ] No panic or race condition under load

---

### 5. Seen-Requests Proper LRU Eviction ⚠️ CRITICAL

**Status**: ❌ Not Implemented (timestamp-based eviction)  
**Priority**: P0 (Blocking)  
**Effort**: 1-2 days

**Problem**: Timestamp-based eviction is O(n log n) and has concurrency issues under heavy load.

**Solution**:

```rust
// Use hashlink::LruCache or sharded map:
use hashlink::LruCache;
use std::sync::Mutex;

struct SeenRequests {
    inner: Mutex<LruCache<RequestId, Instant>>,
}
```

**Files to Modify**:

- `spacepanda-core/src/core_router/rpc_protocol.rs`

**Test Cases**:

- [ ] Heavy concurrent insertion (1000+ threads)
- [ ] Eviction works correctly under lock
- [ ] Capacity limit enforced atomically
- [ ] No panic under concurrent stress

**Dependencies to Add**:

```toml
hashlink = "0.9"
```

---

### 6. Zeroize Sensitive Material ⚠️ CRITICAL

**Status**: ❌ Not Implemented  
**Priority**: P0 (Blocking)  
**Effort**: 0.5-1 day

**Problem**: Private keys left in memory after use (security risk).

**Solution**:

```rust
use zeroize::{Zeroize, ZeroizeOnDrop};
use secrecy::SecretVec;

#[derive(ZeroizeOnDrop)]
struct PrivateKey {
    bytes: SecretVec<u8>,
}
```

**Files to Modify**:

- `spacepanda-core/src/core_identity/keypair.rs`
- `spacepanda-core/src/core_identity/device_key.rs`
- `spacepanda-core/src/core_identity/master_key.rs`

**Test Cases**:

- [ ] Private key bytes zeroized on drop
- [ ] No private keys in Debug output
- [ ] No private keys in error messages
- [ ] No private keys in logs

**Dependencies to Add**:

```toml
zeroize = { version = "1.7", features = ["derive"] }
secrecy = "0.8"
```

---

### 7. CRDT Signature Verification Enforcement ⚠️ CRITICAL

**Status**: ⚠️ Partial (structural tests only)  
**Priority**: P0 (Blocking)  
**Effort**: 2-3 days

**Problem**: CRDT operations not validated cryptographically; malicious/forged deltas could corrupt state.

**Solution**:

```rust
// In CRDT apply/merge paths:
1. Verify signature on every delta
2. Reject unsigned deltas
3. Reject deltas with wrong pseudonym
4. Reject deltas with invalid signature
5. Performance benchmark for signature verification cost
```

**Files to Modify**:

- `spacepanda-core/src/core_store/crdt/*`
- `spacepanda-core/src/core_identity/*` (signature integration)

**Test Cases**:

- [ ] Forged signature in delta rejected
- [ ] Unsigned delta rejected
- [ ] Wrong pseudonym rejected
- [ ] Valid signed delta accepted
- [ ] Byzantine deltas don't corrupt state
- [ ] Benchmark: merge 1000 signed ops
- [ ] Fuzz test: random malformed signed deltas

---

## Medium Priority Improvements

### 8. Per-Peer Rate Limiting & Circuit Breakers

**Status**: ❌ Not Implemented  
**Priority**: P1  
**Effort**: 2-3 days

**Solution**: Add per-peer token bucket and circuit breaker to prevent flooding.

---

### 9. Structured Tracing + Metrics

**Status**: ⚠️ Partial (tracing present, metrics absent)  
**Priority**: P1  
**Effort**: 1-2 days

**Solution**: Add tracing spans and counters for security events (rejected frames, replay attempts, capacity rejections).

---

### 10. Test Harness Hardening

**Status**: ⚠️ Partial  
**Priority**: P2  
**Effort**: 1 day

**Solution**: Use deterministic RNG seeds for reproducible fuzz tests.

---

### 11. Benchmark Reproducibility

**Status**: ⚠️ Partial (benchmarks exist, seed/config missing)  
**Priority**: P2  
**Effort**: 0.5 day

**Solution**: Store benchmark seed, CI config, p50/p95/p99 latencies.

---

### 12. Key Migration Tooling

**Status**: ❌ Not Implemented  
**Priority**: P2  
**Effort**: 1-2 days

**Solution**: Add tool to import old keystore formats and re-sign devices.

---

## Low Priority / Nice-to-Have

- [ ] HMAC/encrypted keystore passphrase UX (OS keyring integration)
- [ ] Persistent snapshots for seen_requests across restarts
- [ ] Fuzzing with cargo-fuzz/AFL (parsers, handshake, CRDT)
- [ ] Property-based testing with proptest (CRDT invariants)
- [ ] CLI/test harness for network partition scenarios

---

## Code Quality Quick Wins

- [ ] **Eliminate `unwrap()`/`expect()` in non-test code** (grep and fix)
- [ ] **Use `zeroize` on all private key containers**
- [ ] **Extend named error constants pattern** (RPC → CRDT, DHT)
- [ ] **Lock granularity**: Consider per-shard locking for seen_requests, routing table
- [ ] **Background tasks lifecycle**: Ensure all spawned tasks have shutdown handles
- [ ] **Shard seen_requests map** if supporting 100k+ entries

---

## Recommended Tests to Add

### Identity Layer

- [ ] `test_device_registration_without_proof_rejected()`
- [ ] `test_device_binding_with_invalid_signature_rejected()`
- [ ] `test_replayed_proof_of_possession_rejected()`

### Router/Session

- [ ] `test_handshake_replay_rejected()`
- [ ] `test_partial_handshake_timeout()`
- [ ] `test_rpc_response_timeout_race_no_double_send()`

### CRDT

- [ ] `test_forged_signature_in_delta_rejected()`
- [ ] `test_unsigned_delta_rejected()`
- [ ] `test_byzantine_deltas_dont_corrupt_state()`

### RPC Protocol

- [ ] `test_seen_requests_heavy_concurrent_insertion()`
- [ ] `test_seen_requests_eviction_under_lock()`

### Keystore

- [ ] `test_corrupted_aead_tag_import_fails()`
- [ ] `test_wrong_passphrase_import_fails()`
- [ ] `test_truncated_keystore_detected()`

### Benchmarks

- [ ] `bench_crdt_merge_signed_ops_1000()`
- [ ] `bench_signature_verification_throughput()`

---

## MLS Integration Readiness Gate

**All P0 items MUST be complete before proceeding to `core_mls` implementation.**

### Readiness Criteria (Must be ✅)

- [x] Device proof-of-possession implemented and tested
- [x] Handshake replay & timeout handling + tests
- [ ] Keystore AEAD/HMAC integrity + encryption at rest
- [ ] CRDT signature verification enforced
- [ ] RPC timeout cancellation (no race conditions)
- [ ] LRU/sharded seen_requests with concurrency tests
- [ ] Zeroize all secrets in memory
- [ ] Benchmark: CRDT merge with signature verification
- [ ] Metrics/tracing for security events
- [ ] All new tests passing in CI

**Current Status**: 🟡 2/10 complete

---

## Next Actions (Prioritized)

### Week 1

1. **Day 1-2**: Implement device proof-of-possession + tests
2. **Day 3**: Add handshake replay/timeout handling + tests
3. **Day 4-5**: Implement keystore AEAD encryption + tests

### Week 2

4. **Day 1**: Add RPC timeout cancellation + race test
5. **Day 2-3**: Convert seen_requests to LRU + stress tests
6. **Day 4**: Add zeroize to all private keys
7. **Day 5**: CRDT signature verification + fuzz tests

### Week 3

8. **Day 1-2**: Benchmark CRDT merge with signatures
9. **Day 3-4**: Add tracing/metrics for security events
10. **Day 5**: Run fuzzing pass on parsers/handshake

---

## References

- **Critique Source**: Security audit (2025-12-02)
- **OpenMLS Integration Plan**: `MLS_INTEGRATION_PLAN.md`
- **Dependency Updates**: `DEPENDENCY_UPDATE_SUMMARY.md`

---

**Decision Point**: After completing all P0 items, reassess MLS integration timeline.
