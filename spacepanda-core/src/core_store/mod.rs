/*
    core_store - Persistent, replicated, secure state layer

    The authoritative state layer for the decentralized E2EE chat platform.
    Handles:
    - Data models (spaces, channels, messages, roles)
    - CRDT-based replication
    - Local persistence
    - DHT synchronization
    - Search and query
*/

pub mod crdt;
pub mod model;
pub mod query;
pub mod store;
pub mod sync;

#[cfg(test)]
pub mod tests;

// Re-export commonly used types
pub use crdt::{Crdt, LWWRegister, VectorClock};
pub use model::{ChannelId, ChannelType, MessageId, PermissionLevel, SpaceId, Timestamp, UserId};
pub use store::{StoreError, StoreResult};
