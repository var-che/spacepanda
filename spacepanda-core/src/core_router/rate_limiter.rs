/*
    Rate Limiter - Per-peer rate limiting with token bucket algorithm

    Prevents DoS attacks from malicious peers flooding the system with requests.
    Uses token bucket for smooth rate limiting and circuit breaker for failing peers.
*/

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tracing::{debug, info, trace, warn};

use super::metrics;
use super::session_manager::PeerId;

/// Configuration for rate limiting
#[derive(Debug, Clone)]
pub struct RateLimiterConfig {
    /// Maximum requests per second per peer (token refill rate)
    pub max_requests_per_sec: u32,
    /// Maximum burst size (bucket capacity)
    pub burst_size: u32,
    /// Circuit breaker: consecutive failures before opening circuit
    pub circuit_breaker_threshold: u32,
    /// Circuit breaker: time to wait before attempting recovery (half-open state)
    pub circuit_breaker_timeout: Duration,
}

impl Default for RateLimiterConfig {
    fn default() -> Self {
        RateLimiterConfig {
            max_requests_per_sec: 100,     // 100 req/s sustained
            burst_size: 200,               // Allow bursts up to 200
            circuit_breaker_threshold: 10, // 10 consecutive failures
            circuit_breaker_timeout: Duration::from_secs(30),
        }
    }
}

/// Token bucket for a single peer
#[derive(Debug)]
struct TokenBucket {
    /// Current number of tokens
    tokens: f64,
    /// Maximum tokens (bucket capacity)
    capacity: f64,
    /// Token refill rate (tokens per second)
    refill_rate: f64,
    /// Last time tokens were refilled
    last_refill: Instant,
}

impl TokenBucket {
    fn new(capacity: u32, refill_rate: u32) -> Self {
        TokenBucket {
            tokens: capacity as f64, // Start with full bucket
            capacity: capacity as f64,
            refill_rate: refill_rate as f64,
            last_refill: Instant::now(),
        }
    }

    /// Refill tokens based on elapsed time
    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();

        // Add tokens based on refill rate and elapsed time
        let new_tokens = elapsed * self.refill_rate;
        self.tokens = (self.tokens + new_tokens).min(self.capacity);
        self.last_refill = now;
    }

    /// Try to consume tokens. Returns true if successful, false if insufficient tokens.
    fn try_consume(&mut self, tokens: f64) -> bool {
        self.refill();

        if self.tokens >= tokens {
            self.tokens -= tokens;
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

/// Circuit breaker states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CircuitState {
    /// Circuit is closed, requests are allowed
    Closed,
    /// Circuit is open, requests are blocked
    Open,
    /// Circuit is half-open, testing if peer has recovered
    HalfOpen,
}

/// Circuit breaker for a single peer
#[derive(Debug)]
struct CircuitBreaker {
    /// Current state
    state: CircuitState,
    /// Consecutive failures in current state
    consecutive_failures: u32,
    /// Threshold for opening circuit
    failure_threshold: u32,
    /// When circuit opened (for timeout calculation)
    opened_at: Option<Instant>,
    /// Timeout before attempting recovery
    timeout: Duration,
}

impl CircuitBreaker {
    fn new(failure_threshold: u32, timeout: Duration) -> Self {
        CircuitBreaker {
            state: CircuitState::Closed,
            consecutive_failures: 0,
            failure_threshold,
            opened_at: None,
            timeout,
        }
    }

    /// Check if request should be allowed
    fn allow_request(&mut self) -> bool {
        match self.state {
            CircuitState::Closed => true,
            CircuitState::HalfOpen => {
                trace!("Circuit breaker in half-open state, allowing test request");
                true // Allow test request
            }
            CircuitState::Open => {
                // Check if timeout has elapsed
                if let Some(opened_at) = self.opened_at {
                    if opened_at.elapsed() >= self.timeout {
                        // Transition to half-open for testing
                        info!(
                            consecutive_failures = self.consecutive_failures,
                            timeout_secs = self.timeout.as_secs(),
                            "Circuit breaker transitioning from OPEN to HALF-OPEN for recovery test"
                        );
                        metrics::circuit_breaker_transition("open_to_halfopen");
                        self.state = CircuitState::HalfOpen;
                        self.consecutive_failures = 0;
                        true
                    } else {
                        trace!("Circuit breaker open, blocking request");
                        false // Still in timeout
                    }
                } else {
                    false
                }
            }
        }
    }

    /// Record a successful request
    fn record_success(&mut self) {
        match self.state {
            CircuitState::Closed => {
                // Reset failure count on success
                if self.consecutive_failures > 0 {
                    debug!(
                        previous_failures = self.consecutive_failures,
                        "Circuit breaker: success, resetting failure count"
                    );
                    self.consecutive_failures = 0;
                }
            }
            CircuitState::HalfOpen => {
                // Success in half-open state -> close circuit
                info!(
                    "Circuit breaker: recovery successful, transitioning from HALF-OPEN to CLOSED"
                );
                metrics::circuit_breaker_transition("halfopen_to_closed");
                self.state = CircuitState::Closed;
                self.consecutive_failures = 0;
                self.opened_at = None;
            }
            CircuitState::Open => {
                // Should not happen (requests blocked in open state)
            }
        }
    }

    /// Record a failed request
    fn record_failure(&mut self) {
        self.consecutive_failures += 1;

        match self.state {
            CircuitState::Closed => {
                if self.consecutive_failures >= self.failure_threshold {
                    // Open circuit
                    warn!(
                        consecutive_failures = self.consecutive_failures,
                        threshold = self.failure_threshold,
                        timeout_secs = self.timeout.as_secs(),
                        "Circuit breaker OPENING: failure threshold reached"
                    );
                    metrics::circuit_breaker_transition("closed_to_open");
                    self.state = CircuitState::Open;
                    self.opened_at = Some(Instant::now());
                } else {
                    debug!(
                        consecutive_failures = self.consecutive_failures,
                        threshold = self.failure_threshold,
                        "Circuit breaker: failure recorded, still closed"
                    );
                }
            }
            CircuitState::HalfOpen => {
                // Failure in half-open state -> reopen circuit
                warn!(
                    consecutive_failures = self.consecutive_failures,
                    "Circuit breaker: recovery failed, reopening circuit"
                );
                metrics::circuit_breaker_transition("halfopen_to_open");
                self.state = CircuitState::Open;
                self.opened_at = Some(Instant::now());
            }
            CircuitState::Open => {
                // Already open, just track failures
                trace!(
                    consecutive_failures = self.consecutive_failures,
                    "Circuit breaker: additional failure while open"
                );
            }
        }
    }

    /// Get current state
    fn state(&self) -> CircuitState {
        self.state
    }
}

/// Per-peer rate limiter with circuit breaker
#[derive(Debug)]
struct PeerLimiter {
    token_bucket: TokenBucket,
    circuit_breaker: CircuitBreaker,
}

impl PeerLimiter {
    fn new(config: &RateLimiterConfig) -> Self {
        PeerLimiter {
            token_bucket: TokenBucket::new(config.burst_size, config.max_requests_per_sec),
            circuit_breaker: CircuitBreaker::new(
                config.circuit_breaker_threshold,
                config.circuit_breaker_timeout,
            ),
        }
    }
}

/// Rate limiter managing multiple peers
pub struct RateLimiter {
    config: RateLimiterConfig,
    limiters: Arc<Mutex<HashMap<PeerId, PeerLimiter>>>,
}

/// Result of rate limit check
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RateLimitResult {
    /// Request allowed
    Allowed,
    /// Request blocked: rate limit exceeded
    RateLimitExceeded,
    /// Request blocked: circuit breaker open
    CircuitBreakerOpen,
}

impl RateLimiter {
    /// Create a new rate limiter with default config
    pub fn new() -> Self {
        Self::new_with_config(RateLimiterConfig::default())
    }

    /// Create a new rate limiter with custom config
    pub fn new_with_config(config: RateLimiterConfig) -> Self {
        RateLimiter { config, limiters: Arc::new(Mutex::new(HashMap::new())) }
    }

    /// Check if a request from a peer should be allowed
    pub async fn check_request(&self, peer_id: &PeerId) -> RateLimitResult {
        let mut limiters = self.limiters.lock().await;

        // Get or create limiter for this peer
        let limiter = limiters.entry(peer_id.clone()).or_insert_with(|| {
            debug!(peer_id = ?peer_id, "Creating new rate limiter for peer");
            PeerLimiter::new(&self.config)
        });

        // Check circuit breaker first
        if !limiter.circuit_breaker.allow_request() {
            warn!(
                peer_id = ?peer_id,
                state = ?limiter.circuit_breaker.state(),
                "Request blocked: circuit breaker open"
            );
            return RateLimitResult::CircuitBreakerOpen;
        }

        // Check rate limit (consume 1 token)
        let tokens_before = limiter.token_bucket.available_tokens();
        if limiter.token_bucket.try_consume(1.0) {
            trace!(
                peer_id = ?peer_id,
                tokens_remaining = limiter.token_bucket.available_tokens(),
                "Request allowed"
            );
            RateLimitResult::Allowed
        } else {
            warn!(
                peer_id = ?peer_id,
                tokens_available = tokens_before,
                capacity = self.config.burst_size,
                refill_rate = self.config.max_requests_per_sec,
                "Request blocked: rate limit exceeded"
            );
            RateLimitResult::RateLimitExceeded
        }
    }

    /// Record a successful request (for circuit breaker)
    pub async fn record_success(&self, peer_id: &PeerId) {
        let mut limiters = self.limiters.lock().await;

        if let Some(limiter) = limiters.get_mut(peer_id) {
            limiter.circuit_breaker.record_success();
        }
    }

    /// Record a failed request (for circuit breaker)
    pub async fn record_failure(&self, peer_id: &PeerId) {
        let mut limiters = self.limiters.lock().await;

        // Get or create limiter for this peer
        let limiter = limiters
            .entry(peer_id.clone())
            .or_insert_with(|| PeerLimiter::new(&self.config));

        limiter.circuit_breaker.record_failure();
    }

    /// Get circuit breaker state for a peer (for testing/monitoring)
    pub async fn get_circuit_state(&self, peer_id: &PeerId) -> Option<CircuitState> {
        let limiters = self.limiters.lock().await;
        limiters.get(peer_id).map(|l| l.circuit_breaker.state())
    }

    /// Get available tokens for a peer (for testing/monitoring)
    pub async fn get_available_tokens(&self, peer_id: &PeerId) -> Option<f64> {
        let mut limiters = self.limiters.lock().await;
        limiters.get_mut(peer_id).map(|l| l.token_bucket.available_tokens())
    }

    /// Remove limiter for a peer (e.g., when peer disconnects)
    pub async fn remove_peer(&self, peer_id: &PeerId) {
        let mut limiters = self.limiters.lock().await;
        limiters.remove(peer_id);
    }

    /// Get number of tracked peers (for testing)
    #[cfg(test)]
    pub async fn peer_count(&self) -> usize {
        self.limiters.lock().await.len()
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_peer_id(id: u8) -> PeerId {
        PeerId(vec![id; 32])
    }

    #[tokio::test]
    async fn test_rate_limiter_allows_within_limit() {
        let config = RateLimiterConfig {
            max_requests_per_sec: 100,
            burst_size: 10,
            circuit_breaker_threshold: 5,
            circuit_breaker_timeout: Duration::from_secs(1),
        };
        let limiter = RateLimiter::new_with_config(config);
        let peer = test_peer_id(1);

        // Should allow up to burst_size requests
        for _ in 0..10 {
            assert_eq!(limiter.check_request(&peer).await, RateLimitResult::Allowed);
        }

        // 11th request should be rate limited
        assert_eq!(limiter.check_request(&peer).await, RateLimitResult::RateLimitExceeded);
    }

    #[tokio::test]
    async fn test_rate_limiter_refills_tokens() {
        let config = RateLimiterConfig {
            max_requests_per_sec: 10, // 10 tokens/sec
            burst_size: 5,
            circuit_breaker_threshold: 5,
            circuit_breaker_timeout: Duration::from_secs(1),
        };
        let limiter = RateLimiter::new_with_config(config);
        let peer = test_peer_id(1);

        // Consume all tokens
        for _ in 0..5 {
            assert_eq!(limiter.check_request(&peer).await, RateLimitResult::Allowed);
        }
        assert_eq!(limiter.check_request(&peer).await, RateLimitResult::RateLimitExceeded);

        // Wait for refill (100ms = 1 token at 10 tokens/sec)
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Should have ~1 token now
        assert_eq!(limiter.check_request(&peer).await, RateLimitResult::Allowed);
    }

    #[tokio::test]
    async fn test_circuit_breaker_opens_on_failures() {
        let config = RateLimiterConfig {
            max_requests_per_sec: 100,
            burst_size: 100,
            circuit_breaker_threshold: 3, // Open after 3 failures
            circuit_breaker_timeout: Duration::from_secs(10),
        };
        let limiter = RateLimiter::new_with_config(config);
        let peer = test_peer_id(1);

        // First request allowed
        assert_eq!(limiter.check_request(&peer).await, RateLimitResult::Allowed);

        // Record 3 failures
        for _ in 0..3 {
            limiter.record_failure(&peer).await;
        }

        // Circuit should be open now
        assert_eq!(limiter.check_request(&peer).await, RateLimitResult::CircuitBreakerOpen);
        assert_eq!(limiter.get_circuit_state(&peer).await, Some(CircuitState::Open));
    }

    #[tokio::test]
    async fn test_circuit_breaker_half_open_recovery() {
        let config = RateLimiterConfig {
            max_requests_per_sec: 100,
            burst_size: 100,
            circuit_breaker_threshold: 2,
            circuit_breaker_timeout: Duration::from_millis(100), // Short timeout
        };
        let limiter = RateLimiter::new_with_config(config);
        let peer = test_peer_id(1);

        // Open circuit
        limiter.record_failure(&peer).await;
        limiter.record_failure(&peer).await;
        assert_eq!(limiter.check_request(&peer).await, RateLimitResult::CircuitBreakerOpen);

        // Wait for timeout
        tokio::time::sleep(Duration::from_millis(150)).await;

        // Should transition to half-open and allow test request
        assert_eq!(limiter.check_request(&peer).await, RateLimitResult::Allowed);
        assert_eq!(limiter.get_circuit_state(&peer).await, Some(CircuitState::HalfOpen));

        // Record success -> circuit closes
        limiter.record_success(&peer).await;
        assert_eq!(limiter.get_circuit_state(&peer).await, Some(CircuitState::Closed));
    }

    #[tokio::test]
    async fn test_circuit_breaker_reopens_on_half_open_failure() {
        let config = RateLimiterConfig {
            max_requests_per_sec: 100,
            burst_size: 100,
            circuit_breaker_threshold: 2,
            circuit_breaker_timeout: Duration::from_millis(100),
        };
        let limiter = RateLimiter::new_with_config(config);
        let peer = test_peer_id(1);

        // Open circuit
        limiter.record_failure(&peer).await;
        limiter.record_failure(&peer).await;

        // Wait for timeout -> half-open
        tokio::time::sleep(Duration::from_millis(150)).await;
        assert_eq!(limiter.check_request(&peer).await, RateLimitResult::Allowed);

        // Record failure in half-open state -> reopens
        limiter.record_failure(&peer).await;
        assert_eq!(limiter.get_circuit_state(&peer).await, Some(CircuitState::Open));
    }

    #[tokio::test]
    async fn test_different_peers_independent_limits() {
        let config = RateLimiterConfig {
            max_requests_per_sec: 100,
            burst_size: 5,
            circuit_breaker_threshold: 10,
            circuit_breaker_timeout: Duration::from_secs(1),
        };
        let limiter = RateLimiter::new_with_config(config);
        let peer1 = test_peer_id(1);
        let peer2 = test_peer_id(2);

        // Exhaust peer1's tokens
        for _ in 0..5 {
            assert_eq!(limiter.check_request(&peer1).await, RateLimitResult::Allowed);
        }
        assert_eq!(limiter.check_request(&peer1).await, RateLimitResult::RateLimitExceeded);

        // Peer2 should still have tokens
        for _ in 0..5 {
            assert_eq!(limiter.check_request(&peer2).await, RateLimitResult::Allowed);
        }
    }

    #[tokio::test]
    async fn test_remove_peer() {
        let limiter = RateLimiter::new();
        let peer = test_peer_id(1);

        // Make request to create limiter entry
        limiter.check_request(&peer).await;
        assert_eq!(limiter.peer_count().await, 1);

        // Remove peer
        limiter.remove_peer(&peer).await;
        assert_eq!(limiter.peer_count().await, 0);
    }

    #[tokio::test]
    async fn test_success_resets_failure_count() {
        let config = RateLimiterConfig {
            max_requests_per_sec: 100,
            burst_size: 100,
            circuit_breaker_threshold: 3,
            circuit_breaker_timeout: Duration::from_secs(10),
        };
        let limiter = RateLimiter::new_with_config(config);
        let peer = test_peer_id(1);

        // Record 2 failures (below threshold)
        limiter.record_failure(&peer).await;
        limiter.record_failure(&peer).await;

        // Record success (should reset count)
        limiter.record_success(&peer).await;

        // Record 2 more failures (should not open circuit)
        limiter.record_failure(&peer).await;
        limiter.record_failure(&peer).await;

        assert_eq!(limiter.get_circuit_state(&peer).await, Some(CircuitState::Closed));
    }

    #[tokio::test]
    async fn test_token_bucket_capacity_bounds() {
        let config = RateLimiterConfig {
            max_requests_per_sec: 1000, // Fast refill
            burst_size: 10,
            circuit_breaker_threshold: 10,
            circuit_breaker_timeout: Duration::from_secs(1),
        };
        let limiter = RateLimiter::new_with_config(config);
        let peer = test_peer_id(1);

        // Make initial request to create limiter entry
        assert_eq!(limiter.check_request(&peer).await, RateLimitResult::Allowed);

        // Wait to allow tokens to accumulate
        tokio::time::sleep(Duration::from_secs(1)).await;

        // Should be capped at burst_size (10), not exceed capacity
        let tokens = limiter.get_available_tokens(&peer).await.unwrap();
        assert!(tokens <= 10.0);
        assert!(tokens >= 9.0); // Allow some timing variance (consumed 1 initially)
    }
}
