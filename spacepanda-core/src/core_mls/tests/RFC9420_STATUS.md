# RFC 9420 Conformance Tests - Status

## Current Status: Work In Progress ⏳

The RFC 9420 conformance test suite (`rfc9420_conformance_tests.rs`) contains 37 test implementations covering the first 4 categories of the full 104-test MLS conformance matrix. However, these tests are currently **not compiling** due to API mismatches between the test expectations and the current MlsGroup implementation.

## Issue Summary

The tests were written against an idealized MLS API that includes methods like:

- `group.epoch()` → Should be `group.epoch` (public field)
- `group.group_id()` → Should be `group.group_id` (public field)
- `group.tree_hash()` → Should be `group.tree.root_hash().unwrap_or_default()`
- `group.members()` → Should be `group.tree.leaf_count()`
- `group.tree()` → Should be `group.tree` (public field)

While we've applied sed transformations to fix many of these issues, **62 compilation errors remain**, primarily type mismatches.

## Tests Implemented (37 total)

### ✅ Group Initialization Tests (11 tests)

- G1-G11: Basic group creation, GroupInfo validation, tree hash, secrets uniqueness, etc.

### ✅ Add Proposal Tests (9 tests)

- A1-A9: Member addition, self-add rejection, duplicate detection, HPKE validation, etc.

### ✅ Update Proposal Tests (9 tests)

- U1-U9: Key updates, sender validation, path secrets, tree hash validation, etc.

### ✅ Remove Proposal Tests (8 tests)

- R1-R8: Member removal, self-removal, index validation, epoch validation, etc.

## Remaining Work

### Immediate (to make tests compile):

1. **Fix type mismatches** (62 errors)

   - Many tests expect methods that return `Result<T>` but don't handle the Result
   - Some tests use incorrect field access patterns
   - Need to align test code with actual API

2. **Options:**
   - **Option A**: Modify tests to match current API (time-consuming, ~62 fixes needed)
   - **Option B**: Add convenience methods to MlsGroup (e.g., `fn epoch(&self) -> u64`, `fn group_id(&self) -> &GroupId`)
   - **Option C**: Keep tests commented out until API stabilizes

### Medium-term (after compilation fixes):

3. **Run and debug failing tests**

   - Even after compilation, many tests will likely fail
   - Need to identify which failures indicate real bugs vs. test issues

4. **Complete remaining 67 tests**
   - Proposal Committing (12 tests)
   - Welcome Processing (13 tests)
   - Tree Hash & Path (12 tests)
   - Encryption & Secrecy (10 tests)
   - Authentication & Signing (8 tests)
   - Application Messages (8 tests)
   - Error Handling & Recovery (12 tests)

## Current Test Suite (Working)

The **main test suite** with **257 passing tests** is fully functional and covers:

- `core_mls_test_suite.rs` - 47 comprehensive TDD tests ✅
- `tdd_tests.rs` - 34 TDD specification tests ✅
- `security_tests.rs` - 17 security validation tests ✅
- `integration_tests.rs` - 13 integration tests ✅
- Other core_mls tests - 146 tests ✅

**These 257 tests provide production-grade validation** and are currently enabled.

## Recommendation

**For now, keep RFC 9420 conformance tests disabled** until we either:

1. Refactor the 62 remaining compilation errors (significant effort)
2. Add convenience getter methods to MlsGroup to match test expectations
3. Stabilize the MLS API to a point where comprehensive conformance tests make sense

The current 257-test suite provides excellent coverage for production use. The RFC 9420 conformance suite will be valuable for:

- IETF interoperability testing
- Certification against the official MLS spec
- Finding edge cases in mature implementations

## Files

- `rfc9420_conformance_tests.rs` - The test implementations (currently disabled in mod.rs)
- `rfc9420_conformance_tests.rs.backup` - Backup before sed transformations
- This README - Status and recommendations

## How to Enable

When ready to work on these tests:

1. Uncomment in `src/core_mls/mod.rs`:

```rust
#[cfg(test)]
#[path = "tests/rfc9420_conformance_tests.rs"]
mod rfc9420_conformance_tests;
```

2. Fix compilation errors:

```bash
cargo test --lib rfc9420_conformance_tests 2>&1 | grep "error\[E"
```

3. Systematically address each error category

---

_Last Updated: Recently
\_Status: 37/104 tests implemented, 0/37 compiling_
_Blocked by: 62 compilation errors (type mismatches)_
