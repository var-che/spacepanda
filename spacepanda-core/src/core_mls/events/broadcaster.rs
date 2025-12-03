//! Event Broadcasting System
//!
//! Provides async event broadcasting for MLS state changes to other subsystems.

use crate::core_mls::events::MlsEvent;
use std::sync::Arc;
use tokio::sync::broadcast;

/// Event broadcaster for MLS events
///
/// Uses tokio broadcast channels to emit events to multiple subscribers.
/// This allows the CRDT layer, router, and other subsystems to react to
/// MLS state changes.
#[derive(Clone)]
pub struct EventBroadcaster {
    tx: broadcast::Sender<MlsEvent>,
}

impl EventBroadcaster {
    /// Create a new event broadcaster
    ///
    /// # Arguments
    /// * `capacity` - Channel capacity (number of events buffered)
    pub fn new(capacity: usize) -> Self {
        let (tx, _rx) = broadcast::channel(capacity);
        Self { tx }
    }
    
    /// Emit an event to all subscribers
    ///
    /// # Arguments
    /// * `event` - The event to broadcast
    ///
    /// # Returns
    /// Number of active subscribers that received the event
    pub fn emit(&self, event: MlsEvent) -> usize {
        match self.tx.send(event) {
            Ok(count) => count,
            Err(_) => 0, // No active receivers
        }
    }
    
    /// Emit multiple events
    ///
    /// # Arguments
    /// * `events` - Vector of events to broadcast
    pub fn emit_many(&self, events: Vec<MlsEvent>) {
        for event in events {
            let _ = self.emit(event);
        }
    }
    
    /// Subscribe to events
    ///
    /// # Returns
    /// A receiver for MLS events
    pub fn subscribe(&self) -> broadcast::Receiver<MlsEvent> {
        self.tx.subscribe()
    }
    
    /// Get number of active subscribers
    pub fn subscriber_count(&self) -> usize {
        self.tx.receiver_count()
    }
}

impl Default for EventBroadcaster {
    fn default() -> Self {
        Self::new(100) // Default capacity of 100 events
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core_mls::events::MlsEvent;
    
    #[tokio::test]
    async fn test_broadcaster_creation() {
        let broadcaster = EventBroadcaster::new(10);
        assert_eq!(broadcaster.subscriber_count(), 0);
    }
    
    #[tokio::test]
    async fn test_subscribe_and_receive() {
        let broadcaster = EventBroadcaster::new(10);
        let mut rx = broadcaster.subscribe();
        
        assert_eq!(broadcaster.subscriber_count(), 1);
        
        let event = MlsEvent::GroupCreated {
            group_id: vec![1, 2, 3],
            creator_id: vec![4, 5, 6],
        };
        
        broadcaster.emit(event.clone());
        
        let received = rx.recv().await.unwrap();
        assert_eq!(received.group_id(), event.group_id());
    }
    
    #[tokio::test]
    async fn test_multiple_subscribers() {
        let broadcaster = EventBroadcaster::new(10);
        let mut rx1 = broadcaster.subscribe();
        let mut rx2 = broadcaster.subscribe();
        let mut rx3 = broadcaster.subscribe();
        
        assert_eq!(broadcaster.subscriber_count(), 3);
        
        let event = MlsEvent::EpochChanged {
            group_id: vec![1, 2, 3],
            old_epoch: 0,
            new_epoch: 1,
        };
        
        let count = broadcaster.emit(event.clone());
        assert_eq!(count, 3); // All 3 subscribers received it
        
        // Verify all subscribers got the event
        let r1 = rx1.recv().await.unwrap();
        let r2 = rx2.recv().await.unwrap();
        let r3 = rx3.recv().await.unwrap();
        
        assert_eq!(r1.group_id(), event.group_id());
        assert_eq!(r2.group_id(), event.group_id());
        assert_eq!(r3.group_id(), event.group_id());
    }
    
    #[tokio::test]
    async fn test_emit_many() {
        let broadcaster = EventBroadcaster::new(10);
        let mut rx = broadcaster.subscribe();
        
        let events = vec![
            MlsEvent::GroupCreated {
                group_id: vec![1],
                creator_id: vec![2],
            },
            MlsEvent::EpochChanged {
                group_id: vec![1],
                old_epoch: 0,
                new_epoch: 1,
            },
            MlsEvent::MessageReceived {
                group_id: vec![1],
                sender_id: vec![2],
                epoch: 1,
                plaintext: vec![10, 20, 30],
            },
        ];
        
        broadcaster.emit_many(events.clone());
        
        // Receive all 3 events
        for expected in events {
            let received = rx.recv().await.unwrap();
            assert_eq!(received.group_id(), expected.group_id());
        }
    }
    
    #[tokio::test]
    async fn test_no_subscribers() {
        let broadcaster = EventBroadcaster::new(10);
        
        let event = MlsEvent::GroupCreated {
            group_id: vec![1],
            creator_id: vec![2],
        };
        
        let count = broadcaster.emit(event);
        assert_eq!(count, 0); // No subscribers, returns 0
    }
    
    #[tokio::test]
    async fn test_dropped_subscriber() {
        let broadcaster = EventBroadcaster::new(10);
        
        {
            let _rx = broadcaster.subscribe();
            assert_eq!(broadcaster.subscriber_count(), 1);
        } // rx dropped here
        
        // Give tokio time to process the drop
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        
        assert_eq!(broadcaster.subscriber_count(), 0);
    }
}
