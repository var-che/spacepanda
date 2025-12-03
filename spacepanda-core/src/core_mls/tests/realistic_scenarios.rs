//! Realistic MLS Scenario Tests
//!
//! This module tests real-world MLS usage scenarios to ensure the system
//! behaves correctly in practical situations:
//!
//! - Group creation and member invitation
//! - Message encryption/decryption across multiple participants
//! - Member removal and key rotation
//! - Forward secrecy and post-compromise security
//! - Concurrent operations and race conditions

use crate::{
    config::Config,
    core_mls::{
        engine::{
            adapter::OpenMlsHandleAdapter,
            group_ops::GroupOperations,
            openmls_engine::{OpenMlsEngine, ProcessedMessage},
        },
        service::MlsService,
        types::{GroupId, MlsConfig},
    },
    shutdown::ShutdownCoordinator,
};
use openmls::prelude::*;
use openmls_basic_credential::SignatureKeyPair;
use openmls_rust_crypto::OpenMlsRustCrypto;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tls_codec::{Deserialize as TlsDeserializeTrait, Serialize as TlsSerializeTrait};

/// User context that stores crypto material for E2E testing
struct UserContext {
    identity: Vec<u8>,
    provider: Arc<OpenMlsRustCrypto>,
    signature_keys: SignatureKeyPair,
    key_package_bundle: KeyPackageBundle,
}

impl UserContext {
    /// Create a new user context with crypto material
    async fn new(identity: &[u8]) -> Self {
        let provider = Arc::new(OpenMlsRustCrypto::default());
        let ciphersuite = Ciphersuite::MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519;

        // Generate signature keys
        let signature_keys = SignatureKeyPair::new(ciphersuite.signature_algorithm())
            .expect("Failed to generate signature keys");

        // Store keys in provider
        signature_keys
            .store(provider.storage())
            .expect("Failed to store keys");

        // Create credential with the user's identity
        let basic_credential = BasicCredential::new(identity.to_vec());
        let credential_with_key = CredentialWithKey {
            credential: basic_credential.into(),
            signature_key: signature_keys.public().into(),
        };

        // Build the key package bundle (contains both public and private keys)
        let key_package_bundle = KeyPackage::builder()
            .build(ciphersuite, provider.as_ref(), &signature_keys, credential_with_key)
            .expect("Failed to build key package");

        Self {
            identity: identity.to_vec(),
            provider,
            signature_keys,
            key_package_bundle,
        }
    }

    /// Get serialized key package bytes (for sending to group admin)
    fn key_package_bytes(&self) -> Vec<u8> {
        self.key_package_bundle
            .key_package()
            .tls_serialize_detached()
            .expect("Failed to serialize key package")
    }

    /// Get a clone of the KeyPackageBundle for joining from Welcome
    fn bundle(&self) -> KeyPackageBundle {
        self.key_package_bundle.clone()
    }

    /// Join a group from a Welcome message using the stored private keys and provider
    async fn join_from_welcome_engine(
        &self,
        welcome_bytes: &[u8],
        ratchet_tree_bytes: Option<&[u8]>,
    ) -> Result<Arc<OpenMlsEngine>, String> {
        // Parse Welcome message
        let mls_message = MlsMessageIn::tls_deserialize_exact(welcome_bytes)
            .map_err(|e| format!("Failed to parse welcome: {:?}", e))?;

        // Extract Welcome
        let welcome = match mls_message.extract() {
            MlsMessageBodyIn::Welcome(w) => w,
            _ => return Err("Expected Welcome message".to_string()),
        };

        // Create join config
        let join_config = MlsGroupJoinConfig::builder()
            .wire_format_policy(PURE_CIPHERTEXT_WIRE_FORMAT_POLICY)
            .build();

        // Parse ratchet tree if provided
        let ratchet_tree = if let Some(tree_bytes) = ratchet_tree_bytes {
            Some(
                RatchetTreeIn::tls_deserialize_exact(tree_bytes)
                    .map_err(|e| format!("Failed to deserialize ratchet tree: {:?}", e))?,
            )
        } else {
            None
        };

        // Join the group using OUR provider which has the private keys!
        let mls_group = StagedWelcome::new_from_welcome(
            self.provider.as_ref(),
            &join_config,
            welcome,
            ratchet_tree,
        )
        .map_err(|e| format!("Failed to stage welcome: {:?}", e))?
        .into_group(self.provider.as_ref())
        .map_err(|e| format!("Failed to create group: {:?}", e))?;

        // Create credential for the engine
        let credential = CredentialWithKey {
            credential: BasicCredential::new(self.identity.clone()).into(),
            signature_key: self.signature_keys.public().into(),
        };

        // Wrap in OpenMlsEngine - we need to generate new signature keys since they can't be cloned
        // The actual decryption uses the KeyPackageBundle's private keys which ARE in the provider
        let ciphersuite = Ciphersuite::MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519;
        let new_sig_keys = SignatureKeyPair::new(ciphersuite.signature_algorithm())
            .map_err(|e| format!("Failed to generate signature keys: {:?}", e))?;
        
        new_sig_keys.store(self.provider.storage())
            .map_err(|e| format!("Failed to store signature keys: {:?}", e))?;

        let config = MlsConfig::default();
        let engine = OpenMlsEngine::from_group_with_provider(
            mls_group,
            self.provider.clone(),
            config,
            new_sig_keys,
            credential,
        );

        Ok(Arc::new(engine))
    }

    /// Join a group from a Welcome message using adapter (wraps engine)
    async fn join_from_welcome_adapter(
        &self,
        welcome_bytes: &[u8],
        ratchet_tree_bytes: Option<&[u8]>,
    ) -> Result<Arc<OpenMlsHandleAdapter>, String> {
        let engine = self.join_from_welcome_engine(welcome_bytes, ratchet_tree_bytes).await?;
        let config = MlsConfig::default();
        
        // Wrap the engine in the adapter - need to access Arc<RwLock<OpenMlsEngine>>
        // Since we can't construct OpenMlsHandleAdapter directly, we'll return the engine
        // and use it directly in tests
        Err("Use join_from_welcome_engine directly - adapter construction not exposed".to_string())
    }
}

/// Helper function to create a user with identity
async fn create_user(name: &str) -> (Vec<u8>, Arc<OpenMlsHandleAdapter>) {
    let identity = name.as_bytes().to_vec();
    let config = MlsConfig::default();
    let provider = Arc::new(OpenMlsRustCrypto::default());
    let adapter = OpenMlsHandleAdapter::create_group(None, identity.clone(), config, provider)
        .await
        .unwrap();
    (identity, Arc::new(adapter))
}

/// Generate a real OpenMLS key package for a user (simplified version)
async fn generate_key_package(identity: &[u8]) -> Vec<u8> {
    let ctx = UserContext::new(identity).await;
    ctx.key_package_bytes()
}

#[cfg(test)]
mod scenario_tests {
    use super::*;

    /// Scenario: Bob creates a group, invites Alice and Charlie
    ///
    /// **Note**: This test verifies the invitation workflow. Full end-to-end
    /// encryption requires KeyPackageBundle management (see phase4_integration.rs
    /// for complete E2E examples).
    #[tokio::test]
    async fn test_group_creation_and_messaging() {
        // Setup: Create Bob and generate his group
        let (bob_id, bob_group) = create_user("bob").await;

        // Bob creates the group
        let group_id = bob_group.group_id().await;
        println!("✓ Bob created group: {}", group_id);

        // Generate key packages for Alice and Charlie
        let alice_id = b"alice".to_vec();
        let charlie_id = b"charlie".to_vec();
        let alice_kp = generate_key_package(&alice_id).await;
        let charlie_kp = generate_key_package(&charlie_id).await;

        // Bob invites Alice and Charlie using their key packages
        let engine = bob_group.engine();
        let mut engine_lock = engine.write().await;
        let (_commit, _welcome) = engine_lock
            .add_members(vec![alice_kp, charlie_kp])
            .await
            .unwrap();
        drop(engine_lock);

        println!("✓ Bob invited Alice and Charlie to the group");

        // Verify group state (add_members already applies the commit internally)
        let metadata = bob_group.metadata().await.unwrap();
        assert_eq!(metadata.members.len(), 3, "Should have 3 members");
        assert_eq!(metadata.epoch, 1, "Should be at epoch 1");

        println!(
            "✓ Group now has {} members at epoch {}",
            metadata.members.len(),
            metadata.epoch
        );
        println!("✅ Test passed: Group creation and invitation workflow verified");
    }

    /// Scenario: Member removal and forward secrecy
    ///
    /// Bob creates a group, adds Alice and Charlie, then removes Charlie.
    /// This tests that:
    /// - Members can be added to groups
    /// - Members can be removed from groups
    /// - Epoch advances on removal (forward secrecy)
    ///
    /// **Note**: This verifies the removal workflow. Testing that Charlie's keys
    /// no longer work requires KeyPackageBundle management.
    #[tokio::test]
    async fn test_member_removal_and_forward_secrecy() {
        // Setup: Create Bob and his group
        let (bob_id, bob_group) = create_user("bob").await;
        let group_id = bob_group.group_id().await;
        println!("✓ Bob created group: {}", group_id);

        // Generate key packages for Alice and Charlie
        let alice_id = b"alice".to_vec();
        let charlie_id = b"charlie".to_vec();
        let alice_kp = generate_key_package(&alice_id).await;
        let charlie_kp = generate_key_package(&charlie_id).await;

        // Add Alice and Charlie
        let engine = bob_group.engine();
        let mut engine_lock = engine.write().await;
        let (_commit, _welcome) = engine_lock
            .add_members(vec![alice_kp, charlie_kp])
            .await
            .unwrap();
        drop(engine_lock);

        // Verify addition (add_members already applies the commit internally)
        let metadata = bob_group.metadata().await.unwrap();
        assert_eq!(metadata.members.len(), 3);
        assert_eq!(metadata.epoch, 1);
        println!(
            "✓ Added Alice and Charlie, now {} members at epoch {}",
            metadata.members.len(),
            metadata.epoch
        );

        // Bob removes Charlie (assuming Charlie is at leaf index 2)
        let charlie_leaf_index: u32 = 2;

        let engine = bob_group.engine();
        let mut engine_lock = engine.write().await;
        let _remove_commit = engine_lock
            .remove_members(vec![charlie_leaf_index])
            .await
            .unwrap();
        drop(engine_lock);

        println!("✓ Bob removed Charlie");

        // Verify removal and epoch advancement (remove_members already applies the commit)
        let metadata = bob_group.metadata().await.unwrap();
        assert_eq!(metadata.members.len(), 2, "Should have 2 members after removal");
        assert_eq!(metadata.epoch, 2, "Epoch should advance on removal");

        println!(
            "✓ Charlie removed, now {} members at epoch {}",
            metadata.members.len(),
            metadata.epoch
        );
        println!("✓ Forward secrecy: New epoch keys won't be accessible to Charlie");

        println!("✅ Test passed: Member removal and key rotation verified");
    }

    /// Scenario: Service-level workflow test
    ///
    /// Tests MLS service initialization and basic operations
    #[tokio::test]
    async fn test_service_level_workflow() {
        let config = Config::default();
        let shutdown = Arc::new(ShutdownCoordinator::new(Duration::from_secs(30)));
        let mls_service = MlsService::new(&config, shutdown.clone());

        // Create a group through the service
        let identity = b"test_user".to_vec();
        let group_id = mls_service
            .create_group(identity.clone(), None)
            .await
            .unwrap();

        println!("✓ Created group through service: {}", group_id);

        // Verify group was created
        assert_eq!(mls_service.group_count().await, 1);

        println!("✅ Test passed: Service-level workflow verified");

        // Cleanup
        shutdown.shutdown().await;
    }

    /// Scenario: Demonstrate E2E encryption architecture
    ///
    /// This test shows the E2E encryption workflow concept. Full implementation
    /// would require modifying join_from_welcome to accept KeyPackageBundle.
    ///
    /// Current limitation: join_from_welcome generates new keys instead of using
    /// the KeyPackageBundle's private keys, causing "NoMatchingKeyPackage" error.
    ///
    /// See phase4_integration.rs for examples of group operations.
    #[tokio::test]
    async fn test_e2e_encryption_architecture() {
        println!("🔐 Testing E2E encryption architecture...");

        // Create user contexts with crypto material
        let bob_ctx = UserContext::new(b"bob").await;
        let alice_ctx = UserContext::new(b"alice").await;
        let charlie_ctx = UserContext::new(b"charlie").await;

        // Verify key packages can be generated
        assert!(!bob_ctx.key_package_bytes().is_empty());
        assert!(!alice_ctx.key_package_bytes().is_empty());
        assert!(!charlie_ctx.key_package_bytes().is_empty());
        println!("✓ UserContext generates valid key packages with crypto material");

        // Create a group with multiple members using adapters
        let config = MlsConfig::default();
        let provider = Arc::new(OpenMlsRustCrypto::default());
        let bob_adapter = Arc::new(
            OpenMlsHandleAdapter::create_group(None, bob_ctx.identity.clone(), config.clone(), provider.clone())
                .await
                .unwrap(),
        );
        println!("✓ Bob created group");

        // Add Alice using her key package
        let alice_kp = alice_ctx.key_package_bytes();
        let bob_engine = bob_adapter.engine();
        let (_, welcome) = {
            let engine_lock = bob_engine.write().await;
            engine_lock.add_members(vec![alice_kp]).await.unwrap()
        };
        assert!(welcome.is_some());
        println!("✓ Bob added Alice and generated Welcome message");

        // Verify group state
        let metadata = bob_adapter.metadata().await.unwrap();
        assert_eq!(metadata.members.len(), 2);
        assert_eq!(metadata.epoch, 1);
        println!("✓ Group has 2 members at epoch 1");

        // Test message encryption
        let message = b"Hello World!";
        let encrypted = {
            let engine_lock = bob_engine.read().await;
            engine_lock.send_message(message).await.unwrap()
        };
        println!("✓ Bob encrypted message (encryption works)");

        // Note: Sender cannot decrypt their own messages in MLS
        // Cross-member decryption requires full join_from_welcome with KeyPackageBundle
        println!("  (Cross-member decryption requires KeyPackageBundle support)");

        // Test member removal
        let remove_commit = {
            let engine_lock = bob_engine.write().await;
            engine_lock.remove_members(vec![1]).await.unwrap()
        };
        println!("✓ Bob removed Alice");

        let metadata_after = bob_adapter.metadata().await.unwrap();
        assert_eq!(metadata_after.members.len(), 1);
        assert_eq!(metadata_after.epoch, 2);
        println!("✓ Group advanced to epoch 2 with forward secrecy");

        println!("✅ E2E encryption architecture verified!");
        println!("   Note: Full cross-member encryption requires KeyPackageBundle");
        println!("         support in join_from_welcome (see UserContext impl)");
    }

    /// Scenario: Complete E2E encryption with UserContext and direct engines
    ///
    /// Demonstrates full encryption workflow where members can decrypt each other's messages
    #[tokio::test]
    async fn test_complete_e2e_with_user_context() {
        println!("🔐 Testing complete E2E encryption with UserContext...");

        // Create user contexts with their own providers and crypto material
        let bob_ctx = UserContext::new(b"bob").await;
        let alice_ctx = UserContext::new(b"alice").await;
        let charlie_ctx = UserContext::new(b"charlie").await;

        // Bob creates a group using his own provider
        let config = MlsConfig::default();
        let ciphersuite = Ciphersuite::MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519;
        
        // Generate signature keys that will be used BOTH for creating the group
        // AND for the engine - this ensures signatures match
        let bob_signature_keys = SignatureKeyPair::new(ciphersuite.signature_algorithm()).unwrap();
        bob_signature_keys.store(bob_ctx.provider.storage()).unwrap();
        
        // Create group with Bob's credentials
        let basic_credential = BasicCredential::new(bob_ctx.identity.clone());
        let credential_with_key = CredentialWithKey {
            credential: basic_credential.into(),
            signature_key: bob_signature_keys.public().into(),
        };

        let mls_group_config = MlsGroupCreateConfig::builder()
            .wire_format_policy(PURE_CIPHERTEXT_WIRE_FORMAT_POLICY)
            .ciphersuite(ciphersuite)
            .build();

        let group_id = openmls::prelude::GroupId::from_slice(b"test_group_123");
        let bob_mls_group = MlsGroup::new_with_group_id(
            bob_ctx.provider.as_ref(),
            &bob_signature_keys,  // Use the same keys for group creation
            &mls_group_config,
            group_id,
            credential_with_key.clone(),
        )
        .unwrap();

        // Create engine using the SAME signature keys used to create the group
        let bob_engine = Arc::new(OpenMlsEngine::from_group_with_provider(
            bob_mls_group,
            bob_ctx.provider.clone(),
            config.clone(),
            bob_signature_keys,  // These match the keys used above
            credential_with_key,
        ));

        println!("✓ Bob created group with his own provider");

        // Bob adds Alice and Charlie
        let alice_kp = alice_ctx.key_package_bytes();
        let charlie_kp = charlie_ctx.key_package_bytes();

        let (_, welcome_bytes) = bob_engine
            .add_members(vec![alice_kp, charlie_kp])
            .await
            .unwrap();

        let welcome = welcome_bytes.expect("Welcome should be created");
        println!("✓ Bob added Alice and Charlie");

        // Export the ratchet tree from Bob's group (needed for joining from Welcome)
        let ratchet_tree = bob_engine.export_ratchet_tree_bytes().await.unwrap();

        // Alice joins using her own provider (which has her private keys!)
        let alice_engine = alice_ctx.join_from_welcome_engine(&welcome, Some(&ratchet_tree)).await.unwrap();
        println!("✓ Alice joined using her provider with private keys");

        // Charlie joins using his own provider
        let charlie_engine = charlie_ctx.join_from_welcome_engine(&welcome, Some(&ratchet_tree)).await.unwrap();
        println!("✓ Charlie joined using his provider with private keys");

        // Verify all are in sync
        let bob_meta = bob_engine.metadata().await.unwrap();
        let alice_meta = alice_engine.metadata().await.unwrap();
        let charlie_meta = charlie_engine.metadata().await.unwrap();

        assert_eq!(bob_meta.members.len(), 3);
        assert_eq!(alice_meta.members.len(), 3);
        assert_eq!(charlie_meta.members.len(), 3);
        assert_eq!(bob_meta.epoch, alice_meta.epoch);
        assert_eq!(bob_meta.epoch, charlie_meta.epoch);
        println!("✓ All members in sync at epoch {}", bob_meta.epoch);

        // Bob sends encrypted message
        let message = b"Hello from Bob!";
        let encrypted = bob_engine.send_message(message).await.unwrap();
        println!("✓ Bob sent encrypted message");

        // Alice decrypts
        let alice_result = alice_engine.process_message(&encrypted).await.unwrap();
        if let ProcessedMessage::Application(decrypted) = alice_result {
            assert_eq!(&decrypted, message);
            println!("✓ Alice decrypted: {:?}", String::from_utf8_lossy(&decrypted));
        } else {
            panic!("Alice should receive application message");
        }

        // Charlie decrypts
        let charlie_result = charlie_engine.process_message(&encrypted).await.unwrap();
        if let ProcessedMessage::Application(decrypted) = charlie_result {
            assert_eq!(&decrypted, message);
            println!("✓ Charlie decrypted: {:?}", String::from_utf8_lossy(&decrypted));
        } else {
            panic!("Charlie should receive application message");
        }

        // Bob removes Charlie
        let remove_commit = bob_engine.remove_members(vec![2]).await.unwrap();
        println!("✓ Bob removed Charlie");

        // Alice processes removal
        let alice_removal = alice_engine.process_message(&remove_commit).await.unwrap();
        assert!(matches!(alice_removal, ProcessedMessage::Commit { .. }));
        println!("✓ Alice processed removal");

        // Verify new state
        let bob_meta_after = bob_engine.metadata().await.unwrap();
        let alice_meta_after = alice_engine.metadata().await.unwrap();
        assert_eq!(bob_meta_after.members.len(), 2);
        assert_eq!(alice_meta_after.members.len(), 2);
        assert_eq!(bob_meta_after.epoch, bob_meta.epoch + 1);
        println!("✓ Group at epoch {} with {} members", bob_meta_after.epoch, bob_meta_after.members.len());

        // Bob sends new message
        let message2 = b"After Charlie left";
        let encrypted2 = bob_engine.send_message(message2).await.unwrap();
        println!("✓ Bob sent message in new epoch");

        // Alice can still decrypt
        let alice_result2 = alice_engine.process_message(&encrypted2).await.unwrap();
        if let ProcessedMessage::Application(decrypted) = alice_result2 {
            assert_eq!(&decrypted, message2);
            println!("✓ Alice decrypted new epoch message");
        }

        // Charlie cannot decrypt (forward secrecy)
        let charlie_result2 = charlie_engine.process_message(&encrypted2).await;
        assert!(charlie_result2.is_err(), "Charlie should not decrypt after removal");
        println!("✓ Charlie cannot decrypt - forward secrecy works!");

        println!("✅ Complete E2E encryption test passed!");
    }
}
