# P1 Task #2: Structured Tracing Implementation

## Overview

Comprehensive structured tracing has been added to the RPC protocol and rate limiting infrastructure to provide operational observability for security events, performance monitoring, and debugging capabilities.

## Implementation

### Tracing Infrastructure

**Dependencies:**

- `tracing` crate for structured logging
- `tracing-subscriber` with JSON features for structured output
- `#[instrument]` macro for automatic span creation

**Key Modules:**

- `spacepanda-core/src/core_router/rate_limiter.rs`
- `spacepanda-core/src/core_router/rpc_protocol.rs`

### Rate Limiter Tracing

#### Circuit Breaker State Transitions

**State Changes:**

```rust
// OPEN → HALF-OPEN (recovery attempt)
info!(
    peer_id = ?peer_id,
    state = "Half-Open",
    "Circuit breaker attempting recovery"
);

// HALF-OPEN → CLOSED (successful recovery)
info!(
    peer_id = ?peer_id,
    state = "Closed",
    "Circuit breaker recovered"
);

// Circuit opening on threshold
warn!(
    peer_id = ?peer_id,
    consecutive_failures,
    threshold,
    "Circuit breaker opened"
);
```

**Failure Tracking:**

```rust
// Failures while closed
debug!(
    peer_id = ?peer_id,
    consecutive_failures,
    threshold,
    "Circuit breaker failure recorded"
);

// Recovery attempt failed (reopening)
warn!(
    peer_id = ?peer_id,
    "Half-open recovery failed, reopening circuit breaker"
);
```

#### Token Bucket Rate Limiting

**Request Decisions:**

```rust
// New peer limiter created
debug!(peer_id = ?peer_id, "Created new rate limiter for peer");

// Request allowed
trace!(
    peer_id = ?peer_id,
    tokens_remaining = tokens,
    "Rate limit check passed"
);

// Rate limit exceeded
warn!(
    peer_id = ?peer_id,
    capacity,
    refill_rate = refill_rate_per_sec,
    "Rate limit exceeded"
);

// Circuit breaker blocking
warn!(
    peer_id = ?peer_id,
    state = ?circuit_breaker.state,
    "Request blocked by circuit breaker"
);
```

### RPC Protocol Tracing

#### Span Instrumentation

**Outgoing Requests:**

```rust
#[instrument(
    skip(self, params, response_tx),
    fields(peer_id = ?peer_id, method = %method)
)]
async fn make_call(...) {
    trace!("Initiating RPC call");
    // ...
}
```

**Incoming Frames:**

```rust
#[instrument(
    skip(self, bytes),
    fields(peer_id = ?peer_id, frame_size = bytes.len())
)]
async fn handle_frame(...) {
    // Security checks with tracing
}
```

**Request Handling:**

```rust
#[instrument(
    skip(self, params),
    fields(peer_id = ?peer_id, request_id = %id, method = %method)
)]
async fn handle_request(...) {
    // Method dispatch with tracing
}
```

**Response Processing:**

```rust
#[instrument(
    skip(self, result),
    fields(request_id = %id)
)]
async fn handle_response(...) {
    // Timeout cancellation with tracing
}
```

#### Security Event Tracing

**Rate Limiting:**

```rust
// Rate limit check passed
trace!("Rate limit check passed");

// Rate limit exceeded
warn!("Request rejected: rate limit exceeded");

// Circuit breaker open
warn!("Request rejected: circuit breaker open");
```

**Frame Size Validation:**

```rust
warn!(
    frame_size,
    max_size,
    "Oversized frame rejected"
);
```

**Replay Attack Detection:**

```rust
warn!(
    request_id = %id,
    method = %method,
    "Replay attack detected: duplicate request ID"
);

debug!("Request ID added to seen cache");
```

#### Handler Event Tracing

**Method Dispatch:**

```rust
// Method not found
warn!(method, "Method not found");

// Handler channel closed
warn!(method = %method_clone, "Handler channel closed: handler crashed");
```

**Handler Responses:**

```rust
// Success response
trace!(method = %method_clone, "Handler returned success");

// Error response
debug!(
    method = %method_clone,
    error_code = e.code,
    "Handler returned error"
);

// Handler dropped without responding
warn!(
    method = %method_clone,
    "Handler dropped without responding"
);
```

**Timeout Handling:**

```rust
// Request timeout
warn!(
    request_id = %request_id_for_timeout,
    method = %method_clone,
    timeout_ms = timeout.as_millis(),
    "Request timeout"
);

// Response received, timeout cancelled
trace!("Response received, timeout cancelled");

// Response for unknown/timed-out request
debug!("Response received for unknown request (likely timed out)");
```

## Log Levels

### trace

- **Purpose:** Verbose diagnostic information
- **Usage:** Allowed requests, timeout cancellations, successful responses
- **Volume:** High - only enable for deep debugging

### debug

- **Purpose:** Diagnostic information for troubleshooting
- **Usage:** New peer limiters, cache operations, handler errors
- **Volume:** Medium - useful for debugging specific issues

### info

- **Purpose:** Significant state changes
- **Usage:** Circuit breaker state transitions (recovery)
- **Volume:** Low - important lifecycle events

### warn

- **Purpose:** Security events and potential issues
- **Usage:** Rate limits, circuit breaker opens, replay attacks, oversized frames, timeouts, handler failures
- **Volume:** Low to Medium - indicates problems that need attention

## Structured Fields

### Common Fields

**Peer Identification:**

- `peer_id`: Identifies the remote peer (Debug format)

**Request Context:**

- `request_id`: Unique request identifier (Display format)
- `method`: RPC method name (Display format)
- `frame_size`: Size of incoming frame in bytes
- `timeout_ms`: Timeout duration in milliseconds

**Rate Limiting:**

- `tokens_remaining`: Available tokens after request
- `capacity`: Maximum burst size
- `refill_rate`: Tokens per second refill rate
- `consecutive_failures`: Number of consecutive failures
- `threshold`: Failure threshold for circuit breaker
- `state`: Circuit breaker state (Closed/Open/Half-Open)

**Error Information:**

- `error_code`: RPC error code
- `frame_size` / `max_size`: For oversized frame rejection

## Usage Examples

### Enabling Tracing

**Development (human-readable):**

```rust
use tracing_subscriber;

tracing_subscriber::fmt()
    .with_max_level(tracing::Level::DEBUG)
    .init();
```

**Production (JSON format):**

```rust
use tracing_subscriber::prelude::*;
use tracing_subscriber::fmt::format::FmtSpan;

tracing_subscriber::registry()
    .with(
        tracing_subscriber::fmt::layer()
            .json()
            .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE)
    )
    .with(tracing_subscriber::EnvFilter::from_default_env())
    .init();
```

### Querying Traces

**Find rate limit violations:**

```bash
# JSON logs
cat logs.json | jq 'select(.message == "Rate limit exceeded")'

# Structured format
grep "Rate limit exceeded" logs.txt | jq '.peer_id, .capacity, .refill_rate'
```

**Track circuit breaker state:**

```bash
# All circuit breaker events for a peer
jq 'select(.peer_id == "peer123" and .message | contains("Circuit breaker"))' logs.json

# State transition timeline
jq -r 'select(.message | contains("Circuit breaker")) |
    "\(.timestamp) \(.peer_id) \(.state) \(.message)"' logs.json
```

**Monitor request flow:**

```bash
# Follow a request through the system
REQUEST_ID="abc123"
jq --arg id "$REQUEST_ID" 'select(.request_id == $id)' logs.json

# Identify timeout issues
jq 'select(.message == "Request timeout") |
    {method, timeout_ms, request_id}' logs.json
```

**Security monitoring:**

```bash
# Replay attack attempts
jq 'select(.message | contains("Replay attack"))' logs.json

# All security rejections
jq 'select(.level == "WARN" and
    (.message | contains("rejected") or contains("blocked")))' logs.json
```

## Integration with Observability Platforms

### Metrics Collection (Future Work)

The tracing infrastructure provides foundation for metrics:

- **Counters:** rate_limit_exceeded_total, circuit_breaker_opened_total, replay_attempts_total
- **Histograms:** request_duration_seconds, frame_size_bytes
- **Gauges:** active_peers, open_circuit_breakers

### Example Metrics Implementation

```rust
use metrics::{counter, histogram};

// In rate limiter
if rate_limited {
    counter!("rate_limit_exceeded_total", "peer_id" => peer_id.to_string()).increment(1);
}

// In circuit breaker
if opening {
    counter!("circuit_breaker_opened_total", "peer_id" => peer_id.to_string()).increment(1);
}

// In RPC protocol
histogram!("request_duration_seconds", "method" => method.clone())
    .record(duration.as_secs_f64());
```

## Performance Impact

### Span Overhead

- `#[instrument]` macro: Minimal overhead when tracing disabled
- Structured field extraction: O(1) per field
- Skipped parameters: No serialization cost for large data (bytes, params)

### Recommended Settings

**Development:**

- `RUST_LOG=debug` - Full diagnostics
- Human-readable format

**Staging:**

- `RUST_LOG=info` - State changes only
- JSON format for structured querying

**Production:**

- `RUST_LOG=warn` - Security events and issues only
- JSON format with log aggregation
- Sampling for high-volume trace events

## Testing

### Tracing Tests (Future Work)

Example test structure:

```rust
#[tokio::test]
async fn test_rate_limit_tracing() {
    let (subscriber, handle) = tracing_subscriber::fmt()
        .with_test_writer()
        .finish();

    tracing::subscriber::with_default(subscriber, || {
        // Test code that triggers rate limiting
        // Verify expected trace events were emitted
    });

    let logs = handle.logs();
    assert!(logs.contains("Rate limit exceeded"));
}
```

## Security Benefits

1. **Attack Detection:** Real-time visibility into security violations
2. **Incident Response:** Structured logs enable rapid investigation
3. **Forensics:** Complete audit trail of security events
4. **Monitoring:** Automated alerting on suspicious patterns
5. **Compliance:** Detailed logging for security audits

## Next Steps

1. **Metrics Collection:** Implement Prometheus-compatible metrics
2. **Tracing Tests:** Add tests to verify trace event emission
3. **Alert Rules:** Define alerting thresholds for security events
4. **Dashboard:** Create operational dashboard for key metrics
5. **Sampling:** Implement adaptive sampling for high-volume events

## Related Documentation

- [P1_RATE_LIMITING.md](./P1_RATE_LIMITING.md) - Rate limiting implementation details
- [P0_SECURITY_SUMMARY.md](./P0_SECURITY_SUMMARY.md) - Security improvements overview
