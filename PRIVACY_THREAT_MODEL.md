# SpacePanda Privacy & Security Threat Model

**Version**: 1.0  
**Date**: December 4, 2025  
**Status**: Active

## Executive Summary

SpacePanda is a privacy-first encrypted messaging platform built on MLS (RFC 9420). This document analyzes privacy threats, implemented defenses, and remaining gaps.

**Current Security Rating**: **B+** (Strong crypto, improving metadata privacy)

---

## Table of Contents

1. [Trust Model](#trust-model)
2. [Threat Actors](#threat-actors)
3. [Attack Surface Analysis](#attack-surface-analysis)
4. [Implemented Defenses](#implemented-defenses)
5. [Remaining Vulnerabilities](#remaining-vulnerabilities)
6. [Roadmap](#roadmap)

---

## Trust Model

### Trusted Components

✅ **Local Device**

- User's hardware and OS
- Local storage encryption
- Process memory protection

✅ **MLS Protocol**

- End-to-end encryption (AES-256-GCM)
- Forward secrecy via epoch rotation
- Post-compromise security

✅ **OpenMLS Library**

- Professionally audited
- RFC 9420 compliant
- Constant-time crypto primitives

### Untrusted Components

⚠️ **Network Layer**

- Internet service providers (ISPs)
- Network operators
- Government surveillance (PRISM, etc.)
- Corporate monitoring

⚠️ **Relay Nodes**

- Message forwarding infrastructure
- Temporary ciphertext storage
- Metadata observation (sender/receiver IPs)

⚠️ **Compromised Devices**

- Malware on recipient devices
- Stolen devices with unlocked storage
- Physical access attacks

---

## Threat Actors

### 1. **Mass Surveillance** (State-level)

**Capabilities:**

- ISP traffic monitoring
- DNS request logging
- TLS interception (via compromised CAs)
- Timing correlation across networks
- Legal data requests to service providers

**Motivation:**

- Political dissent monitoring
- Crime prevention
- National security

**Impact:** **HIGH** - Can affect all users simultaneously

### 2. **Targeted Surveillance** (Law Enforcement)

**Capabilities:**

- Device seizure and forensic analysis
- Legal compulsion of metadata
- Social graph reconstruction
- Wiretapping warrants

**Motivation:**

- Criminal investigations
- Counter-terrorism
- Civil litigation

**Impact:** **MEDIUM** - Affects specific individuals

### 3. **Network Observers** (Passive Attackers)

**Capabilities:**

- Packet size/timing analysis
- Connection pattern monitoring
- Metadata correlation
- Traffic volume tracking

**Motivation:**

- Corporate espionage
- Competitive intelligence
- Blackmail material gathering

**Impact:** **MEDIUM** - Can observe communication patterns

### 4. **Malicious Insiders** (Group Members)

**Capabilities:**

- See message content (by design)
- Screenshot/record conversations
- Forward messages externally
- Invite tracking

**Motivation:**

- Whistleblowing (positive)
- Betrayal (negative)
- Law enforcement cooperation

**Impact:** **LOW** - Limited to group-specific damage

### 5. **Cryptanalytic Attackers** (Nation-state)

**Capabilities:**

- Quantum computers (future threat)
- Zero-day exploits in crypto libs
- Side-channel attacks
- Protocol weaknesses

**Motivation:**

- Intelligence gathering
- Cryptographic research

**Impact:** **CRITICAL** (if successful) - Breaks all encryption

---

## Attack Surface Analysis

### 1. **Message Content** 🟢 SECURE

**Exposure:** Encrypted with AES-256-GCM + MLS protocol

**Defenses:**

- ✅ End-to-end encryption
- ✅ Forward secrecy (epoch-based rotation)
- ✅ Post-compromise security (member removal)
- ✅ Replay protection (sequence numbers)
- ✅ Message padding (NEW - prevents size analysis)

**Threats:**

- ⚠️ Compromised device with decryption keys
- ⚠️ Malicious group member screenshots
- ⚠️ Future quantum cryptanalysis

**Risk Level:** **LOW**

---

### 2. **Message Metadata** 🟡 IMPROVING

#### 2.1 **Channel Names**

**Current Exposure:** Stored in plaintext in GroupMetadata

**Attack:**

```
Channel name: "#whistleblowers"  → Reveals purpose
Channel name: "#iran-protests"    → Political target
Channel name: "#drug-deals"       → Criminal evidence
```

**Defenses:**

- ✅ **NEW**: SealedMetadata encryption (AES-256-GCM)
- ✅ Encrypted at rest
- ⚠️ Still visible during API calls (in-memory)

**Risk Level:** **MEDIUM** → **LOW** (after sealed metadata adoption)

#### 2.2 **Member Lists**

**Current Exposure:** Plaintext member identities in GroupMetadata

**Attack:**

```
Members: [Alice, Bob, Charlie]
→ Social graph reconstruction
→ "Who talks to whom" analysis
→ Network mapping
```

**Defenses:**

- ✅ **NEW**: Encrypted in SealedMetadata
- ❌ Not yet: Anonymous credentials
- ❌ Not yet: Per-channel pseudonyms

**Risk Level:** **HIGH** → **MEDIUM** (partial mitigation)

#### 2.3 **Timestamps**

**Current Exposure:** Creation/update times in metadata

**Attack:**

```
created_at: 1638662400 (Dec 5, 2:00 AM)
→ "Alice is active at 2 AM" (unusual hours)
→ Timezone inference
→ Activity pattern profiling
```

**Defenses:**

- ✅ **NEW**: Encrypted in SealedMetadata
- ❌ Not yet: Timestamp fuzzing (random delays)
- ❌ Not yet: Batched updates

**Risk Level:** **MEDIUM**

---

### 3. **Traffic Analysis** 🔴 VULNERABLE

#### 3.1 **Message Sizes**

**Current Exposure:** Ciphertext length reveals plaintext length + 16 bytes (AEAD tag)

**Attack:**

```
Size: 50 bytes  → "Yes" / "No" / "OK"
Size: 1000 bytes → Paragraph response
Size: 50KB → Image/file attachment
```

**Defenses:**

- ✅ **NEW**: Message padding to fixed buckets (256, 1KB, 4KB, 16KB, 64KB)
- ❌ Perfect padding (all messages same size) - too expensive

**Risk Level:** **HIGH** → **LOW** (after padding)

#### 3.2 **Message Timing**

**Current Exposure:** Immediate message delivery leaks timing patterns

**Attack:**

```
Alice sends → Bob receives (instant)
→ "Bob is online"
→ Response time = thinking time
→ Conversation flow inference
```

**Defenses:**

- ❌ **TODO**: Constant-rate message mixing
- ❌ **TODO**: Dummy traffic injection
- ❌ **TODO**: Random delay insertion

**Risk Level:** **HIGH**

#### 3.3 **Sender Identity**

**Current Exposure:** Sender field in EncryptedEnvelope is plaintext

**Attack:**

```rust
pub struct EncryptedEnvelope {
    pub sender: Vec<u8>,  // ⚠️ VISIBLE
    pub payload: Vec<u8>, // Encrypted
}
```

**Attack:**

- Network observer sees WHO sent message
- Can build communication graphs
- Correlate activity across channels

**Defenses:**

- ❌ **TODO**: Sealed sender (Signal-style)
- ❌ **TODO**: Onion routing (sender anonymity)

**Risk Level:** **CRITICAL**

---

### 4. **Network Layer** 🔴 VULNERABLE

#### 4.1 **IP Addresses**

**Current Exposure:** Sender/receiver IP addresses visible to network

**Attack:**

```
Alice (IP: 192.168.1.100) → Bob (IP: 10.0.0.50)
→ Physical location inference
→ "Alice and Bob are communicating"
→ ISP knows who talks to whom
```

**Defenses:**

- ✅ **PLANNED**: Tor/onion routing integration
- ❌ **TODO**: VPN requirement enforcement
- ❌ **TODO**: Decoy traffic to confuse observers

**Risk Level:** **CRITICAL**

#### 4.2 **DNS Queries**

**Current Exposure:** DNS lookups for relay nodes

**Attack:**

```
DNS query: relay.spacepanda.io
→ "User is using SpacePanda"
→ ISP logs all queries
→ Timing correlation with messaging
```

**Defenses:**

- ❌ **TODO**: DNS-over-HTTPS (DoH)
- ❌ **TODO**: DNS-over-TLS (DoT)
- ✅ **PLANNED**: Hardcoded relay addresses (no DNS)

**Risk Level:** **MEDIUM**

---

### 5. **Storage Security** 🟢 SECURE

**Exposure:** Encrypted at rest with AES-256-GCM

**Defenses:**

- ✅ Argon2id KDF (64MB memory, 3 iterations)
- ✅ Random salts (prevents rainbow tables)
- ✅ AEAD tags (tamper detection)
- ✅ Automatic zeroization on Drop

**Threats:**

- ⚠️ Weak passphrases (user choice)
- ⚠️ Physical device seizure + key extraction
- ⚠️ Cold boot attacks (RAM dumps)

**Risk Level:** **LOW**

---

## Implemented Defenses

### **Phase 1: Core Crypto** ✅ COMPLETE

1. ✅ **MLS Protocol (RFC 9420)**

   - AES-256-GCM encryption
   - X25519 key exchange
   - Ed25519 signatures
   - OpenMLS library integration

2. ✅ **Forward Secrecy**

   - Epoch-based key rotation
   - Tested with member removal (e2e_member_removal.rs)

3. ✅ **Post-Compromise Security**

   - Member removal invalidates old keys
   - Cannot decrypt future messages

4. ✅ **Replay Protection**

   - Per-sender sequence numbers
   - 1000-entry LRU cache

5. ✅ **Secure Persistence**
   - Argon2id KDF
   - AES-256-GCM encryption
   - AEAD authentication

### **Phase 2: Privacy Enhancements** ✅ COMPLETE (THIS SPRINT)

6. ✅ **Message Padding** (NEW)

   - Fixed bucket sizes: 256, 1KB, 4KB, 16KB, 64KB
   - Prevents size-based traffic analysis
   - ~13 unit tests passing

7. ✅ **Sealed Metadata** (NEW)

   - Encrypts channel names, member lists, timestamps
   - AES-256-GCM with random nonces
   - Epoch binding prevents tampering
   - ~10 unit tests passing

8. ✅ **Privacy-First Peer Discovery**
   - NO DHT-based user discovery (removed)
   - Invite-only peer exchange
   - Prevents social graph leakage

---

## Remaining Vulnerabilities

### **Critical** (Must fix before v1.0)

1. 🔴 **Sender Identity Exposure**

   - Impact: Network observers see WHO sends messages
   - Fix: Implement sealed sender (encrypt sender field)
   - Effort: 1-2 weeks
   - Priority: **P0**

2. 🔴 **IP Address Leakage**
   - Impact: ISPs/network operators see communication endpoints
   - Fix: Integrate Tor/onion routing for all connections
   - Effort: 2-3 weeks
   - Priority: **P0**

### **High** (Should fix soon)

3. 🟡 **Message Timing Analysis**

   - Impact: Typing patterns, online status leaks
   - Fix: Constant-rate message mixing + dummy traffic
   - Effort: 1 week
   - Priority: **P1**

4. 🟡 **Member List Visibility**
   - Impact: Social graph reconstruction (partially mitigated by sealed metadata)
   - Fix: Anonymous credentials + per-channel identities
   - Effort: 2-3 weeks
   - Priority: **P1**

### **Medium** (Nice to have)

5. 🟡 **DNS Query Leakage**

   - Impact: "User is using SpacePanda" visible to ISP
   - Fix: DoH/DoT or hardcoded relay IPs
   - Effort: 3-5 days
   - Priority: **P2**

6. 🟡 **Perfect Padding**
   - Impact: Current buckets still leak some size info
   - Fix: All messages same size (e.g., 64KB always)
   - Effort: 1 day
   - Cost: **4x-64x bandwidth overhead**
   - Priority: **P3** (optional)

### **Low** (Future work)

7. ⚪ **Deniable Authentication**
   - Impact: Ed25519 signatures are non-repudiable
   - Fix: HMAC-based auth option for plausible deniability
   - Effort: 1 week
   - Priority: **P3**

---

## Roadmap

### **✅ Phase 1: Immediate (COMPLETE - This Week)**

- [x] Message padding (256/1KB/4KB/16KB/64KB buckets)
- [x] Sealed metadata encryption
- [x] Privacy threat model documentation

### **🚀 Phase 2: Near-term (This Month)**

- [ ] Sealed sender implementation
- [ ] Per-channel identity activation
- [ ] Constant-rate message mixing (optional)

### **🔮 Phase 3: Long-term (Next Quarter)**

- [ ] Tor/onion routing integration
- [ ] Anonymous credentials (ZK proofs)
- [ ] Deniable authentication option
- [ ] Traffic decoys

---

## Security Assumptions

### **What We Protect Against**

✅ Mass surveillance of message content  
✅ Passive network observers reading messages  
✅ Compromised relays reading messages  
✅ Historical decryption (forward secrecy)  
✅ Traffic size analysis (after padding)  
✅ Metadata leakage from storage (after sealed metadata)

### **What We DON'T Protect Against** (Yet)

⚠️ Active network attackers (MitM)  
⚠️ Malicious group members (by design)  
⚠️ Compromised devices with keys  
⚠️ Quantum computers (post-quantum crypto planned)  
⚠️ Timing analysis (constant-rate mixing planned)  
⚠️ IP address correlation (Tor integration planned)

### **Out of Scope**

❌ Physical device security (hardware tamper-proofing)  
❌ Operating system security (assume trusted OS)  
❌ Side-channel attacks (assume constant-time crypto)  
❌ Social engineering (user education required)

---

## Testing & Validation

### **Security Test Coverage**

**MLS Core:**

- 169/169 tests passing (100%)
- Includes adversarial scenarios (alpha_security_tests.rs)

**Privacy Features:**

- Message padding: 13/13 tests passing
- Sealed metadata: 10/10 tests passing
- Forward secrecy: Validated (e2e_member_removal.rs)
- Offline sync: Validated (e2e_offline_sync.rs)

**Total:** 1,184 tests passing (as of Dec 4, 2025)

### **Threat Model Validation**

| Threat                   | Test Coverage              | Status  |
| ------------------------ | -------------------------- | ------- |
| Message content exposure | ✅ E2E encryption tests    | Passing |
| Replay attacks           | ✅ Sequence number tests   | Passing |
| Forward secrecy          | ✅ Member removal tests    | Passing |
| Size analysis            | ✅ Padding roundtrip tests | Passing |
| Metadata leakage         | ✅ Sealed metadata tests   | Passing |
| Timing attacks           | ⚠️ Manual testing only     | TODO    |
| Network correlation      | ❌ No automated tests      | TODO    |

---

## Compliance & Legal

### **Regulations**

- **GDPR (EU)**: End-to-end encryption helps with data minimization
- **CCPA (California)**: Users control their data (local-first)
- **Lawful Access**: We cannot decrypt user messages (E2EE design)

### **Limitations**

- Metadata may be accessible with legal requests (pre-sealed metadata)
- IP addresses visible to network operators
- Group members can screenshot/record (social layer)

---

## References

1. **MLS Protocol**  
   RFC 9420: https://datatracker.ietf.org/doc/rfc9420/

2. **Signal Protocol**  
   Sealed Sender: https://signal.org/blog/sealed-sender/

3. **Tor Project**  
   Onion Routing: https://www.torproject.org/

4. **NIST Post-Quantum**  
   PQC Standardization: https://csrc.nist.gov/projects/post-quantum-cryptography

5. **OpenMLS**  
   Library: https://github.com/openmls/openmls

---

## Contact

For security concerns or to report vulnerabilities:

- **Email**: security@spacepanda.io (TODO)
- **PGP Key**: TODO
- **Responsible Disclosure**: 90-day window

---

**Last Updated**: December 4, 2025  
**Next Review**: March 4, 2026 (quarterly)
