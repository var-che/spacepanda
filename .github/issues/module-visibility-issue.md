# Issue: Module Visibility - core_mvp Submodules Not Accessible Externally

## Problem

Modules declared as `pub mod` in `core_mvp/mod.rs` are not accessible from external crates or integration tests, despite being properly exported.

## Affected Modules

- `spacepanda_core::core_mvp::channel_manager` - Cannot import `ChannelManager` or `Identity`
- `spacepanda_core::core_mvp::test_harness` - Cannot access HTTP harness types/handlers
- `spacepanda_core::core_mvp::errors` - Cannot import `MvpError` or `MvpResult`

## Error Messages

```rust
error[E0432]: unresolved imports `spacepanda_core::core_mvp::ChannelManager`, `spacepanda_core::core_mvp::Identity`
 --> test-harness/src/main.rs:15:33
  |
15 | use spacepanda_core::core_mvp::{ChannelManager, Identity};
   |                                 ^^^^^^^^^^^^^^  ^^^^^^^^ no `Identity` in `core_mvp`
   |                                 |
   |                                 no `ChannelManager` in `core_mvp`
```

## Current State

### What Works

- Unit tests (within `spacepanda-core`) can access everything ✅
- `pub use` re-exports in `core_mvp/mod.rs` work internally ✅
- Library compiles without errors ✅

### What Doesn't Work

- External crates (like `test-harness`) cannot import ❌
- Integration tests (in `tests/`) cannot import ❌
- Re-exporting at crate root in `lib.rs` also fails ❌

## Investigation Done

1. ✅ Verified `pub mod channel_manager;` in `core_mvp/mod.rs`
2. ✅ Verified `pub mod core_mvp;` in `lib.rs`
3. ✅ Removed conflicting `lib.rs` file in `core_mvp/` directory
4. ✅ Tried direct module paths: `core_mvp::channel_manager::{...}`
5. ✅ Tried re-exports: `pub use core_mvp::{ChannelManager, Identity};` in lib.rs
6. ❌ All attempts fail with "could not find X in core_mvp"

## Impact

- **High** - Blocks HTTP test harness implementation
- Prevents external integration testing
- Makes it hard to use the library from other workspace crates

## Suspected Root Causes

1. **Build configuration issue** - Maybe workspace vs package configuration conflict
2. **Stale build cache** - Though `cargo clean` was attempted
3. **Hidden `#[cfg]` gates** - Though grep search found none
4. **Rust edition mismatch** - Different edition settings?
5. **Path resolution bug** - Possible cargo/rustc bug

## Files Involved

- `spacepanda-core/src/lib.rs` - Crate root
- `spacepanda-core/src/core_mvp/mod.rs` - Module declarations
- `spacepanda-core/Cargo.toml` - Package configuration
- `Cargo.toml` (workspace root) - Workspace configuration

## Workaround

Currently none - HTTP endpoints are defined but cannot be compiled into a working server.

## Next Steps to Debug

1. Create minimal reproduction case with just one module
2. Check if other `core_*` modules have the same issue
3. Compare against working modules (like `core_mls`, `core_store`)
4. Try creating a separate integration test with `#[path]` attribute
5. Check Rust forums for similar issues
6. Try different Rust toolchain version

## Blocked Features

- HTTP test harness server (endpoints written, can't compile)
- External integration tests for `ChannelManager`
- Future CLI tools that need to import from `core_mvp`
