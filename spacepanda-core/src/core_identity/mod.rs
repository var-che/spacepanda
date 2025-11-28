//! Identity management module
//!
//! API that we will be exposing:
//! - `create_global_identity()` -> creates a new global identity, generates a new Ed25519 keypair, saves to disk
//! - `load_global_identity()` -> loads the global identity from disk
//! - `create_channel_identity()` -> creates a new channel identity, signed by the global identity
//! - `identity_path(user_home: &Path) -> PathBuf` -> returns the path where the global identity is stored

mod global;

pub use global::{ChannelIdentity, Ed25519Keypair, GlobalIdentity, IdentityError};
