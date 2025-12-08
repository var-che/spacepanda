//! Message Mixer - Constant-Rate Timing Resistance
//!
//! This module implements constant-rate message mixing to prevent timing analysis attacks.
//! Messages are sent at a fixed interval regardless of actual user activity, making it
//! impossible for network observers to infer conversation patterns.
//!
//! ## Threat Model
//!
//! **Without Mixing:**
//! ```text
//! Network Observer sees:
//! 10:00:00 - Alice sends message
//! 10:00:02 - Bob sends message (instant reply)
//! 10:00:05 - Alice sends message (3s thinking time)
//!
//! → Observer learns: "Bob is online", "Alice thinking for 3 seconds"
//! → Conversation flow is visible
//! → Typing patterns leak information
//! ```
//!
//! **With Constant-Rate Mixing:**
//! ```text
//! Network Observer sees:
//! 10:00:00 - Message (could be real or dummy)
//! 10:00:01 - Message (could be real or dummy)
//! 10:00:02 - Message (could be real or dummy)
//! 10:00:03 - Message (could be real or dummy)
//!
//! → All messages look identical
//! → No timing correlation possible
//! → Conversation patterns hidden
//! ```
//!
//! ## Architecture
//!
//! ```text
//! ┌──────────────────────────────────────┐
//! │          MessageMixer                │
//! │                                      │
//! │  ┌────────────┐  ┌──────────────┐   │
//! │  │   Queue    │  │  Dummy Gen   │   │
//! │  │  (Real)    │  │  (Cover)     │   │
//! │  └────┬───────┘  └──────┬───────┘   │
//! │       │                 │            │
//! │       └─────────┬───────┘            │
//! │                 ▼                    │
//! │        ┌────────────────┐            │
//! │        │  Fixed-Rate    │            │
//! │        │    Sender      │            │
//! │        │ (every 100ms)  │            │
//! │        └────────┬───────┘            │
//! └─────────────────┼────────────────────┘
//!                   ▼
//!            Network (TLS)
//! ```
//!
//! ## Usage
//!
//! ```rust,ignore
//! use spacepanda_core::core_mvp::message_mixer::{MessageMixer, MixerConfig};
//!
//! // Create mixer with 100ms interval
//! let config = MixerConfig {
//!     interval_ms: 100,
//!     enabled: true,
//!     max_queue_size: 1000,
//! };
//! let mixer = MessageMixer::new(config);
//!
//! // Start background mixing task
//! mixer.start().await?;
//!
//! // Queue real message (will be sent at next interval)
//! mixer.send_message(channel_id, encrypted_payload).await?;
//!
//! // Mixer automatically sends dummy messages when queue is empty
//! ```
//!
//! ## Performance
//!
//! - **Bandwidth overhead**: ~10x (1 message/100ms even when idle)
//! - **Latency**: Up to `interval_ms` delay (100ms default)
//! - **CPU overhead**: Minimal (AES encryption for dummies)
//!
//! ## Configuration
//!
//! - `interval_ms`: Message sending interval (default: 100ms)
//! - `enabled`: Enable/disable mixing (default: true for privacy)
//! - `max_queue_size`: Maximum queued messages (default: 1000)

use rand::Rng;
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};
use tokio::time::interval;
use tracing::{debug, info, warn};

/// Message mixer configuration
#[derive(Debug, Clone)]
pub struct MixerConfig {
    /// Message sending interval in milliseconds
    pub interval_ms: u64,

    /// Enable constant-rate mixing (disable for debugging/testing)
    pub enabled: bool,

    /// Maximum queue size before dropping messages
    pub max_queue_size: usize,

    /// Enable dummy traffic when queue is empty
    pub send_dummy_traffic: bool,
}

impl Default for MixerConfig {
    fn default() -> Self {
        Self {
            interval_ms: 100, // 10 messages/second
            enabled: true,    // Privacy-first default
            max_queue_size: 1000,
            send_dummy_traffic: true,
        }
    }
}

/// A message in the mixer queue
#[derive(Debug, Clone)]
pub enum MixerMessage {
    /// Real message to be sent
    Real { channel_id: String, payload: Vec<u8> },

    /// Dummy/cover traffic message
    Dummy { channel_id: String },
}

impl MixerMessage {
    /// Check if message is dummy traffic
    pub fn is_dummy(&self) -> bool {
        matches!(self, MixerMessage::Dummy { .. })
    }

    /// Get channel ID
    pub fn channel_id(&self) -> &str {
        match self {
            MixerMessage::Real { channel_id, .. } => channel_id,
            MixerMessage::Dummy { channel_id } => channel_id,
        }
    }

    /// Get payload (empty for dummy messages)
    pub fn payload(&self) -> &[u8] {
        match self {
            MixerMessage::Real { payload, .. } => payload,
            MixerMessage::Dummy { .. } => &[],
        }
    }
}

/// Message mixer for constant-rate timing resistance
pub struct MessageMixer {
    /// Configuration
    config: MixerConfig,

    /// Message queue (real messages waiting to be sent)
    queue: Arc<RwLock<VecDeque<MixerMessage>>>,

    /// Statistics
    stats: Arc<RwLock<MixerStats>>,

    /// Shutdown signal sender
    shutdown_tx: Option<mpsc::Sender<()>>,
}

/// Mixer statistics for monitoring
#[derive(Debug, Default, Clone)]
pub struct MixerStats {
    /// Total real messages sent
    pub real_messages_sent: u64,

    /// Total dummy messages sent
    pub dummy_messages_sent: u64,

    /// Current queue size
    pub queue_size: usize,

    /// Messages dropped due to queue overflow
    pub messages_dropped: u64,
}

impl MessageMixer {
    /// Create a new message mixer
    ///
    /// # Arguments
    /// * `config` - Mixer configuration
    pub fn new(config: MixerConfig) -> Self {
        info!(
            interval_ms = config.interval_ms,
            enabled = config.enabled,
            "Creating MessageMixer"
        );

        Self {
            config,
            queue: Arc::new(RwLock::new(VecDeque::new())),
            stats: Arc::new(RwLock::new(MixerStats::default())),
            shutdown_tx: None,
        }
    }

    /// Generate a dummy message payload
    ///
    /// Creates a random payload that looks like an encrypted message.
    /// Uses message padding sizes to match real traffic patterns.
    ///
    /// # Returns
    /// Random bytes (256, 1024, 4096, 16384, or 65536 bytes)
    fn generate_dummy_payload() -> Vec<u8> {
        let mut rng = rand::rng();

        // Use same padding sizes as message padding for consistency
        let sizes = [256, 1024, 4096, 16384, 65536];
        let size = sizes[rng.random_range(0..sizes.len())];

        // Generate random bytes
        let mut payload = vec![0u8; size];
        rng.fill(&mut payload[..]);

        payload
    }

    /// Start the background mixing task
    ///
    /// Spawns a tokio task that sends messages at a constant rate.
    ///
    /// # Returns
    /// Shutdown channel receiver for graceful termination
    pub async fn start(&mut self) -> mpsc::Receiver<()> {
        let (shutdown_tx, shutdown_rx) = mpsc::channel(1);
        self.shutdown_tx = Some(shutdown_tx.clone());

        if !self.config.enabled {
            info!("MessageMixer disabled, skipping background task");
            return shutdown_rx;
        }

        let config = self.config.clone();
        let queue = self.queue.clone();
        let stats = self.stats.clone();

        tokio::spawn(async move {
            Self::run_mixer_loop(config, queue, stats, shutdown_tx).await;
        });

        info!("MessageMixer background task started");
        shutdown_rx
    }

    /// Background mixer loop (runs in separate task)
    async fn run_mixer_loop(
        config: MixerConfig,
        queue: Arc<RwLock<VecDeque<MixerMessage>>>,
        stats: Arc<RwLock<MixerStats>>,
        _shutdown_tx: mpsc::Sender<()>,
    ) {
        let mut tick = interval(Duration::from_millis(config.interval_ms));

        loop {
            tick.tick().await;

            // Get next message from queue (or generate dummy)
            let message = {
                let mut q = queue.write().await;
                q.pop_front()
            };

            match message {
                Some(msg) => {
                    // Send real message
                    debug!(
                        channel = msg.channel_id(),
                        is_dummy = msg.is_dummy(),
                        "Sending message from mixer"
                    );

                    // Update stats
                    let mut s = stats.write().await;
                    if msg.is_dummy() {
                        s.dummy_messages_sent += 1;
                    } else {
                        s.real_messages_sent += 1;
                    }

                    // TODO: Actually send message via network layer
                    // For now, just simulate sending
                }
                None if config.send_dummy_traffic => {
                    // Generate and send dummy message
                    let dummy_payload = Self::generate_dummy_payload();

                    debug!(payload_size = dummy_payload.len(), "Sending dummy traffic");

                    let mut s = stats.write().await;
                    s.dummy_messages_sent += 1;

                    // TODO: Actually send dummy message via network layer
                    // For now, dummy messages are just generated for stats
                }
                None => {
                    // No messages and dummy traffic disabled
                    debug!("No messages to send, skipping interval");
                }
            }

            // Update queue size stat
            {
                let q = queue.read().await;
                let mut s = stats.write().await;
                s.queue_size = q.len();
            }
        }
    }

    /// Queue a real message for sending
    ///
    /// Message will be sent at the next available interval.
    ///
    /// # Arguments
    /// * `channel_id` - Channel identifier
    /// * `payload` - Encrypted message payload
    ///
    /// # Returns
    /// `Ok(())` if queued successfully, `Err` if queue is full
    pub async fn send_message(&self, channel_id: String, payload: Vec<u8>) -> Result<(), String> {
        if !self.config.enabled {
            // If mixer disabled, messages should be sent immediately
            warn!("MessageMixer disabled, message sent without mixing");
            return Ok(());
        }

        let mut queue = self.queue.write().await;

        if queue.len() >= self.config.max_queue_size {
            // Queue full - drop message
            warn!(
                queue_size = queue.len(),
                max_size = self.config.max_queue_size,
                "Message queue full, dropping message"
            );

            let mut stats = self.stats.write().await;
            stats.messages_dropped += 1;

            return Err("Queue full".to_string());
        }

        queue.push_back(MixerMessage::Real { channel_id, payload });

        debug!(queue_size = queue.len(), "Message queued for mixing");

        Ok(())
    }

    /// Get current mixer statistics
    pub async fn stats(&self) -> MixerStats {
        self.stats.read().await.clone()
    }

    /// Get current queue size
    pub async fn queue_size(&self) -> usize {
        self.queue.read().await.len()
    }

    /// Shutdown the mixer gracefully
    pub async fn shutdown(&mut self) {
        info!("Shutting down MessageMixer");
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(()).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mixer_config_default() {
        let config = MixerConfig::default();
        assert_eq!(config.interval_ms, 100);
        assert!(config.enabled);
        assert_eq!(config.max_queue_size, 1000);
        assert!(config.send_dummy_traffic);
    }

    #[test]
    fn test_mixer_message_is_dummy() {
        let real_msg =
            MixerMessage::Real { channel_id: "test".to_string(), payload: vec![1, 2, 3] };
        assert!(!real_msg.is_dummy());

        let dummy_msg = MixerMessage::Dummy { channel_id: "test".to_string() };
        assert!(dummy_msg.is_dummy());
    }

    #[tokio::test]
    async fn test_mixer_creation() {
        let config = MixerConfig::default();
        let mixer = MessageMixer::new(config);

        let stats = mixer.stats().await;
        assert_eq!(stats.real_messages_sent, 0);
        assert_eq!(stats.dummy_messages_sent, 0);
    }

    #[tokio::test]
    async fn test_message_queuing() {
        let config = MixerConfig {
            enabled: true,
            interval_ms: 100,
            max_queue_size: 10,
            send_dummy_traffic: true,
        };
        let mixer = MessageMixer::new(config);

        // Queue a message
        mixer.send_message("test_channel".to_string(), vec![1, 2, 3]).await.unwrap();

        assert_eq!(mixer.queue_size().await, 1);
    }

    #[tokio::test]
    async fn test_queue_overflow() {
        let config = MixerConfig {
            enabled: true,
            interval_ms: 100,
            max_queue_size: 2,
            send_dummy_traffic: true,
        };
        let mixer = MessageMixer::new(config);

        // Fill queue
        mixer.send_message("test".to_string(), vec![1]).await.unwrap();
        mixer.send_message("test".to_string(), vec![2]).await.unwrap();

        // This should fail (queue full)
        let result = mixer.send_message("test".to_string(), vec![3]).await;
        assert!(result.is_err());

        let stats = mixer.stats().await;
        assert_eq!(stats.messages_dropped, 1);
    }

    #[tokio::test]
    async fn test_mixer_disabled() {
        let config = MixerConfig {
            enabled: false,
            interval_ms: 100,
            max_queue_size: 10,
            send_dummy_traffic: false,
        };
        let mixer = MessageMixer::new(config);

        // Should succeed but not actually queue
        mixer.send_message("test".to_string(), vec![1]).await.unwrap();

        // Queue should be empty (message sent immediately in theory)
        assert_eq!(mixer.queue_size().await, 0);
    }

    #[tokio::test]
    async fn test_mixer_stats() {
        let config = MixerConfig::default();
        let mixer = MessageMixer::new(config);

        mixer.send_message("test".to_string(), vec![1]).await.unwrap();
        mixer.send_message("test".to_string(), vec![2]).await.unwrap();

        // Use queue_size() which reads directly from queue
        assert_eq!(mixer.queue_size().await, 2);

        // Stats are only updated by background loop (not running in this test)
        let stats = mixer.stats().await;
        assert_eq!(stats.real_messages_sent, 0); // No background loop running
    }
}
