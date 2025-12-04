//! Core MVP Layer - Orchestration for E2E Encrypted Chat
//!
//! This module provides the high-level API for SpacePanda's MVP by coordinating
//! `core_identity`, `core_mls`, `core_store`, and `core_dht` subsystems.

pub mod adapters;
pub mod channel_manager;
pub mod errors;
pub mod group_provider;
pub mod network;
pub mod peer_discovery;
pub mod test_harness;
pub mod types;

#[cfg(test)]
mod tests;

// Re-exports
pub use adapters::{CoreMlsAdapter, MockGroupProvider};
pub use channel_manager::{ChannelManager, Identity};
pub use errors::{MvpError, MvpResult};
pub use group_provider::{GroupConfig, GroupHandle, GroupProvider, Welcome};
pub use peer_discovery::{NoPeerDiscovery, PeerDiscovery, PeerDiscoveryService};
pub use types::{ChannelDescriptor, ChatMessage, InviteToken};
