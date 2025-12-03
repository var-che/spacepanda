# MLS Persistence Strategy

## Research Summary: OpenMLS Persistence Pattern

### How OpenMLS Handles State

OpenMLS uses the **StorageProvider** trait pattern for persistence. Key findings:

1. **No Built-in Serialization**: `MlsGroup` does NOT provide `save()` or `serialize()` methods
2. **Storage Provider Pattern**: OpenMLS uses `StorageProvider::load()` to reconstruct groups
3. **Automatic Persistence**: OpenMLS writes to storage automatically during state transitions
4. **Components Stored**:
   - `PublicGroup` (ratchet tree, member list, group context)
   - `GroupEpochSecrets` (encryption secrets per epoch)
   - `SignatureKeyPairs` (member's signing keys)

### OpenMLS `load()` Pattern

```rust
pub fn load<Storage: StorageProvider>(
    storage: &Storage,
    group_id: &GroupId,
) -> Result<Option<MlsGroup>, Storage::Error> {
    let public_group = PublicGroup::load(storage, group_id)?;
    let group_epoch_secrets = storage.group_epoch_secrets(group_id)?;
    // Reconstructs MlsGroup from components
}
```

## Decision: Hybrid Persistence Strategy

### Chosen Approach: **OpenMLS Native + Snapshot Fallback**

We will use a **hybrid approach** combining OpenMLS's native storage with our own snapshot system:

#### Primary: OpenMLS Native Storage (Via StorageProvider)

**Pros:**

- ✅ Automatic persistence during commits/updates
- ✅ Efficient - only changed components updated
- ✅ Works with OpenMLS's internal state machine
- ✅ No manual serialization needed
- ✅ Supports epoch-based key rotation automatically

**Cons:**

- ⚠️ Requires implementing full StorageProvider trait
- ⚠️ State is fragmented across multiple storage calls
- ⚠️ Harder to atomically snapshot entire group state

**Implementation:**

```rust
// OpenMLS calls our StorageProvider automatically:
group.add_members(...)
  → StorageProvider::write_group_epoch_secrets()
  → StorageProvider::write_tree()
  → etc.
```

#### Secondary: Snapshot for Backup/Export

**Use Cases:**

- Atomic state export for CRDT layer
- Disaster recovery
- Migration between storage backends
- Debugging/inspection

**Approach:**

```rust
pub struct GroupSnapshot {
    pub group_id: GroupId,
    pub epoch: u64,
    pub public_tree_bytes: Vec<u8>,    // export_ratchet_tree()
    pub epoch_secrets_bytes: Vec<u8>,  // from StorageProvider
    pub group_context_bytes: Vec<u8>,  // export_group_context()
    pub members: Vec<MemberInfo>,
}
```

### Implementation Plan

#### Phase 1: StorageProvider Integration (PRIORITY 3.1)

Enhance our existing `FileStorage` to implement OpenMLS's `StorageProvider`:

**File:** `src/core_mls/storage/openmls_storage.rs` (NEW)

```rust
use openmls::storage::{StorageProvider, traits::*};

/// Adapter that makes our FileStorage compatible with OpenMLS
pub struct OpenMlsStorageAdapter {
    inner: Arc<FileStorage>,
}

impl StorageProvider for OpenMlsStorageAdapter {
    // Implement all required storage operations:
    // - group_epoch_secrets(), write_group_epoch_secrets()
    // - tree(), write_tree()
    // - signature_key_pair(), write_signature_key_pair()
    // - etc.
}
```

**Benefits:**

- OpenMLS handles persistence automatically
- No manual snapshot/diff logic needed
- Atomic writes at OpenMLS operation level

#### Phase 2: Atomic Snapshot API (PRIORITY 3.2)

Add snapshot capability for backup/export:

**File:** `src/core_mls/state/snapshot.rs` (NEW)

```rust
impl OpenMlsEngine {
    /// Export current group state as atomic snapshot
    pub async fn export_snapshot(&self) -> MlsResult<GroupSnapshot> {
        let group = self.group.read().await;

        // Export all components atomically
        GroupSnapshot {
            group_id: self.group_id().clone(),
            epoch: group.epoch().as_u64(),
            public_tree_bytes: group.export_ratchet_tree().tls_serialize_detached()?,
            epoch_secrets_bytes: self.provider.group_epoch_secrets(&group_id)?,
            group_context_bytes: group.export_group_context().tls_serialize_detached()?,
            members: self.members().await?,
        }
    }

    /// Restore group from snapshot
    pub async fn import_snapshot(
        snapshot: GroupSnapshot,
        provider: Arc<OpenMlsRustCrypto>,
    ) -> MlsResult<Self> {
        // Write all components to storage
        // Then use MlsGroup::load() to reconstruct
    }
}
```

#### Phase 3: Write-Ahead Log (PRIORITY 3.3)

Ensure crash recovery during multi-step operations:

**File:** `src/core_mls/storage/wal.rs` (NEW)

```rust
/// Write-ahead log for atomic multi-operation commits
pub struct WriteAheadLog {
    log_dir: PathBuf,
}

impl WriteAheadLog {
    /// Begin transaction - write intent to log
    pub async fn begin(&self, group_id: &GroupId, operation: WalOperation) -> MlsResult<WalId>;

    /// Commit transaction - mark as complete
    pub async fn commit(&self, wal_id: WalId) -> MlsResult<()>;

    /// Abort transaction - rollback
    pub async fn abort(&self, wal_id: WalId) -> MlsResult<()>;

    /// Recover incomplete transactions on startup
    pub async fn recover(&self) -> MlsResult<Vec<WalEntry>>;
}

#[derive(Serialize, Deserialize)]
pub enum WalOperation {
    AddMembers { key_packages: Vec<Vec<u8>> },
    RemoveMembers { leaf_indices: Vec<u32> },
    SendMessage { plaintext: Vec<u8> },
    ProcessCommit { commit_bytes: Vec<u8> },
}
```

**Usage Pattern:**

```rust
impl OpenMlsEngine {
    pub async fn add_members(&self, key_packages: Vec<Vec<u8>>) -> MlsResult<Vec<u8>> {
        // 1. Write WAL entry
        let wal_id = self.wal.begin(&self.group_id, WalOperation::AddMembers {
            key_packages: key_packages.clone()
        }).await?;

        // 2. Perform operation (OpenMLS auto-persists via StorageProvider)
        let result = self.group.write().await.add_members(...)?;

        // 3. Commit WAL entry
        self.wal.commit(wal_id).await?;

        Ok(result)
    }
}
```

### Storage Layout

```
spacepanda_data/
  mls/
    groups/
      {group_id}/
        openmls/              # OpenMLS native storage
          public_group.bin    # Ratchet tree + context
          epoch_secrets/      # One file per epoch
            epoch_0.bin
            epoch_1.bin
          signatures/         # Signature keypairs
            {key_id}.bin

        snapshots/            # Our snapshot system
          latest.snapshot     # Most recent full snapshot
          epoch_5.snapshot    # Archived snapshots

        wal/                  # Write-ahead log
          {txn_id}.wal        # Pending transactions
          committed/          # Completed (for debugging)
```

### Why This Strategy?

1. **Best of Both Worlds**:

   - Native OpenMLS = automatic, efficient, correct
   - Snapshots = atomic exports for CRDT integration
   - WAL = crash recovery for multi-step operations

2. **Aligns with OpenMLS Design**:

   - Uses intended StorageProvider pattern
   - No custom crypto/serialization needed
   - Future-proof for OpenMLS updates

3. **Meets Our Needs**:
   - ✅ Atomic persistence (via StorageProvider + WAL)
   - ✅ Crash recovery (via WAL)
   - ✅ CRDT integration (via snapshots)
   - ✅ Efficient storage (OpenMLS handles diffs internally)

## Implementation Checklist

- [ ] **Priority 3.1**: Implement `OpenMlsStorageAdapter` wrapping `FileStorage`
- [ ] **Priority 3.2**: Add `export_snapshot()` and `import_snapshot()` methods
- [ ] **Priority 3.3**: Implement `WriteAheadLog` with begin/commit/abort/recover
- [ ] **Testing**: Multi-crash scenario tests
- [ ] **Testing**: Snapshot roundtrip tests
- [ ] **Testing**: WAL recovery tests
- [ ] **Integration**: Wire up to `OpenMlsEngine` constructor

## Open Questions

1. **Epoch Retention**: How many old epochs should we keep for message decryption?

   - Recommendation: Configurable, default 10 epochs (~10 commits back)

2. **Snapshot Frequency**: When to create snapshots?

   - On every Nth commit (e.g., every 10 epochs)
   - On explicit save request
   - Before risky operations (member removal)

3. **WAL Cleanup**: When to delete completed WAL entries?
   - After successful commit + confirmed persistence
   - Keep last N for debugging
   - Periodic cleanup task

## References

- OpenMLS docs: https://openmls.tech/book/
- StorageProvider trait: `openmls::storage::StorageProvider`
- Our storage traits: `src/core_mls/traits/storage.rs`
