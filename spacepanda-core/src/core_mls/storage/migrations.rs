//! Database Schema Migration System
//!
//! Provides versioned migrations for the SQL storage schema.
//! Each migration is applied atomically and tracked in the schema_version table.

use crate::core_mls::errors::{MlsError, MlsResult};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::params;
use std::time::{SystemTime, UNIX_EPOCH};

/// Current schema version
pub const CURRENT_SCHEMA_VERSION: i32 = 2;

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
            description: "Initial schema with MLS groups, key packages, and metadata",
            up_sql: r#"
                -- Schema version tracking
                CREATE TABLE IF NOT EXISTS schema_version (
                    version INTEGER PRIMARY KEY,
                    applied_at INTEGER NOT NULL
                );

                -- MLS group snapshots
                CREATE TABLE IF NOT EXISTS group_snapshots (
                    group_id BLOB PRIMARY KEY,
                    snapshot_data BLOB NOT NULL,
                    epoch INTEGER NOT NULL,
                    created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL
                );

                -- Key packages (for invites)
                CREATE TABLE IF NOT EXISTS key_packages (
                    key_package_id BLOB PRIMARY KEY,
                    key_package_data BLOB NOT NULL,
                    credential_id BLOB NOT NULL,
                    created_at INTEGER NOT NULL,
                    expires_at INTEGER,
                    used BOOLEAN NOT NULL DEFAULT 0
                );

                CREATE INDEX IF NOT EXISTS idx_key_packages_expires 
                    ON key_packages(expires_at) WHERE expires_at IS NOT NULL;

                -- Signature keys
                CREATE TABLE IF NOT EXISTS signature_keys (
                    key_id BLOB PRIMARY KEY,
                    public_key BLOB NOT NULL,
                    private_key BLOB NOT NULL,
                    created_at INTEGER NOT NULL
                );

                -- Pre-shared keys (PSKs)
                CREATE TABLE IF NOT EXISTS psks (
                    psk_id BLOB PRIMARY KEY,
                    psk_data BLOB NOT NULL,
                    created_at INTEGER NOT NULL,
                    expires_at INTEGER
                );

                -- Generic key-value blob storage
                CREATE TABLE IF NOT EXISTS kv_blobs (
                    key TEXT PRIMARY KEY,
                    value BLOB NOT NULL,
                    created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL
                );
            "#,
            down_sql: Some(r#"
                DROP TABLE IF EXISTS kv_blobs;
                DROP TABLE IF EXISTS psks;
                DROP TABLE IF EXISTS signature_keys;
                DROP INDEX IF EXISTS idx_key_packages_expires;
                DROP TABLE IF EXISTS key_packages;
                DROP TABLE IF EXISTS group_snapshots;
                DROP TABLE IF EXISTS schema_version;
            "#),
        },
        Migration {
            version: 2,
            description: "Add privacy-focused channel and message metadata tables",
            up_sql: r#"
                -- Privacy-focused channel metadata
                CREATE TABLE IF NOT EXISTS channels (
                    group_id BLOB PRIMARY KEY,
                    encrypted_name BLOB NOT NULL,
                    encrypted_topic BLOB,
                    created_at INTEGER NOT NULL,
                    encrypted_members BLOB NOT NULL,
                    channel_type INTEGER NOT NULL,
                    archived INTEGER NOT NULL DEFAULT 0
                );

                -- Privacy-focused message metadata
                CREATE TABLE IF NOT EXISTS messages (
                    message_id BLOB PRIMARY KEY,
                    group_id BLOB NOT NULL,
                    encrypted_content BLOB NOT NULL,
                    sender_hash BLOB NOT NULL,
                    sequence INTEGER NOT NULL,
                    processed INTEGER NOT NULL DEFAULT 0,
                    FOREIGN KEY (group_id) REFERENCES channels(group_id) ON DELETE CASCADE
                );

                -- Index for efficient message retrieval
                CREATE INDEX IF NOT EXISTS idx_messages_group_seq 
                    ON messages(group_id, sequence DESC);

                -- Index for unprocessed messages
                CREATE INDEX IF NOT EXISTS idx_messages_unprocessed
                    ON messages(group_id, processed) WHERE processed = 0;
            "#,
            down_sql: Some(r#"
                DROP INDEX IF EXISTS idx_messages_unprocessed;
                DROP INDEX IF EXISTS idx_messages_group_seq;
                DROP TABLE IF EXISTS messages;
                DROP TABLE IF EXISTS channels;
            "#),
        },
    ]
}

/// Get current schema version from database
pub fn get_current_version(pool: &Pool<SqliteConnectionManager>) -> MlsResult<i32> {
    let conn = pool
        .get()
        .map_err(|e| MlsError::Storage(format!("Failed to get connection: {}", e)))?;

    let version = conn
        .query_row(
            "SELECT MAX(version) FROM schema_version",
            [],
            |row| row.get::<_, Option<i32>>(0),
        )
        .unwrap_or(Some(0))
        .unwrap_or(0);

    Ok(version)
}

/// Apply a single migration
fn apply_migration(
    pool: &Pool<SqliteConnectionManager>,
    migration: &Migration,
) -> MlsResult<()> {
    let mut conn = pool
        .get()
        .map_err(|e| MlsError::Storage(format!("Failed to get connection: {}", e)))?;

    let tx = conn
        .transaction()
        .map_err(|e| MlsError::Storage(format!("Failed to begin transaction: {}", e)))?;

    // Execute migration SQL
    tx.execute_batch(migration.up_sql)
        .map_err(|e| MlsError::Storage(format!("Migration {} failed: {}", migration.version, e)))?;

    // Record migration
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs() as i64;

    tx.execute(
        "INSERT INTO schema_version (version, applied_at) VALUES (?, ?)",
        params![migration.version, now],
    )
    .map_err(|e| MlsError::Storage(format!("Failed to record migration: {}", e)))?;

    tx.commit()
        .map_err(|e| MlsError::Storage(format!("Failed to commit migration: {}", e)))?;

    Ok(())
}

/// Run all pending migrations
pub fn migrate(pool: &Pool<SqliteConnectionManager>) -> MlsResult<()> {
    let current_version = get_current_version(pool)?;
    let migrations = get_migrations();

    let pending_migrations: Vec<_> = migrations
        .into_iter()
        .filter(|m| m.version > current_version)
        .collect();

    if pending_migrations.is_empty() {
        return Ok(());
    }

    for migration in pending_migrations {
        println!(
            "Applying migration {}: {}",
            migration.version, migration.description
        );
        apply_migration(pool, &migration)?;
    }

    Ok(())
}

/// Rollback a migration (if down_sql is available)
pub fn rollback_migration(
    pool: &Pool<SqliteConnectionManager>,
    version: i32,
) -> MlsResult<()> {
    let migrations = get_migrations();
    let migration = migrations
        .into_iter()
        .find(|m| m.version == version)
        .ok_or_else(|| MlsError::Storage(format!("Migration version {} not found", version)))?;

    let down_sql = migration
        .down_sql
        .ok_or_else(|| MlsError::Storage(format!("No rollback available for version {}", version)))?;

    let mut conn = pool
        .get()
        .map_err(|e| MlsError::Storage(format!("Failed to get connection: {}", e)))?;

    let tx = conn
        .transaction()
        .map_err(|e| MlsError::Storage(format!("Failed to begin transaction: {}", e)))?;

    // Execute rollback SQL
    tx.execute_batch(down_sql)
        .map_err(|e| MlsError::Storage(format!("Rollback {} failed: {}", version, e)))?;

    // Remove migration record
    tx.execute("DELETE FROM schema_version WHERE version = ?", params![version])
        .map_err(|e| MlsError::Storage(format!("Failed to remove migration record: {}", e)))?;

    tx.commit()
        .map_err(|e| MlsError::Storage(format!("Failed to commit rollback: {}", e)))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use r2d2_sqlite::SqliteConnectionManager;
    use tempfile::tempdir;

    #[test]
    fn test_initial_migration() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("migration_test.db");

        let manager = SqliteConnectionManager::file(&db_path);
        let pool = Pool::builder().build(manager).unwrap();

        // Initially version 0
        let version = get_current_version(&pool).unwrap();
        assert_eq!(version, 0);

        // Run migrations
        migrate(&pool).unwrap();

        // Should be at current version
        let version = get_current_version(&pool).unwrap();
        assert_eq!(version, CURRENT_SCHEMA_VERSION);

        // Verify tables exist
        let conn = pool.get().unwrap();
        let mut stmt = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table'")
            .unwrap();

        let tables: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();

        assert!(tables.contains(&"schema_version".to_string()));
        assert!(tables.contains(&"group_snapshots".to_string()));
        assert!(tables.contains(&"key_packages".to_string()));
        assert!(tables.contains(&"channels".to_string()));
        assert!(tables.contains(&"messages".to_string()));
    }

    #[test]
    fn test_idempotent_migrations() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("idempotent_test.db");

        let manager = SqliteConnectionManager::file(&db_path);
        let pool = Pool::builder().build(manager).unwrap();

        // Run migrations twice
        migrate(&pool).unwrap();
        migrate(&pool).unwrap();

        // Should still be at current version
        let version = get_current_version(&pool).unwrap();
        assert_eq!(version, CURRENT_SCHEMA_VERSION);
    }

    #[test]
    fn test_migration_rollback() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("rollback_test.db");

        let manager = SqliteConnectionManager::file(&db_path);
        let pool = Pool::builder().build(manager).unwrap();

        // Apply all migrations
        migrate(&pool).unwrap();
        assert_eq!(get_current_version(&pool).unwrap(), CURRENT_SCHEMA_VERSION);

        // Rollback version 2
        rollback_migration(&pool, 2).unwrap();
        assert_eq!(get_current_version(&pool).unwrap(), 1);

        // Verify channels table is gone
        let conn = pool.get().unwrap();
        let result = conn.query_row(
            "SELECT name FROM sqlite_master WHERE type='table' AND name='channels'",
            [],
            |_| Ok(()),
        );
        assert!(result.is_err());

        // Re-apply migration
        migrate(&pool).unwrap();
        assert_eq!(get_current_version(&pool).unwrap(), CURRENT_SCHEMA_VERSION);

        // Channels table should be back
        let result = conn.query_row(
            "SELECT name FROM sqlite_master WHERE type='table' AND name='channels'",
            [],
            |_| Ok(()),
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_partial_migration() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("partial_test.db");

        let manager = SqliteConnectionManager::file(&db_path);
        let pool = Pool::builder().build(manager).unwrap();

        // Manually apply only version 1
        let migrations = get_migrations();
        apply_migration(&pool, &migrations[0]).unwrap();

        assert_eq!(get_current_version(&pool).unwrap(), 1);

        // Run migrate to catch up
        migrate(&pool).unwrap();

        assert_eq!(get_current_version(&pool).unwrap(), CURRENT_SCHEMA_VERSION);
    }
}
