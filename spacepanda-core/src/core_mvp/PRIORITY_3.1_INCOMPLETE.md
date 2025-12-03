# Priority 3.1: Key Package Management - INCOMPLETE

## Status: PARTIAL IMPLEMENTATION

**Date**: December 3, 2025

## What Was Implemented

✅ **API Layer** (Complete):

1. Added `generate_key_package()` to `GroupProvider` trait
2. Implemented in `CoreMlsAdapter`
3. Implemented in `MockGroupProvider`
4. Added `generate_key_package()` to `ChannelManager`
5. Updated all tests to use the new API

✅ **Tests Updated** (Complete):

- Removed `generate_key_package()` helper function from tests
- Updated `test_full_join_flow` to use `ChannelManager.generate_key_package()`
- Updated `test_multiple_message_exchange`
- Updated `test_three_party_group`
- Updated `test_invite_creation_with_real_key_package`
- Removed `#[ignore]` attributes from all 3 blocked tests

## What's Still Broken

❌ **Provider Isolation Issue**:

The tests still fail with `NoMatchingKeyPackage` error because:

1. **Problem**: Each OpenMLS operation creates a NEW provider instance

   - `MlsService.generate_key_package()` creates provider A
   - `MlsService.join_group()` creates provider B
   - Provider B can't find the KeyPackageBundle stored in provider A

2. **Root Cause**: `OpenMlsEngine::create_group()` and `OpenMlsEngine::join_from_welcome()` both create their own `Arc<OpenMlsRustCrypto>` providers

3. **What Needs to Happen**:
   - MlsService needs ONE shared provider
   - OpenMlsEngine needs to accept a provider parameter
   - All operations use the same provider instance

## Current State

```rust
// MlsService now has:
pub struct MlsService {
    provider: Arc<OpenMlsRustCrypto>,  // ✅ Added
    // ...
}

// generate_key_package uses self.provider ✅
pub async fn generate_key_package(&self, identity: Vec<u8>) -> MlsResult<Vec<u8>> {
    let provider = self.provider.clone();  // ✅ Uses shared provider
    // ... stores KeyPackageBundle in provider
}

// BUT join_group still creates a new provider ❌
pub async fn join_group(&self, ...) -> MlsResult<GroupId> {
    let adapter = OpenMlsHandleAdapter::join_from_welcome(...).await?;
    // ❌ This calls OpenMlsEngine::join_from_welcome which creates NEW provider
}
```

## Test Results

```
test core_mvp::tests::full_join_flow::test_full_join_flow ... FAILED

Step 4: Bob joins channel using Welcome
Error: Mls(InvalidMessage("Failed to stage welcome: NoMatchingKeyPackage"))
```

## Required Fix - Priority 3.2

### 1. Update OpenMlsEngine to Accept Provider

```rust
// src/core_mls/engine/openmls_engine.rs
impl OpenMlsEngine {
    pub async fn create_group(
        group_id: GroupId,
        identity: Vec<u8>,
        config: MlsConfig,
        provider: Arc<OpenMlsRustCrypto>,  // NEW PARAMETER
    ) -> MlsResult<Self> {
        // Use provided provider instead of creating new one
        // let provider = Arc::new(OpenMlsRustCrypto::default()); // REMOVE THIS

        // ... rest of implementation
    }

    pub async fn join_from_welcome(
        welcome_bytes: &[u8],
        ratchet_tree: Option<Vec<u8>>,
        config: MlsConfig,
        key_package_bundle: Option<KeyPackageBundle>,
        provider: Arc<OpenMlsRustCrypto>,  // NEW PARAMETER
    ) -> MlsResult<Self> {
        // Use provided provider
        // ... rest of implementation
    }
}
```

### 2. Update OpenMlsHandleAdapter

```rust
// src/core_mls/engine/adapter.rs
impl OpenMlsHandleAdapter {
    pub async fn create_group(
        group_id: Option<GroupId>,
        identity: Vec<u8>,
        config: MlsConfig,
        provider: Arc<OpenMlsRustCrypto>,  // NEW PARAMETER
    ) -> MlsResult<Self> {
        let engine = OpenMlsEngine::create_group(
            gid,
            identity,
            config.clone(),
            provider,  // PASS IT THROUGH
        ).await?;

        Ok(Self { engine: Arc::new(RwLock::new(engine)), config })
    }

    pub async fn join_from_welcome(
        welcome_bytes: &[u8],
        ratchet_tree: Option<Vec<u8>>,
        config: MlsConfig,
        key_package_bundle: Option<KeyPackageBundle>,
        provider: Arc<OpenMlsRustCrypto>,  // NEW PARAMETER
    ) -> MlsResult<Self> {
        // ... pass provider to OpenMlsEngine
    }
}
```

### 3. Update MlsService Calls

```rust
// src/core_mls/service.rs
impl MlsService {
    pub async fn create_group(&self, ...) -> MlsResult<GroupId> {
        let adapter = OpenMlsHandleAdapter::create_group(
            group_id,
            identity,
            self.config.clone(),
            self.provider.clone(),  // PASS SHARED PROVIDER
        ).await?;
        // ...
    }

    pub async fn join_group(&self, ...) -> MlsResult<GroupId> {
        let adapter = OpenMlsHandleAdapter::join_from_welcome(
            welcome_bytes,
            ratchet_tree,
            self.config.clone(),
            None,
            self.provider.clone(),  // PASS SHARED PROVIDER
        ).await?;
        // ...
    }
}
```

## Estimated Effort

**Time**: 1-2 hours
**Complexity**: Medium (breaking change to internal APIs)
**Risk**: Low (internal refactor, tests will validate)

## Breaking Changes

This is an INTERNAL API change - no public APIs affected:

- `OpenMlsEngine::create_group` signature
- `OpenMlsEngine::join_from_welcome` signature
- `OpenMlsHandleAdapter::create_group` signature
- `OpenMlsHandleAdapter::join_from_welcome` signature

All call sites are within `spacepanda-core`, so it's a contained refactor.

## Testing Plan

Once implemented:

1. Run `test_full_join_flow` - should pass
2. Run `test_multiple_message_exchange` - should pass
3. Run `test_three_party_group` - should pass
4. Run `test_invite_creation_with_real_key_package` - should pass
5. Run all integration tests to ensure no regressions

## Files Modified So Far

1. `src/core_mvp/group_provider.rs` - Added trait method
2. `src/core_mvp/adapters/core_mls_adapter.rs` - Implemented for CoreMlsAdapter
3. `src/core_mvp/adapters/mock_provider.rs` - Implemented for MockGroupProvider
4. `src/core_mvp/channel_manager.rs` - Added public API
5. `src/core_mls/service.rs` - Added provider field, generate_key_package method
6. `src/core_mvp/tests/full_join_flow.rs` - Updated all tests

## Files That Need Changes (Priority 3.2)

1. `src/core_mls/engine/openmls_engine.rs` - Accept provider parameter
2. `src/core_mls/engine/adapter.rs` - Accept and pass provider
3. `src/core_mls/service.rs` - Pass provider to adapters
4. Any other call sites of OpenMlsHandleAdapter::create_group/join_from_welcome

## Decision Point

**Option A**: Complete Priority 3.2 now (~1-2 hours)

- Pros: Full E2E encryption works, all tests pass
- Cons: Additional time investment

**Option B**: Move to Priority 4 (HTTP test harness)

- Pros: Can demo current functionality
- Cons: Join flow incomplete, tests marked as "known issues"

**Recommendation**: Complete Priority 3.2 - it's a small, focused change that completes the feature.
