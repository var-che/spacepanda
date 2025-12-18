use thiserror::Error;
use tonic::{Code, Status};
use spacepanda_core::core_space::{ChannelError, SpaceError, MembershipError, InviteError};

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),

    #[error("Invalid session token")]
    InvalidSession,

    #[error("Space not found: {0}")]
    SpaceNotFound(String),

    #[error("Channel not found: {0}")]
    ChannelNotFound(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Space error: {0}")]
    SpaceError(#[from] SpaceError),

    #[error("Channel error: {0}")]
    ChannelError(#[from] ChannelError),

    #[error("Membership error: {0}")]
    MembershipError(#[from] MembershipError),

    #[error("Invite error: {0}")]
    InviteError(#[from] InviteError),

    #[error("Internal error: {0}")]
    Internal(#[from] anyhow::Error),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),
}

impl From<ApiError> for Status {
    fn from(err: ApiError) -> Self {
        match err {
            ApiError::AuthenticationFailed(msg) => {
                Status::new(Code::Unauthenticated, msg)
            }
            ApiError::InvalidSession => {
                Status::new(Code::Unauthenticated, "Invalid session token")
            }
            ApiError::SpaceNotFound(id) => {
                Status::new(Code::NotFound, format!("Space not found: {}", id))
            }
            ApiError::ChannelNotFound(id) => {
                Status::new(Code::NotFound, format!("Channel not found: {}", id))
            }
            ApiError::PermissionDenied(msg) => Status::new(Code::PermissionDenied, msg),
            ApiError::SpaceError(e) => Status::new(Code::Internal, e.to_string()),
            ApiError::ChannelError(e) => Status::new(Code::Internal, e.to_string()),
            ApiError::MembershipError(e) => Status::new(Code::Internal, e.to_string()),
            ApiError::InviteError(e) => Status::new(Code::Internal, e.to_string()),
            ApiError::Internal(e) => Status::new(Code::Internal, e.to_string()),
            ApiError::IoError(e) => Status::new(Code::Internal, e.to_string()),
            ApiError::JsonError(e) => Status::new(Code::Internal, e.to_string()),
        }
    }
}

pub type ApiResult<T> = Result<T, ApiError>;
