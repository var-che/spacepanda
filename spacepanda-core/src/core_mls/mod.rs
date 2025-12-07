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
pub mod crypto;
pub mod errors;
pub mod events;
pub mod types;

// Trait boundaries for OpenMLS integration
pub mod traits;

// Provider implementations
pub mod integration;
pub mod providers;
pub mod storage;
pub mod persistent_provider;

// OpenMLS engine wrapper (Phase 3)
pub mod engine;

// High-level service with production integration
pub mod service;

// Message lifecycle (envelopes, routing)
pub mod messages;

// Privacy enhancements
pub mod padding;
pub mod sealed_metadata;
pub mod sealed_sender;

// State management (snapshots, persistence)
pub mod state;

// Rate limiting and DoS protection
pub mod rate_limit;

// Security testing and hardening
pub mod security;

// Feature-gated MLS handle selector
pub mod handle;

// Implemented modules
pub mod api;
pub mod commit;
pub mod discovery;
pub mod encryption;
pub mod group;
pub mod persistence;
pub mod proposals;
pub mod transport;
pub mod tree;
pub mod welcome;

// Testing modules
#[cfg(test)]
#[path = "tests/alpha_security_tests.rs"]
mod alpha_security_tests;
#[cfg(test)]
#[path = "tests/core_mls_test_suite.rs"]
mod core_mls_test_suite;
#[cfg(test)]
#[path = "tests/integration_tests.rs"]
mod integration_tests;
#[cfg(test)]
#[path = "tests/phase4_integration.rs"]
mod phase4_integration;
#[cfg(test)]
#[path = "tests/rfc9420_conformance_tests.rs"]
mod rfc9420_conformance_tests;
#[cfg(test)]
#[path = "tests/security_tests.rs"]
mod security_tests;
#[cfg(test)]
#[path = "tests/tdd_tests.rs"]
mod tdd_tests;
#[cfg(test)]
#[path = "tests/realistic_scenarios.rs"]
mod realistic_scenarios;

// Placeholder modules (to be implemented incrementally)

// Re-exports
pub use commit::{Commit, CommitResult, CommitValidator, UpdatePath};
pub use encryption::{
    decrypt_message, encrypt_message, EncryptedMessage, HpkeContext, KeySchedule, SenderData,
};
pub use errors::{MlsError, MlsResult};
pub use group::MlsGroup;
pub use persistence::{
    decrypt_group_state, encrypt_group_state, load_group_from_file, save_group_to_file,
    EncryptedGroupBlob, GroupSecrets, PersistedGroupState,
};
pub use proposals::{Proposal, ProposalContent, ProposalQueue, ProposalRef, ProposalType};
pub use transport::{MlsEnvelope, MlsMessageType, MlsTransport};
pub use tree::{LeafIndex, MlsTree, NodeIndex, TreeNode};
pub use types::{GroupId, GroupMetadata, MlsConfig};
pub use welcome::{TreeSnapshot, Welcome, WelcomeBuilder, WelcomeGroupSecrets};

// Primary MLS handle (OpenMLS-based)
pub use handle::MlsHandle;

// Discovery and crypto
pub use crypto::{sign_with_key, verify_with_key, MlsSigningKey, MlsVerifyingKey};
pub use discovery::{DiscoveryQuery, GroupPublicInfo};

// OpenMLS engine exports
pub use engine::{
    group_ops::ProcessedMessage, GroupOperations, MessageAdapter, OpenMlsEngine,
    OpenMlsHandleAdapter, WireFormat,
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
