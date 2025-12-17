use std::sync::Arc;
use tonic::{Request, Response, Status};

use crate::proto::*;
use crate::session::SessionManager;
use spacepanda_core::core_store::UserId;

pub struct SpaceServiceImpl {
    session_manager: Arc<SessionManager>,
}

impl SpaceServiceImpl {
    pub fn new(session_manager: Arc<SessionManager>) -> Self {
        Self {
            session_manager,
        }
    }
}

#[tonic::async_trait]
impl space_service_server::SpaceService for SpaceServiceImpl {
    async fn list_spaces(
        &self,
        request: Request<ListSpacesRequest>,
    ) -> Result<Response<ListSpacesResponse>, Status> {
        let req = request.into_inner();
        let session = self
            .session_manager
            .get_session(&req.session_token)
            .await
            .map_err(|e| Status::from(e))?;

        // Get spaces from AsyncSpaceManager
        let core_spaces = session
            .manager
            .list_user_spaces(&session.user_id)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let spaces = core_spaces
            .into_iter()
            .map(|s| Space {
                id: s.id.to_string(),
                name: s.name,
                description: s.description.unwrap_or_default(),
                icon_url: String::new(),
                visibility: match s.visibility {
                    spacepanda_core::core_space::SpaceVisibility::Public => {
                        SpaceVisibility::Public as i32
                    }
                    spacepanda_core::core_space::SpaceVisibility::Private => {
                        SpaceVisibility::Private as i32
                    }
                },
                owner_id: s.owner_id.to_string(),
                member_ids: s.members.keys().map(|id| id.0.clone()).collect(),
                channel_ids: s.channels.iter().map(|id| id.to_string()).collect(),
                created_at: s.created_at.as_millis() as i64,
            })
            .collect();

        Ok(Response::new(ListSpacesResponse { spaces }))
    }

    async fn list_channels(
        &self,
        request: Request<ListChannelsRequest>,
    ) -> Result<Response<ListChannelsResponse>, Status> {
        let req = request.into_inner();
        let session = self
            .session_manager
            .get_session(&req.session_token)
            .await
            .map_err(|e| Status::from(e))?;

        let space_id_bytes = hex::decode(&req.space_id)
            .map_err(|_| Status::invalid_argument("Invalid space ID format"))?;
        let space_id = if space_id_bytes.len() == 32 {
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&space_id_bytes);
            spacepanda_core::core_space::SpaceId::from_bytes(arr)
        } else {
            return Err(Status::invalid_argument("Invalid space ID length"));
        };

        // Get channels from AsyncSpaceManager
        let core_channels = session
            .manager
            .list_space_channels(&space_id)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let channels = core_channels
            .into_iter()
            .map(|c| Channel {
                id: c.id.to_string(),
                space_id: c.space_id.to_string(),
                name: c.name,
                description: c.description.unwrap_or_default(),
                visibility: match c.visibility {
                    spacepanda_core::core_space::ChannelVisibility::Public => {
                        ChannelVisibility::Public as i32
                    }
                    spacepanda_core::core_space::ChannelVisibility::Private => {
                        ChannelVisibility::Private as i32
                    }
                },
                member_ids: c.members.iter().map(|id| id.0.clone()).collect(),
                created_at: c.created_at.as_millis() as i64,
            })
            .collect();

        Ok(Response::new(ListChannelsResponse { channels }))
    }

    async fn create_space(
        &self,
        request: Request<CreateSpaceRequest>,
    ) -> Result<Response<CreateSpaceResponse>, Status> {
        let req = request.into_inner();
        let session = self
            .session_manager
            .get_session(&req.session_token)
            .await
            .map_err(|e| Status::from(e))?;

        let visibility = match req.visibility {
            1 => spacepanda_core::core_space::SpaceVisibility::Public,
            2 => spacepanda_core::core_space::SpaceVisibility::Private,
            _ => spacepanda_core::core_space::SpaceVisibility::Private,
        };

        let core_space = session
            .manager
            .create_space(req.name, session.user_id.clone(), visibility)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let space = Space {
            id: core_space.id.to_string(),
            name: core_space.name,
            description: core_space.description.unwrap_or_default(),
            icon_url: String::new(),
            visibility: req.visibility,
            owner_id: core_space.owner_id.to_string(),
            member_ids: core_space
                .members
                .keys()
                .map(|id| id.0.clone())
                .collect(),
            channel_ids: core_space.channels.iter().map(|id| id.to_string()).collect(),
            created_at: core_space.created_at.as_millis() as i64,
        };

        Ok(Response::new(CreateSpaceResponse { space: Some(space) }))
    }

    async fn get_space(
        &self,
        request: Request<GetSpaceRequest>,
    ) -> Result<Response<Space>, Status> {
        let req = request.into_inner();
        let session = self
            .session_manager
            .get_session(&req.session_token)
            .await
            .map_err(|e| Status::from(e))?;

        let space_id_bytes = hex::decode(&req.space_id)
            .map_err(|_| Status::invalid_argument("Invalid space ID format"))?;
        let space_id = if space_id_bytes.len() == 32 {
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&space_id_bytes);
            spacepanda_core::core_space::SpaceId::from_bytes(arr)
        } else {
            return Err(Status::invalid_argument("Invalid space ID length"));
        };

        let core_space = session
            .manager
            .get_space(&space_id)
            .await
            .map_err(|e| Status::not_found(e.to_string()))?;

        let space = Space {
            id: core_space.id.to_string(),
            name: core_space.name,
            description: core_space.description.unwrap_or_default(),
            icon_url: String::new(),
            visibility: match core_space.visibility {
                spacepanda_core::core_space::SpaceVisibility::Public => SpaceVisibility::Public as i32,
                spacepanda_core::core_space::SpaceVisibility::Private => {
                    SpaceVisibility::Private as i32
                }
            },
            owner_id: core_space.owner_id.to_string(),
            member_ids: core_space
                .members
                .keys()
                .map(|id| id.0.clone())
                .collect(),
            channel_ids: core_space.channels.iter().map(|id| id.to_string()).collect(),
            created_at: core_space.created_at.as_millis() as i64,
        };

        Ok(Response::new(space))
    }

    async fn create_channel(
        &self,
        request: Request<CreateChannelRequest>,
    ) -> Result<Response<CreateChannelResponse>, Status> {
        let req = request.into_inner();
        let session = self
            .session_manager
            .get_session(&req.session_token)
            .await
            .map_err(|e| Status::from(e))?;

        // Parse space_id
        let space_id_bytes = hex::decode(&req.space_id)
            .map_err(|_| Status::invalid_argument("Invalid space ID format"))?;
        let space_id = if space_id_bytes.len() == 32 {
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&space_id_bytes);
            spacepanda_core::core_space::SpaceId::from_bytes(arr)
        } else {
            return Err(Status::invalid_argument("Invalid space ID length"));
        };

        // Convert visibility from proto enum to core enum
        let visibility = match req.visibility() {
            ChannelVisibility::Public => spacepanda_core::core_space::ChannelVisibility::Public,
            ChannelVisibility::Private => spacepanda_core::core_space::ChannelVisibility::Private,
            ChannelVisibility::Unspecified => {
                spacepanda_core::core_space::ChannelVisibility::Public
            }
        };

        // Create channel using AsyncSpaceManager
        let core_channel = session
            .manager
            .create_channel(space_id, req.name, session.user_id.clone(), visibility)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        // Register channel creator with P2P network
        session
            .manager
            .register_channel_member(&core_channel.id, &session.user_id, session.peer_id.clone())
            .await
            .map_err(|e| Status::internal(format!("Failed to register channel member: {}", e)))?;

        // Convert to proto Channel
        let channel = Channel {
            id: core_channel.id.to_string(),
            space_id: core_channel.space_id.to_string(),
            name: core_channel.name,
            description: core_channel.description.unwrap_or_default(),
            visibility: match core_channel.visibility {
                spacepanda_core::core_space::ChannelVisibility::Public => {
                    ChannelVisibility::Public as i32
                }
                spacepanda_core::core_space::ChannelVisibility::Private => {
                    ChannelVisibility::Private as i32
                }
            },
            member_ids: core_channel
                .members
                .iter()
                .map(|id| id.0.clone())
                .collect(),
            created_at: core_channel.created_at.as_millis() as i64,
        };

        Ok(Response::new(CreateChannelResponse {
            channel: Some(channel),
        }))
    }

    async fn add_member_to_channel(
        &self,
        request: Request<AddMemberToChannelRequest>,
    ) -> Result<Response<AddMemberToChannelResponse>, Status> {
        let req = request.into_inner();
        let session = self
            .session_manager
            .get_session(&req.session_token)
            .await
            .map_err(|e| Status::from(e))?;

        // Parse channel ID
        let channel_id_bytes = hex::decode(&req.channel_id)
            .map_err(|_| Status::invalid_argument("Invalid channel ID format"))?;
        let channel_id = if channel_id_bytes.len() == 32 {
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&channel_id_bytes);
            spacepanda_core::core_space::ChannelId::from_bytes(arr)
        } else {
            return Err(Status::invalid_argument("Invalid channel ID length"));
        };

        // Create UserId for the member to add
        let member_user_id = UserId(req.user_id.clone());

        // Add member to channel
        session
            .manager
            .add_member_to_channel(&channel_id, &member_user_id)
            .await
            .map_err(|e| Status::internal(format!("Failed to add member: {}", e)))?;

        Ok(Response::new(AddMemberToChannelResponse {
            success: true,
            message: "Member added successfully".to_string(),
        }))
    }

    async fn remove_member_from_channel(
        &self,
        request: Request<RemoveMemberFromChannelRequest>,
    ) -> Result<Response<RemoveMemberFromChannelResponse>, Status> {
        let req = request.into_inner();
        let session = self
            .session_manager
            .get_session(&req.session_token)
            .await
            .map_err(|e| Status::from(e))?;

        // Parse channel ID
        let channel_id_bytes = hex::decode(&req.channel_id)
            .map_err(|_| Status::invalid_argument("Invalid channel ID format"))?;
        let channel_id = if channel_id_bytes.len() == 32 {
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&channel_id_bytes);
            spacepanda_core::core_space::ChannelId::from_bytes(arr)
        } else {
            return Err(Status::invalid_argument("Invalid channel ID length"));
        };

        // Create UserId for the member to remove
        let member_user_id = UserId(req.user_id);

        // Remove member from channel
        session
            .manager
            .remove_member_from_channel(&channel_id, &member_user_id)
            .await
            .map_err(|e| Status::internal(format!("Failed to remove member: {}", e)))?;

        Ok(Response::new(RemoveMemberFromChannelResponse {
            success: true,
            message: "Member removed successfully".to_string(),
        }))
    }

    async fn generate_key_package(
        &self,
        request: Request<GenerateKeyPackageRequest>,
    ) -> Result<Response<GenerateKeyPackageResponse>, Status> {
        let req = request.into_inner();
        let session = self
            .session_manager
            .get_session(&req.session_token)
            .await
            .map_err(|e| Status::from(e))?;

        // Generate key package for this user
        let key_package = session
            .manager
            .generate_key_package()
            .await
            .map_err(|e| Status::internal(format!("Failed to generate key package: {}", e)))?;

        Ok(Response::new(GenerateKeyPackageResponse { key_package }))
    }

    async fn create_channel_invite(
        &self,
        request: Request<CreateChannelInviteRequest>,
    ) -> Result<Response<CreateChannelInviteResponse>, Status> {
        let req = request.into_inner();
        let session = self
            .session_manager
            .get_session(&req.session_token)
            .await
            .map_err(|e| Status::from(e))?;

        // Parse channel ID
        let channel_id_bytes = hex::decode(&req.channel_id)
            .map_err(|_| Status::invalid_argument("Invalid channel ID format"))?;
        let channel_id = if channel_id_bytes.len() == 32 {
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&channel_id_bytes);
            spacepanda_core::core_space::ChannelId::from_bytes(arr)
        } else {
            return Err(Status::invalid_argument("Invalid channel ID length"));
        };

        // Create invite with provided key package
        let (invite_token, commit, ratchet_tree) = session
            .manager
            .create_channel_invite(&channel_id, req.key_package)
            .await
            .map_err(|e| Status::internal(format!("Failed to create invite: {}", e)))?;

        // Broadcast the commit to all existing members for MLS state sync
        if let Some(commit_data) = &commit {
            if let Some(network_layer) = session.manager.network_layer() {
                eprintln!("[P2P] Broadcasting MLS commit for channel {} after creating invite", req.channel_id);
                let network_channel_id = spacepanda_core::core_store::model::types::ChannelId(req.channel_id.clone());
                if let Err(e) = network_layer.broadcast_commit(&network_channel_id, commit_data.clone()).await {
                    eprintln!("[P2P] Warning: Failed to broadcast commit: {}", e);
                    // Continue anyway - the commit is included in the response
                } else {
                    eprintln!("[P2P] ✓ Commit broadcasted to existing members");
                }
            }
        }

        // Get channel metadata to include in invite
        let channel = session
            .manager
            .get_channel(&channel_id)
            .await
            .map_err(|e| Status::internal(format!("Failed to get channel metadata: {}", e)))?;

        Ok(Response::new(CreateChannelInviteResponse {
            invite_token,
            commit: commit.unwrap_or_default(),
            ratchet_tree: ratchet_tree.unwrap_or_default(),
            space_id: hex::encode(channel.space_id.as_bytes()),
            channel_name: channel.name,
            channel_id: req.channel_id.clone(), // Pass through original channel ID
        }))
    }

    async fn join_channel(
        &self,
        request: Request<JoinChannelRequest>,
    ) -> Result<Response<JoinChannelResponse>, Status> {
        let req = request.into_inner();
        let session = self
            .session_manager
            .get_session(&req.session_token)
            .await
            .map_err(|e| Status::from(e))?;

        // Join channel from invite token
        let ratchet_tree = if req.ratchet_tree.is_empty() {
            None
        } else {
            Some(req.ratchet_tree)
        };

        // Parse space ID from request
        let space_id_bytes = hex::decode(&req.space_id)
            .map_err(|_| Status::invalid_argument("Invalid space ID format"))?;
        let space_id = if space_id_bytes.len() == 32 {
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&space_id_bytes);
            spacepanda_core::core_space::SpaceId::from_bytes(arr)
        } else {
            return Err(Status::invalid_argument("Invalid space ID length"));
        };

        // Parse original channel ID from invite if provided
        let original_channel_id = if !req.channel_id.is_empty() {
            let channel_id_bytes = hex::decode(&req.channel_id)
                .map_err(|_| Status::invalid_argument("Invalid channel ID format"))?;
            if channel_id_bytes.len() == 32 {
                let mut arr = [0u8; 32];
                arr.copy_from_slice(&channel_id_bytes);
                Some(spacepanda_core::core_space::ChannelId::from_bytes(arr))
            } else {
                return Err(Status::invalid_argument("Invalid channel ID length"));
            }
        } else {
            None
        };

        let channel_id = session
            .manager
            .join_channel_from_invite(
                req.invite_token,
                ratchet_tree,
                &session.user_id,
                &space_id,
                &req.channel_name,
                original_channel_id, // Pass original channel ID for P2P consistency
            )
            .await
            .map_err(|e| Status::internal(format!("Failed to join channel: {}", e)))?;

        // Register new member with P2P network
        session
            .manager
            .register_channel_member(&channel_id, &session.user_id, session.peer_id.clone())
            .await
            .map_err(|e| Status::internal(format!("Failed to register channel member: {}", e)))?;

        Ok(Response::new(JoinChannelResponse {
            success: true,
            channel_id: hex::encode(channel_id.as_bytes()),
            message: "Successfully joined channel".to_string(),
        }))
    }
}
