/*
    Sync subsystem - Apply local and remote operations
    
    Handles propagation and reconciliation of CRDT state.
*/

pub mod apply_local;
pub mod apply_remote;
pub mod delta_encoder;
pub mod delta_decoder;
pub mod anti_entropy;

pub use apply_local::{LocalOperation, LocalContext, apply_local_to_channel, apply_local_to_space};
pub use apply_remote::{RemoteOperation, RemoteContext, apply_remote_to_channel, apply_remote_to_space};
pub use delta_encoder::{DeltaEncoder, Delta, DeltaOperation};
pub use delta_decoder::{DeltaDecoder, DeltaApplier};
pub use anti_entropy::{AntiEntropyManager, AntiEntropyConfig, PeerSyncState, SyncRequest, SyncResponse};
