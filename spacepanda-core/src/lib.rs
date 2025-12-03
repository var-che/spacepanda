//! SpacePanda Core Library
//!
//! This library provides the core functionality for the SpacePanda protocol.

// Allow some clippy warnings that are not critical for alpha release
#![allow(
    clippy::needless_borrows_for_generic_args,
    clippy::unnecessary_cast,
    clippy::useless_format,
    clippy::redundant_closure,
    clippy::or_fun_call,
    clippy::manual_is_multiple_of,
    clippy::manual_div_ceil,
    clippy::redundant_pattern_matching,
    clippy::async_yields_async,
    clippy::collapsible_if,
    clippy::ifs_same_cond,
    clippy::manual_range_contains,
    clippy::to_string_in_format_args,
    clippy::derivable_impls,
    clippy::let_and_return,
    clippy::needless_borrow,
    clippy::manual_unwrap_or_default,
    clippy::large_enum_variant,
    clippy::type_complexity,
    clippy::manual_unwrap_or,
    clippy::should_implement_trait,
    clippy::explicit_auto_deref,
    clippy::needless_range_loop,
    clippy::explicit_deref_methods,
    clippy::new_without_default,
    clippy::unnecessary_lazy_evaluations,
    clippy::if_same_then_else,
    clippy::op_ref,
    clippy::unwrap_or_default,
    dead_code,
    unused_variables,
    unused_imports
)]
// Suppress async trait warnings (design decision - requires Send bounds or AFIT stabilization)
#![allow(async_fn_in_trait)]
// Privacy warnings - intentional module design decisions
#![allow(private_interfaces)]

pub mod core_dht;
pub mod core_identity;
pub mod core_mls;
pub mod core_router;
pub mod core_store;
pub mod logging;

#[cfg(test)]
pub mod test_utils;

pub use core_dht::{DhtConfig, DhtKey, DhtNode, DhtValue};
pub use core_identity::{
    ChannelHash, ChannelIdentity, DeviceId, DeviceMetadata, GlobalIdentity, IdentityError, KeyType,
    Keypair, StoredIdentity, UserId, UserMetadata,
};
pub use core_mls::{GroupId, MlsConfig, MlsError, MlsResult};
pub use core_router::{TransportCommand, TransportEvent, TransportManager};
pub use logging::{init_logging, LogLevel};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_library_exports() {
        // Ensure the main exports are accessible
        let _ = LogLevel::Info;
    }
}
