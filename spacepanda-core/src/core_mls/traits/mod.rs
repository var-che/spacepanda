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

pub mod commit_validator;
pub mod crypto;
pub mod identity;
pub mod serializer;
pub mod storage;
pub mod transport;

pub use commit_validator::CommitValidator;
pub use crypto::CryptoProvider;
pub use identity::IdentityBridge;
pub use serializer::MessageSerializer;
pub use storage::StorageProvider;
pub use transport::DhtBridge;
