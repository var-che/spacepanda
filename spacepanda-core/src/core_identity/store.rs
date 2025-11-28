/*
    This module is a local persistence mechanism.

    save_global_identity(global_identity: &GlobalIdentity) -> Result<(), IdentityError>
    load_global_identity() -> Result<GlobalIdentity, IdentityError>
    save_channel_identity(channel_hash: &ChannelHash) -> Result<(), IdentityError>
    load_channel_identity(channel_hash: &ChannelHash) -> Result<ChannelIdentity, IdentityError>

    and encryption at rest that is optional for mobile users.

    For now, we will implement a single local encrypted file (JSON), for the POC
*/

use std::collections::HashMap;
use crate::core_identity::global::GlobalIdentity;
use crate::core_identity::channel::{ChannelIdentity, ChannelHash};

pub struct StoredIdentity {
    pub global: GlobalIdentity,
    pub channels: HashMap<ChannelHash, ChannelIdentity>,
}

