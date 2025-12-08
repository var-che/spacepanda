//! HTTP server implementation for the test harness

use super::api::build_router;
use super::state::AppState;
use crate::core_mvp::channel_manager::ChannelManager;
use anyhow::Result;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::info;

/// HTTP test harness server
pub struct TestHarnessServer {
    state: Arc<AppState>,
    addr: String,
}

impl TestHarnessServer {
    /// Create a new test harness server
    pub fn new(channel_manager: Arc<ChannelManager>, addr: impl Into<String>) -> Self {
        let state = Arc::new(AppState::new(channel_manager));
        Self { state, addr: addr.into() }
    }

    /// Start the server and run until shutdown
    pub async fn run(self) -> Result<()> {
        let router = build_router(self.state);

        let listener = TcpListener::bind(&self.addr).await?;
        info!("HTTP Test Harness listening on {}", self.addr);

        axum::serve(listener, router).await?;

        Ok(())
    }
}

/// Convenience function to start a test harness server
pub async fn start_server(addr: impl Into<String>) -> Result<()> {
    // Create dependencies for ChannelManager
    use crate::config::Config;
    use crate::core_mls::service::MlsService;
    use crate::core_store::model::types::UserId;
    use crate::core_store::store::{LocalStore, LocalStoreConfig};
    use crate::shutdown::ShutdownCoordinator;

    let config = Arc::new(Config::default());
    let shutdown = Arc::new(ShutdownCoordinator::new(std::time::Duration::from_secs(30)));

    let mls_service = Arc::new(MlsService::new(&config, shutdown.clone()));

    // Create a temporary store for testing
    let temp_dir = std::env::temp_dir().join(format!("spacepanda_test_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir)?;
    let store_config = LocalStoreConfig {
        data_dir: temp_dir,
        enable_encryption: false, // Disable for testing
        snapshot_interval: 1000,
        max_log_size: 10_000_000,
        enable_compaction: false,
        require_signatures: false,
        authorized_keys: Vec::new(),
    };
    let store = Arc::new(LocalStore::new(store_config)?);

    // Create a test identity
    let identity = Arc::new(crate::core_mvp::channel_manager::Identity::new(
        UserId(uuid::Uuid::new_v4().to_string()),
        "Test User".to_string(),
        uuid::Uuid::new_v4().to_string(),
    ));

    let channel_manager =
        Arc::new(ChannelManager::new(mls_service, store, identity, config.clone()));

    let server = TestHarnessServer::new(channel_manager, addr);
    server.run().await
}
