# SpacePanda Privacy Architecture

## Overview

SpacePanda is designed as a **privacy-first** Discord alternative using P2P, DHT, CRDT, and MLS technologies with onion routing. This document outlines the privacy features and metadata protection strategies.

## Core Privacy Features

### 1. Sealed Sender (✅ IMPLEMENTED)

**Problem**: Traditional messaging exposes sender identity in metadata
**Solution**: Encrypt sender identity using MLS group exporter secret

#### How It Works

```rust
// Before (INSECURE):
messages table:
  sender_hash: b"alice@example.com"  // ⚠️ PLAINTEXT - network sees this

// After (SECURE):
messages table:
  sealed_sender_bytes: [0x9a, 0x3f, ...]  // ✅ ENCRYPTED
```

#### Implementation Details

- **Key Derivation**: HKDF-SHA256 from MLS group exporter secret
- **Algorithm**: AES-256-GCM
- **Domain Separation**: "Sealed Sender v1" label
- **Binding**: Epoch number in AAD (prevents cross-epoch tampering)
- **Unlinkability**: Random nonce ensures different ciphertexts per message

#### Security Properties

- **Confidentiality**: Only group members can decrypt sender identity
- **Integrity**: AEAD tag prevents tampering
- **Authenticity**: Only valid group members can create sealed senders
- **Unlinkability**: Different messages from same sender → different ciphertexts

#### Code Locations

- **Schema Migration**: `core_mls/storage/migrations.rs` (Migration v5)
- **Sealing Logic**: `core_space/async_manager.rs::send_channel_message`
- **Sealed Sender Module**: `core_mls/sealed_sender.rs`

### 2. Timing Obfuscation (⏳ PLANNED)

**Problem**: Sequence numbers reveal message timing patterns
**Solution**: Add random jitter to sequence numbers

#### Planned Implementation

```rust
// Instead of:
sequence = unix_timestamp();

// Use:
sequence = unix_timestamp() + random_jitter(-30..+30);  // ±30 second window
```

This prevents:

- Correlation of message timing across channels
- Inference of user activity patterns
- Timing-based traffic analysis

### 3. Cover Traffic (⏳ PLANNED)

**Problem**: Real messages distinguishable from network silence
**Solution**: Send dummy messages at random intervals

#### Planned Strategy

- Send dummy encrypted messages when idle
- Indistinguishable from real messages at network level
- Decrypted recipients discard cover traffic
- Prevents activity pattern inference

### 4. Message Size Padding (⏳ PLANNED)

**Problem**: Message length reveals content patterns
**Solution**: Pad all messages to fixed-size buckets

```rust
// Pad to nearest:
- Small: 256 bytes
- Medium: 1 KB
- Large: 4 KB
- XL: 16 KB
```

### 5. Metadata Minimization

#### What We Store (Encrypted)

- ✅ Message content (MLS encrypted)
- ✅ Sender identity (sealed sender encrypted)
- ✅ Channel names (encrypted)
- ✅ Channel topics (encrypted)
- ✅ Member lists (encrypted)

#### What We DON'T Store

- ❌ Updated timestamps (removed in Migration v3)
- ❌ Read receipts (not implemented)
- ❌ Typing indicators (not implemented)
- ❌ Delivery confirmations (not implemented)

#### Local-Only Metadata

- `processed` flag (never synced - prevents correlation)
- `plaintext_content` (for sent messages only, never shared)

### 6. Network-Level Privacy (Via Onion Routing)

**Integration Points**:

- P2P message broadcasting via `NetworkLayer`
- DHT lookups for peer discovery
- Relay servers for NAT traversal (ONLY - not message storage)

**Privacy Properties**:

- Onion routing hides sender/recipient relationship
- Multi-hop relays prevent traffic correlation
- No single node sees both endpoints

## Migration to Sealed Sender

### Database Schema Change (v4 → v5)

```sql
-- OLD (v4):
CREATE TABLE messages (
    sender_hash BLOB NOT NULL,  -- ⚠️ PLAINTEXT SENDER
    ...
);

-- NEW (v5):
CREATE TABLE messages (
    sealed_sender_bytes BLOB NOT NULL,  -- ✅ ENCRYPTED SENDER
    ...
);
```

### Backward Compatibility

Migration v5 preserves existing messages:

- Old `sender_hash` copied to `sealed_sender_bytes` field
- Old messages have plaintext sender (until re-encrypted)
- **New messages use proper sealed sender**
- Gradual migration: old messages eventually expired/deleted

## Privacy Audit Checklist

### Database Schema

- [x] Encrypted message content
- [x] Sealed sender identity
- [x] Encrypted channel metadata
- [x] No updated timestamps
- [ ] Message size padding (planned)
- [ ] Sequence number jitter (planned)

### Network Layer

- [x] P2P architecture (no central server)
- [x] Onion routing integration
- [ ] Cover traffic (planned)
- [ ] DHT privacy-preserving lookups (planned)

### Application Layer

- [x] MLS end-to-end encryption
- [x] Sealed sender for all new messages
- [ ] Read receipt opt-out (not implemented)
- [ ] Typing indicator opt-out (not implemented)

## Threat Model

### What We Protect Against

✅ **Network Observer**:

- Cannot link messages to specific senders (sealed sender)
- Cannot infer communication patterns (cover traffic planned)
- Cannot correlate activity (no timestamps synced)

✅ **Passive Adversary**:

- Cannot decrypt message contents (MLS)
- Cannot identify senders (sealed sender)
- Cannot build social graph (onion routing)

⚠️ **Active Adversary** (Partial Protection):

- Timing attacks partially mitigated (jitter planned)
- Traffic analysis partially mitigated (cover traffic planned)
- Sybil attacks rely on DHT trust mechanisms

❌ **Compromised Group Member**:

- Can decrypt messages (by design - they're in the group)
- Can see sender identities (by design - group members need this)
- Cannot impersonate others (MLS authentication)

### What We DON'T Protect Against

- Malicious group members (expected - they're in the group!)
- Global passive adversaries with timing correlation (hard problem)
- Quantum computers breaking AES-256/ECDH (post-quantum MLS planned)

## Future Privacy Enhancements

### Short-Term (Next 3 Months)

1. ✅ Sealed sender (DONE)
2. [ ] Timing obfuscation with jitter
3. [ ] Message size padding
4. [ ] Cover traffic prototype

### Medium-Term (3-6 Months)

1. [ ] Privacy-preserving DHT lookups
2. [ ] Improved onion routing (multi-hop)
3. [ ] Lazy MLS membership for public channels
4. [ ] Metadata-resistant group invites

### Long-Term (6-12 Months)

1. [ ] Post-quantum MLS ciphersuites
2. [ ] Anonymous credentials for group membership
3. [ ] Private information retrieval for message fetching
4. [ ] Differential privacy for usage statistics

## References

- [Signal Sealed Sender](https://signal.org/blog/sealed-sender/)
- [MLS RFC 9420](https://datatracker.ietf.org/doc/html/rfc9420)
- [Tor Onion Routing](https://www.torproject.org/about/history/)
- [Matrix Privacy Assessment](https://matrix.org/docs/older/privacy_notice/)

## Contact

For privacy concerns or to report privacy issues, contact: [your contact info]
