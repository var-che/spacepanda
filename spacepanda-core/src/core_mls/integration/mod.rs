//! Integration Bridges
//!
//! This module provides bridges between MLS and other subsystems.

pub mod identity_bridge;
pub mod dht_bridge;

pub use identity_bridge::IdentityBridgeImpl;
pub use dht_bridge::DhtBridgeImpl;
