//! Core MLS Trait Boundaries
//!
//! This module defines the trait-based architecture for MLS integration.
//! These traits provide clean boundaries between:
//! - Storage layer (persistence)
//! - Crypto layer (cryptographic operations)
//! - Identity layer (user credentials)
//! - Transport layer (DHT/Router integration)
//! - Serialization (wire format)
//!
//! The trait-based design enables:
//! - Testing with mock implementations
//! - Swapping between different providers (OpenMLS, custom, etc.)
//! - Clear separation of concerns
//! - Future flexibility

pub mod storage;
pub mod crypto;
pub mod identity;
pub mod transport;
pub mod serializer;
pub mod commit_validator;

pub use storage::StorageProvider;
pub use crypto::CryptoProvider;
pub use identity::IdentityBridge;
pub use transport::DhtBridge;
pub use serializer::MessageSerializer;
pub use commit_validator::CommitValidator;
