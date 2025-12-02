# MLS Integration Plan - OpenMLS 0.7.1

## Executive Summary

This document outlines the implementation plan for integrating OpenMLS 0.7.1 into SpacePanda for secure group communication. The plan leverages the updated OpenMLS API and Blake3 hashing.

**Status**: Dependencies updated (openmls 0.7.1, openmls_rust_crypto 0.4, blake3 1.5)  
**Target**: Secure group messaging with forward secrecy and post-compromise security

---

## 1. OpenMLS 0.7.1 API Overview

### Key Components

**Provider Pattern (NEW in 0.7)**

```rust
use openmls_rust_crypto::OpenMlsRustCrypto;

let provider = &OpenMlsRustCrypto::default();
// Provider handles: crypto, storage, random number generation
```

**Ciphersuite (MTI - Must To Implement)**

```rust
use openmls::prelude::Ciphersuite;

let ciphersuite = Ciphersuite::MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519;
// X25519 for ECDH, AES-128-GCM for AEAD, SHA-256 for hashing, Ed25519 for signatures
```

**Credential & SignatureKeyPair**

```rust
use openmls_basic_credential::SignatureKeyPair;
use openmls::prelude::*;

// Generate signature keys
let signature_keys = SignatureKeyPair::new(
    signature_algorithm.into()
)?;

// Store keys (REQUIRED for later retrieval)
signature_keys.store(provider.storage())?;

// Create credential
let credential = BasicCredential::new(identity_bytes.to_vec());
let credential_with_key = CredentialWithKey {
    credential: credential.into(),
    signature_key: signature_keys.public().into(),
};
```

**Key Package Generation (Builder Pattern)**

```rust
use openmls::prelude::*;

let key_package = KeyPackage::builder()
    .leaf_node_capabilities(capabilities)
    .build(
        ciphersuite,
        provider,
        &signature_keys,
        credential_with_key,
    )?;

// Publish key_package to Delivery Service (our DHT)
```

**Group Creation**

```rust
use openmls::prelude::*;

// Create group
let mut group = MlsGroup::new(
    provider,
    &signature_keys,
    &MlsGroupCreateConfig::default(),
    credential_with_key,
)?;

// Returns group_id: GroupId
let group_id = group.group_id().clone();
```

**Adding Members**

```rust
// Add members by their KeyPackages
let (commit_msg, welcome_msg, group_info) = group.add_members(
    provider,
    &signature_keys,
    &[key_package1, key_package2],
)?;

// Merge pending commit (REQUIRED!)
group.merge_pending_commit(provider)?;

// Send commit_msg to all group members (via DHT)
// Send welcome_msg to new members only (via DHT)
```

**Joining a Group**

```rust
use openmls::group::StagedWelcome;

// Receive Welcome message from inviter
let staged_welcome = StagedWelcome::new_from_welcome(
    provider,
    &MlsGroupJoinConfig::default(),
    welcome,
    Some(ratchet_tree), // Optional performance optimization
)?;

// Convert to active group
let mut group = staged_welcome.into_group(provider)?;
```

**Sending Messages**

```rust
// Create encrypted application message
let message = group.create_message(
    provider,
    &signature_keys,
    b"Hello, secure group!",
)?;

// Broadcast to all group members via DHT
```

**Receiving Messages**

```rust
use openmls::group::ProcessedMessage;

// Process incoming MLS message
let processed = group.process_message(
    provider,
    protocol_message,
)?;

match processed.into_content() {
    ProcessedMessageContent::ApplicationMessage(app_msg) => {
        let plaintext = app_msg.into_bytes();
        // Handle application message
    },
    ProcessedMessageContent::ProposalMessage(proposal) => {
        // Proposal stored automatically in group's proposal store
    },
    ProcessedMessageContent::StagedCommitMessage(staged_commit) => {
        // Validate and merge
        group.merge_staged_commit(provider, *staged_commit)?;
    },
    _ => {},
}
```

**Removing Members**

```rust
// Remove by leaf index
let (commit_msg, welcome_opt, group_info) = group.remove_members(
    provider,
    &signature_keys,
    &[member_leaf_index],
)?;

group.merge_pending_commit(provider)?;

// Broadcast commit_msg to all remaining members
```

---

## 2. Storage Requirements

OpenMLS 0.7 **requires** persistent storage for:

1. **Signature Keys** - Must survive restarts
2. **Encryption Keys** - Key package private keys
3. **Group State** - Current epoch, tree, proposals
4. **PSKs** - Pre-shared keys (optional)

**Implementation Options**:

### Option A: Use OpenMlsRustCrypto Default (In-Memory)

```rust
// Simple but loses state on restart
let provider = OpenMlsRustCrypto::default();
```

⚠️ **Not suitable for production** - groups lost on restart

### Option B: Implement Custom Storage (Recommended)

```rust
use openmls::storage::*;

struct SpacePandaStorage {
    // Use existing spacepanda-core storage layer
    store: Arc<KeyValueStore>,
}

impl StorageProvider for SpacePandaStorage {
    type Error = StorageError;

    fn write<V: SerializableStorage>(&self, k: &[u8], v: &V) -> Result<(), Self::Error> {
        let serialized = v.tls_serialize_detached()?;
        self.store.put(k, &serialized)
    }

    fn read<V: DeserializableStorage>(&self, k: &[u8]) -> Result<Option<V>, Self::Error> {
        match self.store.get(k)? {
            Some(bytes) => Ok(Some(V::tls_deserialize_exact(&bytes)?)),
            None => Ok(None),
        }
    }

    fn delete(&self, k: &[u8]) -> Result<(), Self::Error> {
        self.store.delete(k)
    }
}
```

### Option C: File-Based Storage (Dev/Testing)

```rust
use openmls_traits::storage::*;
use std::collections::HashMap;
use std::sync::RwLock;

struct FileBackedStorage {
    path: PathBuf,
    cache: RwLock<HashMap<Vec<u8>, Vec<u8>>>,
}

// Serialize to JSON/bincode for debugging
```

---

## 3. Integration Architecture

```
┌─────────────────────────────────────────────────────────┐
│                   SpacePanda MLS Layer                  │
├─────────────────────────────────────────────────────────┤
│                                                           │
│  ┌─────────────┐  ┌──────────────┐  ┌────────────────┐  │
│  │ MlsGroupMgr │  │ KeyPackageMgr│  │ WelcomeHandler │  │
│  └─────────────┘  └──────────────┘  └────────────────┘  │
│         │                 │                   │          │
│         └─────────────────┴───────────────────┘          │
│                          │                               │
│                   ┌──────▼───────┐                       │
│                   │   MlsGroup   │ (OpenMLS)             │
│                   └──────┬───────┘                       │
│                          │                               │
│              ┌───────────┴───────────┐                   │
│              │                       │                   │
│     ┌────────▼────────┐   ┌──────────▼─────────┐        │
│     │ OpenMlsProvider │   │ SpacePandaStorage  │        │
│     │  (RustCrypto)   │   │  (Custom Storage)  │        │
│     └─────────────────┘   └────────────────────┘        │
│                                     │                    │
└─────────────────────────────────────┼────────────────────┘
                                      │
                        ┌─────────────▼──────────────┐
                        │   Existing SpacePanda      │
                        │   Storage Layer (CRDT)     │
                        └────────────────────────────┘
                                      │
                        ┌─────────────▼──────────────┐
                        │   DHT Transport Layer      │
                        │   (Message Distribution)   │
                        └────────────────────────────┘
```

---

## 4. Module Structure

```
spacepanda-core/src/
├── core_mls/                       # NEW MODULE
│   ├── mod.rs                      # Public API
│   ├── group_manager.rs            # MlsGroup lifecycle
│   ├── key_package_manager.rs      # KeyPackage generation/distribution
│   ├── welcome_handler.rs          # Welcome message processing
│   ├── storage.rs                  # Custom storage implementation
│   ├── provider.rs                 # OpenMlsProvider wrapper
│   ├── message_processor.rs        # Process incoming MLS messages
│   ├── crypto.rs                   # Signature key management
│   └── types.rs                    # MLS type wrappers
│
├── core_dht/                       # EXISTING - Enhanced for MLS
│   ├── key_package_store.rs        # NEW: Store/retrieve KeyPackages
│   └── mls_message_router.rs      # NEW: Route MLS messages
│
└── core_identity/                  # EXISTING - Enhanced for MLS
    └── mls_credential.rs           # NEW: MLS credential from SpacePanda identity
```

---

## 5. Implementation Phases

### Phase 1: Storage Layer (Week 1)

**Deliverables**:

- [ ] Implement `SpacePandaStorage: StorageProvider`
- [ ] Add TLS serialization helpers
- [ ] Unit tests for storage CRUD operations
- [ ] Integration with existing KeyValueStore

**Files to Create**:

- `core_mls/storage.rs`
- `core_mls/storage_tests.rs`

**Dependencies**: Existing storage layer, TLS serialization traits

---

### Phase 2: Credential & Key Management (Week 1-2)

**Deliverables**:

- [ ] Generate MLS credentials from SpacePanda identities
- [ ] SignatureKeyPair generation and storage
- [ ] KeyPackage generation (builder pattern)
- [ ] KeyPackage distribution via DHT
- [ ] KeyPackage verification

**Files to Create**:

- `core_mls/crypto.rs`
- `core_mls/key_package_manager.rs`
- `core_identity/mls_credential.rs`
- `core_dht/key_package_store.rs`

**Example**:

```rust
pub struct MlsCredentialManager {
    identity: Arc<Identity>,
    storage: Arc<SpacePandaStorage>,
}

impl MlsCredentialManager {
    pub async fn generate_credential(&self) -> Result<CredentialWithKey> {
        let identity_pubkey = self.identity.public_key();
        let user_id = UserId::from_public_key(&identity_pubkey);

        // Create BasicCredential from UserId
        let credential = BasicCredential::new(user_id.as_bytes().to_vec());

        // Generate signature keys
        let signature_keys = SignatureKeyPair::new(
            SignatureScheme::ED25519.into()
        )?;

        // Store keys
        signature_keys.store(&self.storage)?;

        Ok(CredentialWithKey {
            credential: credential.into(),
            signature_key: signature_keys.public().into(),
        })
    }
}
```

---

### Phase 3: Group Management (Week 2-3)

**Deliverables**:

- [ ] Create groups (MlsGroup::new)
- [ ] Add members (add_members)
- [ ] Remove members (remove_members)
- [ ] Leave group (leave_group)
- [ ] Merge pending commits
- [ ] Export group info

**Files to Create**:

- `core_mls/group_manager.rs`
- `core_mls/provider.rs`

**Example**:

```rust
pub struct MlsGroupManager {
    provider: Arc<OpenMlsRustCrypto>,
    storage: Arc<SpacePandaStorage>,
    credential_mgr: Arc<MlsCredentialManager>,
}

impl MlsGroupManager {
    pub async fn create_group(&self, group_name: String) -> Result<GroupId> {
        let ciphersuite = Ciphersuite::MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519;
        let credential = self.credential_mgr.get_credential()?;
        let signer = self.credential_mgr.get_signer()?;

        let mut group = MlsGroup::new(
            &*self.provider,
            &signer,
            &MlsGroupCreateConfig::builder()
                .wire_format_policy(PURE_CIPHERTEXT_WIRE_FORMAT_POLICY)
                .build(),
            credential,
        )?;

        let group_id = group.group_id().clone();

        // Persist group state
        self.storage.write_group(&group_id, &group)?;

        Ok(group_id)
    }

    pub async fn add_members(
        &self,
        group_id: &GroupId,
        key_packages: Vec<KeyPackage>,
    ) -> Result<(MlsMessageOut, MlsMessageOut)> {
        let mut group = self.storage.read_group(group_id)?;
        let signer = self.credential_mgr.get_signer()?;

        let (commit, welcome, _group_info) = group.add_members(
            &*self.provider,
            &signer,
            &key_packages,
        )?;

        // CRITICAL: Must merge pending commit
        group.merge_pending_commit(&*self.provider)?;

        // Persist updated group state
        self.storage.write_group(group_id, &group)?;

        Ok((commit, welcome))
    }
}
```

---

### Phase 4: Message Processing (Week 3-4)

**Deliverables**:

- [ ] Process incoming MLS messages
- [ ] Handle proposals (add, remove, update)
- [ ] Handle commits (merge staged commits)
- [ ] Handle application messages (decrypt)
- [ ] Send encrypted messages
- [ ] Message routing via DHT

**Files to Create**:

- `core_mls/message_processor.rs`
- `core_dht/mls_message_router.rs`

**Example**:

```rust
pub struct MlsMessageProcessor {
    group_mgr: Arc<MlsGroupManager>,
    dht: Arc<DhtNode>,
}

impl MlsMessageProcessor {
    pub async fn process_message(
        &self,
        group_id: &GroupId,
        protocol_message: ProtocolMessage,
    ) -> Result<ProcessedContent> {
        let mut group = self.group_mgr.load_group(group_id)?;
        let provider = self.group_mgr.provider();

        let processed = group.process_message(provider, protocol_message)?;

        match processed.into_content() {
            ProcessedMessageContent::ApplicationMessage(app_msg) => {
                let plaintext = app_msg.into_bytes();

                // Update group state
                self.group_mgr.save_group(group_id, &group)?;

                Ok(ProcessedContent::Application(plaintext))
            },
            ProcessedMessageContent::ProposalMessage(_proposal) => {
                // Proposal stored in group automatically
                self.group_mgr.save_group(group_id, &group)?;
                Ok(ProcessedContent::Proposal)
            },
            ProcessedMessageContent::StagedCommitMessage(staged_commit) => {
                // Merge commit
                group.merge_staged_commit(provider, *staged_commit)?;
                self.group_mgr.save_group(group_id, &group)?;
                Ok(ProcessedContent::Commit)
            },
            _ => Ok(ProcessedContent::Other),
        }
    }

    pub async fn send_message(
        &self,
        group_id: &GroupId,
        plaintext: &[u8],
    ) -> Result<()> {
        let mut group = self.group_mgr.load_group(group_id)?;
        let signer = self.group_mgr.get_signer()?;
        let provider = self.group_mgr.provider();

        let message = group.create_message(provider, &signer, plaintext)?;

        // Broadcast to all group members via DHT
        let members = group.members().collect::<Vec<_>>();
        for member in members {
            let peer_id = self.member_to_peer_id(member)?;
            self.dht.send_mls_message(peer_id, message.clone()).await?;
        }

        Ok(())
    }
}
```

---

### Phase 5: Welcome Handler (Week 4)

**Deliverables**:

- [ ] Process Welcome messages
- [ ] Join groups via Welcome
- [ ] Retrieve ratchet tree (optional optimization)
- [ ] Validate group state after joining

**Files to Create**:

- `core_mls/welcome_handler.rs`

**Example**:

```rust
pub struct WelcomeHandler {
    provider: Arc<OpenMlsRustCrypto>,
    storage: Arc<SpacePandaStorage>,
    group_mgr: Arc<MlsGroupManager>,
}

impl WelcomeHandler {
    pub async fn process_welcome(
        &self,
        welcome: MlsMessageIn,
        ratchet_tree: Option<RatchetTreeIn>,
    ) -> Result<GroupId> {
        let staged_welcome = StagedWelcome::new_from_welcome(
            &*self.provider,
            &MlsGroupJoinConfig::default(),
            welcome,
            ratchet_tree,
        )?;

        let mut group = staged_welcome.into_group(&*self.provider)?;
        let group_id = group.group_id().clone();

        // Persist group state
        self.storage.write_group(&group_id, &group)?;

        tracing::info!("Joined group: {:?}", group_id);

        Ok(group_id)
    }
}
```

---

### Phase 6: DHT Integration (Week 5)

**Deliverables**:

- [ ] KeyPackage advertisement via DHT
- [ ] KeyPackage discovery/retrieval
- [ ] MLS message routing
- [ ] Group member discovery
- [ ] Welcome message delivery

**Enhancements to Existing DHT**:

```rust
// In core_dht/dht_node.rs

impl DhtNode {
    /// Store KeyPackage for user
    pub async fn store_key_package(
        &self,
        user_id: &UserId,
        key_package: KeyPackage,
    ) -> Result<()> {
        let key = DhtKey::hash(user_id.as_bytes());
        let value = DhtValue::KeyPackage(key_package);
        self.store(key, value).await
    }

    /// Retrieve KeyPackage for user
    pub async fn find_key_package(
        &self,
        user_id: &UserId,
    ) -> Result<Option<KeyPackage>> {
        let key = DhtKey::hash(user_id.as_bytes());
        match self.find_value(key).await? {
            Some(DhtValue::KeyPackage(kp)) => Ok(Some(kp)),
            _ => Ok(None),
        }
    }

    /// Route MLS message to peer
    pub async fn send_mls_message(
        &self,
        peer_id: PeerId,
        message: MlsMessageOut,
    ) -> Result<()> {
        let serialized = message.tls_serialize_detached()?;
        self.send_to_peer(peer_id, MessageType::Mls, serialized).await
    }
}
```

---

### Phase 7: Testing & Benchmarks (Week 6)

**Deliverables**:

- [ ] Unit tests for all modules
- [ ] Integration tests (2-peer, 3-peer groups)
- [ ] Benchmarks (group creation, message throughput)
- [ ] Fuzzing (malformed messages, invalid commits)
- [ ] Performance regression tests

**Benchmark Suite** (add to `benches/mls_operations.rs`):

```rust
// Group creation latency
group.bench_function("mls_create_group", |b| {
    b.iter(|| {
        let group = MlsGroup::new(...);
    });
});

// Add member throughput
group.bench_function("mls_add_member/batch_size/10", |b| {
    b.iter(|| {
        group.add_members(..., &key_packages_10);
        group.merge_pending_commit(...);
    });
});

// Message encryption throughput
group.bench_function("mls_encrypt_message/size/1kb", |b| {
    let plaintext = vec![0u8; 1024];
    b.iter(|| {
        group.create_message(..., &plaintext);
    });
});

// Message decryption latency
group.bench_function("mls_decrypt_message", |b| {
    b.iter(|| {
        group.process_message(..., message.clone());
    });
});
```

---

## 6. Security Considerations

### Forward Secrecy

✅ **Provided by MLS**: Each epoch uses different encryption keys  
✅ **Action Required**: Ensure old epoch keys are deleted after epoch advance

### Post-Compromise Security

✅ **Provided by MLS**: Self-update proposals rotate member keys  
📋 **Action Required**: Implement periodic self-updates (every N messages or T time)

### Authentication

✅ **Signature Verification**: OpenMLS verifies all signatures automatically  
⚠️ **Identity Binding**: Must bind MLS credentials to SpacePanda identities

```rust
// Verify credential matches expected identity
pub fn verify_credential_identity(
    credential: &Credential,
    expected_user_id: &UserId,
) -> Result<bool> {
    match credential {
        Credential::Basic(basic) => {
            let cred_user_id = UserId::from_bytes(basic.identity().to_vec());
            Ok(cred_user_id == *expected_user_id)
        },
        _ => Err(Error::UnsupportedCredentialType),
    }
}
```

### Storage Security

⚠️ **Critical**: Signature keys must be protected  
**Options**:

1. **Encrypt at rest**: Use device-specific key to encrypt storage
2. **Hardware security**: Use platform keychain/TPM (future)
3. **Memory protection**: Zero keys on drop (use `zeroize` crate)

### Denial of Service

⚠️ **Message Validation**: Validate messages before processing

```rust
// Limit message size
const MAX_MLS_MESSAGE_SIZE: usize = 1_000_000; // 1MB

if serialized_message.len() > MAX_MLS_MESSAGE_SIZE {
    return Err(Error::MessageTooLarge);
}
```

⚠️ **Rate Limiting**: Limit proposals/commits per peer per epoch

```rust
const MAX_PROPOSALS_PER_EPOCH: usize = 100;

if group.pending_proposals().count() > MAX_PROPOSALS_PER_EPOCH {
    return Err(Error::TooManyProposals);
}
```

---

## 7. Performance Targets

Based on OpenMLS benchmarks and our existing performance:

| Operation             | Target Latency | Notes                               |
| --------------------- | -------------- | ----------------------------------- |
| Create Group          | < 50ms         | Signature key generation is slowest |
| Add 1 Member          | < 20ms         | KeyPackage validation + commit      |
| Add 10 Members        | < 100ms        | Batch add more efficient            |
| Remove Member         | < 15ms         | Lighter than add                    |
| Encrypt Message (1KB) | < 5ms          | AEAD encryption is fast             |
| Decrypt Message (1KB) | < 3ms          | Decryption faster than encryption   |
| Process Commit        | < 30ms         | Tree update + epoch advance         |

**Expected Throughput**:

- 50-100 messages/sec/group (encryption bound)
- 20-50 commits/sec (signature verification bound)

---

## 8. Migration Path

### For Existing Groups

**Option 1**: Create new MLS groups, archive old groups  
**Option 2**: Dual operation (legacy + MLS) during transition

### For Existing Messages

- MLS provides **only** confidentiality and integrity
- **Does not provide**: Message ordering, delivery guarantees, consensus
- Still need CRDT for conflict resolution on application data

---

## 9. Open Questions

1. **Group Size Limits**: What's max group size? (OpenMLS supports thousands, but DHT routing?)
2. **Epoch Advancement**: Auto-advance epochs periodically? On what trigger?
3. **KeyPackage Refresh**: How often to generate new KeyPackages?
4. **Ratchet Tree Distribution**: Always include in Welcome? (performance vs. simplicity)
5. **External Commits**: Support joining without invitation? (security implications)

---

## 10. Next Steps

**Immediate Actions**:

1. ✅ Update dependencies (DONE)
2. ✅ Migrate to Blake3 (DONE)
3. Create `core_mls/` module skeleton
4. Implement storage layer (Phase 1)
5. Implement credential management (Phase 2)

**Blocked On**:

- None - ready to proceed with implementation

**Questions for Review**:

- Storage backend preference? (Custom vs. default)
- Group size limits?
- Security policy for key storage?

---

## 11. References

- [OpenMLS 0.7.1 Documentation](https://docs.rs/openmls/0.7.1/openmls/)
- [MLS RFC 9420](https://www.rfc-editor.org/rfc/rfc9420.html)
- [OpenMLS User Manual](https://openmls.tech/book/)
- [OpenMLS Architecture](https://openmls.tech/book/architecture.html)
- [Ciphersuite Registry](https://www.iana.org/assignments/mls/mls.xhtml)

---

**Document Version**: 1.0  
**Last Updated**: 2025-01-XX  
**Author**: GitHub Copilot (Claude Sonnet 4.5)  
**Status**: Ready for Implementation
