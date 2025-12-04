# MLS State Persistence - Implementation Status

## ✅ Completed

### Infrastructure (100%)

- **FileStorageProvider Integration**: MlsService now supports optional file-based storage
- **PersistedGroupSnapshot**: Encrypted snapshot format with metadata (group_id, epoch, created_at)
- **Auto-save Hooks**: Groups automatically save after `create_group()` and `join_group()`
- **CLI Integration**: `load_manager()` initializes MlsService with `~/.spacepanda/mls_groups/`

### Serialization (100%)

- **save_group() Method**: Exports `PersistedGroupSnapshot` and writes to disk
- **Snapshot Format**: Bincode-serialized OpenMLS group state, encrypted with AES-256-GCM
- **File Naming**: `group-{hex_group_id}.snapshot` for easy identification
- **Verified Working**: Tested with channel creation - 878-byte snapshot file created successfully

### Code Changes

```rust
// src/core_mls/service.rs
pub struct MlsService {
    // ...existing fields...
    storage: Option<Arc<dyn StorageProvider>>,  // NEW
}

impl MlsService {
    pub fn with_storage(
        config: &CoreConfig,
        shutdown: ShutdownSignal,
        storage_dir: PathBuf,
    ) -> MlsResult<Self> {
        let storage = Arc::new(FileStorageProvider::new(storage_dir));
        // ...
    }

    pub async fn save_group(&self, group_id: &GroupId) -> MlsResult<()> {
        // Exports snapshot and saves to storage
    }
}
```

### Testing Results

```bash
# Test Case: Channel Creation with Persistence
$ cargo run --bin spacepanda -- init --name "Alice"
✅ Created identity.json (135 bytes)
✅ Created local store directories

$ cargo run --bin spacepanda -- channel create "general"
✅ Channel created: da665ea0-66c4-4435-8004-9378d511dc0c
✅ MLS snapshot saved: group-64613636...snapshot (878 bytes)

$ ls -la ~/.spacepanda/mls_groups/
✅ group-64613636356136363565613.snapshot (878 bytes)
```

**All 1157 tests passing** (1148 lib + 9 network)

---

## ⚠️ Not Yet Implemented

### Group Restoration (0%)

**Problem**: MLS groups save to disk but don't auto-load on service initialization.

**Current Behavior**:

```bash
# Session 1
$ cargo run -- channel create "general"
✅ Channel created: da665ea0-...

# Session 2 (new CLI invocation)
$ cargo run -- send da665ea0-... "Hello"
❌ PANIC: Group not found
   Error: MlsError::GroupNotFound("32323136...")
```

**Root Cause**: `load_persisted_groups()` is a skeleton implementation:

```rust
pub async fn load_persisted_groups(&self) -> MlsResult<usize> {
    // TODO: Add list_snapshots() to StorageProvider trait
    // TODO: Implement group restoration from snapshot bytes
    Ok(0)  // ← Returns 0, doesn't actually load anything
}
```

**Technical Blocker**: OpenMLS group deserialization is complex:

1. **Serialized State**: `PersistedGroupSnapshot.serialized_group` contains bincode-encoded `MlsGroup`
2. **OpenMLS Constraints**: `MlsGroup` requires specific initialization from `GroupEpoch`, `MlsGroupConfig`, crypto backend
3. **Unknown**: Whether OpenMLS supports direct deserialization or requires replay from genesis

### What's Needed

#### 1. Add `list_snapshots()` to StorageProvider trait

```rust
#[async_trait]
pub trait StorageProvider: Send + Sync {
    // ...existing methods...

    async fn list_snapshots(&self) -> std::io::Result<Vec<GroupId>>;
}
```

**FileStorageProvider Implementation**:

```rust
async fn list_snapshots(&self) -> std::io::Result<Vec<GroupId>> {
    let mut group_ids = Vec::new();
    let entries = tokio::fs::read_dir(&self.storage_dir).await?;

    while let Some(entry) = entries.next_entry().await? {
        if let Some(name) = entry.file_name().to_str() {
            if name.starts_with("group-") && name.ends_with(".snapshot") {
                // Parse hex group ID from filename
                let hex = &name[6..name.len()-9];
                if let Ok(bytes) = hex::decode(hex) {
                    group_ids.push(bytes);
                }
            }
        }
    }

    Ok(group_ids)
}
```

#### 2. Implement `restore_group()` method

```rust
async fn restore_group(
    &self,
    snapshot: PersistedGroupSnapshot,
) -> MlsResult<OpenMlsHandleAdapter> {
    // Deserialize OpenMLS group from snapshot.serialized_group
    // This is the complex part - need to research OpenMLS API

    // Option A: Direct deserialization (if supported)
    let mls_group: MlsGroup = bincode::deserialize(&snapshot.serialized_group)?;

    // Option B: Replay from stored epoch + operations
    // May require storing operation history in addition to snapshots

    // Create adapter and restore state
    let adapter = OpenMlsHandleAdapter::from_restored_group(
        mls_group,
        snapshot.epoch,
        self.crypto_provider.clone(),
    )?;

    Ok(adapter)
}
```

**Research Needed**: Check OpenMLS documentation for:

- `MlsGroup` serialization/deserialization support
- State restoration from persisted snapshots
- Required crypto provider re-initialization

#### 3. Update `load_persisted_groups()`

```rust
pub async fn load_persisted_groups(&self) -> MlsResult<usize> {
    let storage = match &self.storage {
        Some(s) => s,
        None => return Ok(0),
    };

    let group_ids = storage.list_snapshots().await
        .map_err(|e| MlsError::Storage(e.to_string()))?;

    let mut loaded = 0;
    for gid in group_ids {
        match storage.load_group_snapshot(&gid).await {
            Ok(snapshot) => {
                match self.restore_group(snapshot).await {
                    Ok(adapter) => {
                        self.groups.write().await.insert(gid.clone(), adapter);
                        loaded += 1;
                    }
                    Err(e) => warn!("Failed to restore group {}: {}", hex::encode(&gid), e),
                }
            }
            Err(e) => warn!("Failed to load snapshot for {}: {}", hex::encode(&gid), e),
        }
    }

    Ok(loaded)
}
```

#### 4. Call on CLI startup

```rust
// spacepanda-cli/src/main.rs - load_manager()
let mls_service = Arc::new(
    MlsService::with_storage(&config, shutdown, mls_storage_dir)?
);

// NEW: Load persisted groups
match mls_service.load_persisted_groups().await {
    Ok(count) => info!("Loaded {} persisted MLS groups", count),
    Err(e) => warn!("Failed to load persisted groups: {}", e),
}

let manager = ChannelManager::new(
    identity.clone(),
    mls_service,
    network,
);
```

---

## 🔬 Research Tasks

### OpenMLS Serialization API

- [ ] Check if `MlsGroup` implements `Serialize`/`Deserialize`
- [ ] Verify crypto provider re-attachment requirements
- [ ] Test round-trip: serialize → deserialize → use group
- [ ] Document any epoch/state consistency requirements

### Alternative: Event Sourcing

If direct deserialization is unsupported:

- [ ] Store MLS operations (Welcome, Commit, etc.) in event log
- [ ] Replay operations on startup to rebuild group state
- [ ] Evaluate performance impact for large groups

### Storage Provider Enhancements

- [ ] Add error recovery for corrupted snapshots
- [ ] Implement snapshot versioning for schema evolution
- [ ] Add periodic background snapshots (not just on operations)
- [ ] Consider snapshot rotation/pruning strategies

---

## 📊 Impact Assessment

**Current Limitation**: CLI is single-session only. Users lose all channels on exit.

**After Restoration**:

- ✅ Multi-session CLI usage
- ✅ Persistent encrypted channels
- ✅ Invite tokens work across sessions
- ✅ Production-ready local storage

**Performance Considerations**:

- Snapshot size: ~878 bytes per group (acceptable)
- Load time: Need to measure with 100+ groups
- Memory: Groups stay in memory after load (current design)

---

## 🧪 Testing Plan

### Unit Tests

```rust
#[tokio::test]
async fn test_group_save_and_restore() {
    let temp_dir = tempdir().unwrap();
    let service = MlsService::with_storage(config, shutdown, temp_dir.path());

    // Create and save group
    let gid = service.create_group("test").await.unwrap();
    service.save_group(&gid).await.unwrap();

    // Create new service instance (simulates restart)
    let service2 = MlsService::with_storage(config, shutdown, temp_dir.path());
    let loaded = service2.load_persisted_groups().await.unwrap();

    assert_eq!(loaded, 1);
    // Verify group is usable
    service2.send_message(&gid, b"test").await.unwrap();
}
```

### Integration Tests

```bash
# End-to-end persistence test
$ cargo run -- init --name "Alice"
$ cargo run -- channel create "general"
$ CHANNEL_ID=$(cargo run -- channel list | grep -oP '[0-9a-f-]{36}')
$ cargo run -- send "$CHANNEL_ID" "Message 1"

# Restart CLI (new process)
$ cargo run -- send "$CHANNEL_ID" "Message 2"  # Should work
$ cargo run -- channel list                    # Should show "general"
```

---

## 📝 Documentation Updates Needed

- [ ] Update `spacepanda-cli/README.md` - remove "No MLS State Persistence" limitation
- [ ] Add persistence troubleshooting section (corrupted snapshots, migration)
- [ ] Document `~/.spacepanda/` directory structure
- [ ] Add performance characteristics (snapshot size, load time)

---

## 🎯 Next Steps

1. **Research OpenMLS API** - Determine if direct deserialization is supported
2. **Implement `list_snapshots()`** - Simple directory listing
3. **Implement `restore_group()`** - Core deserialization logic
4. **Update `load_persisted_groups()`** - Iterate and restore all snapshots
5. **Add CLI startup call** - Load groups in `load_manager()`
6. **Write unit tests** - Verify round-trip save/load
7. **Integration test** - Multi-session CLI usage
8. **Update documentation** - Remove persistence limitation

**Estimated Effort**: 4-8 hours (depending on OpenMLS API complexity)

**Priority**: P0 - Blocking CLI production use
