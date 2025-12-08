# SpacePanda Threat Model

**Version**: 1.0  
**Date**: December 7, 2025  
**Status**: Initial Release  
**Scope**: SpacePanda Core MLS Implementation

---

## Executive Summary

This document provides a comprehensive threat model for SpacePanda, a secure messaging platform built on the Messaging Layer Security (MLS) protocol. The threat model identifies assets, trust boundaries, potential threats, and security controls implemented to protect users' privacy and security.

**Key Security Properties**:

- End-to-end encryption using MLS protocol (RFC 9420)
- Forward secrecy and post-compromise security
- Metadata encryption for sensitive channel data
- HKDF-based key derivation with domain separation
- Privacy-first design (no tracking, minimal metadata)

---

## Table of Contents

1. [System Overview](#system-overview)
2. [Assets](#assets)
3. [Trust Boundaries](#trust-boundaries)
4. [Threat Actors](#threat-actors)
5. [Threat Analysis (STRIDE)](#threat-analysis-stride)
6. [Attack Trees](#attack-trees)
7. [Security Controls](#security-controls)
8. [Residual Risks](#residual-risks)
9. [Security Assumptions](#security-assumptions)

---

## System Overview

### Architecture

SpacePanda consists of several key components:

```
┌─────────────────────────────────────────────────────────────┐
│                        Client Layer                         │
│  ┌─────────────┐  ┌──────────────┐  ┌──────────────────┐  │
│  │ UI/UX Layer │  │ Message Queue│  │ Event Dispatcher │  │
│  └─────────────┘  └──────────────┘  └──────────────────┘  │
└─────────────────────────────────────────────────────────────┘
                            │
┌─────────────────────────────────────────────────────────────┐
│                     Core MLS Layer                          │
│  ┌──────────────┐  ┌───────────────┐  ┌────────────────┐  │
│  │ MLS Protocol │  │ Key Management│  │ Group State Mgr│  │
│  └──────────────┘  └───────────────┘  └────────────────┘  │
│  ┌──────────────┐  ┌───────────────┐  ┌────────────────┐  │
│  │ Crypto Ops   │  │ Message Router│  │ Session Handler│  │
│  └──────────────┘  └───────────────┘  └────────────────┘  │
└─────────────────────────────────────────────────────────────┘
                            │
┌─────────────────────────────────────────────────────────────┐
│                      Storage Layer                          │
│  ┌──────────────┐  ┌───────────────┐  ┌────────────────┐  │
│  │ SQL Storage  │  │ Metadata Enc. │  │ Key Packages   │  │
│  └──────────────┘  └───────────────┘  └────────────────┘  │
└─────────────────────────────────────────────────────────────┘
                            │
┌─────────────────────────────────────────────────────────────┐
│                    Network/Transport                        │
│  ┌──────────────┐  ┌───────────────┐  ┌────────────────┐  │
│  │ RPC Protocol │  │ Noise Protocol│  │ Rate Limiting  │  │
│  └──────────────┘  └───────────────┘  └────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

### Data Flow

1. **Message Sending**:

   - User creates message → Core MLS encrypts → Network layer sends
   - MLS ensures end-to-end encryption, forward secrecy

2. **Message Receiving**:

   - Network receives encrypted message → Core MLS decrypts → UI displays
   - Validates sender, checks epoch, updates group state

3. **Group Management**:

   - Add/remove members → Update MLS group state → Distribute new epoch keys
   - All members re-key, ensuring forward secrecy

4. **Storage**:
   - Channel metadata encrypted with ChaCha20-Poly1305
   - Keys derived per-group using HKDF-SHA256
   - Message history stored encrypted in SQLite

---

## Assets

### Critical Assets

| Asset                 | Description                  | Confidentiality | Integrity | Availability |
| --------------------- | ---------------------------- | --------------- | --------- | ------------ |
| **Message Content**   | Plaintext message bodies     | CRITICAL        | HIGH      | MEDIUM       |
| **MLS Epoch Secrets** | Group encryption keys        | CRITICAL        | CRITICAL  | HIGH         |
| **Private Keys**      | User signing/encryption keys | CRITICAL        | CRITICAL  | HIGH         |
| **Channel Metadata**  | Names, topics, member lists  | HIGH            | MEDIUM    | MEDIUM       |
| **Key Packages**      | Pre-generated public keys    | MEDIUM          | HIGH      | HIGH         |
| **Group State**       | MLS ratchet tree state       | HIGH            | CRITICAL  | HIGH         |

### Supporting Assets

| Asset                   | Description            | Security Impact                  |
| ----------------------- | ---------------------- | -------------------------------- |
| **SQLite Database**     | Persistent storage     | Medium - contains encrypted data |
| **Configuration Files** | System settings        | Low - no secrets stored          |
| **Session State**       | Active connection info | Medium - temporary, in-memory    |
| **Logs/Metrics**        | Operational telemetry  | Low - no sensitive data logged   |

---

## Trust Boundaries

### Boundary 1: User Device

**Inside Boundary**:

- Client application process
- Local SQLite database
- In-memory key material
- OS-provided secure storage (if available)

**Outside Boundary**:

- Network
- Other users' devices
- Servers (if any)

**Trust Assumption**: User device OS and hardware are trusted (secure boot, no malware)

### Boundary 2: MLS Group

**Inside Boundary**:

- Verified group members
- Group secrets and epoch keys
- Shared group state

**Outside Boundary**:

- Non-members
- Former members (after removal)
- Server infrastructure

**Trust Assumption**: Group members are authenticated via MLS credentials

### Boundary 3: Network

**Inside Boundary**:

- Encrypted MLS messages
- Noise protocol encrypted transport
- Authenticated connections

**Outside Boundary**:

- Network infrastructure (ISPs, routers)
- Passive observers
- Active attackers (MITM)

**Trust Assumption**: All network traffic is untrusted and potentially adversarial

### Boundary 4: Storage

**Inside Boundary**:

- Encrypted database on disk
- HKDF-derived encryption keys
- Authenticated ciphertext

**Outside Boundary**:

- OS file system access controls
- Physical disk access
- Backup systems

**Trust Assumption**: Disk encryption at rest (OS-level or hardware) provides additional protection

---

## Threat Actors

### Threat Actor 1: Passive Network Observer

**Capabilities**:

- Monitor all network traffic
- Collect metadata (IP addresses, timing, sizes)
- Store traffic for later analysis

**Motivation**:

- Mass surveillance
- Traffic analysis
- Deanonymization

**Mitigations**:

- End-to-end encryption (MLS)
- Noise protocol for transport encryption
- Minimal metadata in protocol

### Threat Actor 2: Active Network Attacker (MITM)

**Capabilities**:

- Intercept and modify network traffic
- Inject malicious messages
- Perform replay attacks
- Drop or delay messages

**Motivation**:

- Impersonation
- Message tampering
- Denial of service

**Mitigations**:

- MLS authenticated encryption
- Noise protocol mutual authentication
- Replay protection via epoch tracking
- Message authentication codes

### Threat Actor 3: Malicious Group Member

**Capabilities**:

- Participate in group legitimately
- Access group secrets
- Send arbitrary messages
- Export/leak messages

**Motivation**:

- Information disclosure
- Social engineering
- Insider threats

**Mitigations**:

- Per-member authentication
- Forward secrecy (past messages safe after key rotation)
- Post-compromise security (future messages safe after compromise recovery)
- Member removal mechanism

### Threat Actor 4: Compromised Device

**Capabilities**:

- Read process memory
- Access local storage
- Intercept system calls
- Modify application behavior

**Motivation**:

- Key extraction
- Message interception
- Persistent access

**Mitigations**:

- Memory zeroization (`zeroize` crate)
- Encrypted storage
- Minimize key material lifetime
- OS-level protections (assumed trusted)

### Threat Actor 5: Database Attacker

**Capabilities**:

- Read SQLite database files
- Copy database backups
- Analyze file system metadata

**Motivation**:

- Metadata extraction
- Historical message access
- User profiling

**Mitigations**:

- Channel metadata encryption (ChaCha20-Poly1305)
- HKDF key derivation per group
- No plaintext sensitive data in database
- Minimal stored metadata

### Threat Actor 6: Software Supply Chain Attacker

**Capabilities**:

- Compromise dependencies
- Inject malicious code
- Backdoor cryptographic libraries

**Motivation**:

- Widespread compromise
- Long-term access
- Cryptographic backdoors

**Mitigations**:

- Dependency audit (cargo-audit): 353 crates scanned, 0 vulnerabilities
- Use of well-vetted cryptographic libraries
- Reproducible builds (Nix)
- Regular security updates

---

## Threat Analysis (STRIDE)

### Spoofing

| Threat                            | Impact | Likelihood | Mitigation                                               | Residual Risk |
| --------------------------------- | ------ | ---------- | -------------------------------------------------------- | ------------- |
| **Impersonate user in MLS group** | HIGH   | LOW        | MLS credential verification, signature checks            | LOW           |
| **Fake message origin**           | HIGH   | LOW        | MLS sender authentication, authenticated encryption      | LOW           |
| **Server impersonation**          | MEDIUM | MEDIUM     | Noise protocol mutual authentication, public key pinning | LOW           |
| **Key package forgery**           | HIGH   | LOW        | Signature verification, credential binding               | LOW           |

**Controls**:

- MLS protocol provides cryptographic authentication
- Ed25519 signatures for all critical operations
- Noise protocol XX handshake for transport authentication

### Tampering

| Threat                        | Impact   | Likelihood | Mitigation                                                 | Residual Risk |
| ----------------------------- | -------- | ---------- | ---------------------------------------------------------- | ------------- |
| **Modify encrypted messages** | HIGH     | LOW        | AEAD authentication tags, MLS message authentication       | LOW           |
| **Alter group state**         | CRITICAL | LOW        | MLS ratchet tree authentication, hash-based verification   | LOW           |
| **Corrupt database**          | MEDIUM   | LOW        | SQLite integrity checks, encrypted metadata authentication | LOW           |
| **Tamper with key packages**  | HIGH     | LOW        | Signature verification, hash binding                       | LOW           |

**Controls**:

- ChaCha20-Poly1305 AEAD provides integrity
- MLS protocol hash-based tree authentication
- Database integrity checks

### Repudiation

| Threat                    | Impact | Likelihood | Mitigation                             | Residual Risk |
| ------------------------- | ------ | ---------- | -------------------------------------- | ------------- |
| **Deny sending message**  | MEDIUM | HIGH       | MLS signatures provide non-repudiation | MEDIUM        |
| **Deny group membership** | LOW    | MEDIUM     | MLS membership proofs                  | LOW           |

**Controls**:

- MLS signatures provide cryptographic proof of origin
- Note: Deniability is sometimes a desired property for privacy

### Information Disclosure

| Threat                        | Impact   | Likelihood | Mitigation                                                   | Residual Risk |
| ----------------------------- | -------- | ---------- | ------------------------------------------------------------ | ------------- |
| **Plaintext message leakage** | CRITICAL | LOW        | End-to-end encryption (MLS), no plaintext storage            | LOW           |
| **Channel metadata leakage**  | HIGH     | MEDIUM     | Metadata encryption (ChaCha20-Poly1305), HKDF key derivation | LOW           |
| **Traffic analysis**          | MEDIUM   | HIGH       | Encrypted transport, minimal metadata                        | MEDIUM        |
| **Key material extraction**   | CRITICAL | LOW        | Memory zeroization, encrypted storage, short-lived keys      | MEDIUM        |
| **Database compromise**       | HIGH     | MEDIUM     | Encrypted metadata, no plaintext sensitive data              | LOW           |
| **Timing side-channels**      | LOW      | MEDIUM     | Constant-time crypto operations (library-provided)           | LOW           |

**Controls**:

- MLS end-to-end encryption for message content
- ChaCha20-Poly1305 for metadata encryption
- HKDF-SHA256 with domain separation for key derivation
- No timing metadata in database (no `last_updated`, `read_at` columns)
- No IP addresses or geolocation stored
- Memory zeroization for sensitive data (`zeroize` crate)

### Denial of Service

| Threat                      | Impact | Likelihood | Mitigation                                          | Residual Risk |
| --------------------------- | ------ | ---------- | --------------------------------------------------- | ------------- |
| **Message flood**           | MEDIUM | HIGH       | Rate limiting (1000 messages/minute per user)       | LOW           |
| **Large message attack**    | MEDIUM | MEDIUM     | Message size limits (1 MB tested), input validation | LOW           |
| **Database exhaustion**     | MEDIUM | MEDIUM     | Storage limits, cleanup mechanisms                  | MEDIUM        |
| **CPU exhaustion (crypto)** | LOW    | MEDIUM     | Rate limiting, async processing                     | LOW           |
| **Memory exhaustion**       | MEDIUM | LOW        | Bounded buffers, connection limits                  | LOW           |

**Controls**:

- Rate limiting: 1000 messages/minute per user
- Input validation: message size limits, malformed data rejection
- Async processing to prevent blocking
- Connection limits and timeouts

### Elevation of Privilege

| Threat                          | Impact   | Likelihood | Mitigation                                 | Residual Risk |
| ------------------------------- | -------- | ---------- | ------------------------------------------ | ------------- |
| **Gain admin/owner privileges** | HIGH     | LOW        | MLS group roles, permission checks         | LOW           |
| **Access other groups' data**   | HIGH     | LOW        | Per-group key derivation, isolation checks | LOW           |
| **SQL injection**               | HIGH     | VERY LOW   | Parameterized queries (100% coverage)      | VERY LOW      |
| **Exploit crypto library**      | CRITICAL | VERY LOW   | Use audited libraries, dependency scanning | LOW           |

**Controls**:

- All SQL queries use parameterized bindings (no string concatenation)
- Per-group encryption keys (HKDF with group_id as input)
- Group state isolation verified in tests
- Dependency audit: 0 vulnerabilities in 353 crates

---

## Attack Trees

### Attack Goal: Read User's Messages

```
Read User's Messages
├─ OR: Compromise Encryption
│  ├─ AND: Extract MLS Epoch Secret
│  │  ├─ Compromise device [MEDIUM DIFFICULTY]
│  │  │  └─ Malware, physical access, or OS exploit
│  │  └─ Extract from memory [LOW-MEDIUM DIFFICULTY]
│  │     └─ Mitigated by: zeroize, short-lived keys
│  ├─ AND: Break Cryptography [VERY HIGH DIFFICULTY]
│  │  ├─ Break ChaCha20-Poly1305 [INFEASIBLE]
│  │  └─ Break MLS protocol [INFEASIBLE]
│  └─ AND: Compromise Key Material
│     ├─ Steal private key [MEDIUM DIFFICULTY]
│     │  └─ Device compromise or backup theft
│     └─ Only gets past messages if forward secrecy not enabled
│        └─ Mitigated by: MLS ratcheting, epoch rotation
│
├─ OR: Compromise Storage
│  ├─ Read database file [MEDIUM DIFFICULTY]
│  │  └─ Physical access or file system exploit
│  └─ Decrypt encrypted metadata [HIGH DIFFICULTY]
│     ├─ Need per-group HKDF key
│     └─ Requires group_id knowledge + key derivation
│        └─ Mitigated by: HKDF, encrypted storage
│
├─ OR: Man-in-the-Middle
│  ├─ Intercept network traffic [EASY]
│  └─ Decrypt MLS messages [INFEASIBLE]
│     └─ Mitigated by: End-to-end encryption, authenticated channels
│
├─ OR: Insider Threat
│  ├─ Malicious group member reads messages [EASY]
│  │  └─ This is by design (group members can read messages)
│  └─ Export/leak messages [EASY]
│     └─ Cannot prevent (authorized access)
│        └─ Social/organizational control required
│
└─ OR: Supply Chain Attack
   ├─ Compromise dependency [MEDIUM DIFFICULTY]
   │  └─ Inject backdoor in crypto library
   │     └─ Mitigated by: Dependency audit, vetted libraries
   └─ Compromise build system [MEDIUM DIFFICULTY]
      └─ Inject malicious code during build
         └─ Mitigated by: Reproducible builds (Nix), code review
```

### Attack Goal: Identify Group Members

```
Identify Group Members
├─ OR: Traffic Analysis
│  ├─ Monitor network patterns [MEDIUM DIFFICULTY]
│  │  └─ Correlate message timing/sizes
│  │     └─ Residual Risk: MEDIUM (metadata not fully hidden)
│  └─ IP address correlation [EASY-MEDIUM]
│     └─ Network observer sees source/dest IPs
│        └─ Mitigated by: Use Tor/VPN (not built-in)
│
├─ OR: Database Compromise
│  ├─ Read encrypted metadata [HIGH DIFFICULTY]
│  │  └─ Requires HKDF key derivation
│  │     └─ Mitigated by: ChaCha20-Poly1305, HKDF
│  └─ Analyze database schema [LOW DIFFICULTY]
│     └─ No plaintext member names/IDs
│        └─ Mitigated by: Encrypted storage, hashed IDs
│
└─ OR: Group Member Reveals
   └─ Social engineering/coercion [EASY]
      └─ Cannot prevent (authorized access)
```

### Attack Goal: Disrupt Service (DoS)

```
Disrupt Service
├─ OR: Network-Level DoS
│  ├─ Flood network connection [EASY]
│  │  └─ Overwhelm bandwidth
│  │     └─ Mitigated by: Rate limiting, connection limits
│  └─ SYN flood [EASY]
│     └─ OS-level mitigation required
│
├─ OR: Application-Level DoS
│  ├─ Send large messages [MEDIUM DIFFICULTY]
│  │  └─ Exhaust memory/storage
│  │     └─ Mitigated by: 1 MB size limit, input validation
│  ├─ Message flood [EASY]
│  │  └─ Send many messages rapidly
│  │     └─ Mitigated by: Rate limiting (1000 msg/min)
│  └─ Malformed messages [LOW-MEDIUM DIFFICULTY]
│     └─ Trigger parsing errors
│        └─ Mitigated by: Input validation, error handling
│
└─ OR: Resource Exhaustion
   ├─ Fill database [MEDIUM DIFFICULTY]
   │  └─ Send many messages over time
   │     └─ Mitigated by: Storage limits, cleanup
   └─ CPU exhaustion [LOW DIFFICULTY]
      └─ Force expensive crypto operations
         └─ Mitigated by: Rate limiting, async processing
```

---

## Security Controls

### Implemented Controls

| Control                        | Category            | Asset Protected      | Threat Mitigated                             |
| ------------------------------ | ------------------- | -------------------- | -------------------------------------------- |
| **MLS Protocol (RFC 9420)**    | Cryptography        | Messages, Keys       | Spoofing, Information Disclosure             |
| **ChaCha20-Poly1305 AEAD**     | Cryptography        | Metadata             | Information Disclosure, Tampering            |
| **HKDF-SHA256 Key Derivation** | Cryptography        | Encryption Keys      | Information Disclosure, Privilege Escalation |
| **Ed25519 Signatures**         | Cryptography        | Messages, Keys       | Spoofing, Tampering                          |
| **Noise Protocol XX**          | Cryptography        | Transport            | Spoofing, Information Disclosure             |
| **Parameterized SQL Queries**  | Input Validation    | Database             | SQL Injection, Privilege Escalation          |
| **Rate Limiting**              | Resource Management | Service Availability | Denial of Service                            |
| **Input Validation**           | Input Validation    | All Assets           | Tampering, DoS                               |
| **Memory Zeroization**         | Memory Safety       | Key Material         | Information Disclosure                       |
| **Encrypted Storage**          | Data Protection     | Metadata, Messages   | Information Disclosure                       |
| **Dependency Audit**           | Supply Chain        | All Assets           | Supply Chain Attack                          |
| **Domain Separation**          | Cryptography        | Keys                 | Privilege Escalation, Information Disclosure |

### Control Effectiveness

**High Effectiveness** (>90% threat reduction):

- MLS end-to-end encryption
- Authenticated encryption (AEAD)
- Parameterized SQL queries
- Input validation

**Medium Effectiveness** (50-90% threat reduction):

- Rate limiting
- Encrypted metadata storage
- Memory zeroization

**Low Effectiveness** (<50% threat reduction):

- Traffic analysis mitigation (requires external tools like Tor)

### Testing Coverage

| Control                  | Test Count     | Coverage                                       |
| ------------------------ | -------------- | ---------------------------------------------- |
| Cryptographic Primitives | 5 tests        | Key lifecycle, expiration, uniqueness          |
| Privacy Protection       | 7 tests        | No plaintext leaks, minimal metadata           |
| Input Validation         | 10 tests       | SQL injection, large inputs, concurrent writes |
| Metadata Encryption      | 14 tests       | Encryption, HKDF, domain separation            |
| Dependency Audit         | 353 crates     | 0 vulnerabilities found                        |
| **Total**                | **1273 tests** | **100% pass rate**                             |

---

## Residual Risks

### High Residual Risks

1. **Device Compromise**

   - **Risk**: Malware or physical access to user device
   - **Impact**: CRITICAL (full access to keys and messages)
   - **Likelihood**: LOW (depends on user's security practices)
   - **Mitigation**: User education, OS security features, consider hardware security modules
   - **Acceptance**: Assumed trusted device in threat model

2. **Traffic Analysis**
   - **Risk**: Network observers correlate message patterns
   - **Impact**: MEDIUM (metadata leakage, deanonymization)
   - **Likelihood**: HIGH (passive observation is easy)
   - **Mitigation**: Use Tor/VPN (not built-in), padding (future work)
   - **Acceptance**: Metadata privacy is a hard problem, requires network-level solutions

### Medium Residual Risks

3. **Malicious Group Member**

   - **Risk**: Insider leaks messages or metadata
   - **Impact**: HIGH (authorized access cannot be prevented)
   - **Likelihood**: MEDIUM (depends on group trust)
   - **Mitigation**: Organizational/social controls, member vetting
   - **Acceptance**: By design, group members have access

4. **Database Backup Compromise**
   - **Risk**: Encrypted database backups stolen
   - **Impact**: MEDIUM (metadata still encrypted, but targeted attacks possible)
   - **Likelihood**: LOW (requires specific targeting)
   - **Mitigation**: HKDF keys not stored in database, require group_id knowledge
   - **Acceptance**: Encourage users to secure backups

### Low Residual Risks

5. **Timing Side-Channels**

   - **Risk**: Timing attacks on crypto operations
   - **Impact**: LOW (limited information leakage)
   - **Likelihood**: MEDIUM (requires local observation)
   - **Mitigation**: Use constant-time crypto libraries (already done)
   - **Acceptance**: Libraries provide constant-time guarantees

6. **Dependency Vulnerabilities (Future)**
   - **Risk**: New CVEs discovered in dependencies
   - **Impact**: VARIES (depends on vulnerability)
   - **Likelihood**: MEDIUM (new vulnerabilities discovered regularly)
   - **Mitigation**: Regular `cargo audit` runs, automated updates
   - **Acceptance**: Ongoing monitoring required

---

## Security Assumptions

### Cryptographic Assumptions

1. **ChaCha20-Poly1305 is secure**: No practical attacks exist
2. **Ed25519 signatures are unforgeable**: Discrete log problem is hard
3. **SHA-256 is collision-resistant**: No known practical attacks
4. **HKDF is a secure KDF**: RFC 5869 properties hold
5. **MLS protocol is secure**: RFC 9420 security analysis is correct

### Platform Assumptions

1. **OS is not compromised**: Kernel, system libraries are trusted
2. **Hardware is not backdoored**: CPU, TPM (if used) are trusted
3. **Random number generator is secure**: OS-provided RNG is cryptographically secure
4. **Memory protection works**: Process isolation is enforced by OS

### Operational Assumptions

1. **Users protect their devices**: Screen locks, disk encryption, etc.
2. **Build system is trusted**: Nix derivations are not tampered with
3. **Dependencies are maintained**: Security patches are released
4. **Group members are trustworthy**: Social vetting of participants

### Out of Scope

The following are explicitly **out of scope** for this threat model:

1. **Physical security of devices**: User responsibility
2. **Social engineering attacks**: User education required
3. **Legal/lawful intercept**: Compliance not addressed
4. **Quantum computing attacks**: Post-quantum crypto future work
5. **Anonymous communication**: Requires Tor/mixnets (not built-in)
6. **Server-side components**: This model covers client-side only

---

## Security Roadmap

### Completed (Phase 3, Weeks 8-9)

- ✅ Metadata encryption (ChaCha20-Poly1305)
- ✅ HKDF key derivation with domain separation
- ✅ Dependency audit (0 vulnerabilities)
- ✅ Comprehensive security testing (1273 tests)

### In Progress (Week 9)

- 🔄 Threat model documentation (this document)
- ⏳ Privacy audit of all data flows
- ⏳ Timing attack resistance validation

### Planned (Week 10)

- ⏳ Fuzz testing for message parsing
- ⏳ Rate limiting security validation
- ⏳ Penetration testing
- ⏳ Security documentation updates

### Future Enhancements

- Key rotation mechanism (manual/automatic)
- Post-quantum cryptography support (Kyber, Dilithium)
- Traffic padding for metadata privacy
- Hardware security module integration
- Secure enclave support (iOS, Android)

---

## Conclusion

SpacePanda implements strong security controls based on modern cryptographic protocols (MLS, Noise) and best practices. The primary security properties are:

✅ **Confidentiality**: End-to-end encryption protects message content  
✅ **Integrity**: Authenticated encryption prevents tampering  
✅ **Authentication**: Cryptographic signatures verify identity  
✅ **Forward Secrecy**: Past messages protected after key compromise  
✅ **Post-Compromise Security**: Future messages protected after recovery  
✅ **Privacy**: Minimal metadata, encrypted storage, no tracking

**Residual risks** are primarily related to device compromise and traffic analysis, which require defense-in-depth approaches beyond the application layer.

**Ongoing security maintenance** includes regular dependency audits, security testing, and monitoring for new vulnerabilities.

---

## Document History

| Version | Date             | Author        | Changes              |
| ------- | ---------------- | ------------- | -------------------- |
| 1.0     | December 7, 2025 | Security Team | Initial threat model |

---

## References

- [RFC 9420: Messaging Layer Security (MLS)](https://www.rfc-editor.org/rfc/rfc9420.html)
- [RFC 5869: HKDF - HMAC-based Key Derivation Function](https://www.rfc-editor.org/rfc/rfc5869.html)
- [Noise Protocol Framework](https://noiseprotocol.org/)
- [STRIDE Threat Modeling](https://learn.microsoft.com/en-us/azure/security/develop/threat-modeling-tool-threats)
- [ChaCha20-Poly1305 AEAD](https://tools.ietf.org/html/rfc8439)
- [Ed25519 Signatures](https://ed25519.cr.yp.to/)
