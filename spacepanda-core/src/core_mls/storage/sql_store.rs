//! SQLite-based Storage Provider for OpenMLS
//!
//! Provides persistent storage for:
//! - MLS group snapshots
//! - Key packages with expiration tracking
//! - Signature keys
//! - PSKs (Pre-Shared Keys)
//! - Arbitrary key-value blobs
//!
//! Uses connection pooling for concurrent access and transactions for atomicity.

use crate::core_mls::errors::{MlsError, MlsResult};
use crate::core_mls::storage::metadata_encryption::{decrypt_metadata, encrypt_metadata};
use crate::core_mls::traits::storage::{GroupId, PersistedGroupSnapshot, StorageProvider};
use async_trait::async_trait;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

/// SQLite-backed storage provider
pub struct SqlStorageProvider {
    pool: Arc<Pool<SqliteConnectionManager>>,
}

impl SqlStorageProvider {
    /// Create a new SQL storage provider
    ///
    /// # Arguments
    /// * `db_path` - Path to SQLite database file
    pub fn new<P: AsRef<Path>>(db_path: P) -> MlsResult<Self> {
        let manager = SqliteConnectionManager::file(db_path);
        let pool = Pool::builder()
            .max_size(16) // Support concurrent access
            .build(manager)
            .map_err(|e| MlsError::Storage(format!("Failed to create connection pool: {}", e)))?;

        let provider = Self { pool: Arc::new(pool) };

        // Run migrations to ensure schema is up to date
        super::migrations::migrate(&provider.pool)?;

        Ok(provider)
    }

    /// Begin a new transaction for atomic operations
    ///
    /// Returns a connection that can be used to execute multiple operations atomically.
    /// Call `commit()` on the connection to persist changes.
    ///
    /// # Example
    /// ```no_run
    /// # use spacepanda_core::core_mls::storage::SqlStorageProvider;
    /// # async fn example(storage: &SqlStorageProvider) -> Result<(), Box<dyn std::error::Error>> {
    /// let mut conn = storage.begin_transaction().await?;
    /// // Perform operations using conn.execute(...)
    /// conn.commit()?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn begin_transaction(
        &self,
    ) -> MlsResult<r2d2::PooledConnection<SqliteConnectionManager>> {
        let pool = self.pool.clone();

        tokio::task::spawn_blocking(move || {
            let conn = pool
                .get()
                .map_err(|e| MlsError::Storage(format!("Failed to get connection: {}", e)))?;

            // Begin immediate transaction
            conn.execute("BEGIN IMMEDIATE", [])
                .map_err(|e| MlsError::Storage(format!("Failed to begin transaction: {}", e)))?;

            Ok(conn)
        })
        .await
        .map_err(|e| MlsError::Storage(format!("Task join error: {}", e)))?
    }

    /// Save multiple group snapshots atomically
    pub async fn save_group_snapshots_atomic(
        &self,
        snapshots: &[PersistedGroupSnapshot],
    ) -> MlsResult<()> {
        let pool = self.pool.clone();
        let snapshots = snapshots.to_vec();

        tokio::task::spawn_blocking(move || {
            let mut conn = pool
                .get()
                .map_err(|e| MlsError::Storage(format!("Failed to get connection: {}", e)))?;

            let tx = conn
                .transaction()
                .map_err(|e| MlsError::Storage(format!("Failed to begin transaction: {}", e)))?;

            let now = current_timestamp();

            for snapshot in snapshots {
                let snapshot_bytes = serde_json::to_vec(&snapshot).map_err(|e| {
                    MlsError::Serialization(format!("Failed to serialize snapshot: {}", e))
                })?;

                tx.execute(
                    r#"
                    INSERT INTO group_snapshots (group_id, epoch, snapshot_data, created_at)
                    VALUES (?, ?, ?, ?)
                    ON CONFLICT(group_id) DO UPDATE SET
                        epoch = excluded.epoch,
                        snapshot_data = excluded.snapshot_data
                    "#,
                    params![&snapshot.group_id, &snapshot.epoch, &snapshot_bytes, now],
                )
                .map_err(|e| MlsError::Storage(format!("Failed to save group snapshot: {}", e)))?;
            }

            tx.commit()
                .map_err(|e| MlsError::Storage(format!("Failed to commit transaction: {}", e)))?;

            Ok(())
        })
        .await
        .map_err(|e| MlsError::Storage(format!("Task join error: {}", e)))?
    }

    /// Delete multiple groups atomically
    pub async fn delete_groups_atomic(&self, group_ids: &[GroupId]) -> MlsResult<()> {
        let pool = self.pool.clone();
        let group_ids: Vec<Vec<u8>> = group_ids.to_vec();

        tokio::task::spawn_blocking(move || {
            let mut conn = pool
                .get()
                .map_err(|e| MlsError::Storage(format!("Failed to get connection: {}", e)))?;

            let tx = conn
                .transaction()
                .map_err(|e| MlsError::Storage(format!("Failed to begin transaction: {}", e)))?;

            for group_id in group_ids {
                tx.execute("DELETE FROM group_snapshots WHERE group_id = ?", params![&group_id])
                    .map_err(|e| MlsError::Storage(format!("Failed to delete group: {}", e)))?;
            }

            tx.commit()
                .map_err(|e| MlsError::Storage(format!("Failed to commit transaction: {}", e)))?;

            Ok(())
        })
        .await
        .map_err(|e| MlsError::Storage(format!("Task join error: {}", e)))?
    }

    /// Initialize database schema
    /// Clean up expired key packages
    pub fn cleanup_expired_key_packages(&self) -> MlsResult<usize> {
        let conn = self
            .pool
            .get()
            .map_err(|e| MlsError::Storage(format!("Failed to get connection: {}", e)))?;

        let now = current_timestamp();
        let deleted = conn
            .execute(
                "DELETE FROM key_packages WHERE expires_at IS NOT NULL AND expires_at < ?",
                params![now],
            )
            .map_err(|e| {
                MlsError::Storage(format!("Failed to cleanup expired key packages: {}", e))
            })?;

        Ok(deleted)
    }

    /// List all stored group IDs
    pub fn list_all_groups(&self) -> MlsResult<Vec<GroupId>> {
        let conn = self
            .pool
            .get()
            .map_err(|e| MlsError::Storage(format!("Failed to get connection: {}", e)))?;

        let mut stmt = conn
            .prepare("SELECT group_id FROM group_snapshots")
            .map_err(|e| MlsError::Storage(format!("Failed to prepare statement: {}", e)))?;

        let groups = stmt
            .query_map([], |row| row.get::<_, Vec<u8>>(0))
            .map_err(|e| MlsError::Storage(format!("Failed to query groups: {}", e)))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| MlsError::Storage(format!("Failed to collect groups: {}", e)))?;

        Ok(groups)
    }

    /// Store a key package for future use
    ///
    /// # Arguments
    /// * `key_package_id` - Unique identifier for the key package
    /// * `key_package_data` - Serialized key package
    /// * `credential_id` - Associated credential identifier
    /// * `expires_at` - Optional expiration timestamp (Unix seconds)
    pub async fn store_key_package(
        &self,
        key_package_id: &[u8],
        key_package_data: &[u8],
        credential_id: &[u8],
        expires_at: Option<i64>,
    ) -> MlsResult<()> {
        let pool = self.pool.clone();
        let key_package_id = key_package_id.to_vec();
        let key_package_data = key_package_data.to_vec();
        let credential_id = credential_id.to_vec();

        tokio::task::spawn_blocking(move || {
            let conn = pool
                .get()
                .map_err(|e| MlsError::Storage(format!("Failed to get connection: {}", e)))?;

            let now = current_timestamp();

            conn.execute(
                r#"
                INSERT INTO key_packages (key_package_id, key_package_data, credential_id, created_at, expires_at, used)
                VALUES (?, ?, ?, ?, ?, 0)
                ON CONFLICT(key_package_id) DO UPDATE SET
                    key_package_data = excluded.key_package_data,
                    credential_id = excluded.credential_id,
                    expires_at = excluded.expires_at
                "#,
                params![&key_package_id, &key_package_data, &credential_id, now, expires_at],
            )
            .map_err(|e| MlsError::Storage(format!("Failed to store key package: {}", e)))?;

            Ok(())
        })
        .await
        .map_err(|e| MlsError::Storage(format!("Task join error: {}", e)))?
    }

    /// Load an unused key package
    ///
    /// Returns the first unused, non-expired key package and marks it as used.
    pub async fn load_key_package(&self, key_package_id: &[u8]) -> MlsResult<Vec<u8>> {
        let pool = self.pool.clone();
        let key_package_id = key_package_id.to_vec();

        tokio::task::spawn_blocking(move || {
            let conn = pool
                .get()
                .map_err(|e| MlsError::Storage(format!("Failed to get connection: {}", e)))?;

            let now = current_timestamp();

            // Get unused, non-expired key package
            let data: Vec<u8> = conn
                .query_row(
                    r#"
                    SELECT key_package_data FROM key_packages
                    WHERE key_package_id = ?
                      AND used = 0
                      AND (expires_at IS NULL OR expires_at > ?)
                    "#,
                    params![&key_package_id, now],
                    |row| row.get(0),
                )
                .optional()
                .map_err(|e| MlsError::Storage(format!("Failed to load key package: {}", e)))?
                .ok_or_else(|| {
                    MlsError::NotFound(format!(
                        "Key package {:?} not found or expired",
                        key_package_id
                    ))
                })?;

            // Mark as used
            conn.execute(
                "UPDATE key_packages SET used = 1 WHERE key_package_id = ?",
                params![&key_package_id],
            )
            .map_err(|e| MlsError::Storage(format!("Failed to mark key package as used: {}", e)))?;

            Ok(data)
        })
        .await
        .map_err(|e| MlsError::Storage(format!("Task join error: {}", e)))?
    }

    /// Delete a key package
    pub async fn delete_key_package(&self, key_package_id: &[u8]) -> MlsResult<()> {
        let pool = self.pool.clone();
        let key_package_id = key_package_id.to_vec();

        tokio::task::spawn_blocking(move || {
            let conn = pool
                .get()
                .map_err(|e| MlsError::Storage(format!("Failed to get connection: {}", e)))?;

            conn.execute(
                "DELETE FROM key_packages WHERE key_package_id = ?",
                params![&key_package_id],
            )
            .map_err(|e| MlsError::Storage(format!("Failed to delete key package: {}", e)))?;

            Ok(())
        })
        .await
        .map_err(|e| MlsError::Storage(format!("Task join error: {}", e)))?
    }
}

#[async_trait]
impl StorageProvider for SqlStorageProvider {
    async fn save_group_snapshot(&self, snapshot: PersistedGroupSnapshot) -> MlsResult<()> {
        let pool = self.pool.clone();

        tokio::task::spawn_blocking(move || {
            let conn = pool
                .get()
                .map_err(|e| MlsError::Storage(format!("Failed to get connection: {}", e)))?;

            let snapshot_bytes = serde_json::to_vec(&snapshot).map_err(|e| {
                MlsError::Serialization(format!("Failed to serialize snapshot: {}", e))
            })?;

            let now = current_timestamp();

            conn.execute(
                r#"
                INSERT INTO group_snapshots (group_id, snapshot_data, epoch, created_at)
                VALUES (?, ?, ?, ?)
                ON CONFLICT(group_id) DO UPDATE SET
                    snapshot_data = excluded.snapshot_data,
                    epoch = excluded.epoch
                "#,
                params![&snapshot.group_id, &snapshot_bytes, snapshot.epoch, now,],
            )
            .map_err(|e| MlsError::Storage(format!("Failed to save snapshot: {}", e)))?;

            Ok(())
        })
        .await
        .map_err(|e| MlsError::Storage(format!("Task join error: {}", e)))?
    }

    async fn load_group_snapshot(&self, group_id: &GroupId) -> MlsResult<PersistedGroupSnapshot> {
        let pool = self.pool.clone();
        let group_id = group_id.clone();

        tokio::task::spawn_blocking(move || {
            let conn = pool
                .get()
                .map_err(|e| MlsError::Storage(format!("Failed to get connection: {}", e)))?;

            let snapshot_bytes: Vec<u8> = conn
                .query_row(
                    "SELECT snapshot_data FROM group_snapshots WHERE group_id = ?",
                    params![&group_id],
                    |row| row.get(0),
                )
                .optional()
                .map_err(|e| MlsError::Storage(format!("Failed to load snapshot: {}", e)))?
                .ok_or_else(|| MlsError::NotFound(format!("Group {:?} not found", group_id)))?;

            let snapshot: PersistedGroupSnapshot = serde_json::from_slice(&snapshot_bytes)
                .map_err(|e| {
                    MlsError::Serialization(format!("Failed to deserialize snapshot: {}", e))
                })?;

            Ok(snapshot)
        })
        .await
        .map_err(|e| MlsError::Storage(format!("Task join error: {}", e)))?
    }

    async fn delete_group_snapshot(&self, group_id: &GroupId) -> MlsResult<()> {
        let pool = self.pool.clone();
        let group_id = group_id.clone();

        tokio::task::spawn_blocking(move || {
            let conn = pool
                .get()
                .map_err(|e| MlsError::Storage(format!("Failed to get connection: {}", e)))?;

            conn.execute("DELETE FROM group_snapshots WHERE group_id = ?", params![&group_id])
                .map_err(|e| MlsError::Storage(format!("Failed to delete snapshot: {}", e)))?;

            Ok(())
        })
        .await
        .map_err(|e| MlsError::Storage(format!("Task join error: {}", e)))?
    }

    async fn put_blob(&self, key: &str, data: &[u8]) -> MlsResult<()> {
        let pool = self.pool.clone();
        let key = key.to_string();
        let data = data.to_vec();

        tokio::task::spawn_blocking(move || {
            let conn = pool
                .get()
                .map_err(|e| MlsError::Storage(format!("Failed to get connection: {}", e)))?;

            let now = current_timestamp();

            conn.execute(
                r#"
                INSERT INTO kv_blobs (key, value, created_at, updated_at)
                VALUES (?, ?, ?, ?)
                ON CONFLICT(key) DO UPDATE SET
                    value = excluded.value,
                    updated_at = excluded.updated_at
                "#,
                params![&key, &data, now, now],
            )
            .map_err(|e| MlsError::Storage(format!("Failed to put blob: {}", e)))?;

            Ok(())
        })
        .await
        .map_err(|e| MlsError::Storage(format!("Task join error: {}", e)))?
    }

    async fn get_blob(&self, key: &str) -> MlsResult<Vec<u8>> {
        let pool = self.pool.clone();
        let key = key.to_string();

        tokio::task::spawn_blocking(move || {
            let conn = pool
                .get()
                .map_err(|e| MlsError::Storage(format!("Failed to get connection: {}", e)))?;

            let data: Vec<u8> = conn
                .query_row("SELECT value FROM kv_blobs WHERE key = ?", params![&key], |row| {
                    row.get(0)
                })
                .optional()
                .map_err(|e| MlsError::Storage(format!("Failed to get blob: {}", e)))?
                .ok_or_else(|| MlsError::NotFound(format!("Blob '{}' not found", key)))?;

            Ok(data)
        })
        .await
        .map_err(|e| MlsError::Storage(format!("Task join error: {}", e)))?
    }

    async fn list_groups(&self) -> MlsResult<Vec<GroupId>> {
        let pool = self.pool.clone();

        tokio::task::spawn_blocking(move || {
            let conn = pool
                .get()
                .map_err(|e| MlsError::Storage(format!("Failed to get connection: {}", e)))?;

            let mut stmt = conn
                .prepare("SELECT group_id FROM group_snapshots")
                .map_err(|e| MlsError::Storage(format!("Failed to prepare statement: {}", e)))?;

            let groups = stmt
                .query_map([], |row| row.get::<_, Vec<u8>>(0))
                .map_err(|e| MlsError::Storage(format!("Failed to query groups: {}", e)))?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| MlsError::Storage(format!("Failed to collect groups: {}", e)))?;

            Ok(groups)
        })
        .await
        .map_err(|e| MlsError::Storage(format!("Task join error: {}", e)))?
    }
}

// Extension methods for SqlStorageProvider (beyond StorageProvider trait)
impl SqlStorageProvider {
    // ======================
    // Channel Metadata CRUD
    // ======================

    /// Save channel metadata
    pub async fn save_channel_metadata(
        &self,
        group_id: &[u8],
        encrypted_name: &[u8],
        encrypted_topic: Option<&[u8]>,
        encrypted_members: &[u8],
        channel_type: i32,
    ) -> MlsResult<()> {
        // Encrypt metadata before storing
        let enc_name = encrypt_metadata(group_id, encrypted_name)?;
        let enc_topic = encrypted_topic.map(|t| encrypt_metadata(group_id, t)).transpose()?;
        let enc_members = encrypt_metadata(group_id, encrypted_members)?;

        let conn = self
            .pool
            .get()
            .map_err(|e| MlsError::Storage(format!("Failed to get connection: {}", e)))?;

        let now = current_timestamp();

        conn.execute(
            "INSERT OR REPLACE INTO channels 
             (group_id, encrypted_name, encrypted_topic, created_at, encrypted_members, channel_type, archived) 
             VALUES (?, ?, ?, COALESCE((SELECT created_at FROM channels WHERE group_id = ?), ?), ?, ?, 0)",
            params![group_id, enc_name, enc_topic, group_id, now, enc_members, channel_type],
        )
        .map_err(|e| MlsError::Storage(format!("Failed to save channel metadata: {}", e)))?;

        Ok(())
    }

    /// Load channel metadata
    pub async fn load_channel_metadata(
        &self,
        group_id: &[u8],
    ) -> MlsResult<(Vec<u8>, Option<Vec<u8>>, i64, Vec<u8>, i32, bool)> {
        let conn = self
            .pool
            .get()
            .map_err(|e| MlsError::Storage(format!("Failed to get connection: {}", e)))?;

        let result = conn.query_row(
            "SELECT encrypted_name, encrypted_topic, created_at, encrypted_members, channel_type, archived 
             FROM channels WHERE group_id = ?",
            params![group_id],
            |row| {
                Ok((
                    row.get::<_, Vec<u8>>(0)?,
                    row.get::<_, Option<Vec<u8>>>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, Vec<u8>>(3)?,
                    row.get::<_, i32>(4)?,
                    row.get::<_, i32>(5)? != 0,
                ))
            },
        );

        match result {
            Ok((enc_name, enc_topic, created_at, enc_members, channel_type, archived)) => {
                // Decrypt metadata after loading
                let decrypted_name = decrypt_metadata(group_id, &enc_name)?;
                let decrypted_topic =
                    enc_topic.map(|t| decrypt_metadata(group_id, &t)).transpose()?;
                let decrypted_members = decrypt_metadata(group_id, &enc_members)?;

                Ok((
                    decrypted_name,
                    decrypted_topic,
                    created_at,
                    decrypted_members,
                    channel_type,
                    archived,
                ))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                Err(MlsError::NotFound(format!("Channel not found: {:?}", group_id)))
            }
            Err(e) => Err(MlsError::Storage(format!("Failed to load channel metadata: {}", e))),
        }
    }

    /// List all channels (non-archived by default)
    pub async fn list_channels(&self, include_archived: bool) -> MlsResult<Vec<Vec<u8>>> {
        let conn = self
            .pool
            .get()
            .map_err(|e| MlsError::Storage(format!("Failed to get connection: {}", e)))?;

        let query = if include_archived {
            "SELECT group_id FROM channels ORDER BY created_at DESC"
        } else {
            "SELECT group_id FROM channels WHERE archived = 0 ORDER BY created_at DESC"
        };

        let mut stmt = conn
            .prepare(query)
            .map_err(|e| MlsError::Storage(format!("Failed to prepare statement: {}", e)))?;

        let rows = stmt
            .query_map([], |row| row.get::<_, Vec<u8>>(0))
            .map_err(|e| MlsError::Storage(format!("Failed to query channels: {}", e)))?;

        let mut group_ids = Vec::new();
        for row in rows {
            group_ids
                .push(row.map_err(|e| MlsError::Storage(format!("Failed to read row: {}", e)))?);
        }

        Ok(group_ids)
    }

    /// Archive a channel (soft delete)
    pub async fn archive_channel(&self, group_id: &[u8]) -> MlsResult<()> {
        let conn = self
            .pool
            .get()
            .map_err(|e| MlsError::Storage(format!("Failed to get connection: {}", e)))?;

        let updated = conn
            .execute("UPDATE channels SET archived = 1 WHERE group_id = ?", params![group_id])
            .map_err(|e| MlsError::Storage(format!("Failed to archive channel: {}", e)))?;

        if updated == 0 {
            return Err(MlsError::NotFound(format!("Channel not found: {:?}", group_id)));
        }

        Ok(())
    }

    /// Delete channel and all associated messages (hard delete)
    pub async fn delete_channel(&self, group_id: &[u8]) -> MlsResult<()> {
        let conn = self
            .pool
            .get()
            .map_err(|e| MlsError::Storage(format!("Failed to get connection: {}", e)))?;

        // Messages are deleted via CASCADE foreign key
        let deleted = conn
            .execute("DELETE FROM channels WHERE group_id = ?", params![group_id])
            .map_err(|e| MlsError::Storage(format!("Failed to delete channel: {}", e)))?;

        if deleted == 0 {
            return Err(MlsError::NotFound(format!("Channel not found: {:?}", group_id)));
        }

        Ok(())
    }

    // ======================
    // Message Metadata CRUD
    // ======================

    /// Save a message
    pub async fn save_message(
        &self,
        message_id: &[u8],
        group_id: &[u8],
        encrypted_content: &[u8],
        sender_hash: &[u8],
        sequence: i64,
    ) -> MlsResult<()> {
        let conn = self
            .pool
            .get()
            .map_err(|e| MlsError::Storage(format!("Failed to get connection: {}", e)))?;

        conn.execute(
            "INSERT OR REPLACE INTO messages 
             (message_id, group_id, encrypted_content, sender_hash, sequence, processed) 
             VALUES (?, ?, ?, ?, ?, 0)",
            params![message_id, group_id, encrypted_content, sender_hash, sequence],
        )
        .map_err(|e| MlsError::Storage(format!("Failed to save message: {}", e)))?;

        Ok(())
    }

    /// Load messages for a channel with pagination
    pub async fn load_messages(
        &self,
        group_id: &[u8],
        limit: i64,
        offset: i64,
    ) -> MlsResult<Vec<(Vec<u8>, Vec<u8>, Vec<u8>, i64, bool)>> {
        let conn = self
            .pool
            .get()
            .map_err(|e| MlsError::Storage(format!("Failed to get connection: {}", e)))?;

        let mut stmt = conn
            .prepare(
                "SELECT message_id, encrypted_content, sender_hash, sequence, processed 
                 FROM messages 
                 WHERE group_id = ? 
                 ORDER BY sequence DESC 
                 LIMIT ? OFFSET ?",
            )
            .map_err(|e| MlsError::Storage(format!("Failed to prepare statement: {}", e)))?;

        let rows = stmt
            .query_map(params![group_id, limit, offset], |row| {
                Ok((
                    row.get::<_, Vec<u8>>(0)?,
                    row.get::<_, Vec<u8>>(1)?,
                    row.get::<_, Vec<u8>>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i32>(4)? != 0,
                ))
            })
            .map_err(|e| MlsError::Storage(format!("Failed to query messages: {}", e)))?;

        let mut messages = Vec::new();
        for row in rows {
            messages
                .push(row.map_err(|e| MlsError::Storage(format!("Failed to read row: {}", e)))?);
        }

        Ok(messages)
    }

    /// Mark a message as processed
    pub async fn mark_message_processed(&self, message_id: &[u8]) -> MlsResult<()> {
        let conn = self
            .pool
            .get()
            .map_err(|e| MlsError::Storage(format!("Failed to get connection: {}", e)))?;

        let updated = conn
            .execute("UPDATE messages SET processed = 1 WHERE message_id = ?", params![message_id])
            .map_err(|e| MlsError::Storage(format!("Failed to mark message processed: {}", e)))?;

        if updated == 0 {
            return Err(MlsError::NotFound(format!("Message not found: {:?}", message_id)));
        }

        Ok(())
    }

    /// Get count of unprocessed messages for a channel
    pub async fn get_unprocessed_count(&self, group_id: &[u8]) -> MlsResult<i64> {
        let conn = self
            .pool
            .get()
            .map_err(|e| MlsError::Storage(format!("Failed to get connection: {}", e)))?;

        let count = conn
            .query_row(
                "SELECT COUNT(*) FROM messages WHERE group_id = ? AND processed = 0",
                params![group_id],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|e| {
                MlsError::Storage(format!("Failed to count unprocessed messages: {}", e))
            })?;

        Ok(count)
    }

    /// Delete old messages (keep only last N messages per channel)
    pub async fn prune_old_messages(&self, group_id: &[u8], keep_count: i64) -> MlsResult<usize> {
        let conn = self
            .pool
            .get()
            .map_err(|e| MlsError::Storage(format!("Failed to get connection: {}", e)))?;

        let deleted = conn
            .execute(
                "DELETE FROM messages 
                 WHERE group_id = ? 
                 AND message_id NOT IN (
                     SELECT message_id FROM messages 
                     WHERE group_id = ? 
                     ORDER BY sequence DESC 
                     LIMIT ?
                 )",
                params![group_id, group_id, keep_count],
            )
            .map_err(|e| MlsError::Storage(format!("Failed to prune messages: {}", e)))?;

        Ok(deleted)
    }
}

/// Get current Unix timestamp in seconds
fn current_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs() as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_sql_storage_roundtrip() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let storage = SqlStorageProvider::new(&db_path).unwrap();

        let group_id = b"test_group_123".to_vec();
        let snapshot = PersistedGroupSnapshot {
            group_id: group_id.clone(),
            epoch: 42,
            serialized_group: vec![1, 2, 3, 4, 5],
        };

        // Save
        storage.save_group_snapshot(snapshot.clone()).await.unwrap();

        // Load
        let loaded = storage.load_group_snapshot(&group_id).await.unwrap();
        assert_eq!(loaded.group_id, group_id);
        assert_eq!(loaded.epoch, 42);
        assert_eq!(loaded.serialized_group, vec![1, 2, 3, 4, 5]);

        // List
        let groups = storage.list_groups().await.unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0], group_id);

        // Delete
        storage.delete_group_snapshot(&group_id).await.unwrap();
        let result = storage.load_group_snapshot(&group_id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_blob_storage() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let storage = SqlStorageProvider::new(&db_path).unwrap();

        let key = "test_key";
        let value = b"test_value_data";

        // Put
        storage.put_blob(key, value).await.unwrap();

        // Get
        let loaded = storage.get_blob(key).await.unwrap();
        assert_eq!(loaded, value);

        // Update
        let new_value = b"updated_value";
        storage.put_blob(key, new_value).await.unwrap();
        let loaded = storage.get_blob(key).await.unwrap();
        assert_eq!(loaded, new_value);
    }

    #[tokio::test]
    async fn test_persistence_across_instances() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");

        let group_id = b"persistent_group".to_vec();
        let snapshot = PersistedGroupSnapshot {
            group_id: group_id.clone(),
            epoch: 10,
            serialized_group: vec![9, 8, 7, 6, 5],
        };

        // First instance - save
        {
            let storage = SqlStorageProvider::new(&db_path).unwrap();
            storage.save_group_snapshot(snapshot.clone()).await.unwrap();
        }

        // Second instance - load
        {
            let storage = SqlStorageProvider::new(&db_path).unwrap();
            let loaded = storage.load_group_snapshot(&group_id).await.unwrap();
            assert_eq!(loaded.epoch, 10);
            assert_eq!(loaded.serialized_group, vec![9, 8, 7, 6, 5]);
        }
    }

    #[tokio::test]
    async fn test_key_package_storage() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test_kp.db");
        let storage = SqlStorageProvider::new(&db_path).unwrap();

        let kp_id = b"key_package_1";
        let kp_data = b"serialized_key_package_data";
        let cred_id = b"credential_1";

        // Store key package with 1 hour expiration
        let expires_at = current_timestamp() + 3600;
        storage
            .store_key_package(kp_id, kp_data, cred_id, Some(expires_at))
            .await
            .unwrap();

        // Load it (should work and mark as used)
        let loaded = storage.load_key_package(kp_id).await.unwrap();
        assert_eq!(loaded, kp_data);

        // Try to load again (should fail - already used)
        let result = storage.load_key_package(kp_id).await;
        assert!(matches!(result, Err(MlsError::NotFound(_))));

        // Store an expired key package
        let kp_id2 = b"key_package_2";
        let kp_data2 = b"expired_key_package";
        let expired_at = current_timestamp() - 3600; // 1 hour ago
        storage
            .store_key_package(kp_id2, kp_data2, cred_id, Some(expired_at))
            .await
            .unwrap();

        // Try to load expired package (should fail)
        let result = storage.load_key_package(kp_id2).await;
        assert!(matches!(result, Err(MlsError::NotFound(_))));

        // Delete key package
        storage.store_key_package(b"kp_delete", b"data", cred_id, None).await.unwrap();
        storage.delete_key_package(b"kp_delete").await.unwrap();
        let result = storage.load_key_package(b"kp_delete").await;
        assert!(matches!(result, Err(MlsError::NotFound(_))));

        // Cleanup expired packages
        let deleted = storage.cleanup_expired_key_packages().unwrap();
        assert_eq!(deleted, 1); // The expired kp_id2
    }

    #[tokio::test]
    async fn test_atomic_operations() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test_atomic.db");
        let storage = SqlStorageProvider::new(&db_path).unwrap();

        // Create multiple snapshots
        let snapshots = vec![
            PersistedGroupSnapshot {
                group_id: b"group_1".to_vec(),
                epoch: 1,
                serialized_group: vec![1, 2, 3],
            },
            PersistedGroupSnapshot {
                group_id: b"group_2".to_vec(),
                epoch: 2,
                serialized_group: vec![4, 5, 6],
            },
            PersistedGroupSnapshot {
                group_id: b"group_3".to_vec(),
                epoch: 3,
                serialized_group: vec![7, 8, 9],
            },
        ];

        // Save all atomically
        storage.save_group_snapshots_atomic(&snapshots).await.unwrap();

        // Verify all were saved
        for snapshot in &snapshots {
            let loaded = storage.load_group_snapshot(&snapshot.group_id).await.unwrap();
            assert_eq!(loaded.epoch, snapshot.epoch);
            assert_eq!(loaded.serialized_group, snapshot.serialized_group);
        }

        // Delete all atomically
        let group_ids: Vec<_> = snapshots.iter().map(|s| s.group_id.clone()).collect();
        storage.delete_groups_atomic(&group_ids).await.unwrap();

        // Verify all were deleted
        for snapshot in &snapshots {
            let result = storage.load_group_snapshot(&snapshot.group_id).await;
            assert!(matches!(result, Err(MlsError::NotFound(_))));
        }
    }

    #[tokio::test]
    async fn test_channel_metadata_crud() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test_channels.db");
        let storage = SqlStorageProvider::new(&db_path).unwrap();

        let group_id = b"test_channel_group";
        let encrypted_name = b"encrypted_channel_name";
        let encrypted_topic = b"encrypted_topic";
        let encrypted_members = b"encrypted_member_list";
        let channel_type = 1; // Public channel

        // Save channel
        storage
            .save_channel_metadata(
                group_id,
                encrypted_name,
                Some(encrypted_topic),
                encrypted_members,
                channel_type,
            )
            .await
            .unwrap();

        // Load channel
        let (name, topic, created_at, members, ch_type, archived) =
            storage.load_channel_metadata(group_id).await.unwrap();

        assert_eq!(name, encrypted_name);
        assert_eq!(topic, Some(encrypted_topic.to_vec()));
        assert_eq!(members, encrypted_members);
        assert_eq!(ch_type, channel_type);
        assert!(!archived);
        assert!(created_at > 0);

        // List channels
        let channels = storage.list_channels(false).await.unwrap();
        assert_eq!(channels.len(), 1);
        assert_eq!(channels[0], group_id);

        // Archive channel
        storage.archive_channel(group_id).await.unwrap();
        let (_, _, _, _, _, archived) = storage.load_channel_metadata(group_id).await.unwrap();
        assert!(archived);

        // List should exclude archived
        let channels = storage.list_channels(false).await.unwrap();
        assert_eq!(channels.len(), 0);

        // List with archived included
        let channels = storage.list_channels(true).await.unwrap();
        assert_eq!(channels.len(), 1);

        // Delete channel
        storage.delete_channel(group_id).await.unwrap();
        let result = storage.load_channel_metadata(group_id).await;
        assert!(matches!(result, Err(MlsError::NotFound(_))));
    }

    #[tokio::test]
    async fn test_message_crud() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test_messages.db");
        let storage = SqlStorageProvider::new(&db_path).unwrap();

        let group_id = b"test_message_group";

        // Create channel first (required for foreign key)
        storage
            .save_channel_metadata(group_id, b"name", None, b"members", 1)
            .await
            .unwrap();

        // Save messages
        for i in 0..10 {
            let msg_id = format!("msg_{}", i).into_bytes();
            let content = format!("encrypted_content_{}", i).into_bytes();
            let sender = format!("sender_hash_{}", i % 3).into_bytes();

            storage
                .save_message(&msg_id, group_id, &content, &sender, i as i64)
                .await
                .unwrap();
        }

        // Load messages with pagination
        let messages = storage.load_messages(group_id, 5, 0).await.unwrap();
        assert_eq!(messages.len(), 5);

        // Verify reverse chronological order (latest first)
        assert_eq!(messages[0].3, 9); // sequence
        assert_eq!(messages[4].3, 5);

        // Load next page
        let messages = storage.load_messages(group_id, 5, 5).await.unwrap();
        assert_eq!(messages.len(), 5);
        assert_eq!(messages[0].3, 4);
        assert_eq!(messages[4].3, 0);

        // Mark message as processed
        let msg_id = b"msg_5";
        storage.mark_message_processed(msg_id).await.unwrap();

        // Get unprocessed count
        let count = storage.get_unprocessed_count(group_id).await.unwrap();
        assert_eq!(count, 9); // 10 total - 1 processed

        // Prune old messages (keep only 5 most recent)
        let deleted = storage.prune_old_messages(group_id, 5).await.unwrap();
        assert_eq!(deleted, 5);

        // Verify only 5 remain
        let messages = storage.load_messages(group_id, 100, 0).await.unwrap();
        assert_eq!(messages.len(), 5);
        assert_eq!(messages[0].3, 9); // Most recent
        assert_eq!(messages[4].3, 5); // Oldest kept
    }

    #[tokio::test]
    async fn test_cascade_delete() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test_cascade.db");
        let storage = SqlStorageProvider::new(&db_path).unwrap();

        let group_id = b"cascade_test_group";

        // Create channel and messages
        storage
            .save_channel_metadata(group_id, b"name", None, b"members", 1)
            .await
            .unwrap();

        for i in 0..5 {
            let msg_id = format!("msg_{}", i).into_bytes();
            storage.save_message(&msg_id, group_id, b"content", b"sender", i).await.unwrap();
        }

        // Verify messages exist
        let messages = storage.load_messages(group_id, 100, 0).await.unwrap();
        assert_eq!(messages.len(), 5);

        // Delete channel (should cascade to messages)
        storage.delete_channel(group_id).await.unwrap();

        // Verify messages are gone
        let messages = storage.load_messages(group_id, 100, 0).await.unwrap();
        assert_eq!(messages.len(), 0);
    }
}
