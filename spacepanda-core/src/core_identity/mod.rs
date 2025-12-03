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
pub mod bundles;
pub mod device_id;
pub mod device_key;
pub mod keypair;
pub mod keystore;
pub mod master_key;
pub mod metadata;
pub mod signatures;
pub mod user_id;
pub mod validation;

// Legacy modules (to be refactored)
mod channel;
mod global;
mod keys;
mod store;

// Test module
// Temporarily disabled due to rand_core version conflicts
// #[cfg(all(test, feature = "never_enabled"))]
// mod tests;

// Re-exports
pub use bundles::{DeviceBundle, IdentityBundle, KeyPackage};
pub use device_id::DeviceId;
pub use device_key::{DeviceKey, DeviceKeyBinding, KeyVersion};
pub use keypair::{KeyType, Keypair};
pub use keystore::{Keystore, KeystoreError};
pub use master_key::MasterKey;
pub use metadata::{DeviceMetadata, UserMetadata};
pub use signatures::IdentitySignature;
pub use user_id::UserId;
pub use validation::{
    validate_device_bundle, validate_identity_bundle, validate_keypackage, ValidationError,
};

// Legacy exports (for backwards compatibility)
pub use channel::{ChannelHash, ChannelIdentity};
pub use global::{GlobalIdentity, IdentityError};
pub use store::StoredIdentity;
