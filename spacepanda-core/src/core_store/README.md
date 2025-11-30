Below is the **full intro/spec textbook for `/core_store`** — written as if it were the official design document for your system.
This is the _complete conceptual guide_ you keep beside you while building the P2P encrypted Discord clone.

It explains:

- What `/core_store` is
- Why it exists
- How models, CRDTs, sync, and storage work together
- How DHT, Router, and MLS interact with it
- Threat model and security guarantees
- File-by-file responsibilities
- Inputs/outputs for all components

This is something an architect would deliver before implementation begins.

---

# 📘 **/core_store — The Official Architecture Textbook**

### _The Persistent, Replicated, Secure State Layer for a Decentralized E2EE Chat Platform_

---

# 1. **High-Level Purpose of `/core_store`**

`/core_store` is the **authoritative state layer** of your decentralized Discord-like application.

It is responsible for:

### ✔ **Modeling all durable entities**

- Spaces
- Channels
- Roles
- Messages
- Identities
- MLS state

### ✔ **Applying CRDT logic**

- Causal ordering
- Conflict resolution
- Replicated data types
- Operation logs
- Append-only audit trail

### ✔ **Persisting local state**

- On-disk snapshot
- Commit log
- Indexes
- Encrypted storage

### ✔ **Synchronizing with the DHT**

- Publishing deltas
- Fetching missing ops
- Anti-entropy sync loops
- Validating incoming state

### ✔ **Providing search/query capabilities**

- Search index
- Query engine

### ✔ **Security of stored data**

- Per-channel MLS keys
- Local encryption of at-rest state
- Signature validation of all CRDT ops
- Strict schema validation

`/core_store` is the **heart of the application**.
Everything else (networking, DHT, MLS, UI) _feeds into or consumes it_.

---

# 2. **Mental Model: How Everything Connects**

### The Router

➡ securely transmits messages between peers
➡ hands CRDT operations and MLS commits to `/core_store`

### The DHT

➡ acts as a global bulletin board
➡ stores _encrypted_ CRDT deltas
➡ allows offline users to sync back up

### MLS

➡ encrypts actual message content
➡ encrypts commit secrets
➡ prevents kicked users from reading future messages

### `/core_store`

➡ validates incoming operations
➡ applies them to CRDT state
➡ persists results
➡ serves data to the UI

---

# 3. **What `/core_store` Tries To Solve (Design Goals)**

### 🟦 **Goal 1 — Immutable, Verifiable, Replicated Data**

Everything that changes in the app (roles, messages, channel metadata) must be:

- versioned
- signed
- causally ordered
- conflict-free

### 🟩 **Goal 2 — Survive Partial Connectivity**

Nodes may be offline, behind NATs, or sleeping (mobile).

`/core_store` ensures eventual consistency by:

- embedding vector clocks
- storing an operation log
- syncing deltas through the DHT

### 🟧 **Goal 3 — Strict data validation & security**

Every incoming update passes through:

- CRDT type validation
- schema validation
- signature validation
- causal ordering
- MLS identity checks

### 🟥 **Goal 4 — Confidentiality**

Stored content must remain encrypted:

- Even if the disk is stolen
- Even if the DHT is monitored
- Even if network packets are captured

---

# 4. **Directory Structure (Reference Layout)**

```
/core_store
    mod.rs

    /model
        channel.rs
        space.rs
        message.rs
        roles.rs
        identity_meta.rs
        mls_state.rs
        types.rs

    /crdt
        oplog.rs
        traits.rs
        lww_register.rs
        or_set.rs
        or_map.rs
        g_list.rs
        vector_clock.rs
        signer.rs

    /sync
        apply_local.rs
        apply_remote.rs
        delta_encoder.rs
        delta_decoder.rs
        anti_entropy.rs

    /store
        local_store.rs
        commit_log.rs
        index.rs
        snapshot.rs
        encryption.rs
        validator.rs
        errors.rs
        dht_adapter.rs

    /query
        query_engine.rs
        search_index.rs
```

---

# 5. **Subsystem Overview**

## 5.1 **/model — Data Structures**

Defines the logical entities.

### Responsibilities

- Define core schema for messages, channels, space metadata.
- Define how MLS state is attached to a channel.
- Types for role permissions, timestamps, etc.

### Notes

Models themselves contain NO logic.
All mutation is done through CRDTs.

---

## 5.2 **/crdt — Conflict-Free Replication**

Implements CRDT types that represent state changes.

### Responsibilities

- Merge operations from remote peers
- Ensure deterministic outcome
- Maintain causal order (vector clocks)
- Maintain per-channel signatures

### Notes

These CRDT operations are what get transmitted via DHT or Router.

---

## 5.3 **/sync — Propagation & Reconciliation**

Implements how CRDT ops enter the system.

### apply_local.rs

➡ merges local user actions (sending message, editing topic)

### apply_remote.rs

➡ merges CRDT ops received from peers/DHT

### delta_encoder/decoder

➡ packages changes for DHT posting

### anti_entropy

➡ periodically detects missing ops from neighbors and fetches them

---

## 5.4 **/store — Persistent Storage Engine**

This is the “database”.

### local_store.rs

High level interface:

```rust
fn apply_op(op: CrdtOp)
fn get_snapshot() -> Snapshot
fn commit() -> Result<..>
```

### commit_log.rs

Append-only log of all CRDT ops.

### index.rs

Efficient lookup structures (by channel, time, sender, etc.)

### snapshot.rs

Rehydrates entire state from disk at startup.

### encryption.rs

Encrypt all at-rest data (AES-GCM or ChaCha20-Poly1305).

### validator.rs

Validates CRDT ops before applying.

### dht_adapter.rs

Bridges CRDT deltas to/from DHT.

---

## 5.5 **/query — Search & Presentation**

Not part of replication — only for UI.

### Responsibilities

- Visible sorted channels
- Searching messages
- Thread reconstruction (forums)
- Filtering by sender/role/time

---

# 6. **Security Design of `/core_store`**

### ✔ All CRDT operations are signed

Every update includes:

- author’s per-channel signing key
- signature over the operation bytes
- vector clock state
- optional MLS epoch metadata

This prevents:

- Fake messages
- Tampered deltas
- Role escalation attacks

---

### ✔ All local storage is encrypted

Using:

- Key derived from user master identity key
- AES-GCM (recommended)

Protects against:

- Device theft
- DHT poisoning
- Malicious relays

---

### ✔ MLS enforces confidentiality of message bodies

CRDT data is plaintext, but **message.content is encrypted by MLS**.

CRDT stores metadata; MLS stores secret payloads.

---

### ✔ PRP-Hiding for DHT keys

Channel IDs → hashed → used as DHT keys.
Attackers cannot enumerate channels.

---

# 7. **Lifecycle of a State Update**

Here is how one CRDT update moves:

```
User action → apply_local.rs
        ↓
CRDT op generated
        ↓
commit_log.rs stores it
        ↓
index.rs updates indexes
        ↓
delta_encoder.rs produces DHT delta
        ↓
router.send_direct OR dht_adapter.put
        ↓
other peers receive it
        ↓
apply_remote.rs merges it
```

---

# 8. **File-by-File Specification**

---

# `/model/...`

### **channel.rs**

❱ Defines channel metadata: name, topic, creation time, type.
❱ CRDT uses OR-Map or LWWRegister for fields.
**Input:** CRDT ops
**Output:** In-memory read model for UI

---

### **space.rs**

Describes a “server” or “space”.

Includes:

- owner identity
- list of channel IDs
- role policy

---

### **message.rs**

Defines message metadata (ID, author, timestamp).
Content is MLS ciphertext.

---

### **roles.rs**

Role hierarchy: 0–100 integer.
Stored in OR-Map or LWW.

---

### **identity_meta.rs**

Internal metadata for the user's identities on this device.

---

### **mls_state.rs**

Per-channel MLS epoch state:

- epoch counter
- welcome secrets
- commit secrets

---

### **types.rs**

Common timestamp types, errors, IDs.

---

# `/crdt/...`

### **oplog.rs**

Append-only log of CRDT operations.
Vector-clocked.
Signed.

### **traits.rs**

Unified interface:

```rust
trait Crdt {
    fn apply(&mut self, op: CrdtOp);
    fn merge(&mut self, other: &Self);
}
```

### **lww_register.rs**

For single-value fields.

### **or_set.rs**

For membership sets:

- channel members
- roles
- pinned messages

### **or_map.rs**

For maps keyed by user or channel.

### **g_list.rs**

Ordered message list (LSEQ or RGA).

### **vector_clock.rs**

Causal tracking.

### **signer.rs**

Signs CRDT ops per-channel.

---

# `/sync/...`

### **apply_local.rs**

User does something → produce CRDT op:

```rust
fn create_message(text, channel_id)
```

Outputs CRDT op.

---

### **apply_remote.rs**

Validates + applies remote ops.

---

### **delta_encoder.rs**

Compress ops → DHT-friendly bundle.

---

### **delta_decoder.rs**

Parse bundles → CRDT ops.

---

### **anti_entropy.rs**

Periodically ask neighbors:

```
“Do you have ops after these vector clocks?”
```

---

# `/store/...`

### **local_store.rs**

High-level API for the rest of the system.

---

### **commit_log.rs**

Stores raw CRDT ops.

---

### **index.rs**

For looking up messages by time/user/channel.

---

### **snapshot.rs**

Read/write whole state snapshot.

---

### **encryption.rs**

Encrypt at-rest storage.

---

### **validator.rs**

Reject malformed CRDT ops.

---

### **dht_adapter.rs**

Sends and fetches deltas from DHT.

---

### **errors.rs**

Store-specific error types.

---

# `/query/...`

### **query_engine.rs**

Sorting, filtering, projecting state.

### **search_index.rs**

Full-text search index (optional).

---

# 9. **How MLS Interacts With core_store**

📌 MLS governs **who can read message content**
📌 CRDT governs **who can mutate channel state**

The two layers combine:
CRDT resolves conflicts, MLS encrypts payloads.

---

# 10. **Summary (What /core_store Achieves)**

### `/core_store` ensures that your entire distributed system has:

- deterministic state
- replicated consistency
- encrypted persistence
- secure validation
- efficient sync
- fast queries
- CRDT-based conflict resolution
- DHT interoperability
- MLS confidentiality

It is the **foundation** of your product.
