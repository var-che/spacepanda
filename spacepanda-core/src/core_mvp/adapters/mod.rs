//! Adapters for different MLS implementations

pub mod core_mls_adapter;
pub mod mock_provider;

pub use core_mls_adapter::CoreMlsAdapter;
pub use mock_provider::MockGroupProvider;
