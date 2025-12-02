/*
    index.rs - Efficient lookup indices
    
    Maintains indices for fast queries without scanning all data.
    
    Indices:
    - Space ID -> metadata
    - Channel ID -> metadata
    - User ID -> spaces/channels
    - Timestamp -> operations
*/

use crate::core_store::model::{SpaceId, ChannelId, UserId};
use crate::core_store::store::errors::{StoreResult, StoreError};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{RwLock, PoisonError};

/// Helper to convert poison errors into StoreError
fn handle_poison<T>(_err: PoisonError<T>) -> StoreError {
    StoreError::Storage("Lock poisoned: a thread panicked while holding the lock".to_string())
}

/// Index for efficient lookups
pub struct IndexManager {
    indices_dir: PathBuf,
    
    /// Space ID index
    space_index: RwLock<HashSet<SpaceId>>,
    
    /// Channel ID index
    channel_index: RwLock<HashSet<ChannelId>>,
    
    /// User -> Spaces mapping
    user_spaces: RwLock<HashMap<UserId, HashSet<SpaceId>>>,
    
    /// User -> Channels mapping
    user_channels: RwLock<HashMap<UserId, HashSet<ChannelId>>>,
}

impl IndexManager {
    pub fn new(indices_dir: PathBuf) -> StoreResult<Self> {
        std::fs::create_dir_all(&indices_dir)?;
        
        Ok(IndexManager {
            indices_dir,
            space_index: RwLock::new(HashSet::new()),
            channel_index: RwLock::new(HashSet::new()),
            user_spaces: RwLock::new(HashMap::new()),
            user_channels: RwLock::new(HashMap::new()),
        })
    }
    
    /// Index a space
    pub fn index_space(&self, space_id: &SpaceId) -> StoreResult<()> {
        self.space_index.write().map_err(handle_poison)?.insert(space_id.clone());
        Ok(())
    }
    
    /// Index a channel
    pub fn index_channel(&self, channel_id: &ChannelId) -> StoreResult<()> {
        self.channel_index.write().map_err(handle_poison)?.insert(channel_id.clone());
        Ok(())
    }
    
    /// Add user to space mapping
    pub fn add_user_to_space(&self, user_id: &UserId, space_id: &SpaceId) -> StoreResult<()> {
        self.user_spaces.write().map_err(handle_poison)?
            .entry(user_id.clone())
            .or_insert_with(HashSet::new)
            .insert(space_id.clone());
        Ok(())
    }
    
    /// Add user to channel mapping
    pub fn add_user_to_channel(&self, user_id: &UserId, channel_id: &ChannelId) -> StoreResult<()> {
        self.user_channels.write().map_err(handle_poison)?
            .entry(user_id.clone())
            .or_insert_with(HashSet::new)
            .insert(channel_id.clone());
        Ok(())
    }
    
    /// Get all spaces for a user
    pub fn get_user_spaces(&self, user_id: &UserId) -> StoreResult<Vec<SpaceId>> {
        Ok(self.user_spaces.read().map_err(handle_poison)?
            .get(user_id)
            .map(|set| set.iter().cloned().collect())
            .unwrap_or_default())
    }
    
    /// Get all channels for a user
    pub fn get_user_channels(&self, user_id: &UserId) -> StoreResult<Vec<ChannelId>> {
        Ok(self.user_channels.read().map_err(handle_poison)?
            .get(user_id)
            .map(|set| set.iter().cloned().collect())
            .unwrap_or_default())
    }
    
    /// Check if a space exists
    pub fn has_space(&self, space_id: &SpaceId) -> StoreResult<bool> {
        Ok(self.space_index.read().map_err(handle_poison)?.contains(space_id))
    }
    
    /// Check if a channel exists
    pub fn has_channel(&self, channel_id: &ChannelId) -> StoreResult<bool> {
        Ok(self.channel_index.read().map_err(handle_poison)?.contains(channel_id))
    }
    
    /// Get all indexed spaces
    pub fn all_spaces(&self) -> StoreResult<Vec<SpaceId>> {
        Ok(self.space_index.read().map_err(handle_poison)?.iter().cloned().collect())
    }
    
    /// Get all indexed channels
    pub fn all_channels(&self) -> StoreResult<Vec<ChannelId>> {
        Ok(self.channel_index.read().map_err(handle_poison)?.iter().cloned().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    
    #[test]
    fn test_index_manager_creation() {
        let dir = tempdir().unwrap();
        let manager = IndexManager::new(dir.path().to_path_buf());
        assert!(manager.is_ok());
    }
    
    #[test]
    fn test_index_space() {
        let dir = tempdir().unwrap();
        let manager = IndexManager::new(dir.path().to_path_buf()).unwrap();
        
        let space_id = SpaceId::generate();
        manager.index_space(&space_id).unwrap();
        
        assert!(manager.has_space(&space_id).unwrap());
    }
    
    #[test]
    fn test_index_channel() {
        let dir = tempdir().unwrap();
        let manager = IndexManager::new(dir.path().to_path_buf()).unwrap();
        
        let channel_id = ChannelId::generate();
        manager.index_channel(&channel_id).unwrap();
        
        assert!(manager.has_channel(&channel_id).unwrap());
    }
    
    #[test]
    fn test_user_space_mapping() {
        let dir = tempdir().unwrap();
        let manager = IndexManager::new(dir.path().to_path_buf()).unwrap();
        
        let user_id = UserId::generate();
        let space_id = SpaceId::generate();
        
        manager.add_user_to_space(&user_id, &space_id).unwrap();
        
        let spaces = manager.get_user_spaces(&user_id).unwrap();
        assert_eq!(spaces.len(), 1);
        assert_eq!(spaces[0], space_id);
    }
    
    #[test]
    fn test_user_channel_mapping() {
        let dir = tempdir().unwrap();
        let manager = IndexManager::new(dir.path().to_path_buf()).unwrap();
        
        let user_id = UserId::generate();
        let channel_id = ChannelId::generate();
        
        manager.add_user_to_channel(&user_id, &channel_id).unwrap();
        
        let channels = manager.get_user_channels(&user_id).unwrap();
        assert_eq!(channels.len(), 1);
        assert_eq!(channels[0], channel_id);
    }
}
