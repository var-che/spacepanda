//! Request/Response types for the HTTP test harness

use serde::{Deserialize, Serialize};

// Import core_mvp types
pub type ChannelId = String;

// ============================================================================
// Identity Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityCreateResponse {
    pub identity_id: String,
    pub public_key: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityInfoResponse {
    pub identity_id: String,
    pub public_key: Vec<u8>,
}

// ============================================================================
// Channel Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelCreateRequest {
    pub name: String,
    pub is_public: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelCreateResponse {
    pub channel_id: String,
    pub name: String,
    pub is_public: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelInfoResponse {
    pub channel_id: String,
    pub name: String,
    pub is_public: bool,
    pub member_count: usize,
}

// ============================================================================
// Invite Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InviteRequest {
    /// Serialized KeyPackage from invitee
    pub key_package: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InviteResponse {
    /// Serialized InviteToken (contains Welcome + metadata)
    pub invite_token: Vec<u8>,
    /// Optional commit for existing members to process
    pub commit: Option<Vec<u8>>,
}

// ============================================================================
// Join Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoinRequest {
    /// Serialized InviteToken
    pub invite_token: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoinResponse {
    pub channel_id: String,
    pub channel_name: String,
    pub is_public: bool,
    pub success: bool,
}

// ============================================================================
// Member Management Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoveMemberRequest {
    /// Identity of the member to remove (user ID)
    pub member_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoveMemberResponse {
    /// Commit for remaining members to process
    pub commit: Vec<u8>,
    pub removed_member_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromoteMemberRequest {
    /// Identity of the member to promote to Admin
    pub member_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromoteMemberResponse {
    pub member_id: String,
    pub new_role: String, // "Admin"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DemoteMemberRequest {
    /// Identity of the member to demote to Member
    pub member_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DemoteMemberResponse {
    pub member_id: String,
    pub new_role: String, // "Member"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetMemberRoleRequest {
    /// Identity of the member to query
    pub member_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetMemberRoleResponse {
    pub member_id: String,
    pub role: String, // "Admin", "Member", or "ReadOnly"
}

// ============================================================================
// Message Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendMessageRequest {
    pub plaintext: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendMessageResponse {
    pub message_id: String,
    pub encrypted_bytes: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageHistoryResponse {
    pub messages: Vec<MessageInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageInfo {
    pub message_id: String,
    pub sender_id: String,
    pub plaintext: String,
    pub timestamp: u64,
}

// ============================================================================
// Member Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemberListResponse {
    pub members: Vec<MemberInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemberInfo {
    pub identity_id: String,
    pub public_key: Vec<u8>,
}

// ============================================================================
// Error Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
    pub details: Option<String>,
}
