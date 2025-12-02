# core_mls Architecture

## Purpose & Goals

`core_mls` implements Messaging Layer Security (MLS) for secure group messaging with:

- **Confidentiality**: End-to-end encrypted group messages
- **Authenticity**: Cryptographic verification of all messages and state changes
- **Forward Secrecy (FS)**: Compromise of current keys doesn't reveal past messages
- **Post-Compromise Security (PCS)**: Key rotation recovers security after compromise
- **Replay Resistance**: Per-sender sequence numbers prevent replay attacks
- **Tamper Detection**: All persisted data is authenticated with AEAD

## File Structure & Responsibilities

```
src/core_mls/
├─ mod.rs                 # Module entry, re-exports, CoreMls facade
├─ api.rs                 # High-level MlsHandle API for external use
├─ group.rs              # MlsGroup: group lifecycle and operations
├─ tree.rs               # MlsTree: ratchet tree math and path secrets
├─ welcome.rs            # Welcome message creation and import
├─ proposals.rs          # Proposal types (Add, Update, Remove, PSK)
├─ commit.rs             # Commit message verification and application
├─ encryption.rs         # HPKE operations and key schedule
├─ transport.rs          # MLS message transport via Router/RPC
├─ persistence.rs        # Secure AEAD-based group storage
├─ errors.rs             # Centralized MLS error types
├─ tests/
│  ├─ unit/              # Per-file unit tests
│  └─ integration/       # Cross-module integration tests
└─ bench/                # Performance benchmarks
```

## Core Data Flows

### 1. Group Creation Flow

```
User -> MlsHandle::create_group(name, members)
  -> MlsGroup::new() [generate tree, secrets]
  -> persistence::save_group() [AEAD encrypt & persist]
  -> CRDT::publish(GroupPublicInfo) [epoch, root_hash]
  -> DHT::put(GroupDiscoveryRecord) [optional]
```

### 2. Welcome Flow (Adding Members)

```
Owner -> MlsGroup::export_welcome(new_members)
  -> tree::generate_path_secrets()
  -> encryption::hpke_seal() [per member]
  -> WelcomeMessage [encrypted secrets + group_info]
  -> transport::send_welcome(peer_id, welcome)
  -> Router::SessionCommand::SendPlaintext

Recipient -> Router::SessionEvent::PlaintextFrame
  -> MlsHandle::handle_incoming(welcome)
  -> welcome::import_welcome(device_key)
  -> encryption::hpke_open() [decrypt secrets]
  -> MlsGroup [reconstructed state]
  -> persistence::save_group()
```

### 3. Commit Flow (State Changes)

```
Member -> MlsGroup::propose_add/remove/update()
  -> Proposal [signed with device_key]
  -> transport::send_proposal()

Committer -> MlsGroup::commit(proposals)
  -> tree::update_tree() [apply changes]
  -> encryption::derive_new_epoch_keys()
  -> CommitMessage [signed, includes path secrets]
  -> transport::send_commit()
  -> MlsGroup::apply_commit() [advance epoch]
  -> persistence::save_group()

Recipients -> Router::SessionEvent::PlaintextFrame
  -> MlsHandle::handle_incoming(commit)
  -> commit::verify_commit() [signature check]
  -> MlsGroup::apply_commit()
  -> tree::update_tree()
  -> encryption::derive_new_epoch_keys()
  -> persistence::save_group()
```

### 4. Application Message Flow

```
Sender -> MlsGroup::seal_application_message(plaintext)
  -> encryption::derive_app_keys(epoch)
  -> AEAD::encrypt(plaintext, seq_num)
  -> device_key::sign(ciphertext) [outer sig]
  -> transport::send_app(recipients, ciphertext)

Recipient -> Router::SessionEvent::PlaintextFrame
  -> MlsHandle::handle_incoming(app_msg)
  -> verify_signature() [outer sig]
  -> MlsGroup::open_application_message(ciphertext)
  -> check_replay(epoch, sender_idx, seq)
  -> encryption::derive_app_keys(epoch)
  -> AEAD::decrypt(ciphertext)
  -> plaintext
```

## Integration with Existing Subsystems

### Identity (core_identity)

**MLS Needs:**

- Device Ed25519 keypair for signing commits/proposals
- Device X25519 keypair for HPKE encryption/decryption
- Proof-of-possession during device addition

**API:**

```rust
// From core_identity
DeviceKey::sign(data: &[u8]) -> Signature
DeviceKey::x25519_secret() -> X25519Secret  // NEW
DeviceKey::x25519_public() -> X25519Public  // NEW
DeviceKey::prove_possession(challenge: &[u8]) -> Proof  // NEW
```

### Router (core_router)

**MLS Needs:**

- Transport for Welcome, Commit, Proposal, Application messages
- Anti-replay for transport-level message deduplication
- Session management for peer discovery

**API:**

```rust
// Via existing RpcProtocol
RpcProtocol::Call { method: "mls.welcome", params: [...] }
SessionCommand::SendPlaintext { to, data }

// Incoming
SessionEvent::PlaintextFrame { from, data } -> MlsHandle::handle_incoming()
```

**Message Envelope (CBOR/JSON):**

```json
{
  "type": "mls_welcome" | "mls_commit" | "mls_proposal" | "mls_app",
  "group_id": "hex...",
  "epoch": 42,
  "payload": "base64(...)",
  "sig": "base64(...)"  // device-key signature
}
```

### Store (core_store)

**MLS Needs:**

- Encrypted persistence for GroupSecrets (AEAD blob)
- Public metadata storage for discovery
- Atomic save on epoch changes

**API:**

```rust
// Via persistence.rs wrapper
save_group(store, group_id, group, passphrase?) -> Result<()>
load_group(store, group_id, passphrase?) -> Result<MlsGroup>

// Storage layout:
// keystore/groups/<group_id_hex>.mlsblob   [encrypted]
// keystore/groups/<group_id_hex>.json      [public metadata]
```

### CRDT (core_store)

**MLS Publishes:**

- Public group metadata only (never secrets!)
- `GroupPublicInfo { group_id, epoch, root_hash, signature }`
- Used for offline discovery and crash recovery

**API:**

```rust
// Publish on group create or epoch change
CRDT::update(GroupPublicInfo {
    group_id,
    epoch,
    public_root_hash,
    owner_signature,  // signed by creator's device-key
})
```

### DHT (core_dht)

**MLS Uses:**

- Optional: public group discovery records
- Bootstrap endpoints for Welcome retrieval
- All DHT records must be signed and verifiable

**API:**

```rust
// Optional discovery
DHT::put(GroupDiscoveryRecord {
    group_id,
    bootstrap_endpoints: [peer_id1, peer_id2],
    public_info_signature,
})
```

## Security Primitives & Invariants

### Cryptographic Primitives

1. **HPKE (X25519-based)**

   - Encrypt group secrets to new members in Welcome
   - AEAD: ChaCha20-Poly1305 or AES-256-GCM
   - KDF: HKDF-SHA256

2. **Signatures (Ed25519)**

   - Sign all commits and proposals
   - Sign public group info for CRDT/DHT
   - Verify before applying any state change

3. **AEAD for App Messages**

   - Derive keys from MLS key schedule
   - Never reuse keys across epochs
   - Include AAD: group_id || epoch || sender_idx || seq

4. **Persistence AEAD**
   - XChaCha20-Poly1305 or AES-256-GCM
   - KDF: Argon2id (passphrase-based) or HKDF
   - AAD: version || group_id || schema

### Security Invariants

1. **Epoch Monotonicity**

   - Commits MUST increment epoch
   - Reject messages with epoch < current_epoch
   - Reject messages with epoch > current_epoch + 1

2. **Replay Protection**

   - Per-sender sequence numbers for app messages
   - Track seen (group_id, epoch, sender_idx, seq) tuples
   - LRU cache with configurable capacity

3. **Proof-of-Possession**

   - Challenge-response when adding devices
   - Verify device owns X25519 private key
   - Use Router for challenge delivery

4. **Signature Verification**

   - All commits/proposals verified before application
   - Fail-closed on verification failure
   - Log authentication failures

5. **Authenticated Persistence**

   - All blobs AEAD-protected
   - Corrupted data → fail import, log incident
   - Never load partial/unverified data

6. **Forward Secrecy**

   - Delete old epoch keys after commit
   - Zeroize secrets on drop
   - Persist only current epoch secrets

7. **Post-Compromise Security**
   - Update operations rotate keys
   - Self-update on regular interval
   - Remove compromised members immediately

## Persistence Layout

### Encrypted Group Blob

```
File: keystore/groups/<group_id_hex>.mlsblob

Structure:
┌─────────────────────────────┐
│ Header (plaintext)          │
│  - version: u16             │
│  - group_id: [u8; 32]       │
│  - created_at: u64          │
│  - schema: u16              │
├─────────────────────────────┤
│ AEAD Ciphertext             │
│  - serialized GroupSecrets  │
│  - tree private state       │
│  - sequence counters        │
├─────────────────────────────┤
│ AEAD Tag (16 bytes)         │
└─────────────────────────────┘

AAD: header bytes
Key: derived from passphrase via Argon2id or from master key
```

### Public Metadata

```
File: keystore/groups/<group_id_hex>.json

{
  "group_id": "hex...",
  "epoch": 42,
  "created_at": 1234567890,
  "updated_at": 1234567899,
  "public_tree": {
    "num_leaves": 5,
    "root_hash": "hex...",
    "public_keys": ["base64...", ...]
  },
  "members": [
    {"identity": "hex...", "leaf_index": 0, "joined_at": 123456}
  ],
  "owner_signature": "base64..."
}
```

### Migration Strategy

- Always include `version` and `schema` in header
- Maintain `migrate_v{N}_to_v{N+1}()` functions
- Test migrations with fixtures for each version
- Keep backward-compatible public JSON for discovery

## Test Matrix

### Unit Tests (per-file)

- **tree.rs**: node insertion/removal, parent hash calculation, path secret generation
- **encryption.rs**: HPKE seal/open roundtrip, AEAD with AAD, key derivation
- **welcome.rs**: create_welcome/import_welcome roundtrip, multi-member
- **commit.rs**: commit creation, signature verification, epoch increment
- **persistence.rs**: save/load roundtrip, corruption detection, version migration

### Integration Tests

- `test_group_create_and_save_load` - full lifecycle
- `test_welcome_flow_two_devices` - create → welcome → import
- `test_add_member_with_commit` - proposal → commit → apply
- `test_remove_member_cannot_decrypt` - verify FS after removal
- `test_app_message_roundtrip` - seal → transport → open
- `test_replay_rejected` - duplicate seq numbers rejected
- `test_epoch_mismatch_rejected` - old epoch messages rejected

### Adversarial Tests

- Fuzz Welcome/Commit/App messages (bit flips)
- Corrupt AEAD tags in persistence
- Replay old commits after epoch advance
- Send messages from removed members
- Tamper with signatures

### Benchmarks

- `bench_seal_unseal_throughput` - ops/sec for app messages
- `bench_welcome_generation` - latency for N members
- `bench_commit_apply` - epoch transition latency
- `bench_tree_operations` - insertion/removal at scale

## Performance Considerations

1. **Welcome Generation**: O(N) HPKE operations for N members

   - Parallelize with Tokio tasks
   - Consider batching for large groups

2. **Tree Operations**: O(log N) for path computation

   - Cache parent hashes
   - Incremental updates only

3. **Persistence**: Async but atomic

   - Write to temp file
   - fsync + rename for atomicity
   - Background writes via Tokio

4. **Replay Cache**: LRU with bounded memory
   - Default: 10,000 entries
   - Configurable per-group

## Rollout Plan

### Phase 1: Foundation (Week 1)

- [ ] Scaffold module structure
- [ ] Implement persistence with AEAD
- [ ] Add tree.rs with basic math
- [ ] Unit tests for primitives

### Phase 2: Core Operations (Week 2)

- [ ] Implement Welcome create/import
- [ ] Add HPKE encryption wrapper
- [ ] Implement Proposal/Commit flows
- [ ] Integration tests for flows

### Phase 3: Transport Integration (Week 3)

- [ ] Add transport.rs with Router integration
- [ ] Implement MlsHandle API
- [ ] Add device PoP challenge-response
- [ ] End-to-end integration tests

### Phase 4: Discovery & Metadata (Week 4)

- [ ] CRDT public info publication
- [ ] DHT discovery records (optional)
- [ ] Group metadata queries
- [ ] Discovery integration tests

### Phase 5: Hardening (Week 5)

- [ ] Adversarial testing
- [ ] Fuzz testing suite
- [ ] Benchmarks and profiling
- [ ] Remove all production unwraps

### Phase 6: Production Ready (Week 6)

- [ ] Security audit prep
- [ ] Documentation and examples
- [ ] Metrics and monitoring
- [ ] Staging deployment

## Security Checklist

- [ ] All commits/proposals signature-verified
- [ ] Epoch monotonicity enforced
- [ ] Replay protection with seq numbers
- [ ] AEAD for all persistence
- [ ] Proof-of-possession for device addition
- [ ] Zeroize secrets on drop
- [ ] No panics in production paths
- [ ] Constant-time crypto operations
- [ ] Rate limiting on group operations
- [ ] Audit logging for security events

## Example Message Sequence

### Create Group & Send Message

```
1. Owner: create_group("mygroup", [alice, bob])
   → MlsGroup::new()
   → save_group()
   → publish GroupPublicInfo to CRDT

2. Owner: export_welcome([alice])
   → generate_path_secrets()
   → hpke_seal(alice_x25519_pub, secrets)
   → WelcomeMessage

3. Owner: send_welcome(alice_peer_id, welcome)
   → RpcProtocol::Call("mls.welcome", ...)
   → Router delivers

4. Alice: receives SessionEvent::PlaintextFrame
   → parse MLS envelope
   → import_welcome(alice_device_key)
   → hpke_open(alice_x25519_secret, ciphertext)
   → save_group()

5. Owner: seal_application_message("Hello!")
   → derive_app_keys(epoch)
   → aead_encrypt("Hello!", seq=0)
   → sign(ciphertext)
   → send_app([alice], ciphertext)

6. Alice: receives app message
   → verify_signature()
   → check_replay(epoch, sender_idx=0, seq=0)
   → derive_app_keys(epoch)
   → aead_decrypt(ciphertext)
   → "Hello!"
```
