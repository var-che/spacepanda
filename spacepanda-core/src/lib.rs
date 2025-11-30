pub mod core_dht;
pub mod core_identity;
pub mod core_router;
pub mod core_store;
pub mod logging;

pub use core_dht::{DhtConfig, DhtKey, DhtNode, DhtValue};
pub use core_identity::{
    ChannelHash, ChannelIdentity, GlobalIdentity, IdentityError, Keypair,
    StoredIdentity, UserId, DeviceId, KeyType, UserMetadata, DeviceMetadata,
};
pub use logging::{init_logging, LogLevel};
pub use core_router::{TransportCommand, TransportEvent, TransportManager};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_library_exports() {
        // Ensure the main exports are accessible
        let _ = LogLevel::Info;
    }
}
