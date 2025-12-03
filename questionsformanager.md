# Questions for Manager - MLS Provider Architecture Issue

**Date**: December 3, 2025  
**Context**: Priority 3.1 (Key Package Management) Implementation  
**Status**: Blocked by architectural issue

---

## Summary of Issue

We've implemented the key package management API successfully, but tests fail with `NoMatchingKeyPackage` error. The root cause is a **provider isolation problem**: OpenMLS cryptographic operations require the SAME provider instance to be used for generating key packages and joining groups, but the current architecture creates NEW providers for each operation.

---

## Core Architectural Problems

### 1. No Provider Ownership Model

**Current State**: Providers are created ad-hoc throughout the system

- `MlsService.generate_key_package()` creates a temporary provider
- `OpenMlsEngine::create_group()` creates its own provider
- `OpenMlsEngine::join_from_welcome()` creates its own provider

**Problem**: When Bob generates a key package, it's stored in provider A. When Bob tries to join from Welcome message, it creates provider B which can't find the stored keys.

**Missing**: A clear ownership model where ONE provider per user/service instance is shared across ALL operations.

### 2. Deep Constructor Chain Without Dependency Injection

**Current call chain**:

```
ChannelManager
  → MlsService
    → OpenMlsHandleAdapter::create_group() / join_from_welcome()
      → OpenMlsEngine::create_group() / join_from_welcome()
        → Creates NEW provider (❌ breaks crypto continuity)
```

**Problem**: Each layer independently creates providers at the bottom of the chain. There's no mechanism to inject a shared provider from the top.

### 3. Static Factory Methods vs Instance Methods

**Current pattern**:

```rust
// Static factory - can't access shared instance state
OpenMlsEngine::create_group(group_id, identity, config)

// What we need - instance method with injected provider
engine.create_group_with_provider(group_id, identity, config, provider)
```

**Problem**: Static factory methods can't access instance-level shared providers. This pattern works for stateless operations but fails for stateful crypto.

### 4. Stateless Service Pattern for Stateful Crypto

**Architecture mismatch**:

- **Service pattern assumption**: "Create temporary objects for each operation, no persistent state needed"
- **Crypto reality**: "Must reuse the SAME cryptographic storage for all operations by a user"

`MlsService` is designed as a stateless service managing multiple groups, but OpenMLS crypto providers are inherently stateful - they persist key material.

### 5. Missing Abstraction Layer

**What's missing**: A `ProviderManager` or `CryptoContext` abstraction that:

- Is created once per user/service instance
- Owns the `OpenMlsRustCrypto` provider
- Is injected into all MLS operations
- Manages provider lifecycle (creation, cleanup)

**Current state**: Service directly creates OpenMLS primitives with no provider management layer.

---

## Specific Code Breaking Points

### Location 1: `src/core_mls/engine/openmls_engine.rs:64`

```rust
pub async fn create_group(...) -> MlsResult<Self> {
    let provider = Arc::new(OpenMlsRustCrypto::default()); // ❌ NEW every time
    // ... uses this provider
}
```

### Location 2: `src/core_mls/engine/openmls_engine.rs:176`

```rust
pub async fn join_from_welcome(...) -> MlsResult<Self> {
    let provider = Arc::new(OpenMlsRustCrypto::default()); // ❌ NEW every time
    // ... can't find keys from create_group's provider
}
```

### Location 3: `src/core_mls/service.rs:105` (newly added)

```rust
pub async fn generate_key_package(&self, ...) -> MlsResult<Vec<u8>> {
    let provider = self.provider.clone(); // ✅ Uses shared provider
    // ... stores KeyPackageBundle in provider
}

pub async fn join_group(&self, ...) -> MlsResult<GroupId> {
    // ❌ Calls OpenMlsHandleAdapter which creates NEW provider
    // Can't find KeyPackageBundle from generate_key_package()
}
```

---

## Proposed Solutions

### Option A: Minimal Fix (Priority 3.2) - Estimated 1-2 hours

**Changes**:

1. Add `provider: Arc<OpenMlsRustCrypto>` parameter to:

   - `OpenMlsEngine::create_group()`
   - `OpenMlsEngine::join_from_welcome()`
   - `OpenMlsHandleAdapter::create_group()`
   - `OpenMlsHandleAdapter::join_from_welcome()`

2. Update `MlsService` to pass `self.provider` through the call chain

**Impact**:

- ✅ Fixes the immediate issue
- ✅ All 4 join flow tests should pass
- ⚠️ Breaking change to internal APIs (but no public API impact)
- ⚠️ Doesn't address deeper architectural concerns

**Risk**: Low - internal refactor, well-tested

### Option B: Proper Architectural Fix (Future Refactor) - Estimated 1-2 days

**Changes**:

1. Create `CryptoContext` abstraction:

   ```rust
   pub struct CryptoContext {
       provider: Arc<OpenMlsRustCrypto>,
       identity: Vec<u8>,
       // ... other crypto state
   }
   ```

2. Refactor `MlsService` to use dependency injection:
   - Accept `CryptoContext` in constructor
   - Pass context to all operations
3. Switch from static factory methods to builder pattern or instance construction

4. Consider provider persistence layer

**Impact**:

- ✅ Clean separation of concerns
- ✅ Testable, mockable crypto layer
- ✅ Easier to add features like key rotation, multi-user support
- ⚠️ Larger refactor affecting multiple modules
- ⚠️ Requires design review and testing

**Risk**: Medium - broader scope, more test changes

---

## Questions Requiring Manager Input

### 1. Scope Approval

**Q**: Can I modify `OpenMlsEngine` and `OpenMlsHandleAdapter` internal API signatures?  
**Context**: These are internal APIs (not exposed publicly). Changes would affect ~5-10 call sites within `spacepanda-core`.  
**Impact**: Breaking change to internal APIs, but isolated to our codebase.

### 2. Provider Persistence Strategy

**Q**: Should the `OpenMlsRustCrypto` provider be:

- **A) In-memory only** (current - dies with service restart)?
- **B) Backed by persistent storage** (files/database)?
- **C) Configurable per deployment** (dev vs prod)?

**Context**: Current implementation loses all crypto state on restart. For production, we likely need persistent storage for key material.

### 3. Multi-User Support Strategy

**Q**: Currently `MlsService` has ONE shared provider. For services managing groups for multiple users, should we:

- **A) One provider per user** (HashMap<UserId, Provider>)?
- **B) One provider per MlsService instance** (current - assumes one user per service)?
- **C) One global provider** (all users share crypto storage)?
- \*\*D) Something else?

**Context**: Tests create separate `MlsService` per user (Alice, Bob, Charlie), which works. But in production, we might have one service handling multiple users.

### 4. Testing Strategy

**Q**: Should the test pattern change?

- **Current**: Each test user gets their own `MlsService` instance (works well for isolation)
- **Alternative**: Shared `MlsService` with user-specific crypto contexts?

**Context**: Current test pattern naturally provides provider isolation, which is good. But doesn't test multi-user scenarios.

### 5. Timeline Decision

**Q**: Should I:

- **A) Complete Priority 3.2 now** (minimal fix, 1-2 hours)?
- **B) Create detailed design doc first** (for proper architectural fix)?
- **C) Defer and move to Priority 4** (HTTP test harness)?

**Recommendation**: Option A - complete minimal fix now, plan proper refactor for later sprint.

---

## Additional Concerns

### Concern 1: Provider Storage Location

**Issue**: `OpenMlsRustCrypto::default()` creates an in-memory provider. Where does it actually store keys?

**Questions**:

- Does it use temp files?
- Is storage persistent across provider instances?
- What happens on service restart?

**Impact**: If storage is truly ephemeral, we lose all crypto state on restart. This might be fine for MVP but not production.

### Concern 2: Key Package Expiration

**Issue**: OpenMLS key packages can expire. We generate them but don't track expiration.

**Questions**:

- Should we store key package metadata (creation time, expiration)?
- Do we need a cleanup job for expired packages?
- How do we handle expired key packages in invites?

**Impact**: Users might receive invites with expired key packages, causing join failures.

### Concern 3: Concurrent Access to Provider

**Issue**: `Arc<OpenMlsRustCrypto>` is shared, but is it thread-safe?

**Questions**:

- Can multiple operations safely use the same provider concurrently?
- Do we need additional synchronization (Mutex/RwLock)?
- What happens if two threads try to join different groups simultaneously?

**Impact**: Potential race conditions in crypto operations if not properly synchronized.

### Concern 4: Provider Cleanup and Resource Leaks

**Issue**: We create providers but don't explicitly clean them up.

**Questions**:

- Does `OpenMlsRustCrypto` have cleanup requirements?
- Are there file handles, temp files, or memory that needs explicit cleanup?
- Should we implement `Drop` for proper cleanup?

**Impact**: Potential resource leaks in long-running services.

### Concern 5: Error Handling Granularity

**Issue**: "NoMatchingKeyPackage" is generic. Hard to debug.

**Questions**:

- Should we add more specific error types (WrongProvider, KeyPackageExpired, etc.)?
- Can we log which provider IDs are involved for debugging?
- Should we add provider-level health checks?

**Impact**: Poor debuggability in production. Hard to diagnose crypto issues.

### Concern 6: Test Coverage for Provider Sharing

**Issue**: Current tests create isolated providers per user. We don't test provider sharing scenarios.

**Questions**:

- Should we add tests for provider reuse within a service?
- Do we need tests for provider cleanup/lifecycle?
- Should we test error cases (wrong provider, missing keys)?

**Impact**: Incomplete test coverage for the actual production scenario.

---

## Test Results Summary

**Current Status**: 1130 tests passing (library tests only)

**Blocked Tests** (removed `#[ignore]` but still fail):

1. `test_full_join_flow` - Fails at step 4 (Bob joins channel)
2. `test_multiple_message_exchange` - Fails at join
3. `test_three_party_group` - Fails at first join

**Error Message**:

```
Error: Mls(InvalidMessage("Failed to stage welcome: NoMatchingKeyPackage"))
```

**Working Test**:

- `test_invite_creation_with_real_key_package` ✅ (doesn't test join)

---

## Files Modified in Priority 3.1

1. ✅ `src/core_mvp/group_provider.rs` - Added `generate_key_package()` to trait
2. ✅ `src/core_mvp/adapters/core_mls_adapter.rs` - Implemented for CoreMlsAdapter
3. ✅ `src/core_mvp/adapters/mock_provider.rs` - Implemented for MockGroupProvider
4. ✅ `src/core_mvp/channel_manager.rs` - Added public API method
5. ✅ `src/core_mls/service.rs` - Added provider field + generate_key_package method
6. ✅ `src/core_mvp/tests/full_join_flow.rs` - Updated all 4 tests

---

## Files That Need Changes for Priority 3.2 (Minimal Fix)

1. `src/core_mls/engine/openmls_engine.rs` - Add provider parameter to create_group() and join_from_welcome()
2. `src/core_mls/engine/adapter.rs` - Add provider parameter and pass through
3. `src/core_mls/service.rs` - Pass self.provider to adapters
4. Any other call sites of `OpenMlsHandleAdapter::create_group()` / `join_from_welcome()`

**Estimated complexity**: ~20-30 lines changed across 3-4 files

---

## Recommendation

**Immediate**: Proceed with Priority 3.2 (minimal fix) to unblock join flow testing. This is a well-understood, low-risk change.

**Near-term** (next sprint): Design proper architectural fix (Option B) with:

- Provider management abstraction
- Persistent storage strategy
- Multi-user support design
- Comprehensive error handling

**Rationale**: Get the feature working now, plan the proper solution carefully rather than rushing a larger refactor.

---

## Request for Manager

Please review and provide guidance on:

1. **Permission to proceed** with Priority 3.2 internal API changes
2. **Answers to Questions 1-5** above for architectural decisions
3. **Priority guidance** on addressing the Additional Concerns
4. **Timeline approval** for minimal fix now vs. proper refactor later

Let me know if you need any clarification or additional technical details.

---

**Prepared by**: GitHub Copilot (AI Assistant)  
**Date**: December 3, 2025  
**Branch**: mvp-sprint  
**Related Docs**:

- `src/core_mvp/PRIORITY_3_STATUS.md`
- `src/core_mvp/PRIORITY_3.1_INCOMPLETE.md`
