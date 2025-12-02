//! OpenMLS Engine Wrapper
//!
//! This module wraps OpenMLS MlsGroup to provide the same API as our custom implementation,
//! allowing us to maintain backward compatibility while using battle-tested OpenMLS internals.

pub mod openmls_engine;
pub mod message_adapter;
pub mod group_ops;

pub use openmls_engine::OpenMlsEngine;
pub use message_adapter::{MessageAdapter, WireFormat};
pub use group_ops::GroupOperations;
