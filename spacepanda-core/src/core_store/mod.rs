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
pub use crdt::{Crdt, VectorClock, LWWRegister};
pub use model::{
    Timestamp, SpaceId, ChannelId, MessageId, UserId,
    ChannelType, PermissionLevel,
};
pub use store::{StoreError, StoreResult};
