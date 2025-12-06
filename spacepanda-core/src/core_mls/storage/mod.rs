//! Storage Provider Implementations
//!
//! This module provides concrete implementations of the `StorageProvider` trait.

pub mod channel_metadata;
pub mod file_store;
pub mod memory_store;
pub mod migrations;
pub mod sql_store;

#[cfg(test)]
mod recovery_tests;
#[cfg(test)]
mod stress_tests;

pub use channel_metadata::{ChannelMetadata, MessageMetadata};
pub use file_store::FileStorageProvider;
pub use memory_store::MemoryStorageProvider;
pub use migrations::{migrate, CURRENT_SCHEMA_VERSION};
pub use sql_store::SqlStorageProvider;

