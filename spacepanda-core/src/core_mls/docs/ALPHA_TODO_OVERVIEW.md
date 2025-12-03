# Core MLS Alpha Release - Roadmap & TODO Overview

**Last Updated**: December 3, 2025  
**Status**: In Progress  
**Target**: Alpha Release v0.2.0

---

## 📋 Executive Summary

Based on external code review critique (see ALPHA_TODO.md), this document tracks the roadmap for addressing identified issues before the alpha release. The critique identified **9 major risk areas** and **15 priority tests** to implement.

### Current State

- ✅ OpenMLS integration complete
- ✅ Welcome message extraction working
- ✅ 1013 tests passing
- ✅ Basic E2E workflows functional
- ⚠️ Legacy code cleanup needed
- ⚠️ Security hardening required
- ⚠️ Additional test coverage needed

---

## 🎯 Priority Ranking (Critic's Recommendations)

### P0 - CRITICAL (Must Fix Before Alpha)

1. **Canonical MLS Implementation Decision** - Choose OpenMLS vs legacy
2. **Secrets-in-Memory Hardening** - Implement zeroization
3. **OpenMLS Provider Bridging** - Complete StorageProvider/CryptoProvider adapters

### P1 - HIGH (Alpha Blockers)

4. **Rate-Limiting & Bounded Caches** - Prevent DoS attacks
5. **CI & Security Tooling** - cargo-audit, clippy, deny
6. **Priority Test Suite** - 15 tests identified by critic

### P2 - MEDIUM (Post-Alpha, Pre-Beta)

7. **Fuzzing & Property Testing** - cargo-fuzz targets
8. **Cryptographic Audit** - Third-party review or peer review
9. **Documentation & API Stability** - Public API docs

### P3 - LOW (Beta/Production)

10. **Benchmarks & Profiling** - Performance baseline
11. **Secure Deploy Considerations** - Hardware keystores, etc.

---

## 📊 Roadmap Phases

### Phase 1: Foundation Fixes (Week 1) - IN PROGRESS

**Goal**: Address P0 critical items

- [x] Complete Welcome message extraction (DONE Dec 3)
- [x] Fix TODOs in openmls_engine.rs (DONE Dec 3)
- [ ] **Task 1.1**: Decide canonical implementation (OpenMLS)
  - Move legacy code to `legacy/` module with feature flag
  - Update all docs to reflect OpenMLS-only architecture
- [ ] **Task 1.2**: Implement secrets zeroization
  - Audit all secret-bearing structs
  - Add `zeroize` crate dependency
  - Wrap secrets in `Zeroizing<Vec<u8>>`
- [ ] **Task 1.3**: Complete OpenMLS provider adapters
  - Implement `OpenMlsStorageAdapter`
  - Implement `OpenMlsCryptoAdapter`
  - Add adapter unit tests

**Deliverable**: Clean OpenMLS-only architecture with secure secret handling

---

### Phase 2: Security Hardening (Week 2)

**Goal**: Address P1 high-priority security items

- [ ] **Task 2.1**: Rate-limiting implementation
  - Per-peer rate limit counters
  - LRU cache for replay prevention with capacity limits
  - Backpressure handling
- [ ] **Task 2.2**: CI/CD security pipeline
  - Add GitHub Actions workflow
  - Integrate cargo-audit
  - Integrate cargo-deny with deny.toml
  - Add clippy with `-D warnings`
- [ ] **Task 2.3**: Implement priority test suite
  - 15 tests from critic (see below)
  - Test coverage > 80%

**Deliverable**: Hardened implementation with CI/CD security checks

---

### Phase 3: Testing & Validation (Week 3)

**Goal**: Comprehensive test coverage

- [ ] **Task 3.1**: Fuzzing infrastructure
  - cargo-fuzz targets for parsing
  - Fuzz Welcome/HPKE envelopes
  - Fuzz commit validation
- [ ] **Task 3.2**: Property-based testing
  - proptest for tree invariants
  - proptest for commit validator
- [ ] **Task 3.3**: Concurrency & stress tests
  - Large-scale membership churn (500+ members)
  - Concurrent commit conflict resolution
  - Out-of-order message handling

**Deliverable**: Battle-tested implementation with fuzzing

---

### Phase 4: Documentation & Polish (Week 4)

**Goal**: Production-ready alpha release

- [ ] **Task 4.1**: API documentation
  - Document all public traits
  - Add usage examples
  - API stability guarantees
- [ ] **Task 4.2**: Benchmarking
  - Criterion benchmarks for key operations
  - Performance baselines documented
- [ ] **Task 4.3**: Code review preparation
  - Address all TODOs
  - Clean up test helpers
  - Prepare for external audit

**Deliverable**: Alpha v0.2.0 release

---

## 🧪 Priority Test Suite (15 Tests from Critique)

### Security Tests

- [ ] **Test 1**: Welcome HPKE replay/reuse protection
- [ ] **Test 2**: Partial/incomplete Welcome handling
- [ ] **Test 3**: Welcome with mismatched crypto suite
- [ ] **Test 8**: Fuzz test - corrupted envelope parsing
- [ ] **Test 10**: Key zeroization verification
- [ ] **Test 12**: HPKE nonce uniqueness
- [ ] **Test 13**: Commit signature validation edge cases
- [ ] **Test 14**: Recovery after disk corruption

### Operational Tests

- [ ] **Test 4**: Multi-device join + synchronization
- [ ] **Test 5**: Concurrent commit conflict resolution
- [ ] **Test 6**: Commit ordering & missing-proposal recovery
- [ ] **Test 11**: Per-peer rate-limiting

### Stress Tests

- [ ] **Test 7**: Large-scale tree stress (500+ members)
- [ ] **Test 15**: Bounded-memory seen-requests test

### Migration Tests

- [ ] **Test 9**: State migration compatibility

---

## 📝 Detailed Task Breakdown

### TASK 1.1: Canonical Implementation Decision

**Status**: 🟡 PLANNED  
**Owner**: TBD  
**Estimated**: 4 hours

**Actions**:

1. Create `src/core_mls/legacy/` directory
2. Move legacy modules:
   - `api.rs` → `legacy/api.rs`
   - `group.rs` → `legacy/group.rs`
   - `tree.rs` → `legacy/tree.rs`
   - `commit.rs` → `legacy/commit.rs`
   - `encryption.rs` → `legacy/encryption.rs`
3. Add feature flag in `Cargo.toml`:
   ```toml
   [features]
   default = []
   legacy-mls = []
   ```
4. Update `mod.rs` to gate legacy imports
5. Update all documentation

**Success Criteria**:

- All tests pass without `legacy-mls` feature
- Legacy code only compiled with feature flag
- Documentation clearly states OpenMLS-only

---

### TASK 1.2: Secrets Zeroization

**Status**: 🟡 PLANNED  
**Owner**: TBD  
**Estimated**: 8 hours

**Actions**:

1. Add `zeroize = "1.5"` to Cargo.toml
2. Audit secret-bearing types:
   - Group secrets
   - Path secrets
   - Derived keys
   - HPKE shared secrets
   - Signature keys
3. Wrap with `Zeroizing<Vec<u8>>`
4. Implement `Zeroize` trait where needed
5. Add test for zeroization (Test 10)

**Files to Modify**:

- `src/core_mls/crypto.rs`
- `src/core_mls/types.rs`
- `src/core_mls/engine/openmls_engine.rs`
- All provider implementations

**Success Criteria**:

- All secrets wrapped in `Zeroizing`
- Test verifies memory is zeroed on drop
- No clippy warnings about exposed secrets

---

### TASK 1.3: OpenMLS Provider Adapters

**Status**: 🟡 PLANNED  
**Owner**: TBD  
**Estimated**: 16 hours

**Actions**:

1. Create `src/core_mls/adapters/` directory
2. Implement `openmls_storage_adapter.rs`:
   - Implement OpenMLS `StorageProvider` trait
   - Delegate to `FileKeystore`
   - Add namespacing support
3. Implement `openmls_crypto_adapter.rs`:
   - Implement OpenMLS `CryptoProvider` trait
   - Delegate to existing crypto or OpenMlsRustCrypto
4. Add adapter tests (from ALPHA_TODO.md examples):
   - `storage_adapter_tests.rs`
   - `crypto_adapter_tests.rs`
   - `bootstrap_tests.rs`
5. Wire adapters into engine initialization

**Files to Create**:

- `src/core_mls/adapters/mod.rs`
- `src/core_mls/adapters/openmls_storage_adapter.rs`
- `src/core_mls/adapters/openmls_crypto_adapter.rs`
- `tests/storage_adapter_tests.rs`
- `tests/crypto_adapter_tests.rs`

**Success Criteria**:

- OpenMLS engine uses our storage
- All adapter tests pass
- No data loss on persistence roundtrip

---

### TASK 2.1: Rate-Limiting Implementation

**Status**: 🟡 PLANNED  
**Owner**: TBD  
**Estimated**: 12 hours

**Actions**:

1. Add dependencies:
   ```toml
   lru = "0.12"
   ```
2. Create `src/core_mls/rate_limit.rs`:
   - Per-peer rate limiter
   - Token bucket algorithm
   - Configurable limits
3. Integrate into message handlers
4. Add metrics/logging for rate limit hits
5. Implement Test 11 (per-peer rate limiting)

**Configuration**:

```rust
pub struct RateLimitConfig {
    pub max_requests_per_peer: usize,
    pub window_secs: u64,
    pub replay_cache_capacity: usize,
}
```

**Success Criteria**:

- Rate limiter prevents DoS
- Test 11 passes
- Legitimate traffic unaffected

---

### TASK 2.2: CI/CD Security Pipeline

**Status**: 🟡 PLANNED  
**Owner**: TBD  
**Estimated**: 6 hours

**Actions**:

1. Create `.github/workflows/ci.yml`:
   ```yaml
   - cargo test
   - cargo clippy -- -D warnings
   - cargo fmt -- --check
   - cargo audit
   - cargo deny check
   ```
2. Create `deny.toml` for cargo-deny
3. Set up dependency version minimums
4. Add badge to README

**Success Criteria**:

- All CI checks pass
- Security advisories caught
- Clippy warnings = 0

---

## 🔍 Current Status Summary

### Completed ✅

- OpenMLS integration
- Welcome message extraction
- Member join time tracking
- GroupInfo export
- 8 E2E integration tests
- 1013 tests passing

### In Progress 🟡

- Roadmap development (this document)
- Test suite planning

### Blocked/Pending 🔴

- None currently

---

## 📅 Timeline

| Week   | Phase                  | Key Deliverables                                 |
| ------ | ---------------------- | ------------------------------------------------ |
| Week 1 | Foundation Fixes       | OpenMLS-only architecture, zeroization, adapters |
| Week 2 | Security Hardening     | Rate limiting, CI/CD, priority tests             |
| Week 3 | Testing & Validation   | Fuzzing, property tests, stress tests            |
| Week 4 | Documentation & Polish | API docs, benchmarks, alpha release              |

**Target Alpha Release**: End of Week 4 (December 31, 2025)

---

## 🎯 Success Metrics

### Code Quality

- [ ] Test coverage > 80%
- [ ] Zero clippy warnings
- [ ] Zero cargo-audit vulnerabilities
- [ ] All critic's tests implemented

### Security

- [ ] All secrets zeroized
- [ ] Rate limiting functional
- [ ] Fuzzing finds no crashes
- [ ] CI security checks passing

### Performance

- [ ] Benchmarks documented
- [ ] No regressions vs baseline
- [ ] 500+ member groups supported

### Documentation

- [ ] All public APIs documented
- [ ] Usage examples provided
- [ ] Migration guide complete

---

## 🚨 Risk Register

| Risk                             | Probability | Impact | Mitigation                         |
| -------------------------------- | ----------- | ------ | ---------------------------------- |
| OpenMLS API changes              | Low         | High   | Pin exact version, test thoroughly |
| Legacy code removal breaks tests | Medium      | Medium | Feature flag, gradual migration    |
| Performance regression           | Low         | Medium | Benchmark before changes           |
| Security audit delays            | Medium      | High   | Start early, prioritize P0 items   |

---

## 📚 References

- **Main Critique**: `ALPHA_TODO.md`
- **Implementation Status**: `IMPLEMENTATION_STATUS.md`
- **Project Goals**: `UPDATED_GOALS.md`
- **OpenMLS Integration**: `OPENMLS_INTEGRATION.md`

---

## 🔄 Change Log

| Date        | Author       | Changes                        |
| ----------- | ------------ | ------------------------------ |
| Dec 3, 2025 | AI Assistant | Initial roadmap creation       |
| Dec 3, 2025 | AI Assistant | Added detailed task breakdowns |

---

**Next Actions**:

1. Start with TASK 1.3 (OpenMLS Provider Adapters) - most critical
2. Implement Test 1 (Welcome replay protection)
3. Begin zeroization audit (TASK 1.2)
