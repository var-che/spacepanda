//! Distributed tracing support with OpenTelemetry

use std::time::Instant;
use tracing::{span, Level, Span};
use tracing_subscriber::layer::SubscriberExt;

/// Trace context for distributed operations
#[derive(Debug, Clone)]
pub struct TraceContext {
    pub trace_id: String,
    pub span_id: String,
    pub parent_span_id: Option<String>,
}

impl TraceContext {
    /// Create a new trace context
    pub fn new() -> Self {
        use uuid::Uuid;

        Self {
            trace_id: Uuid::new_v4().to_string(),
            span_id: Uuid::new_v4().to_string(),
            parent_span_id: None,
        }
    }

    /// Create a child trace context
    pub fn child(&self) -> Self {
        use uuid::Uuid;

        Self {
            trace_id: self.trace_id.clone(),
            span_id: Uuid::new_v4().to_string(),
            parent_span_id: Some(self.span_id.clone()),
        }
    }
}

impl Default for TraceContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Traced operation wrapper
pub struct TracedOperation {
    span: Span,
    start: Instant,
}

impl TracedOperation {
    /// Start a new traced operation
    pub fn new(operation_name: &str) -> Self {
        let span = span!(Level::INFO, "operation", name = operation_name);

        Self { span, start: Instant::now() }
    }

    /// Start a traced operation with context
    pub fn with_context(operation_name: &str, ctx: &TraceContext) -> Self {
        let span = span!(
            Level::INFO,
            "operation",
            name = operation_name,
            trace_id = %ctx.trace_id,
            span_id = %ctx.span_id,
            parent_span_id = ?ctx.parent_span_id
        );

        Self { span, start: Instant::now() }
    }

    /// Record an event in the trace
    pub fn record_event(&self, event: &str) {
        tracing::info!(parent: &self.span, event = event);
    }

    /// Record an error in the trace
    pub fn record_error(&self, error: &str) {
        tracing::error!(parent: &self.span, error = error);
    }

    /// Complete the operation and record duration
    pub fn complete(self) {
        let duration = self.start.elapsed();
        tracing::info!(
            parent: &self.span,
            duration_ms = duration.as_millis(),
            "operation completed"
        );
    }
}

/// Trace CRDT operations
pub mod crdt {
    use super::*;

    pub fn trace_merge(set_size: usize) -> TracedOperation {
        let op = TracedOperation::new("crdt_merge");
        tracing::info!(parent: &op.span, set_size = set_size);
        op
    }

    pub fn trace_add(element_count: usize) -> TracedOperation {
        let op = TracedOperation::new("crdt_add");
        tracing::info!(parent: &op.span, element_count = element_count);
        op
    }
}

/// Trace DHT operations
pub mod dht {
    use super::*;

    pub fn trace_lookup(key: &str) -> TracedOperation {
        let op = TracedOperation::new("dht_lookup");
        tracing::info!(parent: &op.span, key = key);
        op
    }

    pub fn trace_store(key: &str, value_size: usize) -> TracedOperation {
        let op = TracedOperation::new("dht_store");
        tracing::info!(parent: &op.span, key = key, value_size = value_size);
        op
    }
}

/// Trace MLS operations
pub mod mls {
    use super::*;

    pub fn trace_encrypt(message_size: usize) -> TracedOperation {
        let op = TracedOperation::new("mls_encrypt");
        tracing::info!(parent: &op.span, message_size = message_size);
        op
    }

    pub fn trace_decrypt(ciphertext_size: usize) -> TracedOperation {
        let op = TracedOperation::new("mls_decrypt");
        tracing::info!(parent: &op.span, ciphertext_size = ciphertext_size);
        op
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trace_context() {
        let ctx = TraceContext::new();
        assert!(!ctx.trace_id.is_empty());
        assert!(!ctx.span_id.is_empty());
        assert!(ctx.parent_span_id.is_none());

        let child = ctx.child();
        assert_eq!(child.trace_id, ctx.trace_id);
        assert_ne!(child.span_id, ctx.span_id);
        assert_eq!(child.parent_span_id, Some(ctx.span_id));
    }

    #[test]
    fn test_traced_operation() {
        let op = TracedOperation::new("test_operation");
        op.record_event("test event");
        op.complete();
    }
}
