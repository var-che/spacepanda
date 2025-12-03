# Canonical MLS Implementation Decision

**Date**: December 3, 2025  
**Status**: ✅ DECIDED  
**Decision**: OpenMLS is the canonical production implementation

---

## Summary

This document records the decision to use **OpenMLS** as the canonical, production-ready MLS implementation for SpacePanda, while retaining the legacy custom MLS implementation for testing and educational purposes only.

---

## Background

SpacePanda's `core_mls` module currently contains two MLS implementations:

1. **Custom Implementation** (Legacy)
   - Files: `api.rs`, `transport.rs`, `group.rs`, `tree.rs`, `commit.rs`, `proposals.rs`, `welcome.rs`, `encryption.rs`
   - Purpose: Educational, RFC 9420 conformance testing
   - Status: ⚠️ DEPRECATED (marked for removal in v0.3.0)

2. **OpenMLS Integration** (Production)
   - Files: `engine/openmls_engine.rs`, `engine/group_ops.rs`, `providers/`
   - Purpose: Production use
   - Status: ✅ ACTIVE

---

## Decision

**We choose OpenMLS (`openmls` crate v0.7.1) as the canonical implementation.**

### Rationale

1. **Security & Correctness**
   - OpenMLS is battle-tested, audited, and actively maintained
   - RFC 9420 compliant with extensive test coverage
   - Cryptographic correctness validated by security experts
   - Regular security updates and vulnerability patches

2. **Maintenance Burden**
   - Maintaining two implementations is error-prone
   - Custom implementation requires cryptographic expertise
   - OpenMLS handles protocol evolution (e.g., future RFC updates)
   - Community support and bug fixes

3. **Feature Completeness**
   - OpenMLS supports full MLS protocol suite
   - Extensions and advanced features available
   - Better HPKE, key schedule, and tree management
   - Proven interoperability with other MLS implementations

4. **Code Review Feedback**
   - External code review (ALPHA_TODO.md) recommended this decision
   - Duplicate code paths identified as major risk
   - Recommended: Keep custom code for testing only

---

## Implementation Plan

### Phase 1: Documentation (✅ COMPLETE - Dec 3, 2025)

- [x] Mark legacy modules as deprecated with `#![allow(deprecated)]`
- [x] Add deprecation warnings in module docs
- [x] Update `mod.rs` to not re-export legacy `MlsHandle`
- [x] Create this canonical implementation decision document

### Phase 2: Feature Gating (PLANNED - Week 1)

Add Cargo feature flag to control legacy code compilation:

```toml
[features]
default = []
legacy-mls = []  # Enable legacy custom MLS implementation for testing
```

Update `mod.rs`:

```rust
#[cfg(feature = "legacy-mls")]
pub mod api;

#[cfg(feature = "legacy-mls")]
pub mod transport;

#[cfg(feature = "legacy-mls")]
pub mod group;

// etc.
```

### Phase 3: Test Migration (PLANNED - Week 2)

- Update conformance tests to use OpenMLS engine
- Keep RFC 9420 conformance tests with `legacy-mls` feature flag
- Migrate all E2E tests to OpenMLS-only paths
- Update integration tests in `tests/phase4_integration.rs`

### Phase 4: Legacy Removal (PLANNED - v0.3.0)

- Remove legacy modules entirely
- Remove `legacy-mls` feature flag
- Update all documentation
- Publish breaking change release notes

---

## Current Status

### ✅ Completed

1. All production code uses OpenMLS:
   - `engine/openmls_engine.rs` - Core engine
   - `engine/group_ops.rs` - Group operations trait
   - `providers/openmls_provider.rs` - Provider implementation
   - `messages/outbound.rs` - Message building with OpenMLS

2. Legacy modules marked deprecated:
   - `api.rs` - `#![allow(deprecated)]`
   - `transport.rs` - `#![allow(deprecated)]`
   - Module docs warn: "This module will be removed in v0.3.0"

3. Tests updated:
   - 8 E2E tests in `phase4_integration.rs` use OpenMLS
   - 15 security tests in `alpha_security_tests.rs` use OpenMLS
   - RFC conformance tests still use legacy (educational)

### ⚠️ Pending

1. Add `legacy-mls` feature flag to `Cargo.toml`
2. Gate legacy modules behind feature flag
3. Update documentation to clarify OpenMLS-only API
4. Add migration guide for any external users

---

## API Surface

### Production API (OpenMLS-based)

```rust
use spacepanda_core::core_mls::engine::{OpenMlsEngine, GroupOperations};
use spacepanda_core::core_mls::types::{GroupId, MlsConfig};

// Create a group
let engine = OpenMlsEngine::create_group(
    GroupId::random(),
    b"alice@example.com".to_vec(),
    MlsConfig::default()
).await?;

// Add members
let (commit, welcome) = engine.add_members(vec![key_package_bytes]).await?;

// Send message
let ciphertext = engine.send_message(b"Hello!").await?;

// Process incoming message
let processed = engine.process_message(&message_bytes).await?;
```

### Legacy API (Testing Only)

```rust
#[cfg(feature = "legacy-mls")]
use spacepanda_core::core_mls::api::MlsHandle;  // Deprecated

// Only available with --features legacy-mls
// Will be removed in v0.3.0
```

---

## Migration Checklist for External Users

If you're using the legacy `api::MlsHandle`:

- [ ] Switch to `engine::OpenMlsEngine`
- [ ] Update group creation calls: `create_group()` → `OpenMlsEngine::create_group()`
- [ ] Update message handling: `send_message()` → `engine.send_message()`
- [ ] Update member operations: `propose_add()` → `engine.add_members()`
- [ ] Remove any direct imports of `api::MlsHandle`, `transport::MlsEnvelope`, `group::MlsGroup`

---

## Security Considerations

### OpenMLS Advantages

1. **Cryptographic Correctness**
   - Professionally audited cryptographic implementation
   - Uses `openmls_rust_crypto` with proven algorithms
   - HPKE (RFC 9180) implementation by experts
   - Constant-time operations where needed

2. **Protocol Compliance**
   - RFC 9420 compliance verified by test vectors
   - Interoperability with other MLS implementations
   - Correct epoch handling and key rotation
   - Proper replay prevention

3. **Secret Handling**
   - OpenMLS uses `zeroize` for sensitive data
   - Secure key derivation with HKDF
   - Forward secrecy and post-compromise security
   - Proper cleanup on group deletion

### Legacy Implementation Risks

1. **Unaudited Cryptography**
   - Custom crypto code not reviewed by experts
   - Potential timing side-channels
   - May not handle edge cases correctly

2. **Maintenance Burden**
   - Requires keeping up with protocol changes
   - No external security updates
   - Duplicate bug fixes needed

3. **Missing Features**
   - Limited extension support
   - No PSK support
   - Simplified tree operations

---

## Testing Strategy

### OpenMLS Tests (Production)

- `tests/phase4_integration.rs` - E2E integration tests
- `tests/alpha_security_tests.rs` - Security-focused tests
- `engine/openmls_engine.rs` - Unit tests
- `providers/openmls_provider.rs` - Provider tests

### Legacy Tests (Educational)

- `tests/rfc9420_conformance_tests.rs` - RFC conformance validation
- `tests/tdd_tests.rs` - TDD-style unit tests
- `tests/core_mls_test_suite.rs` - Comprehensive test suite

All legacy tests will be gated behind `--features legacy-mls` in future releases.

---

## Performance Considerations

OpenMLS performance is comparable or better than custom implementation:

- **Key Schedule**: Optimized HKDF usage
- **Tree Operations**: Efficient binary tree algorithms
- **Message Encryption**: Hardware-accelerated AES-GCM
- **Serialization**: Fast TLS codec implementation

Future benchmarks (TASK 2.4) will baseline OpenMLS performance.

---

## References

- [RFC 9420 - Messaging Layer Security Protocol](https://www.rfc-editor.org/rfc/rfc9420.html)
- [OpenMLS Documentation](https://openmls.tech/)
- [OpenMLS GitHub](https://github.com/openmls/openmls)
- [ALPHA_TODO.md](./ALPHA_TODO.md) - External code review
- [ALPHA_TODO_OVERVIEW.md](./ALPHA_TODO_OVERVIEW.md) - Implementation roadmap

---

## Version History

| Date | Version | Change |
|------|---------|--------|
| 2025-12-03 | 0.1.0 | Initial decision document |
| 2025-12-03 | 0.1.0 | OpenMLS chosen as canonical implementation |

---

## Approval

**Decided by**: Development Team  
**Reviewed by**: Security Review (ALPHA_TODO.md)  
**Status**: ✅ APPROVED  
**Effective**: Immediately for new code  
**Legacy Removal**: v0.3.0 (planned)
