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
        storage::file_store::FileStorageProvider,
        traits::storage::StorageProvider,
        types::{GroupId, GroupMetadata, MlsConfig},
    },
    health::{ComponentHealth, HealthStatus},
    metrics::{record_counter, record_histogram, Timer},
    shutdown::ShutdownCoordinator,
    tracing::mls::{trace_decrypt, trace_encrypt},
};

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

// OpenMLS imports for key package generation
use openmls::prelude::*;
use openmls::prelude::tls_codec::Serialize;
use openmls_basic_credential::SignatureKeyPair;
use openmls_rust_crypto::OpenMlsRustCrypto;
use openmls::key_packages::KeyPackage as MlsKeyPackage;

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

    /// Shared crypto provider for all MLS operations
    /// This is necessary for key package bundles to be found when joining from Welcome
    provider: Arc<OpenMlsRustCrypto>,

    /// Storage for KeyPackageBundles indexed by serialized KeyPackage bytes
    /// This allows us to retrieve the correct signature keys when joining from Welcome
    key_package_bundles: Arc<RwLock<HashMap<Vec<u8>, KeyPackageBundle>>>,

    /// Optional storage provider for persisting group state
    storage: Option<Arc<dyn StorageProvider>>,
}

impl MlsService {
    /// Create a new MLS service
    pub fn new(config: &Config, shutdown: Arc<ShutdownCoordinator>) -> Self {
        info!("Initializing MLS service");

        let mls_config = MlsConfig::default();
        let provider = Arc::new(OpenMlsRustCrypto::default());

        Self {
            groups: Arc::new(RwLock::new(HashMap::new())),
            config: mls_config,
            events: EventBroadcaster::default(),
            shutdown,
            provider,
            key_package_bundles: Arc::new(RwLock::new(HashMap::new())),
            storage: None,
        }
    }

    /// Create MLS service with file-based storage for persistence
    pub fn with_storage(
        config: &Config,
        shutdown: Arc<ShutdownCoordinator>,
        storage_dir: PathBuf,
    ) -> MlsResult<Self> {
        info!("Initializing MLS service with storage at: {:?}", storage_dir);

        let mls_config = MlsConfig::default();
        let provider = Arc::new(OpenMlsRustCrypto::default());
        
        // Create storage provider
        let storage = Arc::new(FileStorageProvider::new(storage_dir, None)?);

        Ok(Self {
            groups: Arc::new(RwLock::new(HashMap::new())),
            config: mls_config,
            events: EventBroadcaster::default(),
            shutdown,
            provider,
            key_package_bundles: Arc::new(RwLock::new(HashMap::new())),
            storage: Some(storage),
        })
    }

    /// Load all persisted groups from storage
    ///
    /// Should be called on service initialization to restore previous session state.
    pub async fn load_persisted_groups(&self) -> MlsResult<usize> {
        let Some(storage) = &self.storage else {
            warn!("No storage configured, cannot load persisted groups");
            return Ok(0);
        };

        info!("Loading persisted groups from storage");
        
        // In a real implementation, we'd list all snapshot files in the directory
        // For now, we'll return 0 as we don't have a list operation
        // This is a limitation to fix in production
        
        // TODO: Add list_snapshots() to StorageProvider trait
        // For now, groups must be explicitly loaded by GroupId
        
        Ok(0)
    }

    /// Save a group's state to storage
    pub async fn save_group(&self, group_id: &GroupId) -> MlsResult<()> {
        let Some(storage) = &self.storage else {
            // No storage configured, skip silently
            return Ok(());
        };

        debug!("Saving group {} to storage", group_id);

        let groups = self.groups.read().await;
        let adapter = groups
            .get(group_id)
            .ok_or_else(|| MlsError::GroupNotFound(group_id.to_string()))?;

        // Export snapshot
        let snapshot = adapter.export_snapshot().await?;
        
        // Convert to persisted format
        let snapshot_bytes = snapshot.to_bytes()?;
        let persisted_snapshot = crate::core_mls::traits::storage::PersistedGroupSnapshot {
            group_id: group_id.as_bytes().to_vec(),
            epoch: snapshot.epoch,
            serialized_group: snapshot_bytes,
        };

        // Save to storage
        storage.save_group_snapshot(persisted_snapshot).await?;

        info!("Successfully saved group {} at epoch {}", group_id, snapshot.epoch);
        Ok(())
    }

    /// Load a group from storage and restore it to active groups
    pub async fn load_group(&self, group_id: &GroupId) -> MlsResult<()> {
        let Some(storage) = &self.storage else {
            return Err(MlsError::Storage("No storage configured".to_string()));
        };

        info!("Loading group {} from storage", group_id);

        // Load snapshot from storage
        let persisted_snapshot = storage
            .load_group_snapshot(&group_id.as_bytes().to_vec())
            .await?;

        // TODO: Restore group from snapshot
        // This requires implementing restoration in OpenMlsHandleAdapter
        // For MVP, we'll log that this needs implementation
        
        warn!("Group restoration from snapshot not yet implemented");
        warn!("Loaded snapshot: epoch={}, size={} bytes", 
            persisted_snapshot.epoch, 
            persisted_snapshot.serialized_group.len()
        );

        // For now, return error indicating this is not yet supported
        Err(MlsError::Internal(
            "Group restoration from snapshot not yet implemented".to_string()
        ))
    }

    /// Generate a key package for joining groups
    ///
    /// Creates a KeyPackageBundle with cryptographic material and stores it
    /// in a provider. Returns the serialized public key package that can be
    /// shared with group administrators.
    ///
    /// # Arguments
    ///
    /// * `identity` - User's identity bytes
    ///
    /// # Returns
    ///
    /// Serialized public key package bytes (Vec<u8>) that can be shared.
    /// The private KeyPackageBundle is stored and will be used when joining.
    ///
    /// # Implementation Details
    ///
    /// This method:
    /// 1. Creates a new crypto provider
    /// 2. Generates signature keys
    /// 3. Stores keys in provider
    /// 4. Creates credential
    /// 5. Builds KeyPackageBundle (auto-stored in provider)
    /// 6. Returns serialized public key package
    pub async fn generate_key_package(&self, identity: Vec<u8>) -> MlsResult<Vec<u8>> {
        let timer = Timer::new("mls.generate_key_package.duration_ms");
        
        info!("Generating key package for identity: {:?}", hex::encode(&identity));

        // Check if shutting down
        if self.shutdown.is_shutting_down().await {
            warn!("Cannot generate key package: service is shutting down");
            return Err(MlsError::ServiceUnavailable(
                "MLS service is shutting down".to_string(),
            ));
        }

        // Use the same ciphersuite as our groups
        let ciphersuite = Ciphersuite::MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519;

        // Use the shared provider (critical for join_from_welcome to find the bundle)
        let provider = self.provider.clone();

        // Generate signature keys
        let signature_keys = SignatureKeyPair::new(ciphersuite.signature_algorithm())
            .map_err(|e| MlsError::InvalidMessage(format!("Failed to generate signature keys: {:?}", e)))?;

        // Store keys in provider (OpenMLS stores them indexed by public key hash)
        signature_keys
            .store(provider.storage())
            .map_err(|e| MlsError::InvalidMessage(format!("Failed to store signature keys: {:?}", e)))?;

        // Create credential with the user's identity
        let basic_credential = BasicCredential::new(identity.clone());
        let credential_with_key = CredentialWithKey {
            credential: basic_credential.into(),
            signature_key: signature_keys.public().into(),
        };

        // Build the key package bundle
        // NOTE: The KeyPackageBundle is automatically stored in the provider's storage
        // when built. This allows join_from_welcome to find it later.
        let key_package_bundle = KeyPackage::builder()
            .build(ciphersuite, provider.as_ref(), &signature_keys, credential_with_key)
            .map_err(|e| MlsError::InvalidMessage(format!("Failed to build key package: {:?}", e)))?;

        // Serialize just the public key package
        let key_package_bytes = key_package_bundle
            .key_package()
            .tls_serialize_detached()
            .map_err(|e| MlsError::InvalidMessage(format!("Failed to serialize key package: {:?}", e)))?;

        // Store the KeyPackageBundle indexed by the serialized key package bytes
        // This allows us to retrieve it later when joining from Welcome
        {
            let mut bundles = self.key_package_bundles.write().await;
            bundles.insert(key_package_bytes.clone(), key_package_bundle);
        }

        // Record metrics
        record_counter("mls.key_packages.generated", 1);
        timer.stop();

        info!(
            "Successfully generated key package ({} bytes) for identity: {:?}",
            key_package_bytes.len(),
            hex::encode(&identity)
        );

        Ok(key_package_bytes)
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

        // Create the group with shared provider
        let adapter = OpenMlsHandleAdapter::create_group(
            group_id.clone(),
            identity.clone(),
            self.config.clone(),
            self.provider.clone(),
        )
        .await?;

        let gid = adapter.group_id().await;

        // Store the group
        {
            let mut groups = self.groups.write().await;
            groups.insert(gid.clone(), Arc::new(adapter));
        }

        // Save to storage if configured
        if self.storage.is_some() {
            if let Err(e) = self.save_group(&gid).await {
                warn!("Failed to save newly created group {}: {}", gid, e);
            }
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

        // Parse Welcome to find which KeyPackage was used
        // Then retrieve the corresponding KeyPackageBundle from our storage
        let key_package_bundle = self.find_key_package_bundle_for_welcome(welcome_bytes).await?;

        // Join the group with shared provider and the correct KeyPackageBundle
        let adapter = OpenMlsHandleAdapter::join_from_welcome(
            welcome_bytes,
            ratchet_tree,
            self.config.clone(),
            Some(key_package_bundle), // Pass the KeyPackageBundle so correct signature keys are used
            self.provider.clone(),
        )
        .await?;

        let gid = adapter.group_id().await;

        // Store the group
        {
            let mut groups = self.groups.write().await;
            groups.insert(gid.clone(), Arc::new(adapter));
        }

        // Save to storage if configured
        if self.storage.is_some() {
            if let Err(e) = self.save_group(&gid).await {
                warn!("Failed to save joined group {}: {}", gid, e);
            }
        }

        // Event will be emitted by the engine when processing Welcome

        // Record metrics
        record_counter("mls.groups.joined", 1);
        timer.stop();

        info!("Successfully joined MLS group: {}", gid);
        Ok(gid)
    }

    /// Find the KeyPackageBundle that matches the Welcome message
    /// 
    /// This is a simplified implementation that tries all stored bundles.
    /// In production, we would parse the Welcome to get the specific KeyPackage hash.
    async fn find_key_package_bundle_for_welcome(
        &self,
        _welcome_bytes: &[u8],
    ) -> MlsResult<KeyPackageBundle> {
        let bundles = self.key_package_bundles.read().await;
        
        // For MVP: just return the first (and likely only) bundle
        // TODO: Parse Welcome message to find the correct KeyPackage hash
        if let Some((_, bundle)) = bundles.iter().next() {
            Ok(bundle.clone())
        } else {
            Err(MlsError::InvalidMessage(
                "No KeyPackageBundle found for this Welcome message".to_string(),
            ))
        }
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
    ) -> MlsResult<(Vec<u8>, Vec<u8>, Vec<u8>)> {
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

        // Export ratchet tree for the Welcome recipient
        // This is required when the Welcome doesn't include the tree inline
        let ratchet_tree = if !welcome.is_empty() {
            // Release the engine lock before calling export_ratchet_tree
            drop(engine);
            
            // Get fresh reference and export tree
            let engine_ref = adapter.engine();
            let engine = engine_ref.read().await;
            engine.export_ratchet_tree_bytes().await.unwrap_or_default()
        } else {
            Vec::new()
        };

        // Record metrics
        record_counter("mls.members.added", 1);
        timer.stop();

        info!("Successfully added members to group {}", group_id);

        Ok((commit, welcome, ratchet_tree))
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

    /// Export ratchet tree for a group
    ///
    /// This exports the current ratchet tree state, which is needed
    /// when the Welcome message doesn't include it inline.
    pub async fn export_ratchet_tree(&self, group_id: &GroupId) -> MlsResult<Vec<u8>> {
        info!("Exporting ratchet tree for group {}", group_id);

        let groups = self.groups.read().await;
        let adapter = groups
            .get(group_id)
            .ok_or_else(|| MlsError::GroupNotFound(group_id.to_string()))?;

        let engine_ref = adapter.engine();
        let engine = engine_ref.read().await;
        engine.export_ratchet_tree_bytes().await
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
