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
pub enum ApiError {
    Internal(anyhow::Error),
    NotFound(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        match self {
            ApiError::Internal(err) => {
                let error_response = ErrorResponse {
                    error: err.to_string(),
                    details: None,
                };
                (StatusCode::INTERNAL_SERVER_ERROR, Json(error_response)).into_response()
            }
            ApiError::NotFound(msg) => {
                let error_response = ErrorResponse {
                    error: msg,
                    details: None,
                };
                (StatusCode::NOT_FOUND, Json(error_response)).into_response()
            }
        }
    }
}

impl<E> From<E> for ApiError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        ApiError::Internal(err.into())
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

/// POST /channels/:id/promote-member - Promote a member to Admin
pub async fn promote_member(
    State(state): State<Arc<AppState>>,
    Path(channel_id): Path<String>,
    Json(req): Json<PromoteMemberRequest>,
) -> ApiResult<Json<PromoteMemberResponse>> {
    let channel_id = ChannelId(channel_id);
    let member_identity = req.member_id.as_bytes();
    
    state
        .channel_manager
        .promote_member(&channel_id, member_identity)
        .await?;
    
    Ok(Json(PromoteMemberResponse {
        member_id: req.member_id,
        new_role: "Admin".to_string(),
    }))
}

/// POST /channels/:id/demote-member - Demote a member to regular Member
pub async fn demote_member(
    State(state): State<Arc<AppState>>,
    Path(channel_id): Path<String>,
    Json(req): Json<DemoteMemberRequest>,
) -> ApiResult<Json<DemoteMemberResponse>> {
    let channel_id = ChannelId(channel_id);
    let member_identity = req.member_id.as_bytes();
    
    state
        .channel_manager
        .demote_member(&channel_id, member_identity)
        .await?;
    
    Ok(Json(DemoteMemberResponse {
        member_id: req.member_id,
        new_role: "Member".to_string(),
    }))
}

/// GET /channels/:id/members/:member_id/role - Get a member's role
pub async fn get_member_role(
    State(state): State<Arc<AppState>>,
    Path((channel_id, member_id)): Path<(String, String)>,
) -> ApiResult<Json<GetMemberRoleResponse>> {
    let channel_id = ChannelId(channel_id);
    let member_identity = member_id.as_bytes();
    
    let role = state
        .channel_manager
        .get_member_role(&channel_id, member_identity)
        .await?;
    
    let role_str = match role {
        crate::core_mls::types::MemberRole::Admin => "Admin",
        crate::core_mls::types::MemberRole::Member => "Member",
        crate::core_mls::types::MemberRole::ReadOnly => "ReadOnly",
    };
    
    Ok(Json(GetMemberRoleResponse {
        member_id,
        role: role_str.to_string(),
    }))
}

/// POST /messages/:id/reactions - Add a reaction to a message
pub async fn add_reaction(
    State(state): State<Arc<AppState>>,
    Path(message_id): Path<String>,
    Json(req): Json<AddReactionRequest>,
) -> ApiResult<Json<AddReactionResponse>> {
    use crate::core_store::model::types::MessageId;
    
    let message_id = MessageId(message_id);
    
    state
        .channel_manager
        .add_reaction(&message_id, req.emoji.clone())
        .await?;
    
    Ok(Json(AddReactionResponse {
        message_id: message_id.0,
        emoji: req.emoji,
    }))
}

/// DELETE /messages/:id/reactions/:emoji - Remove a reaction from a message
pub async fn remove_reaction(
    State(state): State<Arc<AppState>>,
    Path((message_id, emoji)): Path<(String, String)>,
) -> ApiResult<StatusCode> {
    use crate::core_store::model::types::MessageId;
    
    let message_id = MessageId(message_id);
    
    state
        .channel_manager
        .remove_reaction(&message_id, emoji)
        .await?;
    
    Ok(StatusCode::NO_CONTENT)
}

/// GET /messages/:id/reactions - Get all reactions for a message
pub async fn get_reactions(
    State(state): State<Arc<AppState>>,
    Path(message_id): Path<String>,
) -> ApiResult<Json<GetReactionsResponse>> {
    use crate::core_store::model::types::MessageId;
    
    let message_id_obj = MessageId(message_id.clone());
    
    let summaries = state
        .channel_manager
        .get_reactions(&message_id_obj)
        .await?;
    
    // Convert to HTTP response format
    let reactions = summaries
        .into_iter()
        .map(|s| ReactionSummaryHttp {
            emoji: s.emoji,
            count: s.count,
            users: s.users.into_iter().map(|u| u.0).collect(),
            user_reacted: s.user_reacted,
        })
        .collect();
    
    Ok(Json(GetReactionsResponse {
        message_id,
        reactions,
    }))
}

/// GET /messages/:id/thread - Get thread info for a message
pub async fn get_thread_info(
    State(state): State<Arc<AppState>>,
    Path(message_id): Path<String>,
) -> ApiResult<Json<GetThreadInfoResponse>> {
    use crate::core_store::model::types::MessageId;
    
    let message_id_obj = MessageId(message_id.clone());
    
    let thread_info = state
        .channel_manager
        .get_thread_info(&message_id_obj)
        .await?;
    
    match thread_info {
        Some(info) => Ok(Json(GetThreadInfoResponse {
            root_message_id: info.root_message_id.0,
            reply_count: info.reply_count,
            participant_count: info.participant_count,
            participants: info.participants.into_iter().map(|u| u.0).collect(),
            last_reply_at: info.last_reply_at.map(|t| t.0),
            last_reply_preview: info.last_reply_preview,
        })),
        None => Err(ApiError::NotFound(format!(
            "No thread found for message {}",
            message_id
        ))),
    }
}

/// GET /messages/:id/replies - Get all replies to a message
pub async fn get_thread_replies(
    State(state): State<Arc<AppState>>,
    Path(message_id): Path<String>,
) -> ApiResult<Json<GetThreadRepliesResponse>> {
    use crate::core_store::model::types::MessageId;
    
    let message_id_obj = MessageId(message_id.clone());
    
    let replies = state
        .channel_manager
        .get_thread_replies(&message_id_obj)
        .await?;
    
    let replies_http: Vec<MessageInfoHttp> = replies
        .into_iter()
        .map(|msg| MessageInfoHttp {
            message_id: msg.message_id.0.clone(),
            channel_id: msg.channel_id.0.clone(),
            sender: msg.sender.0.clone(),
            timestamp: msg.timestamp.0,
            body: msg.body_as_string().unwrap_or_else(|| "<binary>".to_string()),
            reply_to: msg.reply_to.map(|id| id.0),
        })
        .collect();
    
    Ok(Json(GetThreadRepliesResponse {
        root_message_id: message_id,
        replies: replies_http,
    }))
}

/// GET /messages/:id/context - Get message with full thread context
pub async fn get_message_with_thread(
    State(state): State<Arc<AppState>>,
    Path(message_id): Path<String>,
) -> ApiResult<Json<GetMessageWithThreadResponse>> {
    use crate::core_store::model::types::MessageId;
    
    let message_id_obj = MessageId(message_id.clone());
    
    let msg_with_thread = state
        .channel_manager
        .get_message_with_thread(&message_id_obj)
        .await?;
    
    match msg_with_thread {
        Some(mwt) => {
            let message = MessageInfoHttp {
                message_id: mwt.message.message_id.0.clone(),
                channel_id: mwt.message.channel_id.0.clone(),
                sender: mwt.message.sender.0.clone(),
                timestamp: mwt.message.timestamp.0,
                body: mwt.message.body_as_string().unwrap_or_else(|| "<binary>".to_string()),
                reply_to: mwt.message.reply_to.clone().map(|id| id.0),
            };

            let thread_info = mwt.thread_info.map(|info| GetThreadInfoResponse {
                root_message_id: info.root_message_id.0,
                reply_count: info.reply_count,
                participant_count: info.participant_count,
                participants: info.participants.into_iter().map(|u| u.0).collect(),
                last_reply_at: info.last_reply_at.map(|t| t.0),
                last_reply_preview: info.last_reply_preview,
            });

            let parent_message = mwt.parent_message.map(|parent| {
                Box::new(MessageInfoHttp {
                    message_id: parent.message_id.0.clone(),
                    channel_id: parent.channel_id.0.clone(),
                    sender: parent.sender.0.clone(),
                    timestamp: parent.timestamp.0,
                    body: parent.body_as_string().unwrap_or_else(|| "<binary>".to_string()),
                    reply_to: parent.reply_to.map(|id| id.0),
                })
            });

            Ok(Json(GetMessageWithThreadResponse {
                message,
                thread_info,
                parent_message,
            }))
        }
        None => Err(ApiError::NotFound(format!("Message {} not found", message_id))),
    }
}

/// GET /channels/:id/threads - Get all threads in a channel
pub async fn get_channel_threads(
    State(state): State<Arc<AppState>>,
    Path(channel_id): Path<String>,
) -> ApiResult<Json<GetChannelThreadsResponse>> {
    use crate::core_store::model::types::ChannelId;
    
    let channel_id_obj = ChannelId(channel_id.clone());
    
    let threads = state
        .channel_manager
        .get_channel_threads(&channel_id_obj)
        .await?;
    
    let threads_http: Vec<ThreadSummaryHttp> = threads
        .into_iter()
        .map(|thread| {
            let message = MessageInfoHttp {
                message_id: thread.message.message_id.0.clone(),
                channel_id: thread.message.channel_id.0.clone(),
                sender: thread.message.sender.0.clone(),
                timestamp: thread.message.timestamp.0,
                body: thread.message.body_as_string().unwrap_or_else(|| "<binary>".to_string()),
                reply_to: thread.message.reply_to.map(|id| id.0),
            };

            let thread_info = thread.thread_info.map(|info| GetThreadInfoResponse {
                root_message_id: info.root_message_id.0,
                reply_count: info.reply_count,
                participant_count: info.participant_count,
                participants: info.participants.into_iter().map(|u| u.0).collect(),
                last_reply_at: info.last_reply_at.map(|t| t.0),
                last_reply_preview: info.last_reply_preview,
            });

            ThreadSummaryHttp {
                message,
                thread_info,
            }
        })
        .collect();
    
    Ok(Json(GetChannelThreadsResponse {
        channel_id,
        threads: threads_http,
    }))
}

