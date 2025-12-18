use anyhow::Result;
use tonic::transport::Server;
use tracing::{info, Level};
use tracing_subscriber;

mod auth;
mod error;
mod proto;
mod services;
mod session;

use services::{AuthServiceImpl, MessageServiceImpl, NetworkServiceImpl, SpaceServiceImpl};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(Level::DEBUG)
        .init();

    // Get port from environment or command line argument
    let port = std::env::args()
        .nth(1)
        .and_then(|arg| if arg == "--port" { std::env::args().nth(2) } else { Some(arg) })
        .or_else(|| std::env::var("GRPC_PORT").ok())
        .unwrap_or_else(|| "50051".to_string());

    let addr = format!("127.0.0.1:{}", port).parse()?;
    
    info!("🐼 SpacePanda gRPC API Server starting on {}", addr);

    // Initialize SHARED session manager for all services
    let session_manager = std::sync::Arc::new(session::SessionManager::new());

    // Initialize services with shared session manager
    let auth_service = AuthServiceImpl::new(session_manager.clone());
    let space_service = SpaceServiceImpl::new(session_manager.clone());
    let message_service = MessageServiceImpl::new(session_manager.clone());
    let network_service = NetworkServiceImpl::new(session_manager.clone());

    // Build and start server
    Server::builder()
        .add_service(proto::auth_service_server::AuthServiceServer::new(
            auth_service,
        ))
        .add_service(proto::space_service_server::SpaceServiceServer::new(
            space_service,
        ))
        .add_service(proto::message_service_server::MessageServiceServer::new(
            message_service,
        ))
        .add_service(proto::network_service_server::NetworkServiceServer::new(
            network_service,
        ))
        .serve(addr)
        .await?;

    Ok(())
}
