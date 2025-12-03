//! In-memory keystore for testing

use super::{Keystore, KeystoreError};
use crate::core_identity::device_id::DeviceId;
use crate::core_identity::keypair::Keypair;
use std::collections::HashMap;
use std::sync::{Arc, PoisonError, RwLock};

/// Helper to convert poison errors into KeystoreError
fn handle_poison<T>(_err: PoisonError<T>) -> KeystoreError {
    KeystoreError::Other("Lock poisoned: a thread panicked while holding the lock".to_string())
}

/// In-memory keystore (non-persistent, for tests)
#[derive(Clone)]
pub struct MemoryKeystore {
    identity: Arc<RwLock<Option<Keypair>>>,
    devices: Arc<RwLock<HashMap<DeviceId, Keypair>>>,
}

impl MemoryKeystore {
    /// Create a new memory keystore
    pub fn new() -> Self {
        MemoryKeystore {
            identity: Arc::new(RwLock::new(None)),
            devices: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for MemoryKeystore {
    fn default() -> Self {
        Self::new()
    }
}

impl Keystore for MemoryKeystore {
    fn load_identity_keypair(&self) -> Result<Keypair, KeystoreError> {
        self.identity
            .read()
            .map_err(handle_poison)?
            .clone()
            .ok_or_else(|| KeystoreError::NotFound("Identity keypair not found".to_string()))
    }

    fn save_identity_keypair(&self, kp: &Keypair) -> Result<(), KeystoreError> {
        *self.identity.write().map_err(handle_poison)? = Some(kp.clone());
        Ok(())
    }

    fn load_device_keypair(&self, device_id: &DeviceId) -> Result<Keypair, KeystoreError> {
        self.devices
            .read()
            .map_err(handle_poison)?
            .get(device_id)
            .cloned()
            .ok_or_else(|| {
                KeystoreError::NotFound(format!("Device keypair not found: {}", device_id))
            })
    }

    fn save_device_keypair(&self, device_id: &DeviceId, kp: &Keypair) -> Result<(), KeystoreError> {
        self.devices
            .write()
            .map_err(handle_poison)?
            .insert(device_id.clone(), kp.clone());
        Ok(())
    }

    fn list_devices(&self) -> Result<Vec<DeviceId>, KeystoreError> {
        Ok(self.devices.read().map_err(handle_poison)?.keys().cloned().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core_identity::keypair::KeyType;

    #[test]
    fn test_memory_keystore_roundtrip() {
        let keystore = MemoryKeystore::new();

        let kp = Keypair::generate(KeyType::Ed25519);
        keystore.save_identity_keypair(&kp).unwrap();

        let loaded = keystore.load_identity_keypair().unwrap();
        assert_eq!(kp.public_key(), loaded.public_key());
    }

    #[test]
    fn test_memory_keystore_devices() {
        let keystore = MemoryKeystore::new();

        let device1 = DeviceId::generate();
        let device2 = DeviceId::generate();

        let kp1 = Keypair::generate(KeyType::Ed25519);
        let kp2 = Keypair::generate(KeyType::Ed25519);

        keystore.save_device_keypair(&device1, &kp1).unwrap();
        keystore.save_device_keypair(&device2, &kp2).unwrap();

        let devices = keystore.list_devices().unwrap();
        assert_eq!(devices.len(), 2);

        let loaded1 = keystore.load_device_keypair(&device1).unwrap();
        assert_eq!(kp1.public_key(), loaded1.public_key());
    }
}
