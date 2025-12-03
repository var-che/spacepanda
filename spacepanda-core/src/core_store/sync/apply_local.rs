/*
    apply_local.rs - Apply local user actions to CRDT state

    This module handles operations initiated by the local user:
    - Sending a message
    - Editing channel metadata
    - Adding/removing members
    - Assigning roles
    - etc.

    Flow:
    1. User action comes from UI/API
    2. Generate CRDT operation with local node_id and timestamp
    3. Apply to local state
    4. Persist to commit log
    5. Broadcast to peers (via router) and DHT
*/

use crate::core_store::crdt::{AddId, OperationMetadata, VectorClock};
use crate::core_store::model::{Channel, MessageId, Space, Timestamp, UserId};
use crate::core_store::store::errors::{StoreError, StoreResult};
use serde::{Deserialize, Serialize};

/// Local operations that a user can perform
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LocalOperation {
    /// Send a message
    SendMessage {
        channel_id: String,
        content: Vec<u8>, // MLS-encrypted content
        reply_to: Option<MessageId>,
    },

    /// Edit a message
    EditMessage { message_id: MessageId, new_content: Vec<u8> },

    /// Delete a message
    DeleteMessage { message_id: MessageId },

    /// Update channel name
    UpdateChannelName { channel_id: String, new_name: String },

    /// Update channel topic
    UpdateChannelTopic { channel_id: String, new_topic: String },

    /// Add member to channel
    AddChannelMember { channel_id: String, user_id: UserId },

    /// Remove member from channel
    RemoveChannelMember { channel_id: String, user_id: UserId },

    /// Update space name
    UpdateSpaceName { space_id: String, new_name: String },

    /// Assign role to user
    AssignRole { space_id: String, user_id: UserId, role_name: String },
}

/// Context for applying local operations
pub struct LocalContext {
    /// Local node identifier
    pub node_id: String,

    /// Local user ID
    pub user_id: UserId,

    /// Vector clock for this node
    pub vector_clock: VectorClock,
}

impl LocalContext {
    pub fn new(node_id: String, user_id: UserId) -> Self {
        LocalContext { node_id, user_id, vector_clock: VectorClock::new() }
    }

    /// Increment vector clock and return operation metadata
    pub fn next_metadata(&mut self) -> OperationMetadata {
        self.vector_clock.increment(&self.node_id);
        OperationMetadata {
            node_id: self.node_id.clone(),
            timestamp: Timestamp::now().as_millis(),
            vector_clock: self.vector_clock.clone(),
            signature: None,
        }
    }
}

/// Apply a local operation to channel state
pub fn apply_local_to_channel(
    channel: &mut Channel,
    op: LocalOperation,
    ctx: &mut LocalContext,
) -> StoreResult<()> {
    match op {
        LocalOperation::UpdateChannelName { new_name, .. } => {
            let metadata = ctx.next_metadata();
            channel.name.set(
                new_name,
                metadata.timestamp,
                metadata.node_id.clone(),
                metadata.vector_clock,
            );
            Ok(())
        }

        LocalOperation::UpdateChannelTopic { new_topic, .. } => {
            let metadata = ctx.next_metadata();
            channel.topic.set(
                new_topic,
                metadata.timestamp,
                metadata.node_id.clone(),
                metadata.vector_clock,
            );
            Ok(())
        }

        LocalOperation::AddChannelMember { user_id, .. } => {
            let metadata = ctx.next_metadata();
            let add_id = AddId::new(user_id.0.clone(), metadata.timestamp);
            channel.members.add(user_id, add_id, metadata.vector_clock);
            Ok(())
        }

        LocalOperation::RemoveChannelMember { user_id, .. } => {
            let metadata = ctx.next_metadata();
            channel.members.remove(&user_id, metadata.vector_clock);
            Ok(())
        }

        _ => Err(StoreError::InvalidOperation("Operation not applicable to channel".to_string())),
    }
}

/// Apply a local operation to space state
pub fn apply_local_to_space(
    space: &mut Space,
    op: LocalOperation,
    ctx: &mut LocalContext,
) -> StoreResult<()> {
    match op {
        LocalOperation::UpdateSpaceName { new_name, .. } => {
            let metadata = ctx.next_metadata();
            space.name.set(
                new_name,
                metadata.timestamp,
                metadata.node_id.clone(),
                metadata.vector_clock,
            );
            Ok(())
        }

        LocalOperation::AssignRole { user_id, role_name, .. } => {
            let metadata = ctx.next_metadata();
            let mut role_register = crate::core_store::crdt::LWWRegister::new();
            role_register.set(
                role_name,
                metadata.timestamp,
                metadata.node_id.clone(),
                metadata.vector_clock.clone(),
            );

            let add_id = AddId::new(user_id.0.clone(), metadata.timestamp);
            space.member_roles.put(user_id, role_register, add_id, metadata.vector_clock);
            Ok(())
        }

        _ => Err(StoreError::InvalidOperation("Operation not applicable to space".to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core_store::model::{ChannelId, ChannelType, SpaceId};

    #[test]
    fn test_apply_local_channel_name() {
        use std::thread;
        use std::time::Duration;

        let channel_id = ChannelId::generate();
        let creator = UserId::generate();
        let now = Timestamp::now();

        let mut channel = Channel::new(
            channel_id,
            "Old Name".to_string(),
            ChannelType::Text,
            creator.clone(),
            now,
            "node1".to_string(),
        );

        // Small delay to ensure new timestamp is later
        thread::sleep(Duration::from_millis(2));

        let mut ctx = LocalContext::new("node1".to_string(), creator);

        let op = LocalOperation::UpdateChannelName {
            channel_id: "test".to_string(),
            new_name: "New Name".to_string(),
        };

        apply_local_to_channel(&mut channel, op, &mut ctx).unwrap();

        assert_eq!(channel.get_name(), Some(&"New Name".to_string()));
    }

    #[test]
    fn test_apply_local_add_member() {
        let channel_id = ChannelId::generate();
        let creator = UserId::generate();
        let new_member = UserId::generate();
        let now = Timestamp::now();

        let mut channel = Channel::new(
            channel_id,
            "Test".to_string(),
            ChannelType::Text,
            creator.clone(),
            now,
            "node1".to_string(),
        );

        let mut ctx = LocalContext::new("node1".to_string(), creator);

        let op = LocalOperation::AddChannelMember {
            channel_id: "test".to_string(),
            user_id: new_member.clone(),
        };

        apply_local_to_channel(&mut channel, op, &mut ctx).unwrap();

        assert!(channel.has_member(&new_member));
    }
}
