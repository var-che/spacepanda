//! HTTP API handlers for the test harness

use super::state::AppState;
use super::types::*;
use crate::core_mvp::types::InviteToken;
use crate::core_store::model::types::ChannelId;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use std::sync::Arc;

/// Custom error type for API responses
pub struct ApiError(anyhow::Error);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let error_response = ErrorResponse {
            error: self.0.to_string(),
            details: None,
        };
        (StatusCode::INTERNAL_SERVER_ERROR, Json(error_response)).into_response()
    }
}

impl<E> From<E> for ApiError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        ApiError(err.into())
    }
}

type ApiResult<T> = Result<T, ApiError>;

// ============================================================================
// Identity Handlers
// ============================================================================

/// POST /identity/create - Create a new identity
pub async fn create_identity(State(state): State<Arc<AppState>>) -> ApiResult<Json<IdentityCreateResponse>> {
    // For now, generate a simple identity (in production, this would use core_identity)
    // Using the channel manager's underlying provider
    let identity_id = uuid::uuid!("00000000-0000-0000-0000-000000000001").to_string();
    let public_key = vec![0u8; 32]; // Placeholder - would be real public key
    
    let identity = super::state::Identity {
        identity_id: identity_id.clone(),
        public_key: public_key.clone(),
    };
    
    state.set_identity(identity).await;
    
    Ok(Json(IdentityCreateResponse {
        identity_id,
        public_key,
    }))
}

/// GET /identity/me - Get current identity
pub async fn get_identity(State(state): State<Arc<AppState>>) -> ApiResult<Json<IdentityInfoResponse>> {
    let identity = state
        .get_identity()
        .await
        .ok_or_else(|| anyhow::anyhow!("No identity created yet"))?;
    
    Ok(Json(IdentityInfoResponse {
        identity_id: identity.identity_id,
        public_key: identity.public_key,
    }))
}

// ============================================================================
// Channel Handlers
// ============================================================================

/// POST /channels/create - Create a new channel
pub async fn create_channel(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ChannelCreateRequest>,
) -> ApiResult<Json<ChannelCreateResponse>> {
    let channel_id = state
        .channel_manager
        .create_channel(req.name.clone(), req.is_public)
        .await?;
    
    Ok(Json(ChannelCreateResponse {
        channel_id: channel_id.0,
        name: req.name,
        is_public: req.is_public,
    }))
}

/// GET /channels/:id - Get channel info
pub async fn get_channel(
    State(state): State<Arc<AppState>>,
    Path(channel_id): Path<String>,
) -> ApiResult<Json<ChannelInfoResponse>> {
    let channel_id_typed = ChannelId(channel_id.clone());
    let channel = state
        .channel_manager
        .get_channel(&channel_id_typed)
        .await?;
    
    Ok(Json(ChannelInfoResponse {
        channel_id,
        name: channel.name,
        is_public: channel.is_public,
        member_count: 0, // TODO: Get actual member count from MLS group
    }))
}

/// POST /channels/:id/invite - Create an invite
pub async fn create_invite(
    State(state): State<Arc<AppState>>,
    Path(channel_id): Path<String>,
    Json(req): Json<InviteRequest>,
) -> ApiResult<Json<InviteResponse>> {
    let channel_id_typed = ChannelId(channel_id);
    let (invite_token, commit) = state
        .channel_manager
        .create_invite(&channel_id_typed, req.key_package)
        .await?;
    
    // Serialize the invite token
    let invite_bytes = bincode::serialize(&invite_token)
        .map_err(|e| anyhow::anyhow!("Failed to serialize invite: {}", e))?;
    
    Ok(Json(InviteResponse {
        invite_token: invite_bytes,
        commit,
    }))
}

/// POST /channels/:id/join - Join a channel
pub async fn join_channel(
    State(state): State<Arc<AppState>>,
    Path(_channel_id): Path<String>,
    Json(req): Json<JoinRequest>,
) -> ApiResult<Json<JoinResponse>> {
    // Deserialize the invite token
    let invite_token: InviteToken = bincode::deserialize(&req.invite_token)
        .map_err(|e| anyhow::anyhow!("Failed to deserialize invite: {}", e))?;
    
    let channel_name = invite_token.channel_name.clone();
    let is_public = invite_token.is_public;
    
    let channel_id = state
        .channel_manager
        .join_channel(&invite_token)
        .await?;
    
    Ok(Json(JoinResponse {
        channel_id: channel_id.0,
        channel_name,
        is_public,
        success: true,
    }))
}

/// GET /channels/:id/members - List channel members
pub async fn list_members(
    State(state): State<Arc<AppState>>,
    Path(channel_id): Path<String>,
) -> ApiResult<Json<MemberListResponse>> {
    let channel_id_typed = ChannelId(channel_id);
    let _channel = state
        .channel_manager
        .get_channel(&channel_id_typed)
        .await?;
    
    // TODO: Get actual members from MLS group
    let members = vec![];
    
    Ok(Json(MemberListResponse { members }))
}

// ============================================================================
// Message Handlers
// ============================================================================

/// POST /channels/:id/send - Send an encrypted message
pub async fn send_message(
    State(state): State<Arc<AppState>>,
    Path(channel_id): Path<String>,
    Json(req): Json<SendMessageRequest>,
) -> ApiResult<Json<SendMessageResponse>> {
    let channel_id_typed = ChannelId(channel_id);
    let encrypted = state
        .channel_manager
        .send_message(&channel_id_typed, req.plaintext.as_bytes())
        .await?;
    
    let message_id = uuid::Uuid::new_v4().to_string();
    
    Ok(Json(SendMessageResponse {
        message_id,
        encrypted_bytes: encrypted.len(),
    }))
}

/// GET /channels/:id/messages - Get message history
pub async fn get_messages(
    State(state): State<Arc<AppState>>,
    Path(channel_id): Path<String>,
) -> ApiResult<Json<MessageHistoryResponse>> {
    let messages = state.get_messages(&channel_id).await;
    
    let message_infos = messages
        .into_iter()
        .map(|msg| MessageInfo {
            message_id: msg.message_id,
            sender_id: msg.sender_id,
            plaintext: msg.plaintext,
            timestamp: msg.timestamp,
        })
        .collect();
    
    Ok(Json(MessageHistoryResponse {
        messages: message_infos,
    }))
}

// ============================================================================
// Process Commit Handler
// ============================================================================

/// POST /channels/:id/process-commit - Process a commit from another member
pub async fn process_commit(
    State(state): State<Arc<AppState>>,
    Path(channel_id): Path<String>,
    Json(commit): Json<Vec<u8>>,
) -> ApiResult<StatusCode> {
    state
        .channel_manager
        .process_commit(&commit)
        .await?;
    
    Ok(StatusCode::OK)
}

/// POST /channels/:id/remove-member - Remove a member from the channel
pub async fn remove_member(
    State(state): State<Arc<AppState>>,
    Path(channel_id): Path<String>,
    Json(req): Json<RemoveMemberRequest>,
) -> ApiResult<Json<RemoveMemberResponse>> {
    let channel_id = ChannelId(channel_id);
    
    // Convert member_id string to bytes
    let member_identity = req.member_id.as_bytes();
    
    // Remove the member
    let commit = state
        .channel_manager
        .remove_member(&channel_id, member_identity)
        .await?;
    
    Ok(Json(RemoveMemberResponse {
        commit,
        removed_member_id: req.member_id,
    }))
}
