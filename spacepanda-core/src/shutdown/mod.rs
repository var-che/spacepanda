//! Graceful shutdown coordinator

use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use std::time::Duration;
use tracing::{info, warn, error};

/// Shutdown signal
#[derive(Debug, Clone, Copy)]
pub enum ShutdownSignal {
    Graceful,
    Immediate,
}

/// Shutdown state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShutdownState {
    Running,
    ShuttingDown,
    Shutdown,
}

/// Graceful shutdown coordinator
pub struct ShutdownCoordinator {
    state: Arc<RwLock<ShutdownState>>,
    shutdown_tx: broadcast::Sender<ShutdownSignal>,
    timeout: Duration,
}

impl ShutdownCoordinator {
    /// Create a new shutdown coordinator
    pub fn new(timeout: Duration) -> Self {
        let (shutdown_tx, _) = broadcast::channel(16);
        
        Self {
            state: Arc::new(RwLock::new(ShutdownState::Running)),
            shutdown_tx,
            timeout,
        }
    }
    
    /// Subscribe to shutdown notifications
    pub fn subscribe(&self) -> broadcast::Receiver<ShutdownSignal> {
        self.shutdown_tx.subscribe()
    }
    
    /// Initiate graceful shutdown
    pub async fn shutdown(&self) {
        info!("Initiating graceful shutdown");
        
        let mut state = self.state.write().await;
        if *state != ShutdownState::Running {
            warn!("Shutdown already in progress");
            return;
        }
        
        *state = ShutdownState::ShuttingDown;
        drop(state);
        
        // Broadcast shutdown signal
        if let Err(e) = self.shutdown_tx.send(ShutdownSignal::Graceful) {
            error!("Failed to send shutdown signal: {}", e);
        }
        
        // Wait for timeout
        tokio::time::sleep(self.timeout).await;
        
        let mut state = self.state.write().await;
        *state = ShutdownState::Shutdown;
        info!("Shutdown complete");
    }
    
    /// Initiate immediate shutdown
    pub async fn shutdown_immediately(&self) {
        warn!("Initiating immediate shutdown");
        
        let mut state = self.state.write().await;
        *state = ShutdownState::Shutdown;
        drop(state);
        
        if let Err(e) = self.shutdown_tx.send(ShutdownSignal::Immediate) {
            error!("Failed to send immediate shutdown signal: {}", e);
        }
    }
    
    /// Check if shutdown is in progress
    pub async fn is_shutting_down(&self) -> bool {
        let state = self.state.read().await;
        *state == ShutdownState::ShuttingDown || *state == ShutdownState::Shutdown
    }
    
    /// Get current state
    pub async fn state(&self) -> ShutdownState {
        *self.state.read().await
    }
    
    /// Wait for shutdown signal
    pub async fn wait_for_shutdown(&self) {
        let mut rx = self.subscribe();
        let _ = rx.recv().await;
    }
}

/// Shutdown handler for managing component lifecycle
pub struct ShutdownHandler {
    coordinator: Arc<ShutdownCoordinator>,
    component_name: String,
}

impl ShutdownHandler {
    /// Create a new shutdown handler
    pub fn new(coordinator: Arc<ShutdownCoordinator>, component_name: impl Into<String>) -> Self {
        Self {
            coordinator,
            component_name: component_name.into(),
        }
    }
    
    /// Run a component with graceful shutdown support
    pub async fn run<F, Fut>(&self, f: F)
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = ()>,
    {
        info!("Starting component: {}", self.component_name);
        
        let mut shutdown_rx = self.coordinator.subscribe();
        
        tokio::select! {
            _ = f() => {
                info!("Component {} completed normally", self.component_name);
            }
            signal = shutdown_rx.recv() => {
                match signal {
                    Ok(ShutdownSignal::Graceful) => {
                        info!("Component {} received graceful shutdown signal", self.component_name);
                    }
                    Ok(ShutdownSignal::Immediate) => {
                        warn!("Component {} received immediate shutdown signal", self.component_name);
                    }
                    Err(e) => {
                        error!("Component {} shutdown channel error: {}", self.component_name, e);
                    }
                }
            }
        }
        
        info!("Component {} shutdown complete", self.component_name);
    }
}

/// Install signal handlers for graceful shutdown
#[cfg(unix)]
pub fn install_signal_handlers(coordinator: Arc<ShutdownCoordinator>) {
    use tokio::signal::unix::{signal, SignalKind};
    
    tokio::spawn(async move {
        let mut sigterm = signal(SignalKind::terminate()).expect("Failed to install SIGTERM handler");
        let mut sigint = signal(SignalKind::interrupt()).expect("Failed to install SIGINT handler");
        
        tokio::select! {
            _ = sigterm.recv() => {
                info!("Received SIGTERM");
                coordinator.shutdown().await;
            }
            _ = sigint.recv() => {
                info!("Received SIGINT");
                coordinator.shutdown().await;
            }
        }
    });
}

/// Install signal handlers for graceful shutdown (Windows)
#[cfg(windows)]
pub fn install_signal_handlers(coordinator: Arc<ShutdownCoordinator>) {
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.expect("Failed to install Ctrl+C handler");
        info!("Received Ctrl+C");
        coordinator.shutdown().await;
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_shutdown_coordinator() {
        let coordinator = ShutdownCoordinator::new(Duration::from_millis(100));
        
        assert_eq!(coordinator.state().await, ShutdownState::Running);
        
        coordinator.shutdown().await;
        
        assert_eq!(coordinator.state().await, ShutdownState::Shutdown);
    }
    
    #[tokio::test]
    async fn test_shutdown_handler() {
        let coordinator = Arc::new(ShutdownCoordinator::new(Duration::from_millis(100)));
        let handler = ShutdownHandler::new(coordinator.clone(), "test_component");
        
        let task = tokio::spawn(async move {
            handler.run(|| async {
                tokio::time::sleep(Duration::from_secs(10)).await;
            }).await;
        });
        
        tokio::time::sleep(Duration::from_millis(50)).await;
        coordinator.shutdown().await;
        
        task.await.unwrap();
    }
}
