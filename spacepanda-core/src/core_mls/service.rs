//! MLS Service with Production Integration
//!
//! This service provides a high-level API for MLS operations with:
//! - Metrics collection
//! - Distributed tracing
//! - Health checks
//! - Graceful shutdown
//! - Configuration management

use crate::{
    config::Config,
    core_mls::{
        engine::{adapter::OpenMlsHandleAdapter, GroupOperations},
        errors::{MlsError, MlsResult},
        events::{EventBroadcaster, MlsEvent},
        types::{GroupId, GroupMetadata, MlsConfig},
    },
    health::{ComponentHealth, HealthStatus},
    metrics::{record_counter, record_histogram, Timer},
    shutdown::ShutdownCoordinator,
    tracing::mls::{trace_decrypt, trace_encrypt},
};

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// MLS Service for managing multiple groups
pub struct MlsService {
    /// Active MLS groups indexed by GroupId
    groups: Arc<RwLock<HashMap<GroupId, Arc<OpenMlsHandleAdapter>>>>,

    /// Service configuration
    config: MlsConfig,

    /// Event broadcaster
    events: EventBroadcaster,

    /// Shutdown coordinator
    shutdown: Arc<ShutdownCoordinator>,
}

impl MlsService {
    /// Create a new MLS service
    pub fn new(config: &Config, shutdown: Arc<ShutdownCoordinator>) -> Self {
        info!("Initializing MLS service");

        let mls_config = MlsConfig::default();

        Self {
            groups: Arc::new(RwLock::new(HashMap::new())),
            config: mls_config,
            events: EventBroadcaster::default(),
            shutdown,
        }
    }

    /// Create a new MLS group
    pub async fn create_group(
        &self,
        identity: Vec<u8>,
        group_id: Option<GroupId>,
    ) -> MlsResult<GroupId> {
        let timer = Timer::new("mls.create_group.duration_ms");

        info!("Creating new MLS group for identity: {:?}", hex::encode(&identity));

        // Check if shutting down
        if self.shutdown.is_shutting_down().await {
            warn!("Cannot create group: service is shutting down");
            return Err(MlsError::ServiceUnavailable(
                "MLS service is shutting down".to_string(),
            ));
        }

        // Create the group
        let adapter = OpenMlsHandleAdapter::create_group(
            group_id.clone(),
            identity.clone(),
            self.config.clone(),
        )
        .await?;

        let gid = adapter.group_id().await;

        // Store the group
        {
            let mut groups = self.groups.write().await;
            groups.insert(gid.clone(), Arc::new(adapter));
        }

        // Emit event
        self.events.emit(MlsEvent::GroupCreated {
            group_id: gid.as_bytes().to_vec(),
            creator_id: identity,
        });

        // Record metrics
        record_counter("mls.groups.created", 1);
        timer.stop();

        info!("Successfully created MLS group: {}", gid);
        Ok(gid)
    }

    /// Join an existing group from a Welcome message
    pub async fn join_group(
        &self,
        welcome_bytes: &[u8],
        ratchet_tree: Option<Vec<u8>>,
    ) -> MlsResult<GroupId> {
        let timer = Timer::new("mls.join_group.duration_ms");

        info!("Joining MLS group from Welcome message");

        // Check if shutting down
        if self.shutdown.is_shutting_down().await {
            warn!("Cannot join group: service is shutting down");
            return Err(MlsError::ServiceUnavailable(
                "MLS service is shutting down".to_string(),
            ));
        }

        // Join the group
        let adapter = OpenMlsHandleAdapter::join_from_welcome(
            welcome_bytes,
            ratchet_tree,
            self.config.clone(),
            None, // No KeyPackageBundle - generates new keys
        )
        .await?;

        let gid = adapter.group_id().await;

        // Store the group
        {
            let mut groups = self.groups.write().await;
            groups.insert(gid.clone(), Arc::new(adapter));
        }

        // Event will be emitted by the engine when processing Welcome

        // Record metrics
        record_counter("mls.groups.joined", 1);
        timer.stop();

        info!("Successfully joined MLS group: {}", gid);
        Ok(gid)
    }

    /// Send an encrypted message to a group
    pub async fn send_message(&self, group_id: &GroupId, plaintext: &[u8]) -> MlsResult<Vec<u8>> {
        let trace = trace_encrypt(plaintext.len());

        debug!("Sending message to group {}: {} bytes", group_id, plaintext.len());

        // Get the group
        let groups = self.groups.read().await;
        let adapter = groups
            .get(group_id)
            .ok_or_else(|| MlsError::GroupNotFound(group_id.to_string()))?;

        // Encrypt and send
        let engine_ref = adapter.engine();
        let engine = engine_ref.read().await;
        let ciphertext = engine.send_message(plaintext).await?;

        // Record metrics
        record_counter("mls.messages.encrypted", 1);
        record_histogram("mls.message.size_bytes", plaintext.len() as f64);

        trace.record_event("message encrypted");
        trace.complete();

        debug!("Message encrypted: {} bytes plaintext -> {} bytes ciphertext", 
               plaintext.len(), ciphertext.len());

        Ok(ciphertext)
    }

    /// Process an incoming MLS message
    pub async fn process_message(
        &self,
        group_id: &GroupId,
        message_bytes: &[u8],
    ) -> MlsResult<Option<Vec<u8>>> {
        let trace = trace_decrypt(message_bytes.len());

        debug!("Processing message for group {}: {} bytes", group_id, message_bytes.len());

        // Get the group
        let groups = self.groups.read().await;
        let adapter = groups
            .get(group_id)
            .ok_or_else(|| MlsError::GroupNotFound(group_id.to_string()))?;

        // Process the message
        let engine_ref = adapter.engine();
        let engine = engine_ref.read().await;
        let processed = engine.process_message(message_bytes).await?;

        // Handle different message types
        let plaintext = match processed {
            crate::core_mls::engine::openmls_engine::ProcessedMessage::Application(data) => {
                record_counter("mls.messages.decrypted", 1);
                trace.record_event("application message decrypted");
                Some(data)
            }
            crate::core_mls::engine::openmls_engine::ProcessedMessage::Proposal => {
                record_counter("mls.proposals.received", 1);
                trace.record_event("proposal processed");
                None
            }
            crate::core_mls::engine::openmls_engine::ProcessedMessage::Commit { new_epoch } => {
                record_counter("mls.commits.received", 1);
                trace.record_event(&format!("commit processed, new epoch: {}", new_epoch));
                None
            }
        };

        trace.complete();

        Ok(plaintext)
    }

    /// Add members to a group
    pub async fn add_members(
        &self,
        group_id: &GroupId,
        key_packages: Vec<Vec<u8>>,
    ) -> MlsResult<(Vec<u8>, Vec<u8>)> {
        let timer = Timer::new("mls.add_members.duration_ms");

        info!("Adding {} members to group {}", key_packages.len(), group_id);

        // Get the group
        let groups = self.groups.read().await;
        let adapter = groups
            .get(group_id)
            .ok_or_else(|| MlsError::GroupNotFound(group_id.to_string()))?;

        // Add members (using the group_ops trait method)
        let engine_ref = adapter.engine();
        let engine = engine_ref.read().await;
        let (commit, welcome_opt) = engine.add_members(key_packages).await?;

        // Convert Option<Vec<u8>> to Vec<u8> (empty vec if None)
        let welcome = welcome_opt.unwrap_or_default();

        // Record metrics
        record_counter("mls.members.added", 1);
        timer.stop();

        info!("Successfully added members to group {}", group_id);

        Ok((commit, welcome))
    }

    /// Remove members from a group
    pub async fn remove_members(
        &self,
        group_id: &GroupId,
        leaf_indices: Vec<u32>,
    ) -> MlsResult<Vec<u8>> {
        let timer = Timer::new("mls.remove_members.duration_ms");

        info!("Removing {} members from group {}", leaf_indices.len(), group_id);

        // Get the group
        let groups = self.groups.read().await;
        let adapter = groups
            .get(group_id)
            .ok_or_else(|| MlsError::GroupNotFound(group_id.to_string()))?;

        // Remove members (using the group_ops trait method)
        let engine_ref = adapter.engine();
        let engine = engine_ref.read().await;
        let commit = engine.remove_members(leaf_indices).await?;

        // Record metrics
        record_counter("mls.members.removed", 1);
        timer.stop();

        info!("Successfully removed members from group {}", group_id);

        Ok(commit)
    }

    /// Get group metadata
    pub async fn get_metadata(&self, group_id: &GroupId) -> MlsResult<GroupMetadata> {
        let groups = self.groups.read().await;
        let adapter = groups
            .get(group_id)
            .ok_or_else(|| MlsError::GroupNotFound(group_id.to_string()))?;

        adapter.metadata().await
    }

    /// List all active groups
    pub async fn list_groups(&self) -> Vec<GroupId> {
        let groups = self.groups.read().await;
        groups.keys().cloned().collect()
    }

    /// Get the number of active groups
    pub async fn group_count(&self) -> usize {
        let groups = self.groups.read().await;
        groups.len()
    }

    /// Subscribe to MLS events
    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<MlsEvent> {
        self.events.subscribe()
    }

    /// Get health status
    pub async fn health_check(&self) -> ComponentHealth {
        let group_count = self.group_count().await;

        if self.shutdown.is_shutting_down().await {
            ComponentHealth::unhealthy("mls", "Service is shutting down")
        } else if group_count == 0 {
            ComponentHealth::degraded("mls", "No active groups")
        } else {
            ComponentHealth::healthy("mls")
        }
    }

    /// Graceful shutdown - cleanup all groups
    pub async fn shutdown(&self) -> MlsResult<()> {
        info!("Shutting down MLS service");

        let mut groups = self.groups.write().await;
        let group_count = groups.len();

        info!("Cleaning up {} active groups", group_count);

        // Export snapshots for all groups before shutdown
        for (group_id, adapter) in groups.iter() {
            match adapter.export_snapshot().await {
                Ok(_snapshot) => {
                    debug!("Exported snapshot for group {}", group_id);
                }
                Err(e) => {
                    error!("Failed to export snapshot for group {}: {}", group_id, e);
                }
            }
        }

        groups.clear();

        info!("MLS service shutdown complete");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_mls_service_creation() {
        let config = Config::default();
        let shutdown = Arc::new(ShutdownCoordinator::new(Duration::from_secs(30)));
        let service = MlsService::new(&config, shutdown);

        assert_eq!(service.group_count().await, 0);
    }

    #[tokio::test]
    async fn test_create_group() {
        let config = Config::default();
        let shutdown = Arc::new(ShutdownCoordinator::new(Duration::from_secs(30)));
        let service = MlsService::new(&config, shutdown);

        let identity = b"alice".to_vec();
        let group_id = service.create_group(identity, None).await.unwrap();

        assert_eq!(service.group_count().await, 1);
        assert!(service.list_groups().await.contains(&group_id));
    }

    #[tokio::test]
    async fn test_health_check() {
        let config = Config::default();
        let shutdown = Arc::new(ShutdownCoordinator::new(Duration::from_secs(30)));
        let service = MlsService::new(&config, shutdown);

        let health = service.health_check().await;
        assert_eq!(health.status, HealthStatus::Degraded); // No groups yet

        let identity = b"alice".to_vec();
        let _group_id = service.create_group(identity, None).await.unwrap();

        let health = service.health_check().await;
        assert_eq!(health.status, HealthStatus::Healthy);
    }
}
