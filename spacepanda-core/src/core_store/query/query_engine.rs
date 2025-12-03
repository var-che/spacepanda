/*
    query_engine.rs - Query interface for UI/API layer

    Provides high-level queries over CRDT state without modifying it.
    This is read-only access for presentation purposes.

    Features:
    - List channels in a space
    - Get messages in a channel
    - Search messages
    - Filter by user/time/role
    - Thread reconstruction
*/

use crate::core_store::model::{
    Channel, ChannelId, Message, MessageId, Space, SpaceId, Timestamp, UserId,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Query results for channels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelInfo {
    pub id: ChannelId,
    pub name: String,
    pub topic: Option<String>,
    pub member_count: usize,
    pub unread_count: usize,
    pub last_message_time: Option<Timestamp>,
}

/// Query results for messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageInfo {
    pub id: MessageId,
    pub sender: UserId,
    pub content: Vec<u8>, // Encrypted
    pub timestamp: Timestamp,
    pub is_edited: bool,
    pub reply_to: Option<MessageId>,
    pub reaction_count: usize,
}

/// Query results for spaces
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpaceInfo {
    pub id: SpaceId,
    pub name: String,
    pub description: Option<String>,
    pub member_count: usize,
    pub channel_count: usize,
    pub role_count: usize,
}

/// Sorting options for queries
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum SortOrder {
    Ascending,
    Descending,
}

/// Query engine for read-only access
pub struct QueryEngine {
    /// Cache of spaces
    spaces: HashMap<SpaceId, Space>,

    /// Cache of channels
    channels: HashMap<ChannelId, Channel>,

    /// Cache of messages by channel
    messages: HashMap<ChannelId, Vec<Message>>,
}

impl QueryEngine {
    pub fn new() -> Self {
        QueryEngine { spaces: HashMap::new(), channels: HashMap::new(), messages: HashMap::new() }
    }

    /// Add a space to the query cache
    pub fn add_space(&mut self, space: Space) {
        self.spaces.insert(space.id.clone(), space);
    }

    /// Add a channel to the query cache
    pub fn add_channel(&mut self, channel: Channel) {
        self.channels.insert(channel.id.clone(), channel);
    }

    /// Add messages to the query cache
    pub fn add_messages(&mut self, channel_id: ChannelId, messages: Vec<Message>) {
        self.messages.insert(channel_id, messages);
    }

    /// List all spaces
    pub fn list_spaces(&self) -> Vec<SpaceInfo> {
        self.spaces
            .values()
            .map(|space| SpaceInfo {
                id: space.id.clone(),
                name: space.get_name().unwrap_or(&String::new()).clone(),
                description: space.get_description().cloned(),
                member_count: space.get_members().len(),
                channel_count: space.get_channels().len(),
                role_count: space.roles.len(),
            })
            .collect()
    }

    /// Get a specific space by ID
    pub fn get_space(&self, space_id: &SpaceId) -> Option<&Space> {
        self.spaces.get(space_id)
    }

    /// List all channels in a space
    pub fn list_channels_in_space(&self, space_id: &SpaceId) -> Vec<ChannelInfo> {
        let space = match self.spaces.get(space_id) {
            Some(s) => s,
            None => return Vec::new(),
        };

        let channel_ids = space.get_channels();

        channel_ids
            .iter()
            .filter_map(|channel_id| {
                self.channels.get(channel_id).map(|channel| {
                    let empty_vec = Vec::new();
                    let messages = self.messages.get(&channel.id).unwrap_or(&empty_vec);
                    let last_message_time = messages.last().map(|m| m.timestamp);

                    ChannelInfo {
                        id: channel.id.clone(),
                        name: channel.get_name().unwrap_or(&String::new()).clone(),
                        topic: channel.get_topic().cloned(),
                        member_count: channel.get_members().len(),
                        unread_count: 0, // TODO: Track read positions
                        last_message_time,
                    }
                })
            })
            .collect()
    }

    /// Get a specific channel by ID
    pub fn get_channel(&self, channel_id: &ChannelId) -> Option<&Channel> {
        self.channels.get(channel_id)
    }

    /// List messages in a channel
    pub fn list_messages(
        &self,
        channel_id: &ChannelId,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Vec<MessageInfo> {
        let messages = match self.messages.get(channel_id) {
            Some(m) => m,
            None => return Vec::new(),
        };

        let offset = offset.unwrap_or(0);
        let result: Vec<MessageInfo> = messages
            .iter()
            .skip(offset)
            .take(limit.unwrap_or(messages.len()))
            .map(|msg| MessageInfo {
                id: msg.id.clone(),
                sender: msg.sender.clone(),
                content: msg.current_content().to_vec(),
                timestamp: msg.timestamp,
                is_edited: msg.is_edited(),
                reply_to: msg.reply_to.clone(),
                reaction_count: msg.reactions.len(),
            })
            .collect();

        result
    }

    /// Search messages by content (works on encrypted content hash matching)
    pub fn search_messages(&self, channel_id: &ChannelId, _query: &str) -> Vec<MessageInfo> {
        // Note: This is a placeholder - real search needs decryption
        // In production, you'd decrypt and index locally
        let messages = match self.messages.get(channel_id) {
            Some(m) => m,
            None => return Vec::new(),
        };

        // For now, just return all messages
        // TODO: Implement proper search indexing
        messages
            .iter()
            .map(|msg| MessageInfo {
                id: msg.id.clone(),
                sender: msg.sender.clone(),
                content: msg.current_content().to_vec(),
                timestamp: msg.timestamp,
                is_edited: msg.is_edited(),
                reply_to: msg.reply_to.clone(),
                reaction_count: msg.reactions.len(),
            })
            .collect()
    }

    /// Get messages by a specific sender
    pub fn messages_by_sender(&self, channel_id: &ChannelId, sender: &UserId) -> Vec<MessageInfo> {
        let messages = match self.messages.get(channel_id) {
            Some(m) => m,
            None => return Vec::new(),
        };

        messages
            .iter()
            .filter(|msg| &msg.sender == sender)
            .map(|msg| MessageInfo {
                id: msg.id.clone(),
                sender: msg.sender.clone(),
                content: msg.current_content().to_vec(),
                timestamp: msg.timestamp,
                is_edited: msg.is_edited(),
                reply_to: msg.reply_to.clone(),
                reaction_count: msg.reactions.len(),
            })
            .collect()
    }

    /// Get messages in a time range
    pub fn messages_in_range(
        &self,
        channel_id: &ChannelId,
        start: Timestamp,
        end: Timestamp,
    ) -> Vec<MessageInfo> {
        let messages = match self.messages.get(channel_id) {
            Some(m) => m,
            None => return Vec::new(),
        };

        messages
            .iter()
            .filter(|msg| msg.timestamp >= start && msg.timestamp <= end)
            .map(|msg| MessageInfo {
                id: msg.id.clone(),
                sender: msg.sender.clone(),
                content: msg.current_content().to_vec(),
                timestamp: msg.timestamp,
                is_edited: msg.is_edited(),
                reply_to: msg.reply_to.clone(),
                reaction_count: msg.reactions.len(),
            })
            .collect()
    }

    /// Get thread (all replies to a message)
    pub fn get_thread(&self, channel_id: &ChannelId, parent_id: &MessageId) -> Vec<MessageInfo> {
        let messages = match self.messages.get(channel_id) {
            Some(m) => m,
            None => return Vec::new(),
        };

        messages
            .iter()
            .filter(|msg| msg.reply_to.as_ref() == Some(parent_id))
            .map(|msg| MessageInfo {
                id: msg.id.clone(),
                sender: msg.sender.clone(),
                content: msg.current_content().to_vec(),
                timestamp: msg.timestamp,
                is_edited: msg.is_edited(),
                reply_to: msg.reply_to.clone(),
                reaction_count: msg.reactions.len(),
            })
            .collect()
    }

    /// Get members of a channel
    pub fn get_channel_members(&self, channel_id: &ChannelId) -> Vec<UserId> {
        self.channels.get(channel_id).map(|c| c.get_members()).unwrap_or_default()
    }

    /// Get members of a space
    pub fn get_space_members(&self, space_id: &SpaceId) -> Vec<UserId> {
        self.spaces.get(space_id).map(|s| s.get_members()).unwrap_or_default()
    }

    /// Get user's role in a space
    pub fn get_user_role(&self, space_id: &SpaceId, user_id: &UserId) -> Option<String> {
        self.spaces.get(space_id).and_then(|s| s.get_user_role_id(user_id)).cloned()
    }
}

impl Default for QueryEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core_store::model::ChannelType;

    #[test]
    fn test_query_engine_creation() {
        let engine = QueryEngine::new();
        assert_eq!(engine.list_spaces().len(), 0);
    }

    #[test]
    fn test_add_and_list_spaces() {
        let mut engine = QueryEngine::new();

        let space = Space::new(
            SpaceId::generate(),
            "Test Space".to_string(),
            UserId::generate(),
            Timestamp::now(),
            "node1".to_string(),
        );

        let space_id = space.id.clone();
        engine.add_space(space);

        let spaces = engine.list_spaces();
        assert_eq!(spaces.len(), 1);
        assert_eq!(spaces[0].name, "Test Space");

        assert!(engine.get_space(&space_id).is_some());
    }

    #[test]
    fn test_list_channels_in_space() {
        let mut engine = QueryEngine::new();

        let space_id = SpaceId::generate();
        let space = Space::new(
            space_id.clone(),
            "Test Space".to_string(),
            UserId::generate(),
            Timestamp::now(),
            "node1".to_string(),
        );

        engine.add_space(space);

        let channel = Channel::new(
            ChannelId::generate(),
            "general".to_string(),
            ChannelType::Text,
            UserId::generate(),
            Timestamp::now(),
            "node1".to_string(),
        );

        engine.add_channel(channel);

        let channels = engine.list_channels_in_space(&space_id);
        // Note: channels list will be empty because we haven't linked them via space.channels
        assert_eq!(channels.len(), 0);
    }

    #[test]
    fn test_list_messages() {
        let mut engine = QueryEngine::new();

        let channel_id = ChannelId::generate();
        let messages = vec![Message::new(
            MessageId::generate(),
            channel_id.clone(),
            UserId::generate(),
            b"Hello".to_vec(),
            Timestamp::now(),
        )];

        engine.add_messages(channel_id.clone(), messages);

        let result = engine.list_messages(&channel_id, None, None);
        assert_eq!(result.len(), 1);
    }
}
