//! Rate Limiting for MLS Operations
//!
//! Provides per-peer rate limiting and bounded replay prevention caches
//! to protect against DoS attacks and resource exhaustion.
//!
//! # Features
//!
//! - **Per-Peer Rate Limiting**: Token bucket algorithm per identity
//! - **Bounded Replay Cache**: LRU cache with configurable capacity
//! - **Time-Based Windows**: Automatic token refill over time
//! - **Thread-Safe**: Uses Arc<RwLock<>> for concurrent access

use crate::core_mls::errors::{MlsError, MlsResult};
use hashlink::LruCache;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Rate limit configuration
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Maximum requests per peer within the time window
    pub max_requests_per_peer: usize,
    
    /// Time window for rate limiting (in seconds)
    pub window_secs: u64,
    
    /// Maximum capacity for replay prevention cache
    pub replay_cache_capacity: usize,
    
    /// Token refill rate (tokens per second)
    pub refill_rate: f64,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            max_requests_per_peer: 100,  // 100 requests
            window_secs: 60,              // per minute
            replay_cache_capacity: 10_000, // 10k cached message IDs
            refill_rate: 100.0 / 60.0,    // ~1.67 tokens/sec
        }
    }
}

/// Token bucket for rate limiting a single peer
#[derive(Debug, Clone)]
struct TokenBucket {
    /// Current number of tokens available
    tokens: f64,
    
    /// Maximum tokens (bucket capacity)
    capacity: f64,
    
    /// Last time tokens were refilled
    last_refill: Instant,
    
    /// Refill rate (tokens per second)
    refill_rate: f64,
}

impl TokenBucket {
    fn new(capacity: usize, refill_rate: f64) -> Self {
        Self {
            tokens: capacity as f64,
            capacity: capacity as f64,
            last_refill: Instant::now(),
            refill_rate,
        }
    }
    
    /// Refill tokens based on elapsed time
    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        
        // Add tokens based on elapsed time and refill rate
        self.tokens = (self.tokens + (elapsed * self.refill_rate)).min(self.capacity);
        self.last_refill = now;
    }
    
    /// Try to consume one token
    /// Returns true if token was available and consumed
    fn try_consume(&mut self) -> bool {
        self.refill();
        
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
    
    /// Get current token count (after refill)
    fn available_tokens(&mut self) -> f64 {
        self.refill();
        self.tokens
    }
}

/// Per-peer rate limiter using token bucket algorithm
pub struct RateLimiter {
    /// Rate limit configuration
    config: RateLimitConfig,
    
    /// Token buckets per peer identity (hash)
    buckets: Arc<RwLock<HashMap<u64, TokenBucket>>>,
    
    /// Replay prevention cache (bounded LRU)
    /// Maps message hash -> timestamp
    replay_cache: Arc<RwLock<LruCache<u64, Instant>>>,
}

impl RateLimiter {
    /// Create a new rate limiter with the given configuration
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            buckets: Arc::new(RwLock::new(HashMap::new())),
            replay_cache: Arc::new(RwLock::new(LruCache::new(config.replay_cache_capacity))),
            config,
        }
    }
    
    /// Create a rate limiter with default configuration
    pub fn default() -> Self {
        Self::new(RateLimitConfig::default())
    }
    
    /// Check if a request from the given peer should be allowed
    ///
    /// # Arguments
    /// * `peer_identity` - Unique identifier for the peer (e.g., user ID, public key)
    ///
    /// # Returns
    /// * `Ok(())` - Request is allowed
    /// * `Err(MlsError::RateLimitExceeded)` - Request should be rejected
    pub async fn check_rate_limit(&self, peer_identity: &[u8]) -> MlsResult<()> {
        let peer_hash = Self::hash_identity(peer_identity);
        
        let mut buckets = self.buckets.write().await;
        
        // Get or create token bucket for this peer
        let bucket = buckets.entry(peer_hash).or_insert_with(|| {
            TokenBucket::new(
                self.config.max_requests_per_peer,
                self.config.refill_rate,
            )
        });
        
        // Try to consume a token
        if bucket.try_consume() {
            Ok(())
        } else {
            Err(MlsError::RateLimitExceeded(format!(
                "Rate limit exceeded for peer (available: {:.2} tokens)",
                bucket.available_tokens()
            )))
        }
    }
    
    /// Check if a message has been seen before (replay detection)
    ///
    /// # Arguments
    /// * `message_bytes` - The message to check
    ///
    /// # Returns
    /// * `Ok(())` - Message is new
    /// * `Err(MlsError::ReplayDetected)` - Message has been seen before
    pub async fn check_replay(&self, message_bytes: &[u8]) -> MlsResult<()> {
        let message_hash = Self::hash_message(message_bytes);
        
        let mut cache = self.replay_cache.write().await;
        
        if cache.contains_key(&message_hash) {
            return Err(MlsError::ReplayDetected(
                "Message has already been processed".to_string()
            ));
        }
        
        // Add to cache with current timestamp
        cache.insert(message_hash, Instant::now());
        
        Ok(())
    }
    
    /// Combined check: rate limit + replay detection
    ///
    /// This is the recommended method for validating incoming messages.
    pub async fn validate_request(
        &self,
        peer_identity: &[u8],
        message_bytes: &[u8],
    ) -> MlsResult<()> {
        // Check rate limit first (cheaper operation)
        self.check_rate_limit(peer_identity).await?;
        
        // Then check for replay
        self.check_replay(message_bytes).await?;
        
        Ok(())
    }
    
    /// Manually evict entries older than the specified duration
    /// This is automatically handled by LRU, but can be called for cleanup
    pub async fn evict_old_entries(&self, max_age: Duration) {
        let mut cache = self.replay_cache.write().await;
        let now = Instant::now();
        
        // Collect keys to remove
        let mut to_remove = Vec::new();
        for (key, timestamp) in cache.iter() {
            if now.duration_since(*timestamp) > max_age {
                to_remove.push(*key);
            }
        }
        
        // Remove old entries
        for key in to_remove {
            cache.remove(&key);
        }
    }
    
    /// Get current statistics
    pub async fn stats(&self) -> RateLimitStats {
        let buckets = self.buckets.read().await;
        let cache = self.replay_cache.read().await;
        
        RateLimitStats {
            active_peers: buckets.len(),
            cached_messages: cache.len(),
            cache_capacity: self.config.replay_cache_capacity,
        }
    }
    
    /// Hash identity to u64 for HashMap key
    fn hash_identity(identity: &[u8]) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        identity.hash(&mut hasher);
        hasher.finish()
    }
    
    /// Hash message to u64 for replay cache key
    fn hash_message(message: &[u8]) -> u64 {
        use blake3::Hasher;
        
        let hash = Hasher::new().update(message).finalize();
        let hash_bytes = hash.as_bytes();
        
        // Use first 8 bytes as u64
        u64::from_le_bytes([
            hash_bytes[0], hash_bytes[1], hash_bytes[2], hash_bytes[3],
            hash_bytes[4], hash_bytes[5], hash_bytes[6], hash_bytes[7],
        ])
    }
}

/// Rate limiter statistics
#[derive(Debug, Clone)]
pub struct RateLimitStats {
    /// Number of peers currently being tracked
    pub active_peers: usize,
    
    /// Number of messages in replay cache
    pub cached_messages: usize,
    
    /// Maximum cache capacity
    pub cache_capacity: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_rate_limit_allows_within_capacity() {
        let config = RateLimitConfig {
            max_requests_per_peer: 5,
            window_secs: 60,
            replay_cache_capacity: 100,
            refill_rate: 5.0 / 60.0,
        };
        
        let limiter = RateLimiter::new(config);
        let peer_id = b"alice@example.com";
        
        // Should allow 5 requests
        for _ in 0..5 {
            assert!(limiter.check_rate_limit(peer_id).await.is_ok());
        }
        
        // 6th request should be rate-limited
        assert!(limiter.check_rate_limit(peer_id).await.is_err());
    }
    
    #[tokio::test]
    async fn test_rate_limit_per_peer_isolation() {
        let config = RateLimitConfig {
            max_requests_per_peer: 2,
            window_secs: 60,
            replay_cache_capacity: 100,
            refill_rate: 2.0 / 60.0,
        };
        
        let limiter = RateLimiter::new(config);
        
        // Alice uses her quota
        assert!(limiter.check_rate_limit(b"alice").await.is_ok());
        assert!(limiter.check_rate_limit(b"alice").await.is_ok());
        assert!(limiter.check_rate_limit(b"alice").await.is_err());
        
        // Bob has separate quota
        assert!(limiter.check_rate_limit(b"bob").await.is_ok());
        assert!(limiter.check_rate_limit(b"bob").await.is_ok());
        assert!(limiter.check_rate_limit(b"bob").await.is_err());
    }
    
    #[tokio::test]
    async fn test_replay_detection() {
        let limiter = RateLimiter::default();
        
        let message = b"test message";
        
        // First time should succeed
        assert!(limiter.check_replay(message).await.is_ok());
        
        // Second time should be detected as replay
        assert!(limiter.check_replay(message).await.is_err());
    }
    
    #[tokio::test]
    async fn test_replay_cache_bounded() {
        let config = RateLimitConfig {
            max_requests_per_peer: 1000,
            window_secs: 60,
            replay_cache_capacity: 5,  // Very small cache
            refill_rate: 1000.0,
        };
        
        let limiter = RateLimiter::new(config);
        
        // Add 10 different messages
        for i in 0..10 {
            let message = format!("message {}", i);
            let _ = limiter.check_replay(message.as_bytes()).await;
        }
        
        let stats = limiter.stats().await;
        
        // Cache should be bounded to capacity (5)
        assert!(stats.cached_messages <= 5);
    }
    
    #[tokio::test]
    async fn test_token_bucket_refill() {
        let config = RateLimitConfig {
            max_requests_per_peer: 2,
            window_secs: 1,
            replay_cache_capacity: 100,
            refill_rate: 10.0,  // Fast refill for testing
        };
        
        let limiter = RateLimiter::new(config);
        let peer_id = b"alice";
        
        // Consume all tokens
        assert!(limiter.check_rate_limit(peer_id).await.is_ok());
        assert!(limiter.check_rate_limit(peer_id).await.is_ok());
        assert!(limiter.check_rate_limit(peer_id).await.is_err());
        
        // Wait for refill
        tokio::time::sleep(Duration::from_millis(300)).await;
        
        // Should have refilled some tokens
        assert!(limiter.check_rate_limit(peer_id).await.is_ok());
    }
    
    #[tokio::test]
    async fn test_validate_request_combined() {
        let limiter = RateLimiter::default();
        
        let peer_id = b"alice";
        let message = b"test message";
        
        // First request should succeed
        assert!(limiter.validate_request(peer_id, message).await.is_ok());
        
        // Replay should be rejected
        assert!(limiter.validate_request(peer_id, message).await.is_err());
    }
    
    #[tokio::test]
    async fn test_stats() {
        let limiter = RateLimiter::default();
        
        // Add some requests
        limiter.check_rate_limit(b"alice").await.ok();
        limiter.check_rate_limit(b"bob").await.ok();
        limiter.check_replay(b"message1").await.ok();
        limiter.check_replay(b"message2").await.ok();
        
        let stats = limiter.stats().await;
        
        assert_eq!(stats.active_peers, 2);
        assert_eq!(stats.cached_messages, 2);
        assert_eq!(stats.cache_capacity, 10_000);
    }
}
