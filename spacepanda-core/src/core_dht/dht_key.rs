/*
    DHTKey - defines how keys are hashed into the DHT keyspace

    Responsibilities:
    `dht_keys.rs` defines the hashing functions and key representations used in the DHT
    It handles: hashing algorithms(XOR space, SHA256, Blake3 -> truncated to 256 bits), comparison operators, distance metrics, key validation.

    This ensures all nodes agree on the same keyspace representation.

    Inputs:
    - raw byte arrays
    - application domain keys (username, channel id, message id, etc)

    Outputs:
    -256-bit DHT keys
    -key distance calculations
    -key comparison operators
*/

use blake3;
use serde::{Deserialize, Serialize};
use std::fmt;

/// 256-bit DHT key for XOR-based keyspace
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DhtKey([u8; 32]);

impl DhtKey {
    /// Create a DhtKey from raw 32 bytes
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        DhtKey(bytes)
    }

    /// Create a DhtKey from a slice (truncates or pads to 32 bytes)
    pub fn from_slice(data: &[u8]) -> Self {
        let mut bytes = [0u8; 32];
        let len = data.len().min(32);
        bytes[..len].copy_from_slice(&data[..len]);
        DhtKey(bytes)
    }

    /// Hash arbitrary data using Blake3 and use full 256 bits
    pub fn hash(data: &[u8]) -> Self {
        let hash = blake3::hash(data);

        // Blake3 produces 32 bytes (256 bits) by default
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(hash.as_bytes());
        DhtKey(bytes)
    }

    /// Hash a string into the DHT keyspace
    pub fn hash_string(s: &str) -> Self {
        Self::hash(s.as_bytes())
    }

    /// Calculate XOR distance between two keys
    pub fn distance(&self, other: &DhtKey) -> DhtKey {
        let mut result = [0u8; 32];
        for i in 0..32 {
            result[i] = self.0[i] ^ other.0[i];
        }
        DhtKey(result)
    }

    /// Get the raw bytes
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Convert to Vec<u8>
    pub fn to_vec(&self) -> Vec<u8> {
        self.0.to_vec()
    }

    /// Count leading zero bits (used for bucket indexing)
    pub fn leading_zeros(&self) -> u32 {
        let mut count = 0;
        for byte in &self.0 {
            if *byte == 0 {
                count += 8;
            } else {
                count += byte.leading_zeros();
                break;
            }
        }
        count
    }

    /// Get the bucket index for this key relative to a reference key
    /// Returns the bit position of the first differing bit
    pub fn bucket_index(&self, reference: &DhtKey) -> usize {
        let distance = self.distance(reference);
        let leading = distance.leading_zeros();

        // Bucket index is 255 - leading_zeros (for 256-bit keys)
        if leading >= 256 {
            0 // Same key
        } else {
            (255 - leading) as usize
        }
    }

    /// Check if this key is closer to a target than another key
    pub fn is_closer(&self, other: &DhtKey, target: &DhtKey) -> bool {
        let dist_self = self.distance(target);
        let dist_other = other.distance(target);
        dist_self < dist_other
    }

    /// Generate a random key (useful for testing)
    #[cfg(test)]
    pub fn random() -> Self {
        use rand::Rng;
        let mut rng = rand::rng();
        let mut bytes = [0u8; 32];
        rng.fill(&mut bytes);
        DhtKey(bytes)
    }
}

impl fmt::Display for DhtKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Display as hex string (first 8 bytes for readability)
        write!(f, "{}", hex::encode(&self.0[..8]))
    }
}

impl PartialOrd for DhtKey {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for DhtKey {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.cmp(&other.0)
    }
}

impl From<[u8; 32]> for DhtKey {
    fn from(bytes: [u8; 32]) -> Self {
        DhtKey(bytes)
    }
}

impl From<DhtKey> for [u8; 32] {
    fn from(key: DhtKey) -> Self {
        key.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dht_key_from_bytes() {
        let bytes = [1u8; 32];
        let key = DhtKey::from_bytes(bytes);
        assert_eq!(key.as_bytes(), &bytes);
    }

    #[test]
    fn test_dht_key_from_slice() {
        let data = vec![1, 2, 3, 4, 5];
        let key = DhtKey::from_slice(&data);

        let bytes = key.as_bytes();
        assert_eq!(bytes[0], 1);
        assert_eq!(bytes[1], 2);
        assert_eq!(bytes[4], 5);
        assert_eq!(bytes[5], 0); // Padded
    }

    #[test]
    fn test_dht_key_hash() {
        let data = b"hello world";
        let key1 = DhtKey::hash(data);
        let key2 = DhtKey::hash(data);

        // Same input produces same hash
        assert_eq!(key1, key2);

        let key3 = DhtKey::hash(b"different");
        assert_ne!(key1, key3);
    }

    #[test]
    fn test_dht_key_hash_string() {
        let key1 = DhtKey::hash_string("alice");
        let key2 = DhtKey::hash_string("alice");
        let key3 = DhtKey::hash_string("bob");

        assert_eq!(key1, key2);
        assert_ne!(key1, key3);
    }

    #[test]
    fn test_dht_key_distance() {
        let key1 = DhtKey::from_bytes([0xFF; 32]);
        let key2 = DhtKey::from_bytes([0x00; 32]);
        let distance = key1.distance(&key2);

        assert_eq!(distance.as_bytes(), &[0xFF; 32]);
    }

    #[test]
    fn test_dht_key_distance_symmetric() {
        let key1 = DhtKey::hash_string("alice");
        let key2 = DhtKey::hash_string("bob");

        let dist1 = key1.distance(&key2);
        let dist2 = key2.distance(&key1);

        assert_eq!(dist1, dist2);
    }

    #[test]
    fn test_dht_key_distance_self_is_zero() {
        let key = DhtKey::hash_string("test");
        let distance = key.distance(&key);

        assert_eq!(distance.as_bytes(), &[0u8; 32]);
    }

    #[test]
    fn test_dht_key_leading_zeros() {
        let key1 = DhtKey::from_bytes([0; 32]);
        assert_eq!(key1.leading_zeros(), 256);

        let mut bytes = [0u8; 32];
        bytes[0] = 0b10000000;
        let key2 = DhtKey::from_bytes(bytes);
        assert_eq!(key2.leading_zeros(), 0);

        let mut bytes = [0u8; 32];
        bytes[0] = 0b00100000;
        let key3 = DhtKey::from_bytes(bytes);
        assert_eq!(key3.leading_zeros(), 2);
    }

    #[test]
    fn test_dht_key_bucket_index() {
        let reference = DhtKey::from_bytes([0; 32]);

        let mut bytes = [0u8; 32];
        bytes[0] = 0b10000000; // First bit differs
        let key1 = DhtKey::from_bytes(bytes);
        assert_eq!(key1.bucket_index(&reference), 255);

        let mut bytes = [0u8; 32];
        bytes[31] = 0b00000001; // Last bit differs
        let key2 = DhtKey::from_bytes(bytes);
        assert_eq!(key2.bucket_index(&reference), 0);
    }

    #[test]
    fn test_dht_key_is_closer() {
        let target = DhtKey::from_bytes([0xFF; 32]);
        let key1 = DhtKey::from_bytes([0xFE; 32]);
        let key2 = DhtKey::from_bytes([0x00; 32]);

        assert!(key1.is_closer(&key2, &target));
        assert!(!key2.is_closer(&key1, &target));
    }

    #[test]
    fn test_dht_key_ordering() {
        let key1 = DhtKey::from_bytes([1; 32]);
        let key2 = DhtKey::from_bytes([2; 32]);

        assert!(key1 < key2);
        assert!(key2 > key1);
        assert_eq!(key1, key1);
    }

    #[test]
    fn test_dht_key_display() {
        let key = DhtKey::from_bytes([0xAB; 32]);
        let display = format!("{}", key);

        // Should show first 8 bytes as hex
        assert!(display.starts_with("abababab"));
    }

    #[test]
    fn test_dht_key_serialization() {
        let key = DhtKey::hash_string("test_key");

        let serialized = serde_json::to_string(&key).unwrap();
        let deserialized: DhtKey = serde_json::from_str(&serialized).unwrap();

        assert_eq!(key, deserialized);
    }
}
