# MLS Implementation Roadmap

## Current Status ✅

**Phase 0: Foundation - COMPLETE**

- [x] Architecture documented (ARCHITECTURE.md)
- [x] Module structure defined
- [x] Core types implemented (GroupId, MlsConfig, MemberInfo, GroupPublicInfo)
- [x] Error types with proper conversions
- [x] Basic tests passing (8/8)
- [x] Clean compilation with no errors

**Files Created:**

- `mod.rs` - Module entry point with exports
- `types.rs` - Core MLS types (134 lines, 7 tests)
- `errors.rs` - Comprehensive error types (113 lines, 3 tests)
- `ARCHITECTURE.md` - Complete design documentation
- `ROADMAP.md` - This file

## Next Steps

### Phase 1: Secure Persistence (Week 1, Days 1-2)

**Goal**: Implement AEAD-based encrypted storage for group secrets

**Tasks**:

- [ ] Create `persistence.rs` with AEAD encryption
  - [ ] `EncryptedGroupBlob` struct with versioned header
  - [ ] `save_group()` with Argon2id KDF
  - [ ] `load_group()` with integrity verification
  - [ ] `export_backup()` / `import_backup()` with passphrase
  - [ ] Migration helpers `migrate_v{N}_to_v{N+1}()`
- [ ] Add tests (target: 10+ tests)
  - [ ] Round-trip save/load
  - [ ] Corrupted tag detection
  - [ ] Corrupted ciphertext detection
  - [ ] Wrong passphrase rejection
  - [ ] Version migration v0 → v1
  - [ ] Schema validation

**Dependencies**:

- `aes-gcm` or `chacha20poly1305` (already in Cargo.toml)
- `argon2` (already in Cargo.toml)

**Acceptance Criteria**:

- All persistence tests pass
- Fuzzing shows no panics on corrupted input
- Performance: save/load < 10ms for typical group

### Phase 2: Ratchet Tree (Week 1, Days 3-5)

**Goal**: Implement MLS tree operations and path secret generation

**Tasks**:

- [ ] Create `tree.rs` with MlsTree struct

  - [ ] Node representation (leaf vs parent)
  - [ ] `add_leaf()` / `remove_leaf()` / `update_leaf()`
  - [ ] `generate_path_secrets()` for HPKE encryption
  - [ ] `root_hash()` for group authentication
  - [ ] Tree serialization (public parts only)

- [ ] Add tests (target: 15+ tests)
  - [ ] Insert/remove operations
  - [ ] Parent hash computation
  - [ ] Path secret generation
  - [ ] Root hash determinism
  - [ ] Edge cases (empty tree, single node)

**Dependencies**:

- Understanding of MLS tree math (see RFC 9420)
- Hash function (Blake3 or SHA-256)

**Acceptance Criteria**:

- All tree tests pass
- Property tests for tree invariants
- Benchmarks show O(log N) performance

### Phase 3: Encryption & HPKE (Week 2, Days 1-2)

**Goal**: Implement HPKE seal/unseal and key schedule

**Tasks**:

- [ ] Create `encryption.rs`

  - [ ] `hpke_seal()` / `hpke_open()` wrappers
  - [ ] `derive_app_keys()` from MLS key schedule
  - [ ] AEAD operations with proper AAD
  - [ ] Key zeroization on drop

- [ ] Add tests (target: 8+ tests)
  - [ ] HPKE roundtrip
  - [ ] AAD binding verification
  - [ ] Test vectors from RFC
  - [ ] Key derivation determinism

**Dependencies**:

- `hkdf` (already in Cargo.toml)
- Consider using OpenMLS primitives or `hpke` crate

**Acceptance Criteria**:

- HPKE test vectors pass
- Constant-time operations verified
- No key material leaks in tests

### Phase 4: Welcome Messages (Week 2, Days 3-5)

**Goal**: Create and import Welcome messages for new members

**Tasks**:

- [ ] Create `welcome.rs`

  - [ ] `WelcomeMessage` struct
  - [ ] `create_welcome_for_members()`
  - [ ] `import_welcome()` with device key
  - [ ] GroupInfo encryption/decryption

- [ ] Add tests (target: 10+ tests)
  - [ ] Single member welcome
  - [ ] Multi-member welcome
  - [ ] Invalid welcome rejection
  - [ ] Epoch validation

**Dependencies**:

- `encryption.rs` (HPKE)
- `tree.rs` (path secrets)

**Acceptance Criteria**:

- Welcome roundtrip works
- Can add 100 members in < 1s
- Invalid welcomes rejected safely

### Phase 5: Proposals & Commits (Week 3, Days 1-3)

**Goal**: Implement group state change operations

**Tasks**:

- [ ] Create `proposals.rs`

  - [ ] `Proposal` enum (Add, Update, Remove, PSK)
  - [ ] Signature verification helpers
  - [ ] Proposal serialization

- [ ] Create `commit.rs`

  - [ ] `CommitMessage` struct
  - [ ] `verify_commit()` with signature check
  - [ ] Epoch advancement logic

- [ ] Add tests (target: 12+ tests)
  - [ ] Proposal creation/verification
  - [ ] Commit creation/verification
  - [ ] Epoch monotonicity enforcement
  - [ ] Unauthorized commit rejection

**Dependencies**:

- `core_identity` for signature verification
- `tree.rs` for state updates

**Acceptance Criteria**:

- All proposal/commit tests pass
- Fuzzing shows no bypasses
- Proper error messages

### Phase 6: MlsGroup Core (Week 3, Days 4-5 + Week 4, Days 1-2)

**Goal**: Implement high-level group operations

**Tasks**:

- [ ] Create `group.rs`

  - [ ] `MlsGroup` struct with full state
  - [ ] `new()` for group creation
  - [ ] `export_welcome()` wrapper
  - [ ] `apply_proposal()` / `commit()` / `apply_commit()`
  - [ ] `seal_application_message()` / `open_application_message()`
  - [ ] Replay protection (sequence numbers)

- [ ] Add tests (target: 20+ tests)
  - [ ] Group creation
  - [ ] Add member flow
  - [ ] Remove member flow
  - [ ] Update (self-rotation)
  - [ ] Application message encryption
  - [ ] Replay attack prevention

**Dependencies**:

- All previous modules

**Acceptance Criteria**:

- Full group lifecycle works
- Removed members can't decrypt
- Replay attacks blocked

### Phase 7: Transport Integration (Week 4, Days 3-5)

**Goal**: Wire MLS to Router/RPC layer

**Tasks**:

- [ ] Create `transport.rs`

  - [ ] `MlsTransport` wrapper for Router
  - [ ] `send_welcome()` / `send_commit()` / `send_app()`
  - [ ] Message envelope format (JSON/CBOR)
  - [ ] Integration with `SessionCommand`

- [ ] Add tests (target: 8+ tests)
  - [ ] Message serialization
  - [ ] Router integration (mocked)
  - [ ] Envelope parsing

**Dependencies**:

- `core_router` integration

**Acceptance Criteria**:

- Messages delivered via Router
- Envelope format documented
- Integration tests pass

### Phase 8: API Facade (Week 5, Days 1-2)

**Goal**: Create high-level MlsHandle API

**Tasks**:

- [ ] Create `api.rs`

  - [ ] `MlsHandle` struct
  - [ ] `create_group()` / `join_group_via_welcome()`
  - [ ] `propose_add()` / `commit()`
  - [ ] `send_app_message()` / `get_group_info()`
  - [ ] Internal group state management

- [ ] Update `mod.rs` exports

  - [ ] Public API facade
  - [ ] Hide internal modules

- [ ] Add tests (target: 15+ tests)
  - [ ] Full workflows via API
  - [ ] Concurrent operations
  - [ ] Error handling

**Dependencies**:

- All core modules

**Acceptance Criteria**:

- Clean, ergonomic API
- All workflows work via MlsHandle
- Async-safe

### Phase 9: Discovery Integration (Week 5, Days 3-4)

**Goal**: Integrate with CRDT and DHT for group discovery

**Tasks**:

- [ ] CRDT integration

  - [ ] Publish `GroupPublicInfo` on create/commit
  - [ ] Signature verification for public info
  - [ ] Merge semantics for group metadata

- [ ] DHT integration (optional)

  - [ ] Store group discovery records
  - [ ] Bootstrap endpoint resolution

- [ ] Add tests (target: 6+ tests)
  - [ ] Public info publication
  - [ ] Discovery via CRDT
  - [ ] DHT lookup

**Dependencies**:

- `core_store` (CRDT)
- `core_dht` (optional)

**Acceptance Criteria**:

- Groups discoverable offline
- No secrets in public data
- Signatures verified

### Phase 10: Hardening & Testing (Week 5, Day 5 + Week 6, Days 1-3)

**Goal**: Security testing, fuzzing, benchmarks

**Tasks**:

- [ ] Adversarial tests

  - [ ] Fuzz all message types
  - [ ] Bit-flip attacks
  - [ ] Signature tampering
  - [ ] Replay attacks
  - [ ] Epoch confusion

- [ ] Performance benchmarks

  - [ ] `bench_seal_unseal_throughput`
  - [ ] `bench_welcome_generation`
  - [ ] `bench_commit_apply`
  - [ ] `bench_tree_operations`

- [ ] Security audit

  - [ ] Scan for `unwrap()` / `expect()`
  - [ ] Review crypto usage
  - [ ] Check zeroization
  - [ ] Verify constant-time ops

- [ ] Documentation
  - [ ] API documentation
  - [ ] Security guide
  - [ ] Integration examples

**Acceptance Criteria**:

- No panics in fuzzing (24h run)
- All benchmarks within targets
- Security checklist complete
- Snyk scan passes

### Phase 11: Integration & Deployment (Week 6, Days 4-5)

**Goal**: Final integration and staging deployment

**Tasks**:

- [ ] Full integration tests

  - [ ] Multi-device scenarios
  - [ ] Network partitions
  - [ ] Crash recovery

- [ ] Metrics and monitoring

  - [ ] Tracing for critical events
  - [ ] Metrics for group operations
  - [ ] Alert on security events

- [ ] Staging deployment
  - [ ] Deploy to test environment
  - [ ] Monitor for issues
  - [ ] Performance validation

**Acceptance Criteria**:

- All tests pass
- Metrics working
- Staging stable for 72h

## Test Coverage Goals

| Module      | Unit Tests | Integration Tests | Fuzzing | Total    |
| ----------- | ---------- | ----------------- | ------- | -------- |
| types       | 7          | -                 | -       | **7** ✅ |
| errors      | 3          | -                 | -       | **3** ✅ |
| persistence | 10         | 2                 | Yes     | 12       |
| tree        | 15         | -                 | Yes     | 15       |
| encryption  | 8          | -                 | Yes     | 8        |
| welcome     | 10         | 3                 | Yes     | 13       |
| proposals   | 6          | -                 | Yes     | 6        |
| commit      | 6          | 2                 | Yes     | 8        |
| group       | 20         | 5                 | Yes     | 25       |
| transport   | 8          | 4                 | -       | 12       |
| api         | 15         | 10                | -       | 25       |
| **Total**   | **108**    | **26**            | **7**   | **141**  |

## Performance Targets

- **save_group**: < 10ms
- **load_group**: < 5ms
- **seal_message**: < 1ms (100k msg/sec)
- **open_message**: < 1ms
- **welcome_100_members**: < 1s
- **commit_apply**: < 50ms
- **tree_insert**: < 0.1ms (O(log N))

## Security Targets

- **No panics**: 24h fuzz with 0 crashes
- **No unwraps**: Production code paths clean
- **Constant time**: Crypto ops verified
- **Key zeroization**: All secrets cleared on drop
- **Snyk scan**: 0 high/critical vulnerabilities
- **Audit**: External security review passed

## Dependencies to Add

Current (already in Cargo.toml):

- ✅ `openmls` 0.7.1
- ✅ `openmls_rust_crypto` 0.4
- ✅ `openmls_basic_credential` 0.4
- ✅ `aes-gcm` 0.10
- ✅ `argon2` 0.5
- ✅ `hkdf` 0.12
- ✅ `blake3` 1.5

May need to add:

- `hpke` (if not using OpenMLS primitives)
- `cargo-fuzz` (dev dependency for fuzzing)

## Progress Tracking

**Week 1** (Dec 2-6):

- [x] Phase 0: Foundation
- [ ] Phase 1: Persistence
- [ ] Phase 2: Tree

**Week 2** (Dec 9-13):

- [ ] Phase 3: Encryption
- [ ] Phase 4: Welcome

**Week 3** (Dec 16-20):

- [ ] Phase 5: Proposals/Commits
- [ ] Phase 6: MlsGroup

**Week 4** (Dec 23-27):

- [ ] Phase 6: MlsGroup (cont.)
- [ ] Phase 7: Transport

**Week 5** (Dec 30 - Jan 3):

- [ ] Phase 8: API
- [ ] Phase 9: Discovery
- [ ] Phase 10: Hardening

**Week 6** (Jan 6-10):

- [ ] Phase 10: Hardening (cont.)
- [ ] Phase 11: Integration

## Notes

- Each phase builds on previous phases
- Tests must pass before moving to next phase
- Security review required before production
- Keep architecture doc updated as we iterate
- Run Snyk scan after each phase

## Questions / Decisions Needed

1. **HPKE Implementation**: Use OpenMLS primitives or separate `hpke` crate?

   - Recommendation: Start with OpenMLS, can swap later

2. **Persistence Backend**: In-memory for dev, what for production?

   - Recommendation: File-based initially, integrate with existing storage later

3. **Discovery**: DHT required or CRDT sufficient?

   - Recommendation: CRDT first, DHT optional later

4. **OpenMLS Usage**: Direct or wrapper?
   - Current: Minimal wrapper, can expand as needed
