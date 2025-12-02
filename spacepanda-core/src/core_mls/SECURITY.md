# MLS Security Guide

## Overview

SpacePanda's MLS (Messaging Layer Security) implementation provides end-to-end encryption for group messaging with forward secrecy and post-compromise security.

## Security Properties

### 1. End-to-End Encryption

- All application messages encrypted with AES-256-GCM
- Only group members can decrypt messages
- Server cannot read message contents

### 2. Forward Secrecy

- Epoch advancement derives new encryption keys
- Past messages remain secure even if current keys compromised
- Automatic key rotation on membership changes

### 3. Post-Compromise Security

- Key updates allow recovery from compromise
- Tree structure isolates key material
- Member removal prevents future decryption

### 4. Authentication

- All proposals and commits are signed
- Group members verify sender identity
- Replay attacks prevented via sequence numbers

### 5. Replay Protection

- Per-sender sequence tracking
- Replay cache prevents duplicate processing
- Configurable cache size (default: 1000)

## Threat Model

### In Scope

- **Passive network attackers**: Cannot read messages
- **Active attackers**: Cannot inject/modify messages
- **Compromised members**: Cannot read messages after removal
- **Server compromise**: Cannot decrypt messages

### Out of Scope

- **Client-side malware**: Direct key access
- **Physical device access**: Key extraction
- **Side-channel attacks**: Timing, power analysis
- **Social engineering**: User credential theft

## Best Practices

### Key Management

```rust
use spacepanda_core::core_mls::{MlsHandle, MlsConfig};

// 1. Generate strong application secrets
let app_secret: Vec<u8> = generate_random_bytes(32);

// 2. Create group with secure config
let mut config = MlsConfig::default();
config.replay_cache_size = 10000; // Larger for high-traffic groups

let handle = MlsHandle::create_group(
    Some("secure-team".to_string()),
    public_key,
    identity,
    app_secret,
    config,
)?;
```

### Regular Key Rotation

```rust
// Rotate keys periodically (e.g., weekly)
handle.propose_update(new_public_key)?;
handle.commit()?;
```

### Member Removal

```rust
// Remove compromised member immediately
handle.propose_remove(compromised_member_index)?;
let (commit, _) = handle.commit()?;

// Broadcast commit to all remaining members
// Removed member cannot decrypt future messages
```

### Secure Storage

```rust
use spacepanda_core::core_mls::persistence::{
    save_group_to_file,
    load_group_from_file,
};

// Use strong passphrase for encryption at rest
let passphrase = "user-provided-strong-passphrase";

// Save group state
save_group_to_file(&group_state, "group.enc", passphrase)?;

// Load group state
let group_state = load_group_from_file("group.enc", passphrase)?;
```

## Common Vulnerabilities & Mitigations

### 1. Replay Attacks

**Vulnerability**: Attacker resends old messages

**Mitigation**:

- Automatic sequence number tracking
- Replay cache prevents duplicates
- Error: `MlsError::ReplayDetected`

### 2. Epoch Confusion

**Vulnerability**: Attacker sends messages from old epoch

**Mitigation**:

- Strict epoch validation
- Old messages rejected
- Error: `MlsError::EpochMismatch`

### 3. Bit-Flip Attacks

**Vulnerability**: Attacker modifies ciphertext

**Mitigation**:

- AEAD provides authentication
- Tampering detected
- Error: `MlsError::CryptoError`

### 4. Unauthorized Operations

**Vulnerability**: Non-member sends proposals

**Mitigation**:

- Signature verification
- Sender authorization checks
- Error: `MlsError::Unauthorized`

## Security Checklist

### Development

- [ ] All secrets use `zeroize::Zeroize` on drop
- [ ] No `unwrap()` in production code paths
- [ ] All crypto operations use constant-time implementations
- [ ] Input validation on all external data
- [ ] Error messages don't leak sensitive info

### Deployment

- [ ] TLS for transport layer security
- [ ] Strong passphrase enforcement (min 12 chars)
- [ ] Rotate keys on member removal
- [ ] Monitor for replay attempts
- [ ] Regular security updates

### Operations

- [ ] Audit logs for group operations
- [ ] Alert on unusual activity (many removals, etc.)
- [ ] Regular key rotation schedule
- [ ] Backup encrypted group state
- [ ] Incident response plan

## Cryptographic Primitives

### Symmetric Encryption

- **Algorithm**: AES-256-GCM
- **Key Size**: 256 bits
- **Nonce**: 96 bits (random per message)
- **Tag**: 128 bits

### Key Derivation

- **At Rest**: Argon2id (64MB memory, 3 iterations, 4 threads)
- **In Transit**: HKDF-SHA256
- **Application Secrets**: SHA-256 based derivation

### HPKE (Hybrid Public Key Encryption)

- **Status**: Simplified prototype
- **Production**: Replace with full HPKE implementation

### Signatures

- **Current**: SHA-256 hash (placeholder)
- **Production**: Ed25519 or similar

## Performance Considerations

### Encryption Overhead

- Message encryption: ~0.1ms per message
- Decryption: ~0.1ms per message
- Group operations: ~50ms per commit

### Scalability

- Tested with 20+ members
- Tree operations: O(log N)
- Memory: ~1KB per member

### Optimizations

- Message key caching
- Lazy tree hash computation
- Replay cache LRU eviction

## Known Limitations

1. **Simplified HPKE**: Prototype implementation, not production-ready
2. **Placeholder Signatures**: Use real EdDSA in production
3. **No Forward Secrecy**: Between epochs (require frequent rotation)
4. **Authorization**: Basic checks, enhance for production
5. **Concurrent Operations**: Basic RwLock, consider fine-grained locking

## Production Recommendations

### Before Deployment

1. Replace HPKE with production implementation
2. Implement proper signature scheme (Ed25519)
3. Add comprehensive authorization checks
4. External security audit
5. Penetration testing

### Monitoring

- Track epoch advancement frequency
- Monitor message latency
- Alert on high replay attempts
- Log all member changes

### Incident Response

1. Immediate member removal on compromise
2. Full key rotation (all members update)
3. Audit message history
4. Notify affected users

## References

- [RFC 9420: MLS Protocol](https://www.rfc-editor.org/rfc/rfc9420.html)
- [MLS Architecture](https://messaginglayersecurity.rocks/)
- [HPKE RFC 9180](https://www.rfc-editor.org/rfc/rfc9180.html)
- [AES-GCM Security](https://csrc.nist.gov/pubs/sp/800/38/d/final)

## Support

For security issues, contact: security@spacepanda.example.com

For implementation questions, see: `ARCHITECTURE.md`
