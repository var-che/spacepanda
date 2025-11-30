/*
    Store subsystem - Persistence layer
*/

pub mod errors;
pub mod local_store;
pub mod commit_log;
pub mod snapshot;
pub mod index;
pub mod encryption;
pub mod validator;
pub mod dht_adapter;

pub use errors::*;
pub use local_store::{LocalStore, LocalStoreConfig, StoreStats};
pub use commit_log::{CommitLog, LogEntry};
pub use snapshot::{SnapshotManager, Snapshot, SnapshotMetadata};
pub use index::IndexManager;
pub use encryption::EncryptionManager;
pub use validator::{OperationValidator, ValidationRules};
pub use dht_adapter::{DhtAdapter, DhtObjectKey, DhtDelta};
