//! OpenMLS Engine Wrapper
//!
//! This module wraps OpenMLS MlsGroup to provide the same API as our custom implementation,
//! allowing us to maintain backward compatibility while using battle-tested OpenMLS internals.

pub mod adapter;
pub mod group_ops;
pub mod message_adapter;
pub mod openmls_engine;

pub use adapter::OpenMlsHandleAdapter;
pub use group_ops::GroupOperations;
pub use message_adapter::{MessageAdapter, WireFormat};
pub use openmls_engine::OpenMlsEngine;
