//! Test fixtures for creating common test objects
//!
//! Provides builder patterns and factory functions for creating test data.

use crate::core_identity::*;
use crate::core_router::{PeerId, PeerInfo, Capability};
use crate::core_store::crdt::{VectorClock, AddId};
use crate::core_store::model::types::Timestamp;
use crate::core_store::model::{Space, Channel, SpaceId, ChannelId, UserId, ChannelType};
use crate::core_dht::{DhtKey, DhtValue};
use std::net::SocketAddr;

/// Builder for creating test peer IDs
pub struct TestPeerIdBuilder {
    id: u8,
}

impl TestPeerIdBuilder {
    pub fn new(id: u8) -> Self {
        Self { id }
    }

    pub fn build(self) -> PeerId {
        PeerId::from_bytes(vec![self.id])
    }
}

/// Builder for creating test peer info
pub struct TestPeerBuilder {
    id: u8,
    addresses: Vec<String>,
    capabilities: Vec<Capability>,
    asn: Option<u32>,
    latency_ms: Option<u64>,
}

impl TestPeerBuilder {
    pub fn new(id: u8) -> Self {
        Self {
            id,
            addresses: vec![format!("127.0.0.1:{}00", id)],
            capabilities: vec![],
            asn: None,
            latency_ms: None,
        }
    }

    pub fn with_relay(mut self) -> Self {
        self.capabilities.push(Capability::Relay);
        self
    }

    pub fn with_asn(mut self, asn: u32) -> Self {
        self.asn = Some(asn);
        self
    }

    pub fn with_latency(mut self, latency_ms: u64) -> Self {
        self.latency_ms = Some(latency_ms);
        self
    }

    pub fn with_address(mut self, addr: impl Into<String>) -> Self {
        self.addresses = vec![addr.into()];
        self
    }

    pub fn build(self) -> PeerInfo {
        let mut peer = PeerInfo::new(
            PeerId::from_bytes(vec![self.id]),
            self.addresses,
        );
        peer.capabilities = self.capabilities;
        peer.asn = self.asn;
        peer.latency = self.latency_ms.map(std::time::Duration::from_millis);
        peer
    }
}

/// Builder for creating test spaces
pub struct TestSpaceBuilder {
    name: String,
    creator: UserId,
}

impl TestSpaceBuilder {
    pub fn new() -> Self {
        Self {
            name: "Test Space".to_string(),
            creator: UserId::generate(),
        }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    pub fn with_creator(mut self, creator: UserId) -> Self {
        self.creator = creator;
        self
    }

    pub fn build(self) -> Space {
        Space::new(
            SpaceId::generate(),
            self.name,
            self.creator,
            Timestamp::now(),
            "test_node".to_string(),
        )
    }
}

impl Default for TestSpaceBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for creating test channels
pub struct TestChannelBuilder {
    name: String,
    channel_type: ChannelType,
    creator: UserId,
}

impl TestChannelBuilder {
    pub fn new() -> Self {
        Self {
            name: "general".to_string(),
            channel_type: ChannelType::Text,
            creator: UserId::generate(),
        }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    pub fn with_type(mut self, channel_type: ChannelType) -> Self {
        self.channel_type = channel_type;
        self
    }

    pub fn with_creator(mut self, creator: UserId) -> Self {
        self.creator = creator;
        self
    }

    pub fn build(self) -> Channel {
        Channel::new(
            ChannelId::generate(),
            self.name,
            self.channel_type,
            self.creator,
            Timestamp::now(),
            "test_node".to_string(),
        )
    }
}

impl Default for TestChannelBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Quick fixture functions for common test objects

pub fn test_peer_id(id: u8) -> PeerId {
    TestPeerIdBuilder::new(id).build()
}

pub fn test_peer(id: u8) -> PeerInfo {
    TestPeerBuilder::new(id).build()
}

pub fn test_relay_peer(id: u8) -> PeerInfo {
    TestPeerBuilder::new(id).with_relay().build()
}

pub fn test_space() -> Space {
    TestSpaceBuilder::new().build()
}

pub fn test_channel() -> Channel {
    TestChannelBuilder::new().build()
}

pub fn test_keypair() -> Keypair {
    Keypair::generate(KeyType::Ed25519)
}

pub fn test_user_id() -> UserId {
    UserId::generate()
}

pub fn test_device_id() -> DeviceId {
    DeviceId::generate()
}

pub fn test_vector_clock(node_id: &str) -> VectorClock {
    let mut vc = VectorClock::new();
    vc.increment(node_id);
    vc
}

pub fn test_device_metadata(node_id: &str) -> DeviceMetadata {
    let device_id = DeviceId::generate();
    DeviceMetadata::new(device_id, "Test Device".to_string(), node_id)
}

pub fn test_timestamp(offset_millis: u64) -> Timestamp {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now_millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    Timestamp::from_millis(now_millis + 365 * 24 * 3600 * 1000 + offset_millis)
}

pub fn test_add_id(node_id: &str, counter: u64) -> AddId {
    AddId::new(node_id.to_string(), counter)
}

pub fn vc_inc(node_id: &str, count: u64) -> VectorClock {
    let mut vc = VectorClock::new();
    for _ in 0..count {
        vc.increment(node_id);
    }
    vc
}

pub fn test_dht_key(value: &str) -> DhtKey {
    DhtKey::hash_string(value)
}

pub fn test_dht_value(data: Vec<u8>) -> DhtValue {
    DhtValue::new(data)
}
