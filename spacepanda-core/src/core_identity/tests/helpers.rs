//! Test helpers and fixtures

use crate::core_identity::*;
use crate::core_store::crdt::{VectorClock, AddId};
use crate::core_store::model::types::Timestamp;

/// Create a test keypair
pub fn test_keypair() -> Keypair {
    Keypair::generate(KeyType::Ed25519)
}

/// Create a test user ID
pub fn test_user_id() -> UserId {
    let kp = test_keypair();
    UserId::from_public_key(kp.public_key())
}

/// Create a test device ID
pub fn test_device_id() -> DeviceId {
    DeviceId::generate()
}

/// Create a vector clock with one increment
pub fn test_vector_clock(node_id: &str) -> VectorClock {
    let mut vc = VectorClock::new();
    vc.increment(node_id);
    vc
}

/// Create test device metadata
pub fn test_device_metadata(node_id: &str) -> DeviceMetadata {
    let device_id = DeviceId::generate();
    DeviceMetadata::new(device_id, "Test Device".to_string(), node_id)
}

/// Create deterministic timestamp for testing (no sleep needed)
/// Uses milliseconds in far future to avoid conflicts with initialization
pub fn test_timestamp(offset_millis: u64) -> Timestamp {
    use std::time::{SystemTime, UNIX_EPOCH};
    // Start from current time + 1 year to ensure we're always after initialization
    let now_millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    Timestamp::from_millis(now_millis + 365 * 24 * 3600 * 1000 + offset_millis)
}

/// Create AddId for testing
pub fn test_add_id(node_id: &str, counter: u64) -> AddId {
    AddId::new(node_id.to_string(), counter)
}

/// Create a VectorClock with N increments for a specific node
pub fn vc_inc(node_id: &str, count: u64) -> VectorClock {
    let mut vc = VectorClock::new();
    for _ in 0..count {
        vc.increment(node_id);
    }
    vc
}
