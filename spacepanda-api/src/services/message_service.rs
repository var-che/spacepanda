use std::sync::Arc;
use tonic::{Request, Response, Status};

use crate::proto::*;
use crate::session::SessionManager;

pub struct MessageServiceImpl {
    session_manager: Arc<SessionManager>,
}

impl MessageServiceImpl {
    pub fn new(session_manager: Arc<SessionManager>) -> Self {
        Self {
            session_manager,
        }
    }
}

#[tonic::async_trait]
impl message_service_server::MessageService for MessageServiceImpl {
    async fn get_messages(
        &self,
        request: Request<GetMessagesRequest>,
    ) -> Result<Response<GetMessagesResponse>, Status> {
        let req = request.into_inner();
        let session = self
            .session_manager
            .get_session(&req.session_token)
            .await
            .map_err(|e| Status::from(e))?;

        let channel_id_bytes = hex::decode(&req.channel_id)
            .map_err(|_| Status::invalid_argument("Invalid channel ID format"))?;
        let channel_id = if channel_id_bytes.len() == 32 {
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&channel_id_bytes);
            spacepanda_core::core_space::ChannelId::from_bytes(arr)
        } else {
            return Err(Status::invalid_argument("Invalid channel ID length"));
        };

        // Load messages from storage
        let limit = if req.limit > 0 { req.limit.min(100) } else { 50 } as i64;
        let offset = 0i64; // TODO: Implement cursor-based pagination with `before` field

        let stored_messages = session
            .manager
            .load_messages(&channel_id, limit, offset)
            .await
            .map_err(|e| Status::internal(format!("Failed to load messages: {}", e)))?;

        // Convert stored messages to proto Messages
        let mut messages = Vec::new();
        for (message_id_bytes, encrypted_content, sender_hash, sequence, _processed, plaintext_content) in stored_messages {
            // Check if we have plaintext (sent message) or need to decrypt (received message)
            let content = if let Some(plaintext) = plaintext_content {
                // This is a sent message, use stored plaintext
                String::from_utf8(plaintext)
                    .unwrap_or_else(|_| "[Binary content]".to_string())
            } else {
                // This is a received message, decrypt it
                let decrypted_content = session
                    .manager
                    .receive_channel_message(&channel_id, &encrypted_content)
                    .await
                    .map_err(|e| Status::internal(format!("Failed to decrypt message: {}", e)))?;

                String::from_utf8(decrypted_content)
                    .unwrap_or_else(|_| "[Binary content]".to_string())
            };

            // Parse message ID from bytes
            let message_id = if message_id_bytes.len() == 16 {
                uuid::Uuid::from_slice(&message_id_bytes)
                    .map(|id| id.to_string())
                    .unwrap_or_else(|_| hex::encode(&message_id_bytes))
            } else {
                hex::encode(&message_id_bytes)
            };

            // Parse sender ID from hash
            let sender_id = String::from_utf8(sender_hash.clone())
                .unwrap_or_else(|_| hex::encode(&sender_hash));

            messages.push(Message {
                id: message_id,
                channel_id: req.channel_id.clone(),
                sender_id,
                content,
                timestamp: sequence,
                is_e2ee: true,
                attachments: vec![],
            });
        }

        Ok(Response::new(GetMessagesResponse { messages }))
    }

    async fn send_message(
        &self,
        request: Request<SendMessageRequest>,
    ) -> Result<Response<Message>, Status> {
        let req = request.into_inner();
        let session = self
            .session_manager
            .get_session(&req.session_token)
            .await
            .map_err(|e| Status::from(e))?;

        let channel_id_bytes = hex::decode(&req.channel_id)
            .map_err(|_| Status::invalid_argument("Invalid channel ID format"))?;
        let channel_id = if channel_id_bytes.len() == 32 {
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&channel_id_bytes);
            spacepanda_core::core_space::ChannelId::from_bytes(arr)
        } else {
            return Err(Status::invalid_argument("Invalid channel ID length"));
        };

        // Send message via AsyncSpaceManager
        // This will:
        // 1. Encrypt the message with MLS
        // 2. Save it to the sender's local database
        // 3. Broadcast to peers via P2P (if network layer is configured)
        let _encrypted_content = session
            .manager
            .send_channel_message(&channel_id, &session.user_id, req.content.as_bytes())
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        // Retrieve the just-saved message to get the actual stored message ID
        let messages = session
            .manager
            .load_messages(&channel_id, 1, 0)
            .await
            .map_err(|e| Status::internal(format!("Failed to load sent message: {}", e)))?;

        if messages.is_empty() {
            return Err(Status::internal("Message was sent but not found in storage"));
        }

        let (message_id_bytes, _encrypted, sender_hash, sequence, _processed, _plaintext) = &messages[0];

        // Parse message ID from bytes
        let message_id = if message_id_bytes.len() == 16 {
            uuid::Uuid::from_slice(message_id_bytes)
                .map(|id| id.to_string())
                .unwrap_or_else(|_| hex::encode(message_id_bytes))
        } else {
            hex::encode(message_id_bytes)
        };

        let message = Message {
            id: message_id,
            channel_id: hex::encode(channel_id.as_bytes()),
            sender_id: String::from_utf8(sender_hash.clone())
                .unwrap_or_else(|_| hex::encode(sender_hash)),
            content: req.content,
            timestamp: *sequence,
            is_e2ee: true,
            attachments: vec![],
        };

        Ok(Response::new(message))
    }

    type StreamMessagesStream =
        tokio_stream::wrappers::ReceiverStream<Result<Message, Status>>;

    async fn stream_messages(
        &self,
        request: Request<StreamMessagesRequest>,
    ) -> Result<Response<Self::StreamMessagesStream>, Status> {
        let _req = request.into_inner();

        // TODO: Implement real-time message streaming
        // For now, return empty stream
        let (tx, rx) = tokio::sync::mpsc::channel(128);
        drop(tx); // Close immediately

        Ok(Response::new(
            tokio_stream::wrappers::ReceiverStream::new(rx),
        ))
    }
}
