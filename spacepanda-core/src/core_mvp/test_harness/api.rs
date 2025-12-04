//! API routes definition

use super::handlers;
use super::state::AppState;
use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;

/// Build the API router with all endpoints
pub fn build_router(state: Arc<AppState>) -> Router {
    Router::new()
        // Identity routes
        .route("/identity/create", post(handlers::create_identity))
        .route("/identity/me", get(handlers::get_identity))
        // Channel routes
        .route("/channels/create", post(handlers::create_channel))
        .route("/channels/:id", get(handlers::get_channel))
        .route("/channels/:id/invite", post(handlers::create_invite))
        .route("/channels/:id/join", post(handlers::join_channel))
        .route("/channels/:id/members", get(handlers::list_members))
        .route("/channels/:id/remove-member", post(handlers::remove_member))
        .route("/channels/:id/promote-member", post(handlers::promote_member))
        .route("/channels/:id/demote-member", post(handlers::demote_member))
        .route("/channels/:id/members/:member_id/role", get(handlers::get_member_role))
        .route("/channels/:id/process-commit", post(handlers::process_commit))
        // Message routes
        .route("/channels/:id/send", post(handlers::send_message))
        .route("/channels/:id/messages", get(handlers::get_messages))
        // State
        .with_state(state)
}
