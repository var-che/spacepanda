//! Async test helpers
//!
//! Utilities for testing asynchronous code, including channel helpers,
//! timeout utilities, and task management.

use tokio::sync::{mpsc, oneshot};
use tokio::time::{timeout, Duration};
use std::future::Future;

/// Helper for receiving from a channel with a timeout
pub async fn recv_timeout<T>(
    rx: &mut mpsc::Receiver<T>,
    duration: Duration,
) -> Result<T, RecvTimeoutError> {
    timeout(duration, rx.recv())
        .await
        .map_err(|_| RecvTimeoutError::Timeout)?
        .ok_or(RecvTimeoutError::Closed)
}

/// Helper for receiving from a oneshot channel with a timeout
pub async fn recv_oneshot_timeout<T>(
    rx: oneshot::Receiver<T>,
    duration: Duration,
) -> Result<T, RecvTimeoutError> {
    timeout(duration, rx)
        .await
        .map_err(|_| RecvTimeoutError::Timeout)?
        .map_err(|_| RecvTimeoutError::Closed)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecvTimeoutError {
    Timeout,
    Closed,
}

impl std::fmt::Display for RecvTimeoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RecvTimeoutError::Timeout => write!(f, "receive operation timed out"),
            RecvTimeoutError::Closed => write!(f, "channel closed"),
        }
    }
}

impl std::error::Error for RecvTimeoutError {}

/// Helper for sending with a timeout
pub async fn send_timeout<T>(
    tx: &mpsc::Sender<T>,
    value: T,
    duration: Duration,
) -> Result<(), SendTimeoutError> {
    timeout(duration, tx.send(value))
        .await
        .map_err(|_| SendTimeoutError::Timeout)?
        .map_err(|_| SendTimeoutError::Closed)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SendTimeoutError {
    Timeout,
    Closed,
}

impl std::fmt::Display for SendTimeoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SendTimeoutError::Timeout => write!(f, "send operation timed out"),
            SendTimeoutError::Closed => write!(f, "channel closed"),
        }
    }
}

impl std::error::Error for SendTimeoutError {}

/// Run a future with a timeout, returning Ok(result) or Err on timeout
pub async fn with_timeout<F, T>(duration: Duration, future: F) -> Result<T, TimeoutError>
where
    F: Future<Output = T>,
{
    timeout(duration, future)
        .await
        .map_err(|_| TimeoutError::Elapsed)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeoutError {
    Elapsed,
}

impl std::fmt::Display for TimeoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "operation timed out")
    }
}

impl std::error::Error for TimeoutError {}

/// Helper to assert a future completes within duration
pub async fn assert_completes_within<F, T>(duration: Duration, future: F) -> T
where
    F: Future<Output = T>,
{
    match timeout(duration, future).await {
        Ok(result) => result,
        Err(_) => panic!("Future did not complete within {:?}", duration),
    }
}

/// Helper to assert a future does NOT complete within duration
pub async fn assert_times_out<F, T>(duration: Duration, future: F)
where
    F: Future<Output = T>,
{
    match timeout(duration, future).await {
        Ok(_) => panic!("Expected future to timeout, but it completed within {:?}", duration),
        Err(_) => (),
    }
}

/// Helper to collect N messages from a channel with timeout
pub async fn collect_n<T>(
    rx: &mut mpsc::Receiver<T>,
    count: usize,
    per_message_timeout: Duration,
) -> Result<Vec<T>, RecvTimeoutError> {
    let mut results = Vec::with_capacity(count);
    for _ in 0..count {
        let msg = recv_timeout(rx, per_message_timeout).await?;
        results.push(msg);
    }
    Ok(results)
}

/// Helper to drain all available messages from a channel without blocking
pub fn try_drain<T>(rx: &mut mpsc::Receiver<T>) -> Vec<T> {
    let mut results = Vec::new();
    while let Ok(msg) = rx.try_recv() {
        results.push(msg);
    }
    results
}

/// Helper to spawn a task and get a handle for cleanup
pub fn spawn_test_task<F>(future: F) -> TestTaskHandle
where
    F: Future<Output = ()> + Send + 'static,
{
    let handle = tokio::spawn(future);
    TestTaskHandle { handle }
}

/// Handle for a test task that aborts on drop
pub struct TestTaskHandle {
    handle: tokio::task::JoinHandle<()>,
}

impl TestTaskHandle {
    /// Wait for the task to complete
    pub async fn join(mut self) -> Result<(), tokio::task::JoinError> {
        // Take ownership to avoid Drop being called
        let handle = std::mem::replace(&mut self.handle, tokio::spawn(async {}));
        std::mem::forget(self); // Don't call Drop
        handle.await
    }

    /// Abort the task
    pub fn abort(&self) {
        self.handle.abort();
    }
}

impl Drop for TestTaskHandle {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

/// Create a bounded mpsc channel with a default buffer size for tests
pub fn test_channel<T>(buffer: usize) -> (mpsc::Sender<T>, mpsc::Receiver<T>) {
    mpsc::channel(buffer)
}

/// Create an unbounded mpsc channel for tests
pub fn test_unbounded_channel<T>() -> (mpsc::UnboundedSender<T>, mpsc::UnboundedReceiver<T>) {
    mpsc::unbounded_channel()
}

/// Default timeout duration for tests (5 seconds)
pub const DEFAULT_TEST_TIMEOUT: Duration = Duration::from_secs(5);

/// Short timeout for tests that should fail fast (100ms)
pub const SHORT_TEST_TIMEOUT: Duration = Duration::from_millis(100);

/// Very short timeout for race condition tests (10ms)
pub const VERY_SHORT_TIMEOUT: Duration = Duration::from_millis(10);

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_recv_timeout_success() {
        let (tx, mut rx) = test_channel(1);
        tx.send(42).await.unwrap();
        
        let result = recv_timeout(&mut rx, DEFAULT_TEST_TIMEOUT).await;
        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn test_recv_timeout_times_out() {
        let (_tx, mut rx) = test_channel::<i32>(1);
        
        let result = recv_timeout(&mut rx, SHORT_TEST_TIMEOUT).await;
        assert_eq!(result.unwrap_err(), RecvTimeoutError::Timeout);
    }

    #[tokio::test]
    async fn test_recv_timeout_closed() {
        let (tx, mut rx) = test_channel::<i32>(1);
        drop(tx);
        
        let result = recv_timeout(&mut rx, DEFAULT_TEST_TIMEOUT).await;
        assert_eq!(result.unwrap_err(), RecvTimeoutError::Closed);
    }

    #[tokio::test]
    async fn test_collect_n() {
        let (tx, mut rx) = test_channel(10);
        
        for i in 0..5 {
            tx.send(i).await.unwrap();
        }
        
        let results = collect_n(&mut rx, 5, DEFAULT_TEST_TIMEOUT).await.unwrap();
        assert_eq!(results, vec![0, 1, 2, 3, 4]);
    }

    #[tokio::test]
    async fn test_try_drain() {
        let (tx, mut rx) = test_channel(10);
        
        for i in 0..5 {
            tx.send(i).await.unwrap();
        }
        
        // Give messages time to arrive
        tokio::time::sleep(Duration::from_millis(10)).await;
        
        let results = try_drain(&mut rx);
        assert_eq!(results.len(), 5);
    }

    #[tokio::test]
    async fn test_assert_completes_within() {
        let future = async {
            tokio::time::sleep(Duration::from_millis(10)).await;
            42
        };
        
        let result = assert_completes_within(Duration::from_millis(100), future).await;
        assert_eq!(result, 42);
    }

    #[tokio::test]
    async fn test_assert_times_out() {
        let future = async {
            tokio::time::sleep(Duration::from_secs(10)).await;
        };
        
        assert_times_out(Duration::from_millis(10), future).await;
    }

    #[tokio::test]
    async fn test_spawn_test_task() {
        let (tx, mut rx) = test_channel(1);
        
        let _handle = spawn_test_task(async move {
            tx.send(42).await.unwrap();
        });
        
        let result = recv_timeout(&mut rx, DEFAULT_TEST_TIMEOUT).await.unwrap();
        assert_eq!(result, 42);
    }
}
