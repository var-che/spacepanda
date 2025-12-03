/*
    Sync subsystem - Apply local and remote operations

    Handles propagation and reconciliation of CRDT state.
*/

pub mod anti_entropy;
pub mod apply_local;
pub mod apply_remote;
pub mod delta_decoder;
pub mod delta_encoder;

pub use anti_entropy::{
    AntiEntropyConfig, AntiEntropyManager, PeerSyncState, SyncRequest, SyncResponse,
};
pub use apply_local::{apply_local_to_channel, apply_local_to_space, LocalContext, LocalOperation};
pub use apply_remote::{
    apply_remote_to_channel, apply_remote_to_space, RemoteContext, RemoteOperation,
};
pub use delta_decoder::{DeltaApplier, DeltaDecoder};
pub use delta_encoder::{Delta, DeltaEncoder, DeltaOperation};
