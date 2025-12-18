use spacepanda_core::config::Config;
use spacepanda_core::core_space::{AsyncSpaceManager, SpaceSqlStore};
use spacepanda_core::core_mls::service::MlsService;
use spacepanda_core::core_store::model::{UserId, types};
use spacepanda_core::shutdown::ShutdownCoordinator;
use spacepanda_core::core_router::router_handle::RouterHandle;
use spacepanda_core::core_router::session_manager::PeerId;
use spacepanda_core::core_mvp::network::NetworkLayer;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use uuid::Uuid;

use crate::auth::UserProfile;
use crate::error::{ApiError, ApiResult};

/// Session token -> User session data
#[derive(Clone)]
pub struct Session {
    pub token: String,
    pub user_id: UserId,
    pub username: String,
    pub manager: Arc<AsyncSpaceManager>,
    pub network_task: Arc<Option<JoinHandle<()>>>,
    pub peer_id: PeerId,
}

/// Manages active user sessions
pub struct SessionManager {
    sessions: Arc<RwLock<HashMap<String, Session>>>,
    /// Shared router handle for all sessions (enables in-process P2P)
    shared_router: Arc<RouterHandle>,
    /// Shared channel members map for P2P message routing
    shared_channel_members: Arc<RwLock<HashMap<types::ChannelId, HashMap<UserId, PeerId>>>>,
    /// Counter for generating unique peer IDs
    peer_id_counter: Arc<tokio::sync::Mutex<u64>>,
}

impl SessionManager {
    pub fn new() -> Self {
        // Create a single shared router for all sessions
        let (router_handle, _router_task) = RouterHandle::new();
        
        // Start listening on a random port (0 = OS assigns a random available port)
        let router_clone = router_handle.clone();
        tokio::spawn(async move {
            // Use TCP socket address format, not libp2p multiaddr
            if let Err(e) = router_clone.listen("0.0.0.0:0".to_string()).await {
                eprintln!("[P2P] Failed to start shared router: {}", e);
            } else {
                eprintln!("[P2P] Shared router listening on random port");
            }
        });
        
        // Create shared channel members map for all NetworkLayer instances
        let shared_channel_members = Arc::new(RwLock::new(HashMap::new()));
        
        eprintln!("[P2P] Created shared channel members registry for P2P routing");
        
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            shared_router: Arc::new(router_handle),
            shared_channel_members,
            peer_id_counter: Arc::new(tokio::sync::Mutex::new(0)),
        }
    }

    pub async fn create_session(&self, profile: &UserProfile) -> ApiResult<String> {
        let token = Uuid::new_v4().to_string();

        // Initialize storage and MLS service for this user
        let db_path = profile.data_dir.join("spaces.db");
        let manager = SqliteConnectionManager::file(&db_path);
        let pool = Pool::new(manager)
            .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to create connection pool: {}", e)))?;
        
        let store = SpaceSqlStore::new(pool)
            .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to create store: {}", e)))?;
        
        // Create minimal config for MLS service with SQL storage
        let config = Config::default();
        let shutdown = Arc::new(ShutdownCoordinator::new(std::time::Duration::from_secs(30)));
        let mls_storage_dir = profile.data_dir.join("mls");
        let mls_service = Arc::new(
            MlsService::with_storage(&config, shutdown, mls_storage_dir)
                .map_err(|e| ApiError::Internal(anyhow::anyhow!("Failed to create MLS service: {}", e)))?
        );

        // ========================================
        // P2P Network Layer Setup (Using Shared Router & Channel Members)
        // ========================================
        
        // Generate a unique peer ID for this session
        let mut counter = self.peer_id_counter.lock().await;
        *counter += 1;
        let peer_num = *counter;
        drop(counter);
        
        // Use session-specific peer ID
        let mut peer_id_bytes = vec![0u8; 32];
        peer_id_bytes[..8].copy_from_slice(&peer_num.to_le_bytes());
        let peer_id = PeerId::from_bytes(peer_id_bytes);
        
        eprintln!("[P2P] Session created for user {} with peer_id: {:?}", profile.id, peer_id);
        
        // Use the SHARED router
        let router_handle = self.shared_router.as_ref().clone();
        
        // Create network layer with SHARED channel members map
        let (network_layer, mut incoming_rx, mut commits_rx) = NetworkLayer::with_shared_members(
            router_handle,
            peer_id.clone(),
            self.shared_channel_members.clone(),
        );
        let network_layer = Arc::new(network_layer);
        
        eprintln!("[P2P] NetworkLayer created with shared channel member registry");
        
        // CRITICAL: Spawn event processor to poll router events and forward to incoming_tx
        let _event_processor_task = network_layer.clone().spawn_event_processor();
        eprintln!("[P2P] Event processor spawned for peer {:?}", peer_id);
        
        // Create space manager WITH network layer (enables P2P distribution)
        let manager = AsyncSpaceManager::with_network(
            store,
            mls_service,
            network_layer.clone(),
        );
        let manager = Arc::new(manager);
        
        // Spawn background task to handle incoming MLS commits
        let manager_for_commits = manager.clone();
        let user_id_for_commits = UserId(profile.id.clone());
        let local_peer_for_commits = peer_id.clone();
        let _commits_task = tokio::spawn(async move {
            eprintln!("[P2P] Commit processor started for user: {}", user_id_for_commits.0);
            while let Some(incoming_commit) = commits_rx.recv().await {
                // Skip commits from ourselves
                if incoming_commit.sender_peer_id == local_peer_for_commits {
                    eprintln!("[P2P] Skipping own commit (sender == self)");
                    continue;
                }
                
                eprintln!("[P2P] Received commit from peer {:?} for channel {}", 
                    incoming_commit.sender_peer_id, incoming_commit.channel_id.0);
                
                // Decode hex channel ID to bytes
                let channel_id_bytes = match hex::decode(&incoming_commit.channel_id.0) {
                    Ok(bytes) if bytes.len() == 32 => {
                        let mut arr = [0u8; 32];
                        arr.copy_from_slice(&bytes);
                        spacepanda_core::core_space::ChannelId::from_bytes(arr)
                    }
                    Ok(_) => {
                        eprintln!("[P2P] Invalid commit channel ID length");
                        continue;
                    }
                    Err(e) => {
                        eprintln!("[P2P] Failed to decode commit channel ID: {}", e);
                        continue;
                    }
                };
                
                // Process the commit
                if let Err(e) = manager_for_commits.handle_incoming_commit(
                    &channel_id_bytes,
                    &incoming_commit.commit_data
                ).await {
                    eprintln!("[P2P] Error processing commit: {}", e);
                }
            }
            eprintln!("[P2P] Commit processor ended for user: {}", user_id_for_commits.0);
        });
        
        // Spawn background task to handle incoming P2P messages
        let manager_clone = manager.clone();
        let user_id_for_task = UserId(profile.id.clone());
        let local_peer_id = peer_id.clone();
        let network_task = tokio::spawn(async move {
            eprintln!("[P2P] Background task started for user: {}", user_id_for_task.0);
            while let Some(incoming) = incoming_rx.recv().await {
                // Skip messages from ourselves (broadcast loopback)
                if incoming.sender_peer_id == local_peer_id {
                    eprintln!("[P2P] Skipping own message (sender == self)");
                    continue;
                }
                
                eprintln!("[P2P] Received message from peer {:?} for channel {}", 
                    incoming.sender_peer_id, incoming.channel_id.0);
                
                // Decode hex channel ID to bytes
                eprintln!("[P2P] Decoding channel ID: {}", incoming.channel_id.0);
                let channel_id_bytes = match hex::decode(&incoming.channel_id.0) {
                    Ok(bytes) if bytes.len() == 32 => {
                        let mut arr = [0u8; 32];
                        arr.copy_from_slice(&bytes);
                        let decoded_id = spacepanda_core::core_space::ChannelId::from_bytes(arr);
                        eprintln!("[P2P] Decoded channel ID to: {}", hex::encode(decoded_id.as_bytes()));
                        decoded_id
                    }
                    Ok(bytes) => {
                        eprintln!("[P2P] Invalid channel ID length: {} bytes (expected 32)", bytes.len());
                        continue;
                    }
                    Err(e) => {
                        eprintln!("[P2P] Failed to decode hex channel ID: {}", e);
                        continue;
                    }
                };
                
                // Extract sender user ID from peer ID
                // Note: In production, maintain a PeerId -> UserId mapping
                // For now, we use a simple conversion since both are generated from UUIDs
                let sender_id = UserId(format!("peer:{}", hex::encode(&incoming.sender_peer_id.as_bytes()[..8])));
                
                eprintln!("[P2P] Processing message for user {} from sender {}", 
                    user_id_for_task.0, sender_id.0);
                
                // Handle incoming message from P2P network
                match manager_clone.handle_incoming_message(
                    &channel_id_bytes,
                    &sender_id,
                    &incoming.ciphertext
                ).await {
                    Ok(_) => {
                        eprintln!("[P2P] ✓ Successfully processed incoming message");
                    }
                    Err(e) => {
                        let error_str = e.to_string();
                        // Categorize errors for better debugging
                        if error_str.contains("CannotDecryptOwnMessage") {
                            eprintln!("[P2P] Note: MLS detected own message (duplicate detection)");
                        } else if error_str.contains("WrongEpoch") {
                            eprintln!("[P2P] Warning: Message from wrong epoch (out of sync): {}", error_str);
                        } else if error_str.contains("TooDistantInThePast") {
                            eprintln!("[P2P] Warning: Message too old (out of order delivery): {}", error_str);
                        } else if error_str.contains("UnableToDecrypt") {
                            eprintln!("[P2P] Warning: Unable to decrypt message: {}", error_str);
                        } else {
                            eprintln!("[P2P] Error handling incoming P2P message: {}", e);
                        }
                    }
                }
            }
            eprintln!("[P2P] Background task ended for user: {}", user_id_for_task.0);
        });

        let user_id = UserId(profile.id.clone());

        let session = Session {
            token: token.clone(),
            user_id,
            username: profile.username.clone(),
            manager,
            network_task: Arc::new(Some(network_task)),
            peer_id: peer_id.clone(),
        };

        self.sessions
            .write()
            .await
            .insert(token.clone(), session);

        Ok(token)
    }

    pub async fn get_session(&self, token: &str) -> ApiResult<Session> {
        self.sessions
            .read()
            .await
            .get(token)
            .cloned()
            .ok_or(ApiError::InvalidSession)
    }

    pub async fn remove_session(&self, token: &str) -> ApiResult<()> {
        self.sessions.write().await.remove(token);
        Ok(())
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}
