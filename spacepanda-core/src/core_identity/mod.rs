//! Identity management module
//!
//! This subsystem sits between raw networking (router) and secure group messaging (core_mls).
//! It provides:
//! - Global identity (long-term Ed25519 keypair)
//! - Device identity (per-device keypair)
//! - Identity metadata (username, avatar, capabilities)
//! - Identity verification (signatures)
//! - Local keystore (encrypted storage)
//! - Identity syncing (via CRDT)

// New modules following the specification
pub mod user_id;
pub mod device_id;
pub mod keypair;
pub mod metadata;
pub mod bundles;
pub mod signatures;
pub mod validation;
pub mod keystore;

// Legacy modules (to be refactored)
mod channel;
mod global;
mod keys;
mod store;

// Test module
#[cfg(test)]
mod tests;

// Re-exports
pub use user_id::UserId;
pub use device_id::DeviceId;
pub use keypair::{Keypair, KeyType};
pub use metadata::{UserMetadata, DeviceMetadata};
pub use bundles::{KeyPackage, DeviceBundle, IdentityBundle};
pub use signatures::IdentitySignature;
pub use validation::{ValidationError, validate_keypackage, validate_device_bundle, validate_identity_bundle};
pub use keystore::{Keystore, KeystoreError};

// Legacy exports (for backwards compatibility)
pub use channel::{ChannelHash, ChannelIdentity};
pub use global::{GlobalIdentity, IdentityError};
pub use store::StoredIdentity;
