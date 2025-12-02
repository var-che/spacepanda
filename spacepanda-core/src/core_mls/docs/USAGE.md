# MLS Implementation Guide

## Overview

SpacePanda's MLS (Messaging Layer Security) implementation provides secure group messaging with end-to-end encryption, forward secrecy, and post-compromise security.

## Quick Start

### Creating a Group

```rust
use spacepanda_core::core_mls::{MlsHandle, MlsConfig};

// Generate application secret (32 bytes recommended)
let app_secret = vec![0u8; 32]; // Use crypto-secure random in production

// Create group
let alice = MlsHandle::create_group(
    Some("team-chat".to_string()),
    alice_public_key,
    b"alice@example.com".to_vec(),
    app_secret,
    MlsConfig::default(),
)?;
```

### Adding Members

```rust
// Propose adding Bob
alice.propose_add(bob_public_key, b"bob@example.com".to_vec())?;

// Commit the change (generates Welcome message for Bob)
let (commit, welcomes) = alice.commit()?;

// Send commit to existing members
// Send welcomes[0] to Bob

// Bob joins
let bob = MlsHandle::join_group(&welcomes[0], 1, &bob_public_key, MlsConfig::default())?;
```

### Sending Messages

```rust
// Alice sends message
let envelope = alice.send_message(b"Hello team!")?;

// Bob receives
let plaintext = bob.receive_message(&envelope)?;
assert_eq!(plaintext, b"Hello team!");
```

### Removing Members

```rust
// Propose removal
alice.propose_remove(member_index)?;

// Commit (removed member won't be able to decrypt future messages)
let (commit, _) = alice.commit()?;

// Send commit to remaining members
```

### Key Rotation

```rust
// Update your key
alice.propose_update(new_public_key)?;
alice.commit()?;
```

## Architecture

### Core Components

```
┌─────────────────┐
│   MlsHandle     │  ← High-level API
├─────────────────┤
│  MlsTransport   │  ← Message wrapping
├─────────────────┤
│   MlsGroup      │  ← Group state
├─────────────────┤
│ Tree│Encryption │  ← Crypto primitives
└─────────────────┘
```

### Data Flow

#### Adding a Member:

1. Alice calls `propose_add()`
2. Proposal queued locally
3. Alice calls `commit()`
4. Proposals applied, epoch advanced, Welcome generated
5. Alice sends Commit to existing members
6. Alice sends Welcome to new member
7. Existing members call `receive_commit()`
8. New member calls `join_group()`

#### Sending a Message:

1. Alice calls `send_message(plaintext)`
2. Message encrypted with current epoch's key
3. MlsEnvelope created
4. Alice broadcasts envelope
5. Recipients call `receive_message(envelope)`
6. Message decrypted and verified

## API Reference

### MlsHandle

#### Group Lifecycle

- **`create_group(...)`** - Create new group, become first member
- **`join_group(welcome, ...)`** - Join existing group via Welcome message
- **`group_id()`** - Get group identifier
- **`group_name()`** - Get group name
- **`epoch()`** - Get current epoch number

#### Member Management

- **`propose_add(pk, identity)`** - Propose adding member
- **`propose_add_batch(members)`** - Propose adding multiple members
- **`propose_remove(index)`** - Propose removing member
- **`propose_remove_batch(indices)`** - Propose removing multiple members
- **`propose_update(pk)`** - Propose updating own key
- **`commit()`** - Commit pending proposals, returns `(Commit, Vec<Welcome>)`

#### Messaging

- **`send_message(plaintext)`** - Encrypt and wrap message
- **`receive_message(envelope)`** - Decrypt and unwrap message

#### State Queries

- **`members()`** - List all members
- **`member_count()`** - Get number of members
- **`has_member(index)`** - Check if member exists
- **`metadata()`** - Get group metadata

#### Proposals

- **`receive_proposal(envelope)`** - Receive proposal from another member
- **`receive_commit(envelope)`** - Apply commit from another member

#### Thread Safety

- **`clone_handle()`** - Create new handle sharing same state

### MlsConfig

```rust
pub struct MlsConfig {
    /// Replay protection cache size (default: 1000)
    pub replay_cache_size: usize,

    /// Enable strict epoch validation (default: true)
    pub strict_epoch_check: bool,
}
```

### MlsEnvelope

Wire format for transport:

```rust
pub struct MlsEnvelope {
    pub version: u16,                  // Protocol version
    pub message_type: MlsMessageType,  // Welcome | Proposal | Commit | Application
    pub group_id: GroupId,             // Group identifier
    pub sender: Option<Vec<u8>>,       // Sender identity (if applicable)
    pub payload: Vec<u8>,              // Serialized message
}
```

Serialization:

- **`to_json()`** - JSON format (for debugging, Router HTTP)
- **`from_json(json)`** - Parse JSON
- **`to_bytes()`** - Compact binary (for storage, network)
- **`from_bytes(bytes)`** - Parse binary

## Common Patterns

### Multi-Device Support

```rust
// User has multiple devices
let device1 = MlsHandle::create_group(...)?;

// Add second device as separate member
device1.propose_add(device2_pk, b"alice@example.com".to_vec())?;
let (commit, welcomes) = device1.commit()?;

let device2 = MlsHandle::join_group(&welcomes[0], ...)?;

// Both devices can send/receive
```

### Batch Operations

```rust
// Add 10 members at once
let members = vec![
    (pk1, id1),
    (pk2, id2),
    // ...
];

alice.propose_add_batch(members)?;
let (commit, welcomes) = alice.commit()?;

// Send welcomes[i] to corresponding member
```

### Concurrent Access

```rust
// Share handle across threads
let handle1 = MlsHandle::create_group(...)?;
let handle2 = handle1.clone_handle();

// Thread 1
std::thread::spawn(move || {
    handle1.send_message(b"from thread 1")?;
});

// Thread 2
std::thread::spawn(move || {
    handle2.send_message(b"from thread 2")?;
});
```

### Persistence

```rust
use spacepanda_core::core_mls::persistence::{save_group_to_file, load_group_from_file};

// Save group state
save_group_to_file(&group_state, "group.enc", "passphrase")?;

// Load group state
let group_state = load_group_from_file("group.enc", "passphrase")?;
```

### Discovery Integration

```rust
use spacepanda_core::core_mls::discovery::GroupPublicInfo;

// Create public info for CRDT
let public_info = GroupPublicInfo::from_metadata(
    group_id,
    &metadata,
    &tree,
    |data| sign(data),
)?;

// Serialize for storage
let json = public_info.to_json()?;

// Query groups
let query = DiscoveryQuery {
    name_pattern: Some("team".to_string()),
    min_members: Some(2),
    max_members: Some(50),
    ..Default::default()
};

if query.matches(&public_info) {
    // Group matches criteria
}
```

## Error Handling

All operations return `MlsResult<T>` which is `Result<T, MlsError>`.

Common errors:

```rust
match handle.commit() {
    Ok((commit, welcomes)) => { /* success */ },
    Err(MlsError::NoProposalsPending) => { /* nothing to commit */ },
    Err(MlsError::EpochMismatch { expected, got }) => { /* wrong epoch */ },
    Err(MlsError::ReplayDetected(seq)) => { /* duplicate message */ },
    Err(MlsError::CryptoError(msg)) => { /* decryption failed */ },
    Err(MlsError::Unauthorized(msg)) => { /* invalid sender */ },
    Err(e) => { /* other error */ },
}
```

## Performance Characteristics

| Operation         | Time Complexity | Notes                        |
| ----------------- | --------------- | ---------------------------- |
| `create_group`    | O(1)            | Single member initialization |
| `propose_add`     | O(log N)        | Tree path update             |
| `commit`          | O(P + log N)    | P = proposals, N = members   |
| `send_message`    | O(1)            | AES-GCM encryption           |
| `receive_message` | O(1)            | AES-GCM decryption           |
| `join_group`      | O(N)            | Process tree snapshot        |

Benchmarks (on 2020 laptop):

- Message encryption: ~0.1ms
- Message decryption: ~0.1ms
- Commit with 1 add: ~2ms
- Commit with 10 adds: ~15ms
- Join group (20 members): ~5ms

## Security Considerations

### Do's

✅ Rotate keys regularly (weekly/monthly)
✅ Remove compromised members immediately
✅ Use strong application secrets (32+ bytes)
✅ Validate sender identities before trust
✅ Monitor for replay attempts
✅ Enable strict epoch checking (default)
✅ Use TLS for transport layer

### Don'ts

❌ Reuse application secrets across groups
❌ Share passphrase for encrypted storage
❌ Ignore crypto errors
❌ Skip epoch validation
❌ Store plaintexts in logs
❌ Use weak randomness for secrets

### Known Limitations

1. **Simplified HPKE**: Prototype only, replace for production
2. **Placeholder Signatures**: Use Ed25519 for production
3. **Commit Processing**: Receiving member must separately receive proposals (see TODO in code)
4. **No External Commits**: Can't join without Welcome
5. **Basic Authorization**: Enhance for production

See `SECURITY.md` for detailed threat model.

## Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_workflow() {
        // Create group
        let alice = MlsHandle::create_group(
            Some("test".to_string()),
            b"alice_pk".to_vec(),
            b"alice".to_vec(),
            vec![1,2,3,4],
            MlsConfig::default(),
        ).unwrap();

        // Add member
        alice.propose_add(b"bob_pk".to_vec(), b"bob".to_vec()).unwrap();
        let (commit, welcomes) = alice.commit().unwrap();

        let bob = MlsHandle::join_group(
            &welcomes[0],
            1,
            b"bob_pk",
            MlsConfig::default(),
        ).unwrap();

        // Send message
        let msg = alice.send_message(b"Hello Bob!").unwrap();
        let plaintext = bob.receive_message(&msg).unwrap();

        assert_eq!(plaintext, b"Hello Bob!");
    }
}
```

## Integration with SpacePanda

### Router Integration

```rust
use spacepanda_core::core_router::RouterHandle;

// Wrap MLS envelope for routing
let mls_envelope = handle.send_message(b"Hello")?;
let json = mls_envelope.to_json()?;

// Send via Router
router.send_rpc(peer_id, "mls.message", json).await?;

// Receive via Router
match router.receive().await? {
    RouterEvent::Rpc { payload, .. } => {
        let envelope = MlsEnvelope::from_json(&payload)?;
        let plaintext = handle.receive_message(&envelope)?;
    }
}
```

### Store Integration

```rust
use spacepanda_core::core_store::LocalStore;

// Store group metadata
let metadata = handle.metadata()?;
store.save_crdt("mls_group", group_id, metadata)?;

// Store public info for discovery
let public_info = GroupPublicInfo::from_metadata(...)?;
store.save_crdt("mls_discovery", group_id, public_info)?;
```

### Identity Integration

```rust
use spacepanda_core::core_identity::{MasterKey, DeviceKey};

// Use identity public key for MLS
let device_key = DeviceKey::generate(&master_key);
let public_key = device_key.public_key_bytes();

let handle = MlsHandle::create_group(
    Some("group".to_string()),
    public_key,
    user_id.as_bytes().to_vec(),
    app_secret,
    config,
)?;
```

## Debugging

Enable logging:

```rust
env::set_var("RUST_LOG", "spacepanda_core::core_mls=debug");
```

Common issues:

**"EpochMismatch"**: Member out of sync, needs to receive intermediate commits
**"ReplayDetected"**: Duplicate message or clock skew
**"CryptoError"**: Key mismatch, wrong epoch, or tampering
**"NoProposalsPending"**: Called commit() without proposals

## Next Steps

1. Review `SECURITY.md` for threat model
2. Review `ARCHITECTURE.md` for design details
3. Run tests: `cargo test --lib core_mls`
4. Check examples in `integration_tests.rs`
5. Plan production deployment (external audit, replace placeholders)

## References

- [MLS RFC 9420](https://www.rfc-editor.org/rfc/rfc9420.html)
- [MLS Architecture](https://messaginglayersecurity.rocks/)
- [HPKE RFC 9180](https://www.rfc-editor.org/rfc/rfc9180.html)
- SpacePanda `ARCHITECTURE.md`
- SpacePanda `SECURITY.md`
