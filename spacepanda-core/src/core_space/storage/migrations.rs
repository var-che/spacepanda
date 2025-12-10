//! Database migrations for Spaces and Channels
//!
//! Provides versioned migrations for the Space/Channel storage schema.
//! Each migration is applied atomically and tracked in the schema_version table.

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::params;
use std::time::{SystemTime, UNIX_EPOCH};

/// Current schema version for core_space
pub const CURRENT_SPACE_SCHEMA_VERSION: i32 = 1;

/// Migration descriptor
pub struct Migration {
    pub version: i32,
    pub description: &'static str,
    pub up_sql: &'static str,
    pub down_sql: Option<&'static str>,
}

/// All available migrations in order
pub fn get_migrations() -> Vec<Migration> {
    vec![
        Migration {
            version: 1,
            description: "Initial Spaces and Channels schema",
            up_sql: r#"
                -- Schema version tracking for core_space
                CREATE TABLE IF NOT EXISTS space_schema_version (
                    version INTEGER PRIMARY KEY,
                    applied_at INTEGER NOT NULL
                );

                -- Spaces (Discord servers / Slack workspaces)
                CREATE TABLE IF NOT EXISTS spaces (
                    id BLOB PRIMARY KEY,                    -- SpaceId (32 bytes)
                    name TEXT NOT NULL,
                    description TEXT,
                    icon_url TEXT,
                    visibility TEXT NOT NULL CHECK(visibility IN ('Public', 'Private')),
                    owner_id TEXT NOT NULL,                 -- UserId
                    created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL
                );

                CREATE INDEX IF NOT EXISTS idx_spaces_visibility ON spaces(visibility);
                CREATE INDEX IF NOT EXISTS idx_spaces_owner ON spaces(owner_id);

                -- Space Members (join table with roles)
                CREATE TABLE IF NOT EXISTS space_members (
                    space_id BLOB NOT NULL,                 -- SpaceId
                    user_id TEXT NOT NULL,                  -- UserId
                    role TEXT NOT NULL CHECK(role IN ('Owner', 'Admin', 'Member')),
                    joined_at INTEGER NOT NULL,
                    invited_by TEXT,                        -- UserId (optional)
                    PRIMARY KEY (space_id, user_id),
                    FOREIGN KEY (space_id) REFERENCES spaces(id) ON DELETE CASCADE
                );

                CREATE INDEX IF NOT EXISTS idx_space_members_user ON space_members(user_id);
                CREATE INDEX IF NOT EXISTS idx_space_members_role ON space_members(space_id, role);

                -- Channels (communication spaces within a Space)
                CREATE TABLE IF NOT EXISTS channels (
                    id BLOB PRIMARY KEY,                    -- ChannelId (32 bytes)
                    space_id BLOB NOT NULL,                 -- SpaceId
                    name TEXT NOT NULL,
                    description TEXT,
                    visibility TEXT NOT NULL CHECK(visibility IN ('Public', 'Private')),
                    mls_group_id BLOB NOT NULL,             -- GroupId for E2EE
                    created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL,
                    FOREIGN KEY (space_id) REFERENCES spaces(id) ON DELETE CASCADE
                );

                CREATE INDEX IF NOT EXISTS idx_channels_space ON channels(space_id);
                CREATE INDEX IF NOT EXISTS idx_channels_visibility ON channels(space_id, visibility);
                CREATE INDEX IF NOT EXISTS idx_channels_mls_group ON channels(mls_group_id);

                -- Channel Members (join table)
                CREATE TABLE IF NOT EXISTS channel_members (
                    channel_id BLOB NOT NULL,               -- ChannelId
                    user_id TEXT NOT NULL,                  -- UserId
                    joined_at INTEGER NOT NULL,
                    PRIMARY KEY (channel_id, user_id),
                    FOREIGN KEY (channel_id) REFERENCES channels(id) ON DELETE CASCADE
                );

                CREATE INDEX IF NOT EXISTS idx_channel_members_user ON channel_members(user_id);

                -- Space Invites (links, codes, direct invites)
                CREATE TABLE IF NOT EXISTS space_invites (
                    id TEXT PRIMARY KEY,                    -- Invite ID (e.g., "inv_ABC123")
                    space_id BLOB NOT NULL,                 -- SpaceId
                    invite_type TEXT NOT NULL CHECK(invite_type IN ('Link', 'Code', 'Direct')),
                    invite_value TEXT NOT NULL,             -- Code/Link string or target UserId
                    created_by TEXT NOT NULL,               -- UserId
                    created_at INTEGER NOT NULL,
                    expires_at INTEGER,                     -- Optional expiration timestamp
                    max_uses INTEGER,                       -- Optional max use count
                    use_count INTEGER NOT NULL DEFAULT 0,
                    revoked BOOLEAN NOT NULL DEFAULT 0,
                    FOREIGN KEY (space_id) REFERENCES spaces(id) ON DELETE CASCADE
                );

                CREATE INDEX IF NOT EXISTS idx_invites_space ON space_invites(space_id);
                CREATE INDEX IF NOT EXISTS idx_invites_code ON space_invites(invite_value) WHERE invite_type IN ('Link', 'Code');
                CREATE INDEX IF NOT EXISTS idx_invites_expires ON space_invites(expires_at) WHERE expires_at IS NOT NULL;
                CREATE INDEX IF NOT EXISTS idx_invites_active 
                    ON space_invites(space_id, revoked, expires_at) 
                    WHERE revoked = 0;
            "#,
            down_sql: Some(
                r#"
                DROP INDEX IF EXISTS idx_invites_active;
                DROP INDEX IF EXISTS idx_invites_expires;
                DROP INDEX IF EXISTS idx_invites_code;
                DROP INDEX IF EXISTS idx_invites_space;
                DROP TABLE IF EXISTS space_invites;
                
                DROP INDEX IF EXISTS idx_channel_members_user;
                DROP TABLE IF EXISTS channel_members;
                
                DROP INDEX IF EXISTS idx_channels_mls_group;
                DROP INDEX IF EXISTS idx_channels_visibility;
                DROP INDEX IF EXISTS idx_channels_space;
                DROP TABLE IF EXISTS channels;
                
                DROP INDEX IF EXISTS idx_space_members_role;
                DROP INDEX IF EXISTS idx_space_members_user;
                DROP TABLE IF EXISTS space_members;
                
                DROP INDEX IF EXISTS idx_spaces_owner;
                DROP INDEX IF EXISTS idx_spaces_visibility;
                DROP TABLE IF EXISTS spaces;
                
                DROP TABLE IF EXISTS space_schema_version;
            "#,
            ),
        },
    ]
}

/// Get current schema version from database
fn get_current_version(pool: &Pool<SqliteConnectionManager>) -> Result<i32, rusqlite::Error> {
    let conn = pool.get().map_err(|e| {
        rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("Failed to get connection: {}", e),
        )))
    })?;

    // Ensure schema_version table exists
    conn.execute(
        "CREATE TABLE IF NOT EXISTS space_schema_version (
            version INTEGER PRIMARY KEY,
            applied_at INTEGER NOT NULL
        )",
        [],
    )?;

    let version: Result<i32, _> = conn.query_row(
        "SELECT version FROM space_schema_version ORDER BY version DESC LIMIT 1",
        [],
        |row| row.get(0),
    );

    Ok(version.unwrap_or(0))
}

/// Run all pending migrations
pub fn migrate(pool: &Pool<SqliteConnectionManager>) -> Result<(), rusqlite::Error> {
    let current_version = get_current_version(pool)?;
    let migrations = get_migrations();

    let pending_migrations: Vec<_> =
        migrations.into_iter().filter(|m| m.version > current_version).collect();

    if pending_migrations.is_empty() {
        return Ok(());
    }

    let conn = pool.get().map_err(|e| {
        rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("Failed to get connection: {}", e),
        )))
    })?;

    for migration in pending_migrations {
        let tx = conn.unchecked_transaction()?;

        // Run migration SQL
        tx.execute_batch(migration.up_sql)?;

        // Record migration
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_millis() as i64;

        tx.execute(
            "INSERT INTO space_schema_version (version, applied_at) VALUES (?, ?)",
            params![migration.version, now],
        )?;

        tx.commit()?;

        eprintln!(
            "Applied migration v{}: {}",
            migration.version, migration.description
        );
    }

    Ok(())
}

/// Get the latest migration version available
pub fn get_latest_version() -> i32 {
    let migrations = get_migrations();
    migrations.iter().map(|m| m.version).max().unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn setup_test_pool() -> Pool<SqliteConnectionManager> {
        let manager = SqliteConnectionManager::memory();
        Pool::new(manager).expect("Failed to create pool")
    }

    #[test]
    fn test_initial_migration() {
        let pool = setup_test_pool();
        migrate(&pool).expect("Migration failed");

        let conn = pool.get().unwrap();

        // Check that all tables exist
        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();

        assert!(tables.contains(&"spaces".to_string()));
        assert!(tables.contains(&"space_members".to_string()));
        assert!(tables.contains(&"channels".to_string()));
        assert!(tables.contains(&"channel_members".to_string()));
        assert!(tables.contains(&"space_invites".to_string()));
    }

    #[test]
    fn test_migration_version_tracking() {
        let pool = setup_test_pool();
        migrate(&pool).expect("Migration failed");

        let version = get_current_version(&pool).expect("Failed to get version");
        assert_eq!(version, CURRENT_SPACE_SCHEMA_VERSION);
    }

    #[test]
    fn test_idempotent_migrations() {
        let pool = setup_test_pool();

        // Run migrations twice
        migrate(&pool).expect("First migration failed");
        migrate(&pool).expect("Second migration failed");

        // Version should still be correct
        let version = get_current_version(&pool).expect("Failed to get version");
        assert_eq!(version, CURRENT_SPACE_SCHEMA_VERSION);
    }

    #[test]
    fn test_foreign_key_constraints() {
        let pool = setup_test_pool();
        migrate(&pool).expect("Migration failed");

        let conn = pool.get().unwrap();

        // Enable foreign keys
        conn.execute("PRAGMA foreign_keys = ON", []).unwrap();

        // Create a space
        let space_id = vec![1u8; 32];
        let now = 1000i64;
        conn.execute(
            "INSERT INTO spaces (id, name, visibility, owner_id, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?)",
            params![space_id, "Test Space", "Public", "user1", now, now],
        )
        .unwrap();

        // Create a channel in that space
        let channel_id = vec![2u8; 32];
        let mls_group_id = vec![3u8; 32];
        conn.execute(
            "INSERT INTO channels (id, space_id, name, visibility, mls_group_id, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            params![channel_id, space_id, "general", "Public", mls_group_id, now, now],
        )
        .unwrap();

        // Delete the space - should cascade to channel
        conn.execute("DELETE FROM spaces WHERE id = ?", params![space_id])
            .unwrap();

        // Channel should be gone
        let count: i32 = conn
            .query_row("SELECT COUNT(*) FROM channels WHERE id = ?", params![channel_id], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 0);
    }
}
