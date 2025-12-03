//! MLS (Messaging Layer Security) implementation for SpacePanda
//!
//! This module provides secure group messaging with:
//! - End-to-end encryption with forward secrecy (FS)
//! - Post-compromise security (PCS) through key rotation
//! - Authenticated group operations (add/remove members)
//! - Secure persistence with AEAD
//! - Integration with SpacePanda identity, router, and storage
//!
//! # Architecture
//!
//! See ARCHITECTURE.md for complete design documentation.
//!
//! ## Core Components
//!
//! - `MlsGroup`: Group state and operations
//! - `MlsTree`: Ratchet tree math and path secrets
//! - `Welcome`: New member onboarding
//! - `Commit`: State change application
//! - `MlsHandle`: High-level API facade
//!
//! ## Security Invariants
//!
//! - Epoch monotonicity: commits increment epoch
//! - Replay protection: per-sender sequence numbers
//! - Signature verification: all commits/proposals verified
//! - AEAD persistence: all secrets encrypted at rest
//! - Proof-of-possession: devices prove key ownership

// Core types and errors
pub mod types;
pub mod errors;
pub mod crypto;
pub mod events;

// Trait boundaries for OpenMLS integration
pub mod traits;

// Provider implementations
pub mod providers;
pub mod storage;
pub mod integration;

// OpenMLS engine wrapper (Phase 3)
pub mod engine;

// Message lifecycle (envelopes, routing)
pub mod messages;

// State management (snapshots, persistence)
pub mod state;

// Feature-gated MLS handle selector
pub mod handle;

// Implemented modules
pub mod persistence;
pub mod tree;
pub mod encryption;
pub mod welcome;
pub mod proposals;
pub mod commit;
pub mod group;
pub mod transport;
pub mod api;
pub mod discovery;

// Testing modules
#[cfg(test)]
#[path = "tests/security_tests.rs"]
mod security_tests;
#[cfg(test)]
#[path = "tests/alpha_security_tests.rs"]
mod alpha_security_tests;
#[cfg(test)]
#[path = "tests/integration_tests.rs"]
mod integration_tests;
#[cfg(test)]
#[path = "tests/tdd_tests.rs"]
mod tdd_tests;
#[cfg(test)]
#[path = "tests/core_mls_test_suite.rs"]
mod core_mls_test_suite;
#[cfg(test)]
#[path = "tests/rfc9420_conformance_tests.rs"]
mod rfc9420_conformance_tests;
#[cfg(test)]
#[path = "tests/phase4_integration.rs"]
mod phase4_integration;

// Placeholder modules (to be implemented incrementally)

// Re-exports
pub use types::{GroupId, MlsConfig, GroupMetadata};
pub use errors::{MlsError, MlsResult};
pub use persistence::{
    encrypt_group_state, decrypt_group_state,
    save_group_to_file, load_group_from_file,
    EncryptedGroupBlob, GroupSecrets, PersistedGroupState,
};
pub use tree::{MlsTree, TreeNode, NodeIndex, LeafIndex};
pub use encryption::{
    encrypt_message, decrypt_message,
    KeySchedule, EncryptedMessage, SenderData, HpkeContext,
};
pub use welcome::{Welcome, WelcomeBuilder, WelcomeGroupSecrets, TreeSnapshot};
pub use proposals::{
    Proposal, ProposalType, ProposalContent, ProposalRef, ProposalQueue,
};
pub use commit::{Commit, UpdatePath, CommitResult, CommitValidator};
pub use group::MlsGroup;
pub use transport::{MlsTransport, MlsEnvelope, MlsMessageType};

// Primary MLS handle (OpenMLS-based)
pub use handle::MlsHandle;

// Discovery and crypto
pub use discovery::{GroupPublicInfo, DiscoveryQuery};
pub use crypto::{MlsSigningKey, MlsVerifyingKey, sign_with_key, verify_with_key};

// OpenMLS engine exports
pub use engine::{
    OpenMlsEngine, MessageAdapter, WireFormat, GroupOperations,
    OpenMlsHandleAdapter,
    group_ops::ProcessedMessage,
};

// Note: api::MlsHandle (legacy) is deprecated and not re-exported to avoid ambiguity.
// Tests in api.rs can still use it directly via `use super::api::MlsHandle`.

/// Default ciphersuite for SpacePanda MLS
///
/// MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519:
/// - X25519 for ECDH
/// - AES-128-GCM for AEAD
/// - SHA-256 for hashing
/// - Ed25519 for signatures
pub const DEFAULT_CIPHERSUITE: openmls::prelude::Ciphersuite = 
    openmls::prelude::Ciphersuite::MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_exports() {
        let _ = DEFAULT_CIPHERSUITE;
    }
}
