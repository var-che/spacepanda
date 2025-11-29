/*
    DHTValue - defines DHT-stored value format and serialization

    Responsibilities:
    DHT values must include: metadata (timestamp, owner, version), expiration time, CRDT payload (for later when CRDT is implemented), signature (optional), protocol version.
    `dht_value.rs` defines the structure, serialization (e.g. bincode, serde), validation (expiration, signature verification), and versioning of DHT values.

    Format for serialization can be CBOR or bincode for compactness.

    Inputs:
    - raw application values
    - crdt updates
    - signed messages

    Outputs
    - encoded value bytes
    - metadata for replication
    - value validation results
*/

use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::dht_key::DhtKey;

/// Protocol version for DHT values
const PROTOCOL_VERSION: u32 = 1;

/// DHT value with metadata
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DhtValue {
    /// Protocol version
    pub version: u32,
    
    /// The actual data payload
    pub data: Vec<u8>,
    
    /// Owner/publisher of this value (optional)
    pub owner: Option<DhtKey>,
    
    /// Timestamp when value was created (Unix timestamp in seconds)
    pub timestamp: u64,
    
    /// TTL in seconds (time-to-live)
    pub ttl: u64,
    
    /// Sequence number for versioning/updates
    pub sequence: u64,
    
    /// Optional signature for verification
    pub signature: Option<Vec<u8>>,
    
    /// Optional CRDT metadata (for future use)
    pub crdt_metadata: Option<Vec<u8>>,
}

impl DhtValue {
    /// Create a new DHT value
    pub fn new(data: Vec<u8>) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        DhtValue {
            version: PROTOCOL_VERSION,
            data,
            owner: None,
            timestamp: now,
            ttl: 86400, // 24 hours default
            sequence: 0,
            signature: None,
            crdt_metadata: None,
        }
    }
    
    /// Create a value with owner
    pub fn with_owner(mut self, owner: DhtKey) -> Self {
        self.owner = Some(owner);
        self
    }
    
    /// Set TTL (time-to-live) in seconds
    pub fn with_ttl(mut self, ttl: u64) -> Self {
        self.ttl = ttl;
        self
    }
    
    /// Set TTL using Duration
    pub fn with_ttl_duration(mut self, duration: Duration) -> Self {
        self.ttl = duration.as_secs();
        self
    }
    
    /// Set sequence number
    pub fn with_sequence(mut self, sequence: u64) -> Self {
        self.sequence = sequence;
        self
    }
    
    /// Set signature
    pub fn with_signature(mut self, signature: Vec<u8>) -> Self {
        self.signature = Some(signature);
        self
    }
    
    /// Set CRDT metadata
    pub fn with_crdt_metadata(mut self, metadata: Vec<u8>) -> Self {
        self.crdt_metadata = Some(metadata);
        self
    }
    
    /// Check if the value has expired
    pub fn is_expired(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        let expiration = self.timestamp.saturating_add(self.ttl);
        now >= expiration
    }
    
    /// Get expiration timestamp
    pub fn expiration_time(&self) -> u64 {
        self.timestamp.saturating_add(self.ttl)
    }
    
    /// Get time remaining before expiration (in seconds)
    pub fn time_remaining(&self) -> Option<u64> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        let expiration = self.expiration_time();
        if now < expiration {
            Some(expiration - now)
        } else {
            None
        }
    }
    
    /// Validate the value
    pub fn validate(&self, max_size: usize, require_signature: bool) -> Result<(), String> {
        // Check protocol version
        if self.version != PROTOCOL_VERSION {
            return Err(format!(
                "Unsupported protocol version: {} (expected {})",
                self.version, PROTOCOL_VERSION
            ));
        }
        
        // Check size
        if self.data.len() > max_size {
            return Err(format!(
                "Value size {} exceeds maximum {}",
                self.data.len(),
                max_size
            ));
        }
        
        // Check expiration
        if self.is_expired() {
            return Err("Value has expired".to_string());
        }
        
        // Check signature if required
        if require_signature && self.signature.is_none() {
            return Err("Signature required but not present".to_string());
        }
        
        Ok(())
    }
    
    /// Serialize to bytes using JSON (can be switched to bincode later)
    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        serde_json::to_vec(self)
            .map_err(|e| format!("Serialization error: {}", e))
    }
    
    /// Deserialize from bytes
    pub fn from_bytes(data: &[u8]) -> Result<Self, String> {
        serde_json::from_slice(data)
            .map_err(|e| format!("Deserialization error: {}", e))
    }
    
    /// Check if this value is newer than another (based on sequence number)
    pub fn is_newer_than(&self, other: &DhtValue) -> bool {
        self.sequence > other.sequence
    }
    
    /// Merge with another value (keep the newer one)
    pub fn merge(&self, other: &DhtValue) -> DhtValue {
        if self.is_newer_than(other) {
            self.clone()
        } else {
            other.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dht_value_new() {
        let data = b"test data".to_vec();
        let value = DhtValue::new(data.clone());
        
        assert_eq!(value.data, data);
        assert_eq!(value.version, PROTOCOL_VERSION);
        assert!(value.timestamp > 0);
        assert_eq!(value.ttl, 86400);
        assert_eq!(value.sequence, 0);
        assert!(value.owner.is_none());
        assert!(value.signature.is_none());
    }

    #[test]
    fn test_dht_value_with_owner() {
        let owner_key = DhtKey::hash_string("owner");
        let value = DhtValue::new(vec![1, 2, 3]).with_owner(owner_key);
        
        assert_eq!(value.owner, Some(owner_key));
    }

    #[test]
    fn test_dht_value_with_ttl() {
        let value = DhtValue::new(vec![1, 2, 3]).with_ttl(3600);
        assert_eq!(value.ttl, 3600);
    }

    #[test]
    fn test_dht_value_with_ttl_duration() {
        let value = DhtValue::new(vec![1, 2, 3])
            .with_ttl_duration(Duration::from_secs(7200));
        assert_eq!(value.ttl, 7200);
    }

    #[test]
    fn test_dht_value_with_sequence() {
        let value = DhtValue::new(vec![1, 2, 3]).with_sequence(42);
        assert_eq!(value.sequence, 42);
    }

    #[test]
    fn test_dht_value_with_signature() {
        let signature = vec![0xDE, 0xAD, 0xBE, 0xEF];
        let value = DhtValue::new(vec![1, 2, 3])
            .with_signature(signature.clone());
        assert_eq!(value.signature, Some(signature));
    }

    #[test]
    fn test_dht_value_not_expired() {
        let value = DhtValue::new(vec![1, 2, 3]).with_ttl(3600);
        assert!(!value.is_expired());
    }

    #[test]
    fn test_dht_value_expired() {
        let mut value = DhtValue::new(vec![1, 2, 3]);
        // Set timestamp to past
        value.timestamp = 0;
        value.ttl = 1;
        
        assert!(value.is_expired());
    }

    #[test]
    fn test_dht_value_expiration_time() {
        let value = DhtValue::new(vec![1, 2, 3]).with_ttl(3600);
        let expiration = value.expiration_time();
        
        assert_eq!(expiration, value.timestamp + 3600);
    }

    #[test]
    fn test_dht_value_time_remaining() {
        let value = DhtValue::new(vec![1, 2, 3]).with_ttl(3600);
        let remaining = value.time_remaining();
        
        assert!(remaining.is_some());
        assert!(remaining.unwrap() <= 3600);
    }

    #[test]
    fn test_dht_value_time_remaining_expired() {
        let mut value = DhtValue::new(vec![1, 2, 3]);
        value.timestamp = 0;
        value.ttl = 1;
        
        assert!(value.time_remaining().is_none());
    }

    #[test]
    fn test_dht_value_validate_success() {
        let value = DhtValue::new(vec![1, 2, 3]);
        let result = value.validate(1024, false);
        
        assert!(result.is_ok());
    }

    #[test]
    fn test_dht_value_validate_size_exceeded() {
        let value = DhtValue::new(vec![0u8; 2000]);
        let result = value.validate(1000, false);
        
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("exceeds maximum"));
    }

    #[test]
    fn test_dht_value_validate_expired() {
        let mut value = DhtValue::new(vec![1, 2, 3]);
        value.timestamp = 0;
        value.ttl = 1;
        
        let result = value.validate(1024, false);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("expired"));
    }

    #[test]
    fn test_dht_value_validate_signature_required() {
        let value = DhtValue::new(vec![1, 2, 3]);
        let result = value.validate(1024, true);
        
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Signature required"));
    }

    #[test]
    fn test_dht_value_validate_with_signature() {
        let value = DhtValue::new(vec![1, 2, 3])
            .with_signature(vec![0xAA, 0xBB]);
        let result = value.validate(1024, true);
        
        assert!(result.is_ok());
    }

    #[test]
    fn test_dht_value_serialization() {
        let value = DhtValue::new(vec![1, 2, 3, 4, 5])
            .with_owner(DhtKey::hash_string("test"))
            .with_ttl(7200)
            .with_sequence(10);
        
        let bytes = value.to_bytes().unwrap();
        let decoded = DhtValue::from_bytes(&bytes).unwrap();
        
        assert_eq!(value, decoded);
    }

    #[test]
    fn test_dht_value_is_newer_than() {
        let value1 = DhtValue::new(vec![1, 2, 3]).with_sequence(5);
        let value2 = DhtValue::new(vec![4, 5, 6]).with_sequence(3);
        
        assert!(value1.is_newer_than(&value2));
        assert!(!value2.is_newer_than(&value1));
    }

    #[test]
    fn test_dht_value_merge() {
        let value1 = DhtValue::new(vec![1, 2, 3]).with_sequence(5);
        let value2 = DhtValue::new(vec![4, 5, 6]).with_sequence(10);
        
        let merged = value1.merge(&value2);
        assert_eq!(merged.sequence, 10);
        assert_eq!(merged.data, vec![4, 5, 6]);
    }

    #[test]
    fn test_dht_value_with_crdt_metadata() {
        let metadata = vec![0x01, 0x02, 0x03];
        let value = DhtValue::new(vec![1, 2, 3])
            .with_crdt_metadata(metadata.clone());
        
        assert_eq!(value.crdt_metadata, Some(metadata));
    }
}
