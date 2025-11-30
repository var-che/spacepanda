/*
    apply_remote.rs - Apply remote CRDT operations
    
    This module handles operations received from other peers:
    - Via direct P2P connection (Router)
    - Via DHT sync
    
    Flow:
    1. Receive CRDT operation from network
    2. Validate signature
    3. Check vector clock for causality
    4. Apply using CRDT merge semantics
    5. Update local state
    6. Persist to commit log
*/

use crate::core_store::crdt::{Crdt, OperationMetadata};
use crate::core_store::model::{Channel, Space};
use crate::core_store::store::errors::{StoreResult, StoreError};
use serde::{Deserialize, Serialize};

/// Remote CRDT operation wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteOperation<T> {
    /// The actual CRDT operation
    pub operation: T,
    
    /// Metadata (timestamp, node_id, vector clock)
    pub metadata: OperationMetadata,
    
    /// Signature over (operation + metadata)
    pub signature: Vec<u8>,
    
    /// Public key of sender
    pub sender_pubkey: Vec<u8>,
}

/// Validate and apply a remote operation to a CRDT
pub fn apply_remote_operation<C: Crdt>(
    crdt: &mut C,
    op: C::Operation,
    _metadata: &OperationMetadata,
) -> StoreResult<()> {
    // TODO: Validate signature
    // TODO: Check vector clock for causality violations
    
    crdt.apply(op)?;
    Ok(())
}

/// Apply a remote operation to channel state
pub fn apply_remote_to_channel(
    channel: &mut Channel,
    remote_channel: &Channel,
) -> StoreResult<()> {
    // Merge all CRDT fields
    channel.name.merge(&remote_channel.name);
    channel.topic.merge(&remote_channel.topic);
    channel.members.merge(&remote_channel.members)?;
    channel.pinned_messages.merge(&remote_channel.pinned_messages)?;
    channel.permissions.merge_nested(&remote_channel.permissions)?;
    channel.mls_identity.merge(&remote_channel.mls_identity)?;
    
    Ok(())
}

/// Apply a remote operation to space state
pub fn apply_remote_to_space(
    space: &mut Space,
    remote_space: &Space,
) -> StoreResult<()> {
    // Merge all CRDT fields
    space.name.merge(&remote_space.name);
    space.description.merge(&remote_space.description);
    space.channels.merge(&remote_space.channels)?;
    space.members.merge(&remote_space.members)?;
    space.roles.merge_nested(&remote_space.roles)?;
    space.member_roles.merge_nested(&remote_space.member_roles)?;
    space.mls_identity.merge(&remote_space.mls_identity)?;
    
    Ok(())
}

/// Context for remote operations
pub struct RemoteContext {
    /// Whether to validate signatures
    pub validate_signatures: bool,
    
    /// Whether to check causality
    pub check_causality: bool,
}

impl Default for RemoteContext {
    fn default() -> Self {
        RemoteContext {
            validate_signatures: true,
            check_causality: true,
        }
    }
}

impl RemoteContext {
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Create context with signature validation disabled (for testing)
    pub fn no_validation() -> Self {
        RemoteContext {
            validate_signatures: false,
            check_causality: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core_store::model::{ChannelId, UserId, ChannelType, Timestamp};
    use crate::core_store::crdt::{VectorClock, AddId};
    
    #[test]
    fn test_apply_remote_to_channel() {
        let channel_id = ChannelId::generate();
        let creator = UserId::generate();
        let now = Timestamp::now();
        
        let mut local_channel = Channel::new(
            channel_id.clone(),
            "Local".to_string(),
            ChannelType::Text,
            creator.clone(),
            now,
            "node1".to_string(),
        );
        
        let mut remote_channel = Channel::new(
            channel_id,
            "Remote".to_string(),
            ChannelType::Text,
            creator,
            now,
            "node2".to_string(),
        );
        
        // Update remote with later timestamp
        let mut vc = VectorClock::new();
        vc.increment("node2");
        let ts = Timestamp::now().as_millis();
        remote_channel.name.set("Updated".to_string(), ts + 1000, "node2".to_string(), vc);
        
        // Apply remote to local
        apply_remote_to_channel(&mut local_channel, &remote_channel).unwrap();
        
        // Should have the later name
        assert_eq!(local_channel.get_name(), Some(&"Updated".to_string()));
    }
    
    #[test]
    fn test_apply_remote_members_merge() {
        let channel_id = ChannelId::generate();
        let creator = UserId::generate();
        let now = Timestamp::now();
        
        let mut local_channel = Channel::new(
            channel_id.clone(),
            "Test".to_string(),
            ChannelType::Text,
            creator.clone(),
            now,
            "node1".to_string(),
        );
        
        let mut remote_channel = Channel::new(
            channel_id,
            "Test".to_string(),
            ChannelType::Text,
            creator,
            now,
            "node2".to_string(),
        );
        
        // Add different members
        let alice = UserId::generate();
        let bob = UserId::generate();
        
        let mut vc1 = VectorClock::new();
        vc1.increment("node1");
        let mut vc2 = VectorClock::new();
        vc2.increment("node2");
        
        let add_id1 = AddId::new(alice.0.clone(), Timestamp::now().as_millis());
        let add_id2 = AddId::new(bob.0.clone(), Timestamp::now().as_millis());
        
        local_channel.members.add(alice.clone(), add_id1, vc1);
        remote_channel.members.add(bob.clone(), add_id2, vc2);
        
        // Merge
        apply_remote_to_channel(&mut local_channel, &remote_channel).unwrap();
        
        // Should have both members
        assert!(local_channel.has_member(&alice));
        assert!(local_channel.has_member(&bob));
    }
}
