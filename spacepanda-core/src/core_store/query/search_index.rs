/*
    search_index.rs - Full-text search indexing for messages
    
    Provides search capabilities for encrypted messages once decrypted locally.
    Uses a simple in-memory inverted index for fast text search.
    
    In production, this would integrate with:
    - Local decryption of MLS ciphertext
    - Persistent search index on disk
    - Advanced ranking algorithms (BM25, etc.)
*/

use crate::core_store::model::{MessageId, ChannelId, UserId, Timestamp};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// A token extracted from message content
type Token = String;

/// Search result with relevance score
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub message_id: MessageId,
    pub channel_id: ChannelId,
    pub sender: UserId,
    pub timestamp: Timestamp,
    pub score: f64,
    pub snippet: String,
}

/// Indexed message metadata
#[derive(Debug, Clone)]
struct IndexedMessage {
    message_id: MessageId,
    channel_id: ChannelId,
    sender: UserId,
    timestamp: Timestamp,
    plaintext: String,
    tokens: HashSet<Token>,
}

/// In-memory search index
pub struct SearchIndex {
    /// Inverted index: token -> set of message IDs
    index: HashMap<Token, HashSet<MessageId>>,
    
    /// Message metadata by ID
    messages: HashMap<MessageId, IndexedMessage>,
    
    /// Channel-specific indices for faster scoped search
    channel_indices: HashMap<ChannelId, HashSet<MessageId>>,
}

impl SearchIndex {
    pub fn new() -> Self {
        SearchIndex {
            index: HashMap::new(),
            messages: HashMap::new(),
            channel_indices: HashMap::new(),
        }
    }
    
    /// Index a message after decryption
    pub fn index_message(
        &mut self,
        message_id: MessageId,
        channel_id: ChannelId,
        sender: UserId,
        timestamp: Timestamp,
        plaintext: String,
    ) {
        let tokens = Self::tokenize(&plaintext);
        
        // Add to inverted index
        for token in &tokens {
            self.index.entry(token.clone())
                .or_insert_with(HashSet::new)
                .insert(message_id.clone());
        }
        
        // Add to channel index
        self.channel_indices.entry(channel_id.clone())
            .or_insert_with(HashSet::new)
            .insert(message_id.clone());
        
        // Store message metadata
        let indexed_msg = IndexedMessage {
            message_id: message_id.clone(),
            channel_id,
            sender,
            timestamp,
            plaintext,
            tokens,
        };
        
        self.messages.insert(message_id, indexed_msg);
    }
    
    /// Remove a message from the index (e.g., when deleted)
    pub fn remove_message(&mut self, message_id: &MessageId) {
        if let Some(msg) = self.messages.remove(message_id) {
            // Remove from inverted index
            for token in &msg.tokens {
                if let Some(msg_set) = self.index.get_mut(token) {
                    msg_set.remove(message_id);
                    if msg_set.is_empty() {
                        self.index.remove(token);
                    }
                }
            }
            
            // Remove from channel index
            if let Some(channel_msgs) = self.channel_indices.get_mut(&msg.channel_id) {
                channel_msgs.remove(message_id);
            }
        }
    }
    
    /// Search all messages
    pub fn search(&self, query: &str, limit: usize) -> Vec<SearchResult> {
        let query_tokens = Self::tokenize(query);
        if query_tokens.is_empty() {
            return Vec::new();
        }
        
        // Find messages matching any query token
        let mut candidates: HashMap<MessageId, usize> = HashMap::new();
        
        for token in &query_tokens {
            if let Some(msg_ids) = self.index.get(token) {
                for msg_id in msg_ids {
                    *candidates.entry(msg_id.clone()).or_insert(0) += 1;
                }
            }
        }
        
        // Score and rank results
        self.rank_results(candidates, &query_tokens, limit)
    }
    
    /// Search within a specific channel
    pub fn search_in_channel(
        &self,
        channel_id: &ChannelId,
        query: &str,
        limit: usize,
    ) -> Vec<SearchResult> {
        let query_tokens = Self::tokenize(query);
        if query_tokens.is_empty() {
            return Vec::new();
        }
        
        // Get messages in this channel
        let channel_msgs = match self.channel_indices.get(channel_id) {
            Some(msgs) => msgs,
            None => return Vec::new(),
        };
        
        // Find matching messages
        let mut candidates: HashMap<MessageId, usize> = HashMap::new();
        
        for token in &query_tokens {
            if let Some(msg_ids) = self.index.get(token) {
                for msg_id in msg_ids {
                    if channel_msgs.contains(msg_id) {
                        *candidates.entry(msg_id.clone()).or_insert(0) += 1;
                    }
                }
            }
        }
        
        self.rank_results(candidates, &query_tokens, limit)
    }
    
    /// Search messages by a specific sender
    pub fn search_by_sender(
        &self,
        sender: &UserId,
        query: &str,
        limit: usize,
    ) -> Vec<SearchResult> {
        let query_tokens = Self::tokenize(query);
        if query_tokens.is_empty() {
            return Vec::new();
        }
        
        // Find matching messages from this sender
        let mut candidates: HashMap<MessageId, usize> = HashMap::new();
        
        for token in &query_tokens {
            if let Some(msg_ids) = self.index.get(token) {
                for msg_id in msg_ids {
                    if let Some(msg) = self.messages.get(msg_id) {
                        if &msg.sender == sender {
                            *candidates.entry(msg_id.clone()).or_insert(0) += 1;
                        }
                    }
                }
            }
        }
        
        self.rank_results(candidates, &query_tokens, limit)
    }
    
    /// Rank and format search results
    fn rank_results(
        &self,
        candidates: HashMap<MessageId, usize>,
        query_tokens: &HashSet<Token>,
        limit: usize,
    ) -> Vec<SearchResult> {
        let mut results: Vec<SearchResult> = candidates.iter()
            .filter_map(|(msg_id, match_count)| {
                self.messages.get(msg_id).map(|msg| {
                    // Simple scoring: number of matching tokens / total query tokens
                    let score = (*match_count as f64) / (query_tokens.len() as f64);
                    
                    // Generate snippet
                    let snippet = Self::generate_snippet(&msg.plaintext, query_tokens);
                    
                    SearchResult {
                        message_id: msg_id.clone(),
                        channel_id: msg.channel_id.clone(),
                        sender: msg.sender.clone(),
                        timestamp: msg.timestamp,
                        score,
                        snippet,
                    }
                })
            })
            .collect();
        
        // Sort by score descending, then timestamp descending
        results.sort_by(|a, b| {
            b.score.partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(b.timestamp.cmp(&a.timestamp))
        });
        
        results.truncate(limit);
        results
    }
    
    /// Tokenize text into search tokens
    fn tokenize(text: &str) -> HashSet<Token> {
        text.to_lowercase()
            .split_whitespace()
            .map(|word| {
                // Remove punctuation from edges
                word.trim_matches(|c: char| !c.is_alphanumeric())
                    .to_string()
            })
            .filter(|token| !token.is_empty() && token.len() > 1)
            .collect()
    }
    
    /// Generate a snippet showing query matches in context
    fn generate_snippet(text: &str, query_tokens: &HashSet<Token>) -> String {
        const SNIPPET_LENGTH: usize = 150;
        
        if text.len() <= SNIPPET_LENGTH {
            return text.to_string();
        }
        
        // Find first matching position
        let text_lower = text.to_lowercase();
        let mut first_match = None;
        
        for token in query_tokens {
            if let Some(pos) = text_lower.find(token) {
                if first_match.is_none() || pos < first_match.unwrap() {
                    first_match = Some(pos);
                }
            }
        }
        
        if let Some(pos) = first_match {
            // Extract context around match
            let start = pos.saturating_sub(50);
            let end = (pos + 100).min(text.len());
            
            let mut snippet = text[start..end].to_string();
            
            if start > 0 {
                snippet = format!("...{}", snippet);
            }
            if end < text.len() {
                snippet = format!("{}...", snippet);
            }
            
            snippet
        } else {
            // No match found, return beginning
            format!("{}...", &text[..SNIPPET_LENGTH.min(text.len())])
        }
    }
    
    /// Get statistics about the index
    pub fn stats(&self) -> IndexStats {
        IndexStats {
            total_messages: self.messages.len(),
            total_tokens: self.index.len(),
            indexed_channels: self.channel_indices.len(),
        }
    }
    
    /// Clear the entire index
    pub fn clear(&mut self) {
        self.index.clear();
        self.messages.clear();
        self.channel_indices.clear();
    }
}

impl Default for SearchIndex {
    fn default() -> Self {
        Self::new()
    }
}

/// Index statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexStats {
    pub total_messages: usize,
    pub total_tokens: usize,
    pub indexed_channels: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_tokenize() {
        let tokens = SearchIndex::tokenize("Hello, world! This is a test.");
        assert!(tokens.contains("hello"));
        assert!(tokens.contains("world"));
        assert!(tokens.contains("this"));
        assert!(tokens.contains("test"));
        assert!(!tokens.contains("a")); // Single char filtered out
    }
    
    #[test]
    fn test_index_and_search() {
        let mut index = SearchIndex::new();
        
        let msg_id = MessageId::generate();
        let channel_id = ChannelId::generate();
        let sender = UserId::generate();
        let timestamp = Timestamp::now();
        
        index.index_message(
            msg_id.clone(),
            channel_id.clone(),
            sender.clone(),
            timestamp,
            "Hello world, this is a test message".to_string(),
        );
        
        let results = index.search("hello", 10);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].message_id, msg_id);
        
        let results = index.search("test", 10);
        assert_eq!(results.len(), 1);
        
        let results = index.search("nonexistent", 10);
        assert_eq!(results.len(), 0);
    }
    
    #[test]
    fn test_search_in_channel() {
        let mut index = SearchIndex::new();
        
        let channel1 = ChannelId::generate();
        let channel2 = ChannelId::generate();
        let sender = UserId::generate();
        
        index.index_message(
            MessageId::generate(),
            channel1.clone(),
            sender.clone(),
            Timestamp::now(),
            "Message in channel 1".to_string(),
        );
        
        index.index_message(
            MessageId::generate(),
            channel2.clone(),
            sender.clone(),
            Timestamp::now(),
            "Message in channel 2".to_string(),
        );
        
        let results = index.search_in_channel(&channel1, "message", 10);
        assert_eq!(results.len(), 1);
        
        let results = index.search_in_channel(&channel2, "message", 10);
        assert_eq!(results.len(), 1);
    }
    
    #[test]
    fn test_remove_message() {
        let mut index = SearchIndex::new();
        
        let msg_id = MessageId::generate();
        let channel_id = ChannelId::generate();
        let sender = UserId::generate();
        
        index.index_message(
            msg_id.clone(),
            channel_id,
            sender,
            Timestamp::now(),
            "Test message".to_string(),
        );
        
        let results = index.search("test", 10);
        assert_eq!(results.len(), 1);
        
        index.remove_message(&msg_id);
        
        let results = index.search("test", 10);
        assert_eq!(results.len(), 0);
    }
    
    #[test]
    fn test_ranking() {
        let mut index = SearchIndex::new();
        let channel_id = ChannelId::generate();
        let sender = UserId::generate();
        
        // Add messages with different relevance
        index.index_message(
            MessageId::generate(),
            channel_id.clone(),
            sender.clone(),
            Timestamp::now(),
            "rust programming".to_string(),
        );
        
        std::thread::sleep(std::time::Duration::from_millis(2));
        
        index.index_message(
            MessageId::generate(),
            channel_id.clone(),
            sender.clone(),
            Timestamp::now(),
            "rust is great for programming systems".to_string(),
        );
        
        let results = index.search("rust programming", 10);
        assert_eq!(results.len(), 2);
        
        // Second message should rank higher (matches both tokens)
        assert!(results[0].score >= results[1].score);
    }
    
    #[test]
    fn test_stats() {
        let mut index = SearchIndex::new();
        let channel_id = ChannelId::generate();
        let sender = UserId::generate();
        
        index.index_message(
            MessageId::generate(),
            channel_id.clone(),
            sender.clone(),
            Timestamp::now(),
            "First message".to_string(),
        );
        
        index.index_message(
            MessageId::generate(),
            channel_id.clone(),
            sender,
            Timestamp::now(),
            "Second message".to_string(),
        );
        
        let stats = index.stats();
        assert_eq!(stats.total_messages, 2);
        assert!(stats.total_tokens > 0);
        assert_eq!(stats.indexed_channels, 1);
    }
}
