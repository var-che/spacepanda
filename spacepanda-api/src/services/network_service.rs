use std::sync::Arc;
use tonic::{Request, Response, Status};

use crate::proto::*;
use crate::session::SessionManager;

pub struct NetworkServiceImpl {
    session_manager: Arc<SessionManager>,
}

impl NetworkServiceImpl {
    pub fn new(session_manager: Arc<SessionManager>) -> Self {
        Self {
            session_manager,
        }
    }
}

#[tonic::async_trait]
impl network_service_server::NetworkService for NetworkServiceImpl {
    async fn connect_peer(
        &self,
        request: Request<ConnectPeerRequest>,
    ) -> Result<Response<ConnectPeerResponse>, Status> {
        let req = request.into_inner();
        let session = self
            .session_manager
            .get_session(&req.session_token)
            .await
            .map_err(|e| Status::from(e))?;

        // Get the network layer from session
        let network = session
            .manager
            .network_layer()
            .ok_or_else(|| Status::internal("Network layer not initialized"))?;

        // Connect to the peer
        network
            .dial(&req.peer_address)
            .await
            .map_err(|e| Status::internal(format!("Failed to connect to peer: {}", e)))?;

        Ok(Response::new(ConnectPeerResponse {
            success: true,
            message: format!("Connected to peer: {}", req.peer_address),
        }))
    }

    async fn get_network_status(
        &self,
        request: Request<NetworkStatusRequest>,
    ) -> Result<Response<NetworkStatusResponse>, Status> {
        let req = request.into_inner();
        let session = self
            .session_manager
            .get_session(&req.session_token)
            .await
            .map_err(|e| Status::from(e))?;

        // Get the network layer from session
        let network = session
            .manager
            .network_layer()
            .ok_or_else(|| Status::internal("Network layer not initialized"))?;

        // For now, return basic status
        // TODO: Implement actual peer tracking in NetworkLayer
        let peer_id = format!("{:?}", network.local_peer_id());
        
        Ok(Response::new(NetworkStatusResponse {
            peer_id,
            listen_address: "0.0.0.0:0".to_string(), // TODO: Get actual listen address
            connected_peers: vec![], // TODO: Track connected peers
        }))
    }
}
