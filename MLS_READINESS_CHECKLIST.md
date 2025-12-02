# MLS Readiness Checklist

**Status**: 🟡 APPROACHING READY - 6/7 P0 issues complete (86%)  
**Last Updated**: 2025-12-02  
**Progress**: 6/7 P0 issues complete (86%)  
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

**Status**: ✅ COMPLETE  
**Priority**: P0 (Blocking)  
**Effort**: 1-2 days (COMPLETED)

**Problem**: Keystore exports/imports lack integrity protection and encryption at rest.

**Solution**:

```rust
// Wrap serialized keystore with:
1. AEAD (AES-256-GCM) for confidentiality + integrity
2. KDF (Argon2id) for deriving encryption key from passphrase
3. Version + magic header ("SPKS0001") + random salt + random nonce
4. AEAD provides integrity (no separate HMAC needed)
```

**Implementation**:

- ✅ Encrypted file format: `[MAGIC:8][VERSION:1][SALT:16][NONCE:12][CIPHERTEXT+TAG]`
- ✅ AES-256-GCM AEAD for confidentiality and integrity
- ✅ Argon2id KDF (19 MiB memory, 2 iterations) for password-based key derivation
- ✅ Random salt per encryption (16 bytes, stored in file)
- ✅ Random nonce per encryption (12 bytes, stored in file)
- ✅ Magic header verification ("SPKS0001" for encrypted, "SPKS_RAW" for unencrypted)
- ✅ Version checking (FORMAT_VERSION = 1)
- ✅ Atomic file writes (write to .tmp, then rename)
- ✅ Password verification via AEAD tag validation

**Files Modified**:

- ✅ `spacepanda-core/src/core_identity/keystore/file_keystore.rs`
  - Complete rewrite of encrypt/decrypt with proper AEAD
  - Fixed critical security flaw: was using fixed nonce `[0u8; 12]`
  - Added structured encrypted file format
  - Added comprehensive error handling

**Test Cases**:

- ✅ Corrupted AEAD tag causes import failure (`test_corrupted_aead_tag`)
- ✅ Corrupted ciphertext causes import failure (`test_corrupted_ciphertext`)
- ✅ Wrong passphrase causes import failure (`test_wrong_passphrase`)
- ✅ Unencrypted mode still works (`test_unencrypted_mode`)
- ✅ Truncated keystore file detected (`test_truncated_file`)
- ✅ Invalid magic header detected (`test_invalid_magic_header`)
- ✅ Unsupported version detected (`test_unsupported_version`)
- ✅ Nonce uniqueness verified (`test_nonce_uniqueness`)
- ✅ Salt uniqueness verified (`test_salt_uniqueness`)
- ✅ Mixed encrypted/unencrypted rejected (`test_encrypted_keystore_rejects_unencrypted_file`)

**Test Results**: All 16 file_keystore tests passing ✅

**Security Properties Achieved**:

- ✅ Confidentiality: AES-256-GCM encryption
- ✅ Integrity: AEAD tag verification (detects tampering)
- ✅ Authenticity: Password verification via AEAD
- ✅ Replay prevention: Random nonce per encryption
- ✅ Rainbow table protection: Random salt + Argon2id KDF

---

### 4. RPC Timeout Cancellation ⚠️ CRITICAL

**Status**: ✅ COMPLETE  
**Priority**: P0 (Blocking)  
**Effort**: 0.5-1 day (COMPLETED)

**Problem**: `make_call` spawns timeout task; if response arrives, both timeout and response race to remove pending entry.

**Solution**:

```rust
// Use AbortHandle to cancel timeout when response arrives:
let timeout_task = tokio::spawn(timeout_logic);
let timeout_handle = timeout_task.abort_handle();
// Store handle in PendingRequest
// On response: timeout_handle.abort();
```

**Implementation**:

- ✅ Added `timeout_handle: AbortHandle` to `PendingRequest` struct
- ✅ Store `AbortHandle` when spawning timeout task
- ✅ Abort timeout task in `handle_response` when response arrives
- ✅ Timeout task only sends error if request still pending
- ✅ No race condition: only one of (response, timeout) delivers result

**Files Modified**:

- ✅ `spacepanda-core/src/core_router/rpc_protocol.rs`
  - Modified `PendingRequest` to include `timeout_handle`
  - Updated `make_call` to store abort handle
  - Updated `handle_response` to abort timeout on response

**Test Cases**:

- ✅ Fast response cancels timeout (no double-send) (`test_timeout_cancellation_on_fast_response`)
- ✅ Timeout fires when no response (proper cleanup) (`test_timeout_fires_when_no_response`)
- ✅ Concurrent response+timeout handled safely (`test_concurrent_response_and_timeout_race`)
- ✅ No panic or race condition under load (`test_multiple_concurrent_calls`)

**Test Results**: All 13 RPC protocol tests passing ✅

---

### 5. Seen-Requests Proper LRU Eviction ⚠️ CRITICAL

**Status**: ✅ COMPLETE  
**Priority**: P0 (Blocking)  
**Effort**: 1-2 days (COMPLETED)

**Problem**: Timestamp-based eviction is O(n log n) and has concurrency issues under heavy load.

**Solution**:

```rust
// Use hashlink::LruCache for O(1) operations:
use hashlink::LruCache;

// seen_requests: Arc<Mutex<LruCache<String, ()>>>
// LRU automatically evicts least-recently-used entries at capacity
// No background pruning task needed
```

**Implementation**:

- ✅ Replaced `HashMap<String, SeenRequest>` with `LruCache<String, ()>`
- ✅ Removed background pruning task (LRU handles eviction automatically)
- ✅ Simplified `new_with_config` from 5 params to 3 (removed TTL, prune_interval)
- ✅ O(1) insert, check, and evict operations
- ✅ Capacity-based eviction (no time-based pruning)
- ✅ Atomic check-and-insert under single mutex

**Files Modified**:

- ✅ `spacepanda-core/Cargo.toml` - Added `hashlink = "0.9"` dependency
- ✅ `spacepanda-core/src/core_router/rpc_protocol.rs`
  - Removed `SeenRequest` struct (no timestamp needed)
  - Changed `seen_requests` type to `LruCache<String, ()>`
  - Removed `prune_shutdown_tx`, `prune_task_handle` fields
  - Simplified `handle_request` (no capacity checks, LRU auto-evicts)
  - Removed `shutdown()` method (no background task)
  - Updated all tests to use new 3-parameter signature

**Test Cases**:

- ✅ LRU starts empty (`test_seen_requests_capacity_limit`)
- ✅ Capacity limit enforced via automatic eviction (`test_seen_requests_capacity_limit`)
- ✅ Oldest entry evicted when at capacity (`test_lru_eviction`)
- ✅ Evicted IDs can be reused (`test_lru_eviction`)
- ✅ No race conditions under concurrent load (`test_lru_no_race_conditions`)
- ✅ Heavy concurrent insertion (2000 tasks) (`test_heavy_concurrent_seen_requests`)
- ✅ Duplicate request detection works (`test_lru_no_race_conditions`)
- ✅ All existing RPC protocol tests still pass (13 updated tests)

**Test Results**: All 15 RPC protocol tests passing ✅

**Performance Improvements**:

- ✅ Insert: O(n log n) → O(1) (no timestamp sorting)
- ✅ Check: O(1) → O(1) (unchanged)
- ✅ Eviction: O(n log n) → O(1) (automatic LRU eviction)
- ✅ Memory: Reduced (no timestamp per entry, no background task)
- ✅ Concurrency: Improved (single lock, no background task coordination)

---

### 6. Zeroize Sensitive Material ⚠️ CRITICAL

**Status**: ✅ COMPLETE  
**Priority**: P0 (Blocking)  
**Effort**: 0.5-1 day (COMPLETED)

**Problem**: Private keys left in memory after use (security risk).

**Solution**:

```rust
use zeroize::{Zeroize, Zeroizing};

// Keypair automatically zeroizes secret on drop
impl Drop for Keypair {
    fn drop(&mut self) {
        self.secret.zeroize();
    }
}

// Password wrapped in Zeroizing for automatic cleanup
password: Option<Zeroizing<String>>
```

**Implementation**:

- ✅ Added `zeroize = { version = "1.7", features = ["derive"] }` dependency
- ✅ Keypair secret field zeroized on drop using `zeroize()` method
- ✅ FileKeystore password field wrapped in `Zeroizing<String>`
- ✅ `derive_key_from_password` returns `Zeroizing<Vec<u8>>`
- ✅ Debug impl for Keypair redacts secret (shows `<redacted>`)
- ✅ MasterKey and DeviceKey inherit zeroization (they wrap Keypair)

**Files Modified**:

- ✅ `spacepanda-core/Cargo.toml` - Added zeroize dependency
- ✅ `spacepanda-core/src/core_identity/keypair.rs`
  - Added `use zeroize::{Zeroize, ZeroizeOnDrop}`
  - Implemented `Drop` trait with `self.secret.zeroize()`
  - Debug impl already redacted secret keys
  - Added test for debug output redaction
  - Added test documenting zeroization behavior
- ✅ `spacepanda-core/src/core_identity/keystore/file_keystore.rs`
  - Added `use zeroize::Zeroizing`
  - Changed `password: Option<String>` to `Option<Zeroizing<String>>`
  - Updated `derive_key_from_password` to return `Zeroizing<Vec<u8>>`
  - Key material automatically zeroized on drop

**Test Cases**:

- ✅ Secret zeroized on drop (documented behavior) (`test_secret_zeroized_on_drop`)
- ✅ No private keys in Debug output (`test_debug_does_not_leak_secret`)
- ✅ FileKeystore tests pass with Zeroizing passwords (16 tests)
- ✅ Keypair tests pass with zeroization (11 tests)
- ✅ All existing identity tests still pass

**Test Results**: All 27 identity/keystore tests passing ✅

**Security Properties Achieved**:

- ✅ Keypair secrets zeroized on drop (compiler-enforced)
- ✅ Passwords zeroized on drop (Zeroizing wrapper)
- ✅ Derived encryption keys zeroized on drop
- ✅ Debug output never shows secrets
- ✅ No secret leakage in error messages (Debug redacted)

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
- [x] Keystore AEAD/HMAC integrity + encryption at rest
- [ ] CRDT signature verification enforced
- [x] RPC timeout cancellation (no race conditions)
- [ ] LRU/sharded seen_requests with concurrency tests
- [ ] Zeroize all secrets in memory
- [ ] Benchmark: CRDT merge with signature verification
- [ ] Metrics/tracing for security events
- [ ] All new tests passing in CI

**Current Status**: 🟡 4/10 complete

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
