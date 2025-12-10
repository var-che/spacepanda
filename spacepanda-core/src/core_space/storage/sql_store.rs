//! SQL-based storage implementation for Spaces and Channels

use super::super::channel::{Channel, ChannelError, ChannelVisibility};
use super::super::invite::{InviteError, InviteType, SpaceInvite};
use super::super::space::{Space, SpaceError, SpaceMember, SpaceRole, SpaceVisibility};
use super::super::types::{ChannelId, SpaceId};
use crate::core_mls::types::GroupId;
use crate::core_store::model::types::{Timestamp, UserId};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{params, OptionalExtension, Row};
use std::collections::{HashMap, HashSet};

/// SQL-based storage for Spaces and Channels
pub struct SpaceSqlStore {
    pool: Pool<SqliteConnectionManager>,
}

impl SpaceSqlStore {
    /// Create a new SQL store with the given connection pool
    pub fn new(pool: Pool<SqliteConnectionManager>) -> Result<Self, rusqlite::Error> {
        // Run migrations
        super::migrations::migrate(&pool)?;

        Ok(Self { pool })
    }

    /// Create a new in-memory store (for testing)
    #[cfg(test)]
    pub fn memory() -> Result<Self, rusqlite::Error> {
        let manager = SqliteConnectionManager::memory();
        let pool = Pool::new(manager).map_err(|e| {
            rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to create pool: {}", e),
            )))
        })?;

        Self::new(pool)
    }

    // ===== Space Operations =====

    /// Insert a new Space into the database
    pub fn create_space(&self, space: &Space) -> Result<(), SpaceError> {
        let conn = self.pool.get().map_err(|e| {
            SpaceError::PermissionDenied // TODO: Add StorageError variant
        })?;

        let tx = conn.unchecked_transaction().map_err(|_| SpaceError::PermissionDenied)?;

        // Insert space
        tx.execute(
            "INSERT INTO spaces (id, name, description, icon_url, visibility, owner_id, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                space.id.as_bytes(),
                &space.name,
                &space.description,
                &space.icon_url,
                match space.visibility {
                    SpaceVisibility::Public => "Public",
                    SpaceVisibility::Private => "Private",
                },
                space.owner_id.to_string(),
                space.created_at.as_millis() as i64,
                space.updated_at.as_millis() as i64,
            ],
        )
        .map_err(|_| SpaceError::PermissionDenied)?;

        // Insert members
        for (user_id, member) in &space.members {
            tx.execute(
                "INSERT INTO space_members (space_id, user_id, role, joined_at, invited_by)
                 VALUES (?, ?, ?, ?, ?)",
                params![
                    space.id.as_bytes(),
                    user_id.to_string(),
                    match member.role {
                        SpaceRole::Owner => "Owner",
                        SpaceRole::Admin => "Admin",
                        SpaceRole::Member => "Member",
                    },
                    member.joined_at.as_millis() as i64,
                    member.invited_by.as_ref().map(|id| id.to_string()),
                ],
            )
            .map_err(|_| SpaceError::PermissionDenied)?;
        }

        tx.commit().map_err(|_| SpaceError::PermissionDenied)?;

        Ok(())
    }

    /// Get a Space by ID
    pub fn get_space(&self, space_id: &SpaceId) -> Result<Space, SpaceError> {
        let conn = self.pool.get().map_err(|_| SpaceError::PermissionDenied)?;

        // Get space metadata
        let mut space: Space = conn
            .query_row(
                "SELECT id, name, description, icon_url, visibility, owner_id, created_at, updated_at
                 FROM spaces WHERE id = ?",
                params![space_id.as_bytes()],
                |row| {
                    let visibility_str: String = row.get(4)?;
                    let visibility = match visibility_str.as_str() {
                        "Public" => SpaceVisibility::Public,
                        "Private" => SpaceVisibility::Private,
                        _ => SpaceVisibility::Private,
                    };

                    Ok(Space {
                        id: SpaceId::from_bytes(*space_id.as_bytes()),
                        name: row.get(1)?,
                        description: row.get(2)?,
                        icon_url: row.get(3)?,
                        visibility,
                        owner_id: UserId::new(row.get(5)?),
                        members: HashMap::new(),
                        channels: Vec::new(),
                        created_at: Timestamp::from_millis(row.get::<_, i64>(6)?.max(0) as u64),
                        updated_at: Timestamp::from_millis(row.get::<_, i64>(7)?.max(0) as u64),
                    })
                },
            )
            .optional()
            .map_err(|_| SpaceError::PermissionDenied)?
            .ok_or(SpaceError::PermissionDenied)?; // TODO: Add NotFound variant

        // Get members
        let mut stmt = conn
            .prepare(
                "SELECT user_id, role, joined_at, invited_by
                 FROM space_members WHERE space_id = ?",
            )
            .map_err(|_| SpaceError::PermissionDenied)?;

        let members = stmt
            .query_map(params![space_id.as_bytes()], |row| {
                let user_id = UserId::new(row.get(0)?);
                let role_str: String = row.get(1)?;
                let role = match role_str.as_str() {
                    "Owner" => SpaceRole::Owner,
                    "Admin" => SpaceRole::Admin,
                    "Member" => SpaceRole::Member,
                    _ => SpaceRole::Member,
                };

                let member = SpaceMember {
                    user_id: user_id.clone(),
                    role,
                    joined_at: Timestamp::from_millis(row.get::<_, i64>(2)?.max(0) as u64),
                    invited_by: row.get::<_, Option<String>>(3)?.map(UserId::new),
                };

                Ok((user_id, member))
            })
            .map_err(|_| SpaceError::PermissionDenied)?
            .collect::<Result<HashMap<_, _>, _>>()
            .map_err(|_| SpaceError::PermissionDenied)?;

        space.members = members;

        // Get channel IDs
        let mut stmt = conn
            .prepare("SELECT id FROM channels WHERE space_id = ? ORDER BY created_at")
            .map_err(|_| SpaceError::PermissionDenied)?;

        let channels = stmt
            .query_map(params![space_id.as_bytes()], |row| {
                let id_bytes: Vec<u8> = row.get(0)?;
                let mut arr = [0u8; 32];
                arr.copy_from_slice(&id_bytes);
                Ok(ChannelId::from_bytes(arr))
            })
            .map_err(|_| SpaceError::PermissionDenied)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| SpaceError::PermissionDenied)?;

        space.channels = channels;

        Ok(space)
    }

    /// Update a Space's metadata
    pub fn update_space(&self, space: &Space) -> Result<(), SpaceError> {
        let conn = self.pool.get().map_err(|_| SpaceError::PermissionDenied)?;

        conn.execute(
            "UPDATE spaces SET name = ?, description = ?, icon_url = ?, visibility = ?, updated_at = ?
             WHERE id = ?",
            params![
                &space.name,
                &space.description,
                &space.icon_url,
                match space.visibility {
                    SpaceVisibility::Public => "Public",
                    SpaceVisibility::Private => "Private",
                },
                space.updated_at.as_millis() as i64,
                space.id.as_bytes(),
            ],
        )
        .map_err(|_| SpaceError::PermissionDenied)?;

        Ok(())
    }

    /// Delete a Space (cascades to channels and members)
    pub fn delete_space(&self, space_id: &SpaceId) -> Result<(), SpaceError> {
        let conn = self.pool.get().map_err(|_| SpaceError::PermissionDenied)?;

        conn.execute("DELETE FROM spaces WHERE id = ?", params![space_id.as_bytes()])
            .map_err(|_| SpaceError::PermissionDenied)?;

        Ok(())
    }

    /// List all public Spaces
    pub fn list_public_spaces(&self) -> Result<Vec<Space>, SpaceError> {
        let conn = self.pool.get().map_err(|_| SpaceError::PermissionDenied)?;

        let mut stmt = conn
            .prepare(
                "SELECT id FROM spaces WHERE visibility = 'Public' ORDER BY created_at DESC",
            )
            .map_err(|_| SpaceError::PermissionDenied)?;

        let space_ids = stmt
            .query_map([], |row| {
                let id_bytes: Vec<u8> = row.get(0)?;
                let mut arr = [0u8; 32];
                arr.copy_from_slice(&id_bytes);
                Ok(SpaceId::from_bytes(arr))
            })
            .map_err(|_| SpaceError::PermissionDenied)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| SpaceError::PermissionDenied)?;

        let mut spaces = Vec::new();
        for space_id in space_ids {
            spaces.push(self.get_space(&space_id)?);
        }

        Ok(spaces)
    }

    /// List Spaces a user is a member of
    pub fn list_user_spaces(&self, user_id: &UserId) -> Result<Vec<Space>, SpaceError> {
        let conn = self.pool.get().map_err(|_| SpaceError::PermissionDenied)?;

        let mut stmt = conn
            .prepare(
                "SELECT DISTINCT space_id FROM space_members 
                 WHERE user_id = ? ORDER BY joined_at DESC",
            )
            .map_err(|_| SpaceError::PermissionDenied)?;

        let space_ids = stmt
            .query_map(params![user_id.to_string()], |row| {
                let id_bytes: Vec<u8> = row.get(0)?;
                let mut arr = [0u8; 32];
                arr.copy_from_slice(&id_bytes);
                Ok(SpaceId::from_bytes(arr))
            })
            .map_err(|_| SpaceError::PermissionDenied)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| SpaceError::PermissionDenied)?;

        let mut spaces = Vec::new();
        for space_id in space_ids {
            spaces.push(self.get_space(&space_id)?);
        }

        Ok(spaces)
    }

    // ===== Channel Operations =====

    /// Create a new Channel
    pub fn create_channel(&self, channel: &Channel) -> Result<(), ChannelError> {
        let conn = self.pool.get().map_err(|_| ChannelError::PermissionDenied)?;

        let tx = conn.unchecked_transaction().map_err(|_| ChannelError::PermissionDenied)?;

        // Insert channel
        tx.execute(
            "INSERT INTO channels (id, space_id, name, description, visibility, mls_group_id, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                channel.id.as_bytes(),
                channel.space_id.as_bytes(),
                &channel.name,
                &channel.description,
                match channel.visibility {
                    ChannelVisibility::Public => "Public",
                    ChannelVisibility::Private => "Private",
                },
                &channel.mls_group_id.0,
                channel.created_at.as_millis() as i64,
                channel.updated_at.as_millis() as i64,
            ],
        )
        .map_err(|_| ChannelError::PermissionDenied)?;

        // Insert members
        for user_id in &channel.members {
            tx.execute(
                "INSERT INTO channel_members (channel_id, user_id, joined_at)
                 VALUES (?, ?, ?)",
                params![
                    channel.id.as_bytes(),
                    user_id.to_string(),
                    channel.created_at.as_millis() as i64,
                ],
            )
            .map_err(|_| ChannelError::PermissionDenied)?;
        }

        tx.commit().map_err(|_| ChannelError::PermissionDenied)?;

        Ok(())
    }

    /// Get a Channel by ID
    pub fn get_channel(&self, channel_id: &ChannelId) -> Result<Channel, ChannelError> {
        let conn = self.pool.get().map_err(|_| ChannelError::PermissionDenied)?;

        // Get channel metadata
        let mut channel: Channel = conn
            .query_row(
                "SELECT id, space_id, name, description, visibility, mls_group_id, created_at, updated_at
                 FROM channels WHERE id = ?",
                params![channel_id.as_bytes()],
                |row| {
                    let space_id_bytes: Vec<u8> = row.get(1)?;
                    let mut space_id_arr = [0u8; 32];
                    space_id_arr.copy_from_slice(&space_id_bytes);

                    let visibility_str: String = row.get(4)?;
                    let visibility = match visibility_str.as_str() {
                        "Public" => ChannelVisibility::Public,
                        "Private" => ChannelVisibility::Private,
                        _ => ChannelVisibility::Private,
                    };

                    let mls_group_id_bytes: Vec<u8> = row.get(5)?;

                    Ok(Channel {
                        id: ChannelId::from_bytes(*channel_id.as_bytes()),
                        space_id: SpaceId::from_bytes(space_id_arr),
                        name: row.get(2)?,
                        description: row.get(3)?,
                        visibility,
                        mls_group_id: GroupId::from(mls_group_id_bytes),
                        members: HashSet::new(),
                        created_at: Timestamp::from_millis(row.get::<_, i64>(6)?.max(0) as u64),
                        updated_at: Timestamp::from_millis(row.get::<_, i64>(7)?.max(0) as u64),
                    })
                },
            )
            .optional()
            .map_err(|_| ChannelError::PermissionDenied)?
            .ok_or(ChannelError::PermissionDenied)?;

        // Get members
        let mut stmt = conn
            .prepare("SELECT user_id FROM channel_members WHERE channel_id = ?")
            .map_err(|_| ChannelError::PermissionDenied)?;

        let members = stmt
            .query_map(params![channel_id.as_bytes()], |row| {
                Ok(UserId::new(row.get(0)?))
            })
            .map_err(|_| ChannelError::PermissionDenied)?
            .collect::<Result<HashSet<_>, _>>()
            .map_err(|_| ChannelError::PermissionDenied)?;

        channel.members = members;

        Ok(channel)
    }

    /// Update a Channel's metadata
    pub fn update_channel(&self, channel: &Channel) -> Result<(), ChannelError> {
        let conn = self.pool.get().map_err(|_| ChannelError::PermissionDenied)?;

        conn.execute(
            "UPDATE channels SET name = ?, description = ?, visibility = ?, updated_at = ?
             WHERE id = ?",
            params![
                &channel.name,
                &channel.description,
                match channel.visibility {
                    ChannelVisibility::Public => "Public",
                    ChannelVisibility::Private => "Private",
                },
                channel.updated_at.as_millis() as i64,
                channel.id.as_bytes(),
            ],
        )
        .map_err(|_| ChannelError::PermissionDenied)?;

        Ok(())
    }

    /// Delete a Channel
    pub fn delete_channel(&self, channel_id: &ChannelId) -> Result<(), ChannelError> {
        let conn = self.pool.get().map_err(|_| ChannelError::PermissionDenied)?;

        conn.execute("DELETE FROM channels WHERE id = ?", params![channel_id.as_bytes()])
            .map_err(|_| ChannelError::PermissionDenied)?;

        Ok(())
    }

    /// Add a member to a channel
    pub fn add_channel_member(
        &self,
        channel_id: &ChannelId,
        user_id: &UserId,
    ) -> Result<(), ChannelError> {
        let conn = self.pool.get().map_err(|_| ChannelError::PermissionDenied)?;

        conn.execute(
            "INSERT INTO channel_members (channel_id, user_id, joined_at)
             VALUES (?, ?, ?)",
            params![
                channel_id.as_bytes(),
                user_id.to_string(),
                Timestamp::now().as_millis() as i64,
            ],
        )
        .map_err(|_| ChannelError::MemberAlreadyExists)?;

        Ok(())
    }

    /// Remove a member from a channel
    pub fn remove_channel_member(
        &self,
        channel_id: &ChannelId,
        user_id: &UserId,
    ) -> Result<(), ChannelError> {
        let conn = self.pool.get().map_err(|_| ChannelError::PermissionDenied)?;

        let rows = conn
            .execute(
                "DELETE FROM channel_members WHERE channel_id = ? AND user_id = ?",
                params![channel_id.as_bytes(), user_id.to_string()],
            )
            .map_err(|_| ChannelError::PermissionDenied)?;

        if rows == 0 {
            return Err(ChannelError::MemberNotFound);
        }

        Ok(())
    }

    /// List all channels in a Space
    pub fn list_space_channels(&self, space_id: &SpaceId) -> Result<Vec<Channel>, ChannelError> {
        let conn = self.pool.get().map_err(|_| ChannelError::PermissionDenied)?;

        let mut stmt = conn
            .prepare("SELECT id FROM channels WHERE space_id = ? ORDER BY created_at")
            .map_err(|_| ChannelError::PermissionDenied)?;

        let channel_ids = stmt
            .query_map(params![space_id.as_bytes()], |row| {
                let id_bytes: Vec<u8> = row.get(0)?;
                let mut arr = [0u8; 32];
                arr.copy_from_slice(&id_bytes);
                Ok(ChannelId::from_bytes(arr))
            })
            .map_err(|_| ChannelError::PermissionDenied)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| ChannelError::PermissionDenied)?;

        let mut channels = Vec::new();
        for channel_id in channel_ids {
            channels.push(self.get_channel(&channel_id)?);
        }

        Ok(channels)
    }

    // ===== Invite Operations =====

    /// Create a Space invite
    pub fn create_invite(&self, invite: &SpaceInvite) -> Result<(), InviteError> {
        let conn = self.pool.get().map_err(|_| InviteError::InvalidInviteCode)?;

        let (invite_type_str, invite_value) = match &invite.invite_type {
            InviteType::Link(code) => ("Link", code.clone()),
            InviteType::Code(code) => ("Code", code.clone()),
            InviteType::Direct(user_id) => ("Direct", user_id.to_string()),
        };

        conn.execute(
            "INSERT INTO space_invites (id, space_id, invite_type, invite_value, created_by, created_at, expires_at, max_uses, use_count, revoked)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                &invite.id,
                invite.space_id.as_bytes(),
                invite_type_str,
                invite_value,
                invite.created_by.to_string(),
                invite.created_at.as_millis() as i64,
                invite.expires_at.map(|t| t.as_millis() as i64),
                invite.max_uses.map(|u| u as i64),
                invite.use_count as i64,
                invite.revoked as i64,
            ],
        )
        .map_err(|_| InviteError::InvalidInviteCode)?;

        Ok(())
    }

    /// Get an invite by ID
    pub fn get_invite(&self, invite_id: &str) -> Result<SpaceInvite, InviteError> {
        let conn = self.pool.get().map_err(|_| InviteError::InviteNotFound)?;

        conn.query_row(
            "SELECT id, space_id, invite_type, invite_value, created_by, created_at, expires_at, max_uses, use_count, revoked
             FROM space_invites WHERE id = ?",
            params![invite_id],
            |row| {
                let space_id_bytes: Vec<u8> = row.get(1)?;
                let mut space_id_arr = [0u8; 32];
                space_id_arr.copy_from_slice(&space_id_bytes);

                let invite_type_str: String = row.get(2)?;
                let invite_value: String = row.get(3)?;

                let invite_type = match invite_type_str.as_str() {
                    "Link" => InviteType::Link(invite_value),
                    "Code" => InviteType::Code(invite_value),
                    "Direct" => InviteType::Direct(UserId::new(invite_value)),
                    _ => return Err(rusqlite::Error::InvalidQuery),
                };

                Ok(SpaceInvite {
                    id: row.get(0)?,
                    space_id: SpaceId::from_bytes(space_id_arr),
                    invite_type,
                    created_by: UserId::new(row.get(4)?),
                    created_at: Timestamp::from_millis(row.get::<_, i64>(5)?.max(0) as u64),
                    expires_at: row.get::<_, Option<i64>>(6)?.map(|t| Timestamp::from_millis(t.max(0) as u64)),
                    max_uses: row.get::<_, Option<i64>>(7)?.map(|u| u.max(0) as u32),
                    use_count: row.get::<_, i64>(8)?.max(0) as u32,
                    revoked: row.get::<_, i64>(9)? != 0,
                })
            },
        )
        .optional()
        .map_err(|_| InviteError::InviteNotFound)?
        .ok_or(InviteError::InviteNotFound)
    }

    /// Update an invite (for use count, revocation)
    pub fn update_invite(&self, invite: &SpaceInvite) -> Result<(), InviteError> {
        let conn = self.pool.get().map_err(|_| InviteError::InviteNotFound)?;

        conn.execute(
            "UPDATE space_invites SET use_count = ?, revoked = ? WHERE id = ?",
            params![invite.use_count as i64, invite.revoked as i64, &invite.id],
        )
        .map_err(|_| InviteError::InviteNotFound)?;

        Ok(())
    }

    /// List all active invites for a Space
    pub fn list_space_invites(&self, space_id: &SpaceId) -> Result<Vec<SpaceInvite>, InviteError> {
        let conn = self.pool.get().map_err(|_| InviteError::InviteNotFound)?;

        let mut stmt = conn
            .prepare(
                "SELECT id FROM space_invites 
                 WHERE space_id = ? AND revoked = 0 
                 ORDER BY created_at DESC",
            )
            .map_err(|_| InviteError::InviteNotFound)?;

        let invite_ids = stmt
            .query_map(params![space_id.as_bytes()], |row| row.get::<_, String>(0))
            .map_err(|_| InviteError::InviteNotFound)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| InviteError::InviteNotFound)?;

        let mut invites = Vec::new();
        for invite_id in invite_ids {
            invites.push(self.get_invite(&invite_id)?);
        }

        Ok(invites)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_and_get_space() {
        let store = SpaceSqlStore::memory().unwrap();
        let owner = UserId::new("alice".to_string());

        let space = Space::new("Test Space".to_string(), owner.clone(), SpaceVisibility::Public);

        store.create_space(&space).unwrap();
        let retrieved = store.get_space(&space.id).unwrap();

        assert_eq!(retrieved.name, "Test Space");
        assert_eq!(retrieved.owner_id, owner);
        assert_eq!(retrieved.visibility, SpaceVisibility::Public);
    }

    #[test]
    fn test_list_public_spaces() {
        let store = SpaceSqlStore::memory().unwrap();
        let owner = UserId::new("alice".to_string());

        let space1 = Space::new("Public Space".to_string(), owner.clone(), SpaceVisibility::Public);
        let space2 =
            Space::new("Private Space".to_string(), owner.clone(), SpaceVisibility::Private);

        store.create_space(&space1).unwrap();
        store.create_space(&space2).unwrap();

        let public_spaces = store.list_public_spaces().unwrap();
        assert_eq!(public_spaces.len(), 1);
        assert_eq!(public_spaces[0].name, "Public Space");
    }

    #[test]
    fn test_create_and_get_channel() {
        let store = SpaceSqlStore::memory().unwrap();
        let owner = UserId::new("alice".to_string());

        let space = Space::new("Test Space".to_string(), owner.clone(), SpaceVisibility::Public);
        store.create_space(&space).unwrap();

        let mls_group_id = GroupId::from(vec![1, 2, 3, 4]);
        let channel = Channel::new(
            space.id,
            "general".to_string(),
            ChannelVisibility::Public,
            mls_group_id.clone(),
            owner,
        );

        store.create_channel(&channel).unwrap();
        let retrieved = store.get_channel(&channel.id).unwrap();

        assert_eq!(retrieved.name, "general");
        assert_eq!(retrieved.space_id, space.id);
        assert_eq!(retrieved.mls_group_id, mls_group_id);
    }

    #[test]
    fn test_create_and_get_invite() {
        let store = SpaceSqlStore::memory().unwrap();
        let owner = UserId::new("alice".to_string());

        let space = Space::new("Test Space".to_string(), owner.clone(), SpaceVisibility::Public);
        store.create_space(&space).unwrap();

        let invite = SpaceInvite::new_link(space.id, owner, Some(10), None);
        store.create_invite(&invite).unwrap();

        let retrieved = store.get_invite(&invite.id).unwrap();
        assert_eq!(retrieved.space_id, space.id);
        assert_eq!(retrieved.max_uses, Some(10));
    }

    #[test]
    fn test_cascade_delete_space() {
        let store = SpaceSqlStore::memory().unwrap();
        let owner = UserId::new("alice".to_string());

        let space = Space::new("Test Space".to_string(), owner.clone(), SpaceVisibility::Public);
        store.create_space(&space).unwrap();

        let mls_group_id = GroupId::from(vec![1, 2, 3, 4]);
        let channel = Channel::new(
            space.id,
            "general".to_string(),
            ChannelVisibility::Public,
            mls_group_id,
            owner,
        );
        store.create_channel(&channel).unwrap();

        // Delete space should cascade to channel
        store.delete_space(&space.id).unwrap();

        // Channel should be gone
        assert!(store.get_channel(&channel.id).is_err());
    }
}
