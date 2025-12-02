/*
    Metrics - Security and performance metrics for monitoring
    
    Provides counters, histograms, and gauges for:
    - Security events (replay attacks, rate limiting, circuit breaker)
    - Performance metrics (RPC latency, request throughput)
    - System health (peer connections, queue depths)
    
    Metrics can be exported via Prometheus or other backends.
*/

use metrics::{counter, histogram, gauge, describe_counter, describe_histogram, describe_gauge};

/// Initialize metric descriptions (call once at startup)
pub fn init_metrics() {
    // Security Events
    describe_counter!(
        "spacepanda_rpc_requests_total",
        "Total number of RPC requests received, labeled by result (allowed, rate_limited, circuit_breaker_open)"
    );
    
    describe_counter!(
        "spacepanda_replay_attacks_detected_total",
        "Total number of replay attacks detected (duplicate request IDs)"
    );
    
    describe_counter!(
        "spacepanda_oversized_frames_rejected_total",
        "Total number of frames rejected due to exceeding MAX_FRAME_SIZE"
    );
    
    describe_counter!(
        "spacepanda_handshake_replay_detected_total",
        "Total number of handshake replay attempts detected"
    );
    
    describe_counter!(
        "spacepanda_expired_handshakes_rejected_total",
        "Total number of expired handshakes rejected"
    );
    
    describe_counter!(
        "spacepanda_handshake_timeouts_total",
        "Total number of handshakes that timed out"
    );
    
    // Rate Limiting
    describe_counter!(
        "spacepanda_rate_limit_exceeded_total",
        "Total number of requests blocked due to rate limit exceeded"
    );
    
    describe_counter!(
        "spacepanda_circuit_breaker_open_total",
        "Total number of requests blocked due to circuit breaker open"
    );
    
    describe_counter!(
        "spacepanda_circuit_breaker_state_transitions_total",
        "Total number of circuit breaker state transitions, labeled by transition (closed_to_open, open_to_halfopen, halfopen_to_closed, halfopen_to_open)"
    );
    
    // RPC Protocol
    describe_histogram!(
        "spacepanda_rpc_call_duration_seconds",
        "Duration of RPC calls from request to response"
    );
    
    describe_counter!(
        "spacepanda_rpc_calls_total",
        "Total number of outgoing RPC calls, labeled by result (success, timeout, error)"
    );
    
    describe_counter!(
        "spacepanda_rpc_methods_total",
        "Total number of RPC requests by method name"
    );
    
    describe_counter!(
        "spacepanda_rpc_handler_errors_total",
        "Total number of RPC handler errors (method not found, handler crashed)"
    );
    
    // System Health
    describe_gauge!(
        "spacepanda_active_peers",
        "Current number of active peer connections"
    );
    
    describe_gauge!(
        "spacepanda_pending_rpc_requests",
        "Current number of pending RPC requests awaiting response"
    );
    
    describe_gauge!(
        "spacepanda_seen_requests_cache_size",
        "Current size of seen requests cache (for replay detection)"
    );
    
    describe_histogram!(
        "spacepanda_session_handshake_duration_seconds",
        "Duration of session handshake completion"
    );
}

/// Record RPC request allowed
pub fn rpc_request_allowed() {
    counter!("spacepanda_rpc_requests_total", "result" => "allowed").increment(1);
}

/// Record RPC request rate limited
pub fn rpc_request_rate_limited() {
    counter!("spacepanda_rpc_requests_total", "result" => "rate_limited").increment(1);
    counter!("spacepanda_rate_limit_exceeded_total").increment(1);
}

/// Record RPC request circuit breaker open
pub fn rpc_request_circuit_breaker_open() {
    counter!("spacepanda_rpc_requests_total", "result" => "circuit_breaker_open").increment(1);
    counter!("spacepanda_circuit_breaker_open_total").increment(1);
}

/// Record replay attack detected
pub fn replay_attack_detected() {
    counter!("spacepanda_replay_attacks_detected_total").increment(1);
}

/// Record oversized frame rejected
pub fn oversized_frame_rejected(size: usize) {
    counter!("spacepanda_oversized_frames_rejected_total").increment(1);
    histogram!("spacepanda_rejected_frame_size_bytes").record(size as f64);
}

/// Record handshake replay detected
pub fn handshake_replay_detected() {
    counter!("spacepanda_handshake_replay_detected_total").increment(1);
}

/// Record expired handshake rejected
pub fn expired_handshake_rejected() {
    counter!("spacepanda_expired_handshakes_rejected_total").increment(1);
}

/// Record handshake timeout
pub fn handshake_timeout() {
    counter!("spacepanda_handshake_timeouts_total").increment(1);
}

/// Record circuit breaker state transition
pub fn circuit_breaker_transition(transition: &str) {
    counter!("spacepanda_circuit_breaker_state_transitions_total", "transition" => transition.to_string()).increment(1);
}

/// Record RPC call duration
pub fn rpc_call_duration(duration_secs: f64) {
    histogram!("spacepanda_rpc_call_duration_seconds").record(duration_secs);
}

/// Record RPC call result
pub fn rpc_call_result(result: &str) {
    counter!("spacepanda_rpc_calls_total", "result" => result.to_string()).increment(1);
}

/// Record RPC method invocation
pub fn rpc_method_invoked(method: &str) {
    counter!("spacepanda_rpc_methods_total", "method" => method.to_string()).increment(1);
}

/// Record RPC handler error
pub fn rpc_handler_error(error_type: &str) {
    counter!("spacepanda_rpc_handler_errors_total", "error_type" => error_type.to_string()).increment(1);
}

/// Update active peers gauge
pub fn set_active_peers(count: usize) {
    gauge!("spacepanda_active_peers").set(count as f64);
}

/// Update pending RPC requests gauge
pub fn set_pending_rpc_requests(count: usize) {
    gauge!("spacepanda_pending_rpc_requests").set(count as f64);
}

/// Update seen requests cache size gauge
pub fn set_seen_requests_cache_size(count: usize) {
    gauge!("spacepanda_seen_requests_cache_size").set(count as f64);
}

/// Record session handshake duration
pub fn session_handshake_duration(duration_secs: f64) {
    histogram!("spacepanda_session_handshake_duration_seconds").record(duration_secs);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_compilation() {
        // Just verify all metric calls compile
        init_metrics();
        rpc_request_allowed();
        rpc_request_rate_limited();
        rpc_request_circuit_breaker_open();
        replay_attack_detected();
        oversized_frame_rejected(1024);
        handshake_replay_detected();
        expired_handshake_rejected();
        handshake_timeout();
        circuit_breaker_transition("closed_to_open");
        rpc_call_duration(0.5);
        rpc_call_result("success");
        rpc_method_invoked("test_method");
        rpc_handler_error("method_not_found");
        set_active_peers(10);
        set_pending_rpc_requests(5);
        set_seen_requests_cache_size(100);
        session_handshake_duration(0.1);
    }
}
