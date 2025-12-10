//! Space invite system

use super::types::SpaceId;
use crate::core_store::model::types::{Timestamp, UserId};
use serde::{Deserialize, Serialize};

/// Space invitation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpaceInvite {
    /// Unique identifier
    pub id: String,

    /// Target Space
    pub space_id: SpaceId,

    /// Type of invite (link, code, or direct)
    pub invite_type: InviteType,

    /// Who created the invite
    pub created_by: UserId,

    /// When the invite was created
    pub created_at: Timestamp,

    /// Optional expiration time
    pub expires_at: Option<Timestamp>,

    /// Maximum number of uses (None = unlimited)
    pub max_uses: Option<u32>,

    /// Current use count
    pub use_count: u32,

    /// Whether the invite has been revoked
    pub revoked: bool,
}

impl SpaceInvite {
    /// Create a new invite link
    pub fn new_link(
        space_id: SpaceId,
        created_by: UserId,
        max_uses: Option<u32>,
        expires_at: Option<Timestamp>,
    ) -> Self {
        let invite_code = Self::generate_invite_code();

        SpaceInvite {
            id: format!("inv_{}", invite_code),
            space_id,
            invite_type: InviteType::Link(invite_code),
            created_by,
            created_at: Timestamp::now(),
            expires_at,
            max_uses,
            use_count: 0,
            revoked: false,
        }
    }

    /// Create a new invite code
    pub fn new_code(
        space_id: SpaceId,
        created_by: UserId,
        max_uses: Option<u32>,
        expires_at: Option<Timestamp>,
    ) -> Self {
        let invite_code = Self::generate_invite_code();

        SpaceInvite {
            id: format!("code_{}", invite_code),
            space_id,
            invite_type: InviteType::Code(invite_code),
            created_by,
            created_at: Timestamp::now(),
            expires_at,
            max_uses,
            use_count: 0,
            revoked: false,
        }
    }

    /// Create a direct invite to a specific user
    pub fn new_direct(
        space_id: SpaceId,
        created_by: UserId,
        target_user: UserId,
        expires_at: Option<Timestamp>,
    ) -> Self {
        SpaceInvite {
            id: format!("direct_{}_{}", space_id, target_user),
            space_id,
            invite_type: InviteType::Direct(target_user),
            created_by,
            created_at: Timestamp::now(),
            expires_at,
            max_uses: Some(1), // Direct invites are single-use
            use_count: 0,
            revoked: false,
        }
    }

    /// Check if the invite is valid
    pub fn is_valid(&self) -> Result<(), InviteError> {
        if self.revoked {
            return Err(InviteError::InviteRevoked);
        }

        if let Some(expires_at) = self.expires_at {
            if Timestamp::now() > expires_at {
                return Err(InviteError::InviteExpired);
            }
        }

        if let Some(max_uses) = self.max_uses {
            if self.use_count >= max_uses {
                return Err(InviteError::InviteMaxUsesReached);
            }
        }

        Ok(())
    }

    /// Increment the use count
    pub fn increment_use(&mut self) -> Result<(), InviteError> {
        self.is_valid()?;
        self.use_count += 1;
        Ok(())
    }

    /// Revoke the invite
    pub fn revoke(&mut self) {
        self.revoked = true;
    }

    /// Generate a random invite code
    fn generate_invite_code() -> String {
        use rand::Rng;
        const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
        const CODE_LEN: usize = 8;

        let mut rng = rand::rng();
        (0..CODE_LEN)
            .map(|_| {
                let idx = rng.random_range(0..CHARSET.len());
                CHARSET[idx] as char
            })
            .collect()
    }
}

/// Types of invites
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InviteType {
    /// Shareable link (e.g., "https://app.com/invite/ABC123")
    Link(String),

    /// Code that can be entered (e.g., "ABC123")
    Code(String),

    /// Direct invite to a specific user
    Direct(UserId),
}

/// Invite operation errors
#[derive(Debug, thiserror::Error)]
pub enum InviteError {
    #[error("Invite has been revoked")]
    InviteRevoked,

    #[error("Invite has expired")]
    InviteExpired,

    #[error("Invite has reached maximum uses")]
    InviteMaxUsesReached,

    #[error("Invite not found")]
    InviteNotFound,

    #[error("User not authorized to use this invite")]
    UnauthorizedUser,

    #[error("Invalid invite code")]
    InvalidInviteCode,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_link_invite() {
        let space_id = SpaceId::generate();
        let creator = UserId::new("alice".to_string());

        let invite = SpaceInvite::new_link(space_id, creator, None, None);

        assert!(matches!(invite.invite_type, InviteType::Link(_)));
        assert_eq!(invite.use_count, 0);
        assert!(!invite.revoked);
        assert!(invite.is_valid().is_ok());
    }

    #[test]
    fn test_create_code_invite() {
        let space_id = SpaceId::generate();
        let creator = UserId::new("alice".to_string());

        let invite = SpaceInvite::new_code(space_id, creator, Some(10), None);

        assert!(matches!(invite.invite_type, InviteType::Code(_)));
        assert_eq!(invite.max_uses, Some(10));
        assert!(invite.is_valid().is_ok());
    }

    #[test]
    fn test_create_direct_invite() {
        let space_id = SpaceId::generate();
        let creator = UserId::new("alice".to_string());
        let target = UserId::new("bob".to_string());

        let invite = SpaceInvite::new_direct(space_id, creator, target.clone(), None);

        assert!(matches!(invite.invite_type, InviteType::Direct(_)));
        assert_eq!(invite.max_uses, Some(1)); // Direct invites are single-use
    }

    #[test]
    fn test_invite_use_count() {
        let space_id = SpaceId::generate();
        let creator = UserId::new("alice".to_string());

        let mut invite = SpaceInvite::new_link(space_id, creator, Some(3), None);

        invite.increment_use().unwrap();
        assert_eq!(invite.use_count, 1);

        invite.increment_use().unwrap();
        assert_eq!(invite.use_count, 2);

        invite.increment_use().unwrap();
        assert_eq!(invite.use_count, 3);

        let result = invite.increment_use();
        assert!(matches!(result, Err(InviteError::InviteMaxUsesReached)));
    }

    #[test]
    fn test_revoke_invite() {
        let space_id = SpaceId::generate();
        let creator = UserId::new("alice".to_string());

        let mut invite = SpaceInvite::new_link(space_id, creator, None, None);
        assert!(invite.is_valid().is_ok());

        invite.revoke();
        assert!(invite.revoked);

        let result = invite.is_valid();
        assert!(matches!(result, Err(InviteError::InviteRevoked)));
    }

    #[test]
    fn test_expired_invite() {
        let space_id = SpaceId::generate();
        let creator = UserId::new("alice".to_string());

        // Create an invite that expired 1 hour ago
        let past_time = Timestamp::from_millis(
            Timestamp::now().as_millis() - 3600000
        );

        let invite = SpaceInvite::new_link(space_id, creator, None, Some(past_time));

        let result = invite.is_valid();
        assert!(matches!(result, Err(InviteError::InviteExpired)));
    }

    #[test]
    fn test_invite_code_format() {
        let space_id = SpaceId::generate();
        let creator = UserId::new("alice".to_string());

        let invite = SpaceInvite::new_code(space_id, creator, None, None);

        if let InviteType::Code(code) = invite.invite_type {
            assert_eq!(code.len(), 8); // Default code length
            assert!(code.chars().all(|c| c.is_ascii_alphanumeric()));
        } else {
            panic!("Expected Code invite type");
        }
    }
}
