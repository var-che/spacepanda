//! MLS Events
//!
//! Events emitted by the MLS engine for consumption by other subsystems.

use serde::{Deserialize, Serialize};

/// MLS event type
///
/// Events are emitted by the MLS engine to notify other subsystems
/// (router, store, DHT) of important state changes.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum MlsEvent {
    /// A new member was added to the group
    MemberAdded {
        group_id: Vec<u8>,
        member_id: Vec<u8>,
        epoch: u64,
    },

    /// A member was removed from the group
    MemberRemoved {
        group_id: Vec<u8>,
        member_id: Vec<u8>,
        epoch: u64,
    },

    /// A member updated their leaf key
    MemberUpdated {
        group_id: Vec<u8>,
        member_id: Vec<u8>,
        epoch: u64,
    },

    /// Epoch advanced (commit was applied)
    EpochChanged {
        group_id: Vec<u8>,
        old_epoch: u64,
        new_epoch: u64,
    },

    /// Application message received and decrypted
    MessageReceived {
        group_id: Vec<u8>,
        sender_id: Vec<u8>,
        epoch: u64,
        plaintext: Vec<u8>,
    },

    /// Welcome message processed (joined group)
    GroupJoined {
        group_id: Vec<u8>,
        epoch: u64,
        member_count: usize,
    },

    /// Group was created
    GroupCreated {
        group_id: Vec<u8>,
        creator_id: Vec<u8>,
    },

    /// Left the group
    GroupLeft {
        group_id: Vec<u8>,
        final_epoch: u64,
    },

    /// Proposal was created
    ProposalCreated {
        group_id: Vec<u8>,
        proposal_type: ProposalType,
        epoch: u64,
    },

    /// Commit was created
    CommitCreated {
        group_id: Vec<u8>,
        epoch: u64,
        proposal_count: usize,
    },

    /// Error occurred
    Error {
        group_id: Vec<u8>,
        error: String,
    },
}

/// Proposal type enumeration
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ProposalType {
    Add,
    Remove,
    Update,
    PreSharedKey,
    ReInit,
    ExternalInit,
    GroupContextExtensions,
}

impl MlsEvent {
    /// Get the group ID associated with this event
    pub fn group_id(&self) -> &[u8] {
        match self {
            MlsEvent::MemberAdded { group_id, .. } => group_id,
            MlsEvent::MemberRemoved { group_id, .. } => group_id,
            MlsEvent::MemberUpdated { group_id, .. } => group_id,
            MlsEvent::EpochChanged { group_id, .. } => group_id,
            MlsEvent::MessageReceived { group_id, .. } => group_id,
            MlsEvent::GroupJoined { group_id, .. } => group_id,
            MlsEvent::GroupCreated { group_id, .. } => group_id,
            MlsEvent::GroupLeft { group_id, .. } => group_id,
            MlsEvent::ProposalCreated { group_id, .. } => group_id,
            MlsEvent::CommitCreated { group_id, .. } => group_id,
            MlsEvent::Error { group_id, .. } => group_id,
        }
    }

    /// Get the epoch associated with this event (if applicable)
    pub fn epoch(&self) -> Option<u64> {
        match self {
            MlsEvent::MemberAdded { epoch, .. } => Some(*epoch),
            MlsEvent::MemberRemoved { epoch, .. } => Some(*epoch),
            MlsEvent::MemberUpdated { epoch, .. } => Some(*epoch),
            MlsEvent::EpochChanged { new_epoch, .. } => Some(*new_epoch),
            MlsEvent::MessageReceived { epoch, .. } => Some(*epoch),
            MlsEvent::GroupJoined { epoch, .. } => Some(*epoch),
            MlsEvent::GroupLeft { final_epoch, .. } => Some(*final_epoch),
            MlsEvent::ProposalCreated { epoch, .. } => Some(*epoch),
            MlsEvent::CommitCreated { epoch, .. } => Some(*epoch),
            _ => None,
        }
    }

    /// Check if this is an error event
    pub fn is_error(&self) -> bool {
        matches!(self, MlsEvent::Error { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_group_id() {
        let event = MlsEvent::MemberAdded {
            group_id: vec![1, 2, 3],
            member_id: vec![4, 5, 6],
            epoch: 5,
        };
        assert_eq!(event.group_id(), &[1, 2, 3]);
    }

    #[test]
    fn test_event_epoch() {
        let event = MlsEvent::EpochChanged {
            group_id: vec![1, 2, 3],
            old_epoch: 4,
            new_epoch: 5,
        };
        assert_eq!(event.epoch(), Some(5));
    }

    #[test]
    fn test_is_error() {
        let error_event = MlsEvent::Error {
            group_id: vec![1, 2, 3],
            error: "test error".to_string(),
        };
        assert!(error_event.is_error());

        let normal_event = MlsEvent::GroupCreated {
            group_id: vec![1, 2, 3],
            creator_id: vec![4, 5, 6],
        };
        assert!(!normal_event.is_error());
    }

    #[test]
    fn test_event_serialization() {
        let event = MlsEvent::MessageReceived {
            group_id: vec![1, 2, 3],
            sender_id: vec![4, 5, 6],
            epoch: 10,
            plaintext: vec![7, 8, 9],
        };

        let serialized = serde_json::to_string(&event).unwrap();
        let deserialized: MlsEvent = serde_json::from_str(&serialized).unwrap();

        assert_eq!(event.group_id(), deserialized.group_id());
        assert_eq!(event.epoch(), deserialized.epoch());
    }
}
