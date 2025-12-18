use std::sync::Arc;
use tonic::{Request, Response, Status};

use crate::auth::AuthManager;
use crate::proto::*;
use crate::session::SessionManager;

pub struct AuthServiceImpl {
    auth_manager: Arc<AuthManager>,
    session_manager: Arc<SessionManager>,
}

impl AuthServiceImpl {
    pub fn new(session_manager: Arc<SessionManager>) -> Self {
        Self {
            auth_manager: Arc::new(AuthManager::new()),
            session_manager,
        }
    }
}

#[tonic::async_trait]
impl auth_service_server::AuthService for AuthServiceImpl {
    async fn unlock(
        &self,
        request: Request<UnlockRequest>,
    ) -> Result<Response<UnlockResponse>, Status> {
        let req = request.into_inner();

        let profile = self
            .auth_manager
            .unlock(
                if req.username.is_empty() {
                    None
                } else {
                    Some(req.username)
                },
                &req.password,
            )
            .await
            .map_err(|e| Status::from(e))?;

        let session_token = self
            .session_manager
            .create_session(&profile)
            .await
            .map_err(|e| Status::from(e))?;

        let user = User {
            id: profile.id.clone(),
            username: profile.username.clone(),
            display_name: profile.username.clone(),
            avatar_url: String::new(),
            status: UserStatus::Online as i32,
        };

        Ok(Response::new(UnlockResponse {
            session_token,
            user: Some(user),
        }))
    }

    async fn create_profile(
        &self,
        request: Request<CreateProfileRequest>,
    ) -> Result<Response<CreateProfileResponse>, Status> {
        let req = request.into_inner();

        let profile = self
            .auth_manager
            .create_profile(
                if req.username.is_empty() {
                    None
                } else {
                    Some(req.username)
                },
                &req.password,
            )
            .await
            .map_err(|e| Status::from(e))?;

        let session_token = self
            .session_manager
            .create_session(&profile)
            .await
            .map_err(|e| Status::from(e))?;

        let user = User {
            id: profile.id.clone(),
            username: profile.username.clone(),
            display_name: profile.username.clone(),
            avatar_url: String::new(),
            status: UserStatus::Online as i32,
        };

        Ok(Response::new(CreateProfileResponse {
            session_token,
            user: Some(user),
        }))
    }

    async fn lock(&self, request: Request<LockRequest>) -> Result<Response<LockResponse>, Status> {
        let req = request.into_inner();

        self.session_manager
            .remove_session(&req.session_token)
            .await
            .map_err(|e| Status::from(e))?;

        Ok(Response::new(LockResponse { success: true }))
    }
}
