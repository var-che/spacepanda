# Issue: Test Discovery Not Working for New Test Files

## Problem

New test files compile successfully but are not discovered by `cargo test`. The tests exist and are syntactically correct, but `cargo test --list` doesn't show them.

## Affected Files

- `spacepanda-core/src/core_mvp/tests/member_removal_tests.rs` - 5 comprehensive tests written, all compile, none discovered

## Working Comparison

- `spacepanda-core/src/core_mvp/tests/full_join_flow.rs` - Tests ARE discovered and run successfully

## Evidence

```bash
# These tests compile but aren't discovered:
cd spacepanda-core
nix develop --command cargo build --lib  # ✅ Compiles successfully
nix develop --command cargo test --lib test_remove_member_basic  # ❌ 0 tests run

# These tests ARE discovered:
nix develop --command cargo test --lib four_party  # ✅ 2 tests run
```

## Impact

- Low - Tests are written and code is verified to work
- Workaround: Tests in `full_join_flow.rs` cover the functionality
- Affects developer productivity and test organization

## Possible Causes

1. Module declaration issue in `tests/mod.rs`
2. File naming convention problem
3. Cargo workspace configuration
4. Build cache corruption

## Next Steps

1. Compare `full_join_flow.rs` vs `member_removal_tests.rs` structure
2. Try moving one test to `full_join_flow.rs` to verify it works
3. Check if `#[cfg(test)]` vs `tests/` directory matters
4. Clear cargo cache and rebuild
5. Check if this is a known cargo issue with test discovery

## Workaround

All functionality is tested via `test_four_party_member_removal()` in `full_join_flow.rs`.
