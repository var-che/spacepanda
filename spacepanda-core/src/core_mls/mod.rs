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

// Implemented modules
pub mod persistence;
pub mod tree;

// Placeholder modules (to be implemented incrementally)
// pub mod group;
// pub mod welcome;
// pub mod proposals;
// pub mod commit;
// pub mod encryption;
// pub mod transport;
// pub mod api;

// Re-exports
pub use types::{GroupId, MlsConfig, GroupMetadata};
pub use errors::{MlsError, MlsResult};
pub use persistence::{
    encrypt_group_state, decrypt_group_state,
    save_group_to_file, load_group_from_file,
    EncryptedGroupBlob, GroupSecrets, PersistedGroupState,
};
pub use tree::{MlsTree, TreeNode, NodeIndex, LeafIndex};

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
