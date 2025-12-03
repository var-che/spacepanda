//! DHT Bridge Implementation
//!
//! Bridges core_dht with MLS transport requirements.

use crate::core_mls::errors::MlsResult;
use crate::core_mls::traits::transport::{DhtBridge, GroupId, WireMessage};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

#[cfg(test)]
use crate::core_mls::traits::transport::MessageType;

/// DHT bridge implementation
///
/// This is a simplified in-memory implementation for testing.
/// In production, this would wrap the actual DHT subsystem.
pub struct DhtBridgeImpl {
    /// Subscriptions: group_id -> channel sender
    subscriptions: Arc<RwLock<HashMap<Vec<u8>, Vec<mpsc::Sender<WireMessage>>>>>,
}

impl DhtBridgeImpl {
    /// Create a new DHT bridge
    pub fn new() -> Self {
        Self { subscriptions: Arc::new(RwLock::new(HashMap::new())) }
    }
}

impl Default for DhtBridgeImpl {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl DhtBridge for DhtBridgeImpl {
    async fn publish(&self, group_id: &GroupId, wire: WireMessage) -> MlsResult<()> {
        // Get all subscribers for this group
        let subscriptions = self.subscriptions.read().await;

        if let Some(senders) = subscriptions.get(group_id) {
            // Send to all subscribers
            for sender in senders {
                // Non-blocking send (drop if receiver is slow)
                let _ = sender.try_send(wire.clone());
            }
        }

        Ok(())
    }

    async fn subscribe(
        &self,
        group_id: &GroupId,
    ) -> MlsResult<tokio::sync::mpsc::Receiver<WireMessage>> {
        let (tx, rx) = mpsc::channel(100);

        let mut subscriptions = self.subscriptions.write().await;
        subscriptions.entry(group_id.to_vec()).or_insert_with(Vec::new).push(tx);

        Ok(rx)
    }

    async fn unsubscribe(&self, group_id: &GroupId) -> MlsResult<()> {
        let mut subscriptions = self.subscriptions.write().await;
        subscriptions.remove(group_id);
        Ok(())
    }

    async fn send_direct(&self, peer_id: &[u8], wire: WireMessage) -> MlsResult<()> {
        // In a real implementation, this would use direct peer-to-peer routing
        // For now, fall back to publish
        let group_id = wire.group_id.clone();
        self.publish(&group_id, wire).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_publish_subscribe() {
        let bridge = DhtBridgeImpl::new();
        let group_id = vec![1, 2, 3, 4];

        // Subscribe
        let mut rx = bridge.subscribe(&group_id).await.unwrap();

        // Publish
        let message = WireMessage {
            group_id: group_id.clone(),
            epoch: 5,
            payload: vec![10, 20, 30],
            msg_type: MessageType::Application,
        };

        bridge.publish(&group_id, message.clone()).await.unwrap();

        // Receive
        let received = rx.recv().await.unwrap();
        assert_eq!(received.epoch, message.epoch);
        assert_eq!(received.payload, message.payload);
    }

    #[tokio::test]
    async fn test_multiple_subscribers() {
        let bridge = DhtBridgeImpl::new();
        let group_id = vec![5, 6, 7, 8];

        // Multiple subscribers
        let mut rx1 = bridge.subscribe(&group_id).await.unwrap();
        let mut rx2 = bridge.subscribe(&group_id).await.unwrap();

        // Publish
        let message = WireMessage {
            group_id: group_id.clone(),
            epoch: 10,
            payload: vec![40, 50, 60],
            msg_type: MessageType::Commit,
        };

        bridge.publish(&group_id, message.clone()).await.unwrap();

        // Both should receive
        let received1 = rx1.recv().await.unwrap();
        let received2 = rx2.recv().await.unwrap();

        assert_eq!(received1.epoch, message.epoch);
        assert_eq!(received2.epoch, message.epoch);
    }

    #[tokio::test]
    async fn test_unsubscribe() {
        let bridge = DhtBridgeImpl::new();
        let group_id = vec![9, 10, 11];

        let mut rx = bridge.subscribe(&group_id).await.unwrap();
        bridge.unsubscribe(&group_id).await.unwrap();

        // Publish after unsubscribe
        let message = WireMessage {
            group_id: group_id.clone(),
            epoch: 1,
            payload: vec![70, 80],
            msg_type: MessageType::Proposal,
        };

        bridge.publish(&group_id, message).await.unwrap();

        // Should not receive (channel closed or no message)
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn test_send_direct() {
        let bridge = DhtBridgeImpl::new();
        let group_id = vec![12, 13, 14];
        let peer_id = vec![15, 16, 17];

        let mut rx = bridge.subscribe(&group_id).await.unwrap();

        let message = WireMessage {
            group_id: group_id.clone(),
            epoch: 2,
            payload: vec![90, 100],
            msg_type: MessageType::Welcome,
        };

        bridge.send_direct(&peer_id, message.clone()).await.unwrap();

        // Should receive via publish fallback
        let received = rx.recv().await.unwrap();
        assert_eq!(received.epoch, message.epoch);
    }
}
