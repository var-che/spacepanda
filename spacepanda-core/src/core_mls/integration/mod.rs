//! Integration Bridges
//!
//! This module provides bridges between MLS and other subsystems.

pub mod dht_bridge;
pub mod identity_bridge;

pub use dht_bridge::DhtBridgeImpl;
pub use identity_bridge::IdentityBridgeImpl;
