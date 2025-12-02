/*
    snapshot.rs - State snapshots for fast recovery
    
    Periodically snapshots the full CRDT state to disk.
    Enables fast startup without replaying entire commit log.
    
    Features:
    - Atomic snapshot creation (write to temp, then rename)
    - Versioned snapshots with metadata
    - Automatic cleanup of old snapshots
*/

use crate::core_store::model::{Space, Channel, SpaceId, ChannelId};
use crate::core_store::store::errors::StoreResult;
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::fs::{File, create_dir_all};
use std::io::{Write, Read};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

/// Snapshot metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotMetadata {
    /// Snapshot version
    pub version: u32,
    
    /// Timestamp when created
    pub timestamp: u64,
    
    /// Number of spaces
    pub spaces_count: usize,
    
    /// Number of channels
    pub channels_count: usize,
}

/// Snapshot data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub metadata: SnapshotMetadata,
    pub spaces: HashMap<SpaceId, Space>,
    pub channels: HashMap<ChannelId, Channel>,
}

/// Manages snapshots
pub struct SnapshotManager {
    snapshots_dir: PathBuf,
    current_version: AtomicU32,
}

impl SnapshotManager {
    pub fn new(snapshots_dir: PathBuf) -> StoreResult<Self> {
        create_dir_all(&snapshots_dir)?;
        
        Ok(SnapshotManager {
            snapshots_dir,
            current_version: AtomicU32::new(0),
        })
    }
    
    /// Create a new snapshot
    pub fn create_snapshot(
        &self,
        spaces: HashMap<SpaceId, Space>,
        channels: HashMap<ChannelId, Channel>,
    ) -> StoreResult<()> {
        let version = self.current_version.fetch_add(1, Ordering::SeqCst) + 1;
        let metadata = SnapshotMetadata {
            version,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            spaces_count: spaces.len(),
            channels_count: channels.len(),
        };
        
        let snapshot = Snapshot {
            metadata,
            spaces,
            channels,
        };
        
        // Serialize snapshot
        let data = bincode::serialize(&snapshot)?;
        
        // Write to temporary file first
        let temp_path = self.snapshots_dir.join(format!("snapshot_{}.tmp", snapshot.metadata.version));
        let mut file = File::create(&temp_path)?;
        file.write_all(&data)?;
        file.sync_all()?;
        drop(file);
        
        // Atomically rename to final name
        let final_path = self.snapshots_dir.join(format!("snapshot_{}.bin", snapshot.metadata.version));
        std::fs::rename(temp_path, final_path)?;
        
        Ok(())
    }
    
    /// Load the latest snapshot
    pub fn load_latest(&self) -> StoreResult<(HashMap<SpaceId, Space>, HashMap<ChannelId, Channel>)> {
        // Find latest snapshot file
        let mut snapshots: Vec<_> = std::fs::read_dir(&self.snapshots_dir)?
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                entry.path().extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| ext == "bin")
                    .unwrap_or(false)
            })
            .collect();
        
        if snapshots.is_empty() {
            return Ok((HashMap::new(), HashMap::new()));
        }
        
        // Sort by filename (version)
        snapshots.sort_by_key(|entry| entry.file_name());
        
        let latest = snapshots.last().unwrap();
        
        // Read and deserialize
        let mut file = File::open(latest.path())?;
        let mut data = Vec::new();
        file.read_to_end(&mut data)?;
        
        let snapshot: Snapshot = bincode::deserialize(&data)?;
        
        Ok((snapshot.spaces, snapshot.channels))
    }
    
    /// Load a specific space from snapshot
    pub fn load_space(&self, space_id: &SpaceId) -> StoreResult<Option<Space>> {
        let (spaces, _) = self.load_latest()?;
        Ok(spaces.get(space_id).cloned())
    }
    
    /// Load a specific channel from snapshot
    pub fn load_channel(&self, channel_id: &ChannelId) -> StoreResult<Option<Channel>> {
        let (_, channels) = self.load_latest()?;
        Ok(channels.get(channel_id).cloned())
    }
    
    /// Clean up old snapshots, keeping only the N most recent
    pub fn cleanup_old_snapshots(&self, keep_count: usize) -> StoreResult<()> {
        let mut snapshots: Vec<_> = std::fs::read_dir(&self.snapshots_dir)?
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                entry.path().extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| ext == "bin")
                    .unwrap_or(false)
            })
            .collect();
        
        if snapshots.len() <= keep_count {
            return Ok(());
        }
        
        snapshots.sort_by_key(|entry| entry.file_name());
        
        // Remove oldest snapshots
        for entry in snapshots.iter().take(snapshots.len() - keep_count) {
            std::fs::remove_file(entry.path())?;
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core_store::model::{ChannelType, UserId, Timestamp};
    use tempfile::tempdir;
    
    #[test]
    fn test_snapshot_creation() {
        let dir = tempdir().unwrap();
        let manager = SnapshotManager::new(dir.path().to_path_buf()).unwrap();
        
        let mut spaces = HashMap::new();
        let space = Space::new(
            SpaceId::generate(),
            "Test Space".to_string(),
            UserId::generate(),
            Timestamp::now(),
            "node1".to_string(),
        );
        spaces.insert(space.id.clone(), space);
        
        let result = manager.create_snapshot(spaces, HashMap::new());
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_load_latest_empty() {
        let dir = tempdir().unwrap();
        let manager = SnapshotManager::new(dir.path().to_path_buf()).unwrap();
        
        let (spaces, channels) = manager.load_latest().unwrap();
        assert_eq!(spaces.len(), 0);
        assert_eq!(channels.len(), 0);
    }
    
    #[test]
    fn test_create_and_load() {
        let dir = tempdir().unwrap();
        let manager = SnapshotManager::new(dir.path().to_path_buf()).unwrap();
        
        let mut spaces = HashMap::new();
        let space = Space::new(
            SpaceId::generate(),
            "Test Space".to_string(),
            UserId::generate(),
            Timestamp::now(),
            "node1".to_string(),
        );
        let space_id = space.id.clone();
        spaces.insert(space.id.clone(), space);
        
        let mut channels = HashMap::new();
        let channel = Channel::new(
            ChannelId::generate(),
            "general".to_string(),
            ChannelType::Text,
            UserId::generate(),
            Timestamp::now(),
            "node1".to_string(),
        );
        let channel_id = channel.id.clone();
        channels.insert(channel.id.clone(), channel);
        
        manager.create_snapshot(spaces.clone(), channels.clone()).unwrap();
        
        let (loaded_spaces, loaded_channels) = manager.load_latest().unwrap();
        
        assert_eq!(loaded_spaces.len(), 1);
        assert_eq!(loaded_channels.len(), 1);
        assert!(loaded_spaces.contains_key(&space_id));
        assert!(loaded_channels.contains_key(&channel_id));
    }
    
    #[test]
    fn test_cleanup_old_snapshots() {
        let dir = tempdir().unwrap();
        let manager = SnapshotManager::new(dir.path().to_path_buf()).unwrap();
        
        // Create multiple snapshots
        for _ in 0..5 {
            manager.create_snapshot(HashMap::new(), HashMap::new()).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        
        // Cleanup, keeping only 2
        manager.cleanup_old_snapshots(2).unwrap();
        
        let count = std::fs::read_dir(dir.path())
            .unwrap()
            .filter(|e| e.is_ok())
            .count();
        
        assert_eq!(count, 2);
    }
}
