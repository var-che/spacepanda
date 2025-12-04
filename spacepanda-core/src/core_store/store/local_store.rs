/*
    local_store.rs - High-level local storage interface

    Provides the main API for persisting and retrieving CRDT state.
    Coordinates between commit log, snapshots, and indices.

    Architecture:
    - Append-only commit log for all operations
    - Periodic snapshots for fast rehydration
    - Indices for efficient queries
    - At-rest encryption for all data
*/

use crate::core_store::crdt::{Crdt, OperationMetadata};
use crate::core_store::model::{Channel, ChannelId, Message, MessageId, Space, SpaceId};
use crate::core_store::store::commit_log::CommitLog;
use crate::core_store::store::encryption::EncryptionManager;
use crate::core_store::store::errors::{StoreError, StoreResult};
use crate::core_store::store::index::IndexManager;
use crate::core_store::store::snapshot::SnapshotManager;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, PoisonError, RwLock};

/// Helper to convert poison errors into StoreError
fn handle_poison<T>(_err: PoisonError<T>) -> StoreError {
    StoreError::Storage("Lock poisoned: a thread panicked while holding the lock".to_string())
}

/// Configuration for local storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalStoreConfig {
    /// Base directory for all storage
    pub data_dir: PathBuf,

    /// Enable at-rest encryption
    pub enable_encryption: bool,

    /// Snapshot interval (number of operations)
    pub snapshot_interval: usize,

    /// Maximum commit log size before rotation
    pub max_log_size: usize,

    /// Enable automatic compaction
    pub enable_compaction: bool,

    /// Require signatures on all operations (production mode)
    pub require_signatures: bool,

    /// Authorized public keys for signature verification (Ed25519, 32 bytes each)
    pub authorized_keys: Vec<Vec<u8>>,
}

impl Default for LocalStoreConfig {
    fn default() -> Self {
        LocalStoreConfig {
            data_dir: PathBuf::from("./data"),
            enable_encryption: true,
            snapshot_interval: 1000,
            max_log_size: 10_000_000, // 10MB
            enable_compaction: true,
            require_signatures: false, // Disabled by default for backwards compatibility
            authorized_keys: Vec::new(),
        }
    }
}

/// Main local storage interface
pub struct LocalStore {
    config: LocalStoreConfig,
    commit_log: Arc<RwLock<CommitLog>>,
    snapshot_manager: Arc<SnapshotManager>,
    index_manager: Arc<IndexManager>,
    encryption: Option<Arc<EncryptionManager>>,

    /// In-memory cache of spaces
    spaces_cache: Arc<RwLock<HashMap<SpaceId, Space>>>,

    /// In-memory cache of channels
    channels_cache: Arc<RwLock<HashMap<ChannelId, Channel>>>,

    /// In-memory cache of messages (ChannelId -> Vec<Message>)
    messages_cache: Arc<RwLock<HashMap<ChannelId, Vec<Message>>>>,

    /// Operation counter for snapshots
    operation_count: Arc<RwLock<usize>>,
}

impl LocalStore {
    /// Create a new local store with the given configuration
    pub fn new(config: LocalStoreConfig) -> StoreResult<Self> {
        // Create data directory if it doesn't exist
        std::fs::create_dir_all(&config.data_dir)?;

        let commit_log = Arc::new(RwLock::new(CommitLog::new(config.data_dir.join("commit_log"))?));

        let snapshot_manager = Arc::new(SnapshotManager::new(config.data_dir.join("snapshots"))?);

        let index_manager = Arc::new(IndexManager::new(config.data_dir.join("indices"))?);

        let encryption = if config.enable_encryption {
            Some(Arc::new(EncryptionManager::new()?))
        } else {
            None
        };

        Ok(LocalStore {
            config,
            commit_log,
            snapshot_manager,
            index_manager,
            encryption,
            spaces_cache: Arc::new(RwLock::new(HashMap::new())),
            channels_cache: Arc::new(RwLock::new(HashMap::new())),
            messages_cache: Arc::new(RwLock::new(HashMap::new())),
            operation_count: Arc::new(RwLock::new(0)),
        })
    }

    /// Store a space
    pub fn store_space(&self, space: &Space) -> StoreResult<()> {
        // Serialize space
        let data = bincode::serialize(space)?;

        // Encrypt if enabled
        let data = if let Some(enc) = &self.encryption {
            enc.encrypt(&data)?
        } else {
            data
        };

        // Write to commit log
        self.commit_log.write().map_err(handle_poison)?.append(&data)?;

        // Update cache
        self.spaces_cache
            .write()
            .map_err(handle_poison)?
            .insert(space.id.clone(), space.clone());

        // Update indices
        self.index_manager.index_space(&space.id)?;

        // Check if we need to snapshot
        self.maybe_snapshot()?;

        Ok(())
    }

    /// Retrieve a space by ID
    pub fn get_space(&self, space_id: &SpaceId) -> StoreResult<Option<Space>> {
        // Check cache first
        if let Some(space) = self.spaces_cache.read().map_err(handle_poison)?.get(space_id) {
            return Ok(Some(space.clone()));
        }

        // Try to load from snapshot
        if let Some(space) = self.snapshot_manager.load_space(space_id)? {
            // Update cache
            self.spaces_cache
                .write()
                .map_err(handle_poison)?
                .insert(space_id.clone(), space.clone());
            return Ok(Some(space));
        }

        // Not found
        Ok(None)
    }

    /// Store a channel
    pub fn store_channel(&self, channel: &Channel) -> StoreResult<()> {
        let data = bincode::serialize(channel)?;

        let data = if let Some(enc) = &self.encryption {
            enc.encrypt(&data)?
        } else {
            data
        };

        self.commit_log.write().map_err(handle_poison)?.append(&data)?;
        self.channels_cache
            .write()
            .map_err(handle_poison)?
            .insert(channel.id.clone(), channel.clone());
        self.index_manager.index_channel(&channel.id)?;
        self.maybe_snapshot()?;

        Ok(())
    }

    /// Retrieve a channel by ID
    pub fn get_channel(&self, channel_id: &ChannelId) -> StoreResult<Option<Channel>> {
        if let Some(channel) = self.channels_cache.read().map_err(handle_poison)?.get(channel_id) {
            return Ok(Some(channel.clone()));
        }

        if let Some(channel) = self.snapshot_manager.load_channel(channel_id)? {
            self.channels_cache
                .write()
                .map_err(handle_poison)?
                .insert(channel_id.clone(), channel.clone());
            return Ok(Some(channel));
        }

        Ok(None)
    }

    /// List all spaces
    pub fn list_spaces(&self) -> StoreResult<Vec<SpaceId>> {
        Ok(self.spaces_cache.read().map_err(handle_poison)?.keys().cloned().collect())
    }

    /// List all channels
    pub fn list_channels(&self) -> StoreResult<Vec<ChannelId>> {
        Ok(self.channels_cache.read().map_err(handle_poison)?.keys().cloned().collect())
    }

    /// Store a message
    pub fn store_message(&self, message: &Message) -> StoreResult<()> {
        // Serialize message
        let data = bincode::serialize(message)?;

        // Encrypt if enabled
        let data = if let Some(enc) = &self.encryption {
            enc.encrypt(&data)?
        } else {
            data
        };

        // Write to commit log
        self.commit_log.write().map_err(handle_poison)?.append(&data)?;

        // Update cache
        self.messages_cache
            .write()
            .map_err(handle_poison)?
            .entry(message.channel_id.clone())
            .or_insert_with(Vec::new)
            .push(message.clone());

        // Check if we need to snapshot
        self.maybe_snapshot()?;

        Ok(())
    }

    /// Get a single message by ID
    pub fn get_message(&self, message_id: &MessageId) -> StoreResult<Option<Message>> {
        // Search in cache
        let cache = self.messages_cache.read().map_err(handle_poison)?;
        for messages in cache.values() {
            if let Some(msg) = messages.iter().find(|m| &m.id == message_id) {
                return Ok(Some(msg.clone()));
            }
        }
        Ok(None)
    }

    /// Get all messages for a channel
    pub fn get_channel_messages(&self, channel_id: &ChannelId) -> StoreResult<Vec<Message>> {
        let cache = self.messages_cache.read().map_err(handle_poison)?;
        Ok(cache.get(channel_id).cloned().unwrap_or_default())
    }

    /// Get messages for a channel with pagination
    pub fn get_channel_messages_paginated(
        &self,
        channel_id: &ChannelId,
        limit: usize,
        offset: usize,
    ) -> StoreResult<Vec<Message>> {
        let cache = self.messages_cache.read().map_err(handle_poison)?;
        let messages = cache.get(channel_id).cloned().unwrap_or_default();
        
        // Sort by timestamp (newest first)
        let mut sorted: Vec<Message> = messages;
        sorted.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        
        // Apply pagination
        Ok(sorted.into_iter().skip(offset).take(limit).collect())
    }

    /// Get thread replies (messages that reply to a specific message)
    pub fn get_thread_replies(&self, parent_id: &MessageId) -> StoreResult<Vec<Message>> {
        let cache = self.messages_cache.read().map_err(handle_poison)?;
        let mut replies = Vec::new();
        
        for messages in cache.values() {
            for msg in messages {
                if let Some(reply_to) = &msg.reply_to {
                    if reply_to == parent_id {
                        replies.push(msg.clone());
                    }
                }
            }
        }
        
        // Sort by timestamp
        replies.sort_by_key(|m| m.timestamp);
        
        Ok(replies)
    }

    /// Apply a CRDT operation and persist it
    ///
    /// Note: For cryptographic signature verification, wrap CRDTs with ValidatedCrdt.
    /// This method persists operations without verifying signatures - that's the CRDT layer's job.
    pub fn apply_operation<T: Crdt + Serialize>(
        &self,
        target_id: &str,
        operation: &T,
        metadata: &OperationMetadata,
    ) -> StoreResult<()> {
        // Serialize the operation
        let op_data = bincode::serialize(&(target_id, operation, metadata))?;

        // Encrypt if needed
        let op_data = if let Some(enc) = &self.encryption {
            enc.encrypt(&op_data)?
        } else {
            op_data
        };

        // Append to commit log
        self.commit_log.write().map_err(handle_poison)?.append(&op_data)?;

        // Increment operation counter
        *self.operation_count.write().map_err(handle_poison)? += 1;

        // Maybe snapshot
        self.maybe_snapshot()?;

        Ok(())
    }

    /// Create a snapshot if needed
    fn maybe_snapshot(&self) -> StoreResult<()> {
        let count = *self.operation_count.read().map_err(handle_poison)?;

        if count >= self.config.snapshot_interval {
            self.create_snapshot()?;
            *self.operation_count.write().map_err(handle_poison)? = 0;
        }

        Ok(())
    }

    /// Force create a snapshot
    pub fn create_snapshot(&self) -> StoreResult<()> {
        let spaces = self.spaces_cache.read().map_err(handle_poison)?.clone();
        let channels = self.channels_cache.read().map_err(handle_poison)?.clone();

        self.snapshot_manager.create_snapshot(spaces, channels)?;

        Ok(())
    }

    /// Load all state from snapshots and replay commit log
    pub fn load(&self) -> StoreResult<()> {
        // Load latest snapshot
        let (spaces, channels) = self.snapshot_manager.load_latest()?;

        *self.spaces_cache.write().map_err(handle_poison)? = spaces;
        *self.channels_cache.write().map_err(handle_poison)? = channels;

        // TODO: Replay commit log entries after snapshot

        Ok(())
    }

    /// Compact the commit log
    pub fn compact(&self) -> StoreResult<()> {
        // Create snapshot
        self.create_snapshot()?;

        // Truncate commit log
        self.commit_log.write().map_err(handle_poison)?.truncate()?;

        Ok(())
    }

    /// Get storage statistics
    pub fn stats(&self) -> StoreResult<StoreStats> {
        Ok(StoreStats {
            spaces_count: self.spaces_cache.read().map_err(handle_poison)?.len(),
            channels_count: self.channels_cache.read().map_err(handle_poison)?.len(),
            operation_count: *self.operation_count.read().map_err(handle_poison)?,
            log_size: self.commit_log.read().map_err(handle_poison)?.size(),
        })
    }
}

/// Storage statistics
#[derive(Debug, Clone)]
pub struct StoreStats {
    pub spaces_count: usize,
    pub channels_count: usize,
    pub operation_count: usize,
    pub log_size: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core_store::model::{ChannelType, Timestamp, UserId};
    use tempfile::tempdir;

    #[test]
    fn test_local_store_creation() {
        let dir = tempdir().unwrap();
        let config = LocalStoreConfig {
            data_dir: dir.path().to_path_buf(),
            enable_encryption: false,
            require_signatures: false,
            authorized_keys: Vec::new(),
            ..Default::default()
        };

        let store = LocalStore::new(config);
        assert!(store.is_ok());
    }

    #[test]
    fn test_store_and_retrieve_space() {
        let dir = tempdir().unwrap();
        let config = LocalStoreConfig {
            data_dir: dir.path().to_path_buf(),
            enable_encryption: false,
            require_signatures: false,
            authorized_keys: Vec::new(),
            ..Default::default()
        };

        let store = LocalStore::new(config).unwrap();

        let space = Space::new(
            SpaceId::generate(),
            "Test Space".to_string(),
            UserId::generate(),
            Timestamp::now(),
            "node1".to_string(),
        );

        let space_id = space.id.clone();

        store.store_space(&space).unwrap();

        let retrieved = store.get_space(&space_id).unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().id, space_id);
    }

    #[test]
    fn test_store_and_retrieve_channel() {
        let dir = tempdir().unwrap();
        let config = LocalStoreConfig {
            data_dir: dir.path().to_path_buf(),
            enable_encryption: false,
            require_signatures: false,
            authorized_keys: Vec::new(),
            ..Default::default()
        };

        let store = LocalStore::new(config).unwrap();

        let channel = Channel::new(
            ChannelId::generate(),
            "general".to_string(),
            ChannelType::Text,
            UserId::generate(),
            Timestamp::now(),
            "node1".to_string(),
        );

        let channel_id = channel.id.clone();

        store.store_channel(&channel).unwrap();

        let retrieved = store.get_channel(&channel_id).unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().id, channel_id);
    }

    #[test]
    fn test_stats() {
        let dir = tempdir().unwrap();
        let config = LocalStoreConfig {
            data_dir: dir.path().to_path_buf(),
            enable_encryption: false,
            require_signatures: false,
            authorized_keys: Vec::new(),
            ..Default::default()
        };

        let store = LocalStore::new(config).unwrap();

        let stats = store.stats().unwrap();
        assert_eq!(stats.spaces_count, 0);
        assert_eq!(stats.channels_count, 0);
    }

    /// Example: Using ValidatedCrdt for signature enforcement
    ///
    /// This test demonstrates the recommended pattern for enforcing signatures on CRDT operations.
    /// Rather than enforcing at the store layer, wrap CRDTs with ValidatedCrdt at the application layer.
    #[test]
    fn test_validated_crdt_example() {
        use crate::core_identity::keypair::{KeyType, Keypair};
        use crate::core_store::crdt::{
            or_set::{AddId, ORSetOperation},
            ORSet, SignatureConfig, ValidatedCrdt,
        };

        let keypair = Keypair::generate(KeyType::Ed25519);
        let public_key = keypair.public_key().to_vec();

        // Create signature config for a channel
        let config = SignatureConfig::required("channel-123".to_string(), vec![public_key]);

        // Wrap OR-Set with signature validation
        let mut validated_set: ValidatedCrdt<ORSet<UserId>> =
            ValidatedCrdt::new(ORSet::new(), config);

        // Operations applied to validated_set will have signatures verified
        // This is the recommended pattern for production channels
        assert!(validated_set.inner().elements().is_empty());
    }
}
