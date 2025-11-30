Below is **the complete, detailed, textbook-level specification** for your **/core_identity** subsystem.

This is the subsystem that sits **between raw networking (router)** and **secure group messaging (core_mls)**.
It is also the only subsystem that all other modules rely on:

- **Router** uses identity keys for authenticated sessions
- **DHT** stores identity metadata
- **MLS** uses identity keys to validate key packages
- **CRDT store** uses identity to authorize writes
- **UI** displays user/device identity metadata

So this subsystem must be clean, stable, and absolutely clear.

---

# 📦 `/core_identity` — Overview

### **What this subsystem solves**

1. **Global Identity**

   - A long-term Ed25519 keypair representing a person.
   - Identifies the user across spaces/channels/devices.

2. **Device Identity**

   - Each physical client (phone, laptop, desktop) gets its own keypair.
   - Allows revocation of a stolen device without deleting the user.

3. **Identity Metadata**

   - Username / nickname
   - Avatar hash
   - Capabilities & client version
   - Device bundles needed by MLS

4. **Identity Verification**

   - Signatures proving:

     - user owns device
     - user created a space
     - user joins a channel
     - user publishes key packages

5. **Local Keystore**

   - Encrypted storage for all keys.
   - Provides serialization/deserialization for persistence.

6. **Identity Syncing**

   - Identity metadata replicated via CRDT.
   - Device bundles published to DHT for lookups.

7. **Integration**

   - Router: peers identify themselves via long-term identity key.
   - DHT: stores identity metadata.
   - MLS: uses identity key to sign key packages and group membership.

---

# 📂 Folder Layout

Here is the complete recommended layout:

```
/core_identity
    identity_manager.rs
    user_id.rs
    device_id.rs
    keypair.rs
    metadata.rs
    bundles.rs
    signatures.rs
    keystore/
        mod.rs
        file_keystore.rs
        memory_keystore.rs
    dht_sync.rs
    validation.rs
    mod.rs
```

Now we go file-by-file.

---

# 📘 1. `identity_manager.rs`

### **Responsibility**

The "main controller" of all identity-related operations.

It does:

- Generate/load global identity keypair
- Generate/load device keypair
- Produce a `UserId` and `DeviceId`
- Manage identity metadata
- Produce signed identity statements
- Interface with keystore
- Interface with DHT

This is the **core entrypoint**.

---

### **Inputs**

- Optional existing keystore
- Optional password/passphrase
- DHT handle (for publishing metadata)
- Local CRDT store handle

---

### **Outputs**

- `UserId`
- `DeviceId`
- `IdentityKeypair`
- `DeviceKeypair`
- Signed identity bundles
- Metadata CRDT updates

---

### **Notes**

This module is what other subsystems call:

```
identity = identity_manager.current_identity()
identity.sign(message)
identity.publish_metadata()
identity.publish_key_package()
```

---

# 📘 2. `user_id.rs`

### **Responsibility**

Defines the stable global identity identifier.

### Typical structure:

```rust
pub struct UserId(Vec<u8>); // 32 byte hash: blake3(public_key)
```

### Inputs

- Public key bytes

### Outputs

- 32-byte hash
- Display string (base58, hex, or bech32)

### Notes

This ID never changes for the same identity keypair.

---

# 📘 3. `device_id.rs`

### **Responsibility**

Uniquely identify each device under the same user.

### Typical structure:

```rust
pub struct DeviceId(Vec<u8>); // random 16-byte number or hash of device pubkey
```

### Inputs

- Device public key
- Random seed

### Outputs

- Unique device identifier

### Notes

Used inside MLS key packages.

---

# 📘 4. `keypair.rs`

### **Responsibility**

Handles the raw crypto material:

- identity keypair (Ed25519)
- device keypair (Ed25519)

Also provides:

- serialization
- deserialization
- signing
- verifying

### Inputs

- entropy
- secret bytes
- encoded keystore material

### Outputs

- signatures
- verification results
- ser/de representations

---

# 📘 5. `metadata.rs`

### **Responsibility**

Represent metadata about the user and devices.
This is replicated as a CRDT object across peers.

### Fields typically:

```rust
pub struct UserMetadata {
    pub display_name: LWWRegister<String>,
    pub avatar_hash: LWWRegister<Hash>,
    pub devices: ORMap<DeviceId, DeviceMetadata>,
}
```

`DeviceMetadata` includes:

- device_name
- last_seen timestamp
- capabilities (protocol version)
- key package references

---

### Inputs

- Local user choosing name/avatar
- Remote peers updating their metadata

### Outputs

- CRDT operations
- snapshots for DHT

---

# 📘 6. `bundles.rs`

### **Responsibility**

Generate bundles needed for MLS:

- Key packages (per device)
- Signed identity bundle
- Signed device bundle
- Key bundle advertisements

### Inputs

- device keypair
- identity keypair
- MLS credential format

### Outputs

- MLS KeyPackage
- Signature over KeyPackage
- Bundle advertised into DHT

---

# 📘 7. `signatures.rs`

### **Responsibility**

Defines all the signed statements produced by your system.

Examples:

- "I am the owner of this device"
- "I created this space"
- "I created this channel"
- "This key package belongs to me"

The format might look like:

```rust
pub enum IdentitySignature {
    DeviceOwnership { device_id, user_id, signature },
    SpaceOwnership { space_id, user_id, signature },
    ChannelCreation { channel_id, user_id, signature },
    KeyPackage { hash, user_id, device_id, signature },
}
```

### Inputs

- identity keypair
- object ID to sign

### Outputs

- signed statement blob

---

# 📘 8. `keystore/mod.rs`

### **Responsibility**

High-level interface for storing secrets securely.

Methods like:

- `save_identity_keypair`
- `save_device_keypair`
- `load_identity_keypair`
- `load_device_keypair`

---

# 📘 9. `keystore/file_keystore.rs`

### **Responsibility**

Encrypted file-based keystore.

### Inputs

- filesystem path
- password
- keypair bytes

### Outputs

- encrypted file
- deserialized keypair

---

# 📘 10. `keystore/memory_keystore.rs`

### **Responsibility**

Non-persistent keystore used for tests.

---

# 📘 11. `dht_sync.rs`

### **Responsibility**

Publish identity metadata to DHT and retrieve remote identities.

This includes:

- user metadata
- device bundles
- key packages

### Inputs

- metadata CRDT operations
- key packages
- DHT RPC

### Outputs

- updated DHT records
- remote metadata fetched

---

# 📘 12. `validation.rs`

### **Responsibility**

Verify all identity-related signatures and metadata.

It checks:

- that the user identity signature is correct
- that device is owned by the right user
- that MLS key package is valid
- that DHT identity record is not tampered

### Inputs

- signed statement
- public keys
- hashes

### Outputs

- `Ok(())` or `IdentityValidationError`

---

# 📘 13. `mod.rs`

### **Responsibility**

Convenience imports and re-exports:

```
pub mod identity_manager;
pub mod keypair;
pub mod user_id;
pub mod device_id;
pub mod metadata;
pub mod bundles;
pub mod signatures;
pub mod validation;
pub mod dht_sync;
pub mod keystore;
```

---

# 🧠 **How the whole `/core_identity` subsystem works together**

Here is the workflow:

```
Start app →
  IdentityManager.load_or_create_identity() →
    load identity keypair
    load device keypair
    compute user_id + device_id
    load CRDT metadata (name/avatar/devices)
    publish device bundle to DHT
    publish metadata to DHT
 → begin network sync + MLS
```

Everything in the system depends on this.

---

# 📦 **Input/Output Summary Table**

| File                | Inputs                             | Outputs                                        |
| ------------------- | ---------------------------------- | ---------------------------------------------- |
| identity_manager.rs | entropy, keystore, DHT, CRDT store | UserId, DeviceId, keypairs, published metadata |
| user_id.rs          | public key bytes                   | stable 32-byte hash                            |
| device_id.rs        | device pubkey                      | unique device ID                               |
| keypair.rs          | secret bytes                       | signatures, verification                       |
| metadata.rs         | CRDT ops                           | updated user/device metadata                   |
| bundles.rs          | identity+device keys               | MLS key packages, signed bundles               |
| signatures.rs       | object IDs                         | signed statements                              |
| keystore/\*         | password, keypair bytes            | encrypted file, loaded keypair                 |
| dht_sync.rs         | metadata, key packages             | DHT entries                                    |
| validation.rs       | signatures, pubkeys                | validation results                             |

---

Here is a clean ASCII pipeline diagram showing how the subsystems relate. This is **not** an internal architecture diagram, but a _data-flow_ / _responsibility_ pipeline, which is what you asked:

```
┌──────────────┐        ┌───────────┐        ┌──────────┐        ┌────────┐        ┌──────────┐
│   Identity    │  ⇄     │   MLS     │  ⇄     │   CRDT    │  ⇄     │   DHT   │  ⇄     │   Router   │
└──────────────┘        └───────────┘        └──────────┘        └────────┘        └──────────┘
        │                    │                     │                 │                    │
        │ verifies           │ secures             │ merges & sorts  │ stores, fetches    │ sends/receives
        │ keys, users        │ membership, crypto  │ application data│ distributed data   │ packets (LAN/WAN)
        │                    │                     │                 │                    │
```

### **Data Flow (High-level)**

Below is the expanded flow showing _what each stage produces and consumes_:

```
User Credentials
        │
        ▼
┌────────────────────────────┐
│        Identity            │
│ - login()                  │
│ - load_local_keys()        │
│ - provision_device()       │
└────────────────────────────┘
        │  outputs:
        │   { user_id, device_id, identity_keypair }
        ▼
┌────────────────────────────┐
│           MLS              │
│ - join_group()             │
│ - encrypt(msg)             │
│ - decrypt(msg)             │
│ - update_epoch()           │
└────────────────────────────┘
        │  outputs:
        │   { ciphertext | plaintext, group_state }
        ▼
┌────────────────────────────┐
│           CRDT             │
│ - apply_operation()        │
│ - generate_op()            │
│ - merge(remote_ops)        │
└────────────────────────────┘
        │  outputs:
        │   { crdt_ops, state_snapshot }
        ▼
┌────────────────────────────┐
│            DHT             │
│ - put(key, value)          │
│ - get(key)                 │
│ - gossip(crdt_ops)         │
└────────────────────────────┘
        │  outputs:
        │   distributed storage events
        ▼
┌────────────────────────────┐
│           Router           │
│ - send_packet(peer, data)  │
│ - recv_packet()            │
│ - NAT traversal            │
└────────────────────────────┘
        │
        ▼
   Network Peers
```

---

### **Short Description of Each**

#### **Identity**

Handles:

- Local device keys
- User login & provisioning
- Import/export identity bundle for device syncing
  Inputs: credentials
  Outputs: `{ user_id, device_id, identity_keypair }`

#### **MLS**

Handles:

- Secure group messaging
- Epoch keys, handshake messages
- Encrypts CRDT ops for transport
  Inputs: identity keys, plaintext operations
  Outputs: ciphertext packets OR decrypted plaintext

#### **CRDT**

Handles:

- Transforming user actions into CRDT operations
- Merging remote operations
- Conflict-free convergence
  Inputs: MLS-decrypted ops
  Outputs: new CRDT ops, updated document state

#### **DHT**

Handles:

- Distributed storage
- Finding peers
- Gossipping CRDT ops
  Inputs: CRDT ops
  Outputs: remote ops to merge, stored records

#### **Router**

Handles:

- Packet transmission over TCP/UDP/QUIC/Bluetooth/etc.
- Peer discovery
- Handles incoming network packets and sends them up stack
  Inputs: DHT get/put requests, MLS ciphertext
  Outputs: raw incoming bytes

---

Nice — below is a complete, developer-focused **pseudocode reference** for the entire `/core_identity` subsystem.
I wrote it to be copy-paste friendly and directly translatable to Rust files and tests. It includes:

- file-by-file pseudocode (APIs, data structures, functions)
- inputs / outputs for each function
- important error cases and invariants
- example flows (startup, create device, publish bundle)
- simple unit-test ideas at the end

I assume your codebase style is Rust-like and that CRDT / DHT / Router handles are available. If anything needs to be more concrete, I baked in names that match your earlier modules (`dht_adapter`, `local_store`, `VectorClock`, etc.).

# /core_identity — PSEUDOCODE SPEC

---

## Files (canonical list)

```
/core_identity
    mod.rs
    identity_manager.rs
    user_id.rs
    device_id.rs
    keypair.rs
    metadata.rs
    bundles.rs
    signatures.rs
    validation.rs
    dht_sync.rs
    /keystore
        mod.rs
        file_keystore.rs
        memory_keystore.rs
```

---

# 1) `mod.rs`

```rust
// re-export
pub mod identity_manager;
pub mod user_id;
pub mod device_id;
pub mod keypair;
pub mod metadata;
pub mod bundles;
pub mod signatures;
pub mod validation;
pub mod dht_sync;
pub mod keystore;
```

---

# 2) `user_id.rs`

### Purpose

Deterministic stable ID for a user (derived from long-term public key).

### Types & helpers (pseudocode)

```rust
type Bytes = Vec<u8>;

struct UserId(Bytes); // e.g. 32 bytes

impl UserId {
    fn from_public_key(pubkey: &Bytes) -> UserId {
        // use BLAKE3 or SHA-256, truncated to 32 bytes
        let hash = blake3(pubkey);
        UserId(hash[0..32])
    }

    fn to_string(&self) -> String { base58(self.0) }
}
```

### Input

- public key bytes

### Output

- deterministic 32-byte ID string

---

# 3) `device_id.rs`

### Purpose

Identify a device (unique per device). Derived from device pubkey or random.

```rust
struct DeviceId(Bytes); // e.g. 16 or 32 bytes

impl DeviceId {
    fn from_pubkey(pubkey: &Bytes) -> DeviceId {
        DeviceId(blake3(pubkey)[0..16])
    }
}
```

---

# 4) `keypair.rs`

### Purpose

Create/serialize/deserialize keys and provide cryptographic operations.

```rust
enum KeyType { Ed25519, X25519 }

struct Keypair {
    key_type: KeyType,
    public: Bytes,
    secret: Bytes, // encrypted at rest
}

impl Keypair {
    fn generate(key_type: KeyType) -> Keypair
    fn sign(&self, msg: &Bytes) -> Bytes
    fn verify(pubkey: &Bytes, msg: &Bytes, sig: &Bytes) -> bool
    fn derive_x25519_from_ed25519(ed: &Keypair) -> Keypair // for E2EE
    fn serialize(&self) -> Bytes // suitable for keystore
    fn deserialize(bytes: &Bytes) -> Keypair
}
```

### Important notes

- Use ed25519 for signatures. Use x25519 for DH if required by MLS.
- Keep secret memory protected, zero on drop if language supports.

---

# 5) `keystore/mod.rs`

### Purpose

Abstract keystore API so implementations can be swapped (file, memory, OS keyring).

```rust
trait Keystore {
    fn load_identity_keypair(&self) -> Result<Keypair, Error>;
    fn save_identity_keypair(&self, kp: &Keypair) -> Result<(), Error>;
    fn load_device_keypair(&self, device_id: &DeviceId) -> Result<Keypair, Error>;
    fn save_device_keypair(&self, device_id: &DeviceId, kp: &Keypair) -> Result<(), Error>;
    fn list_devices(&self) -> Result<Vec<DeviceId>, Error>;
    fn rotate_master_key(&self, password: &str) -> Result<(), Error>;
}
```

#### `file_keystore.rs` (simple encrypted file)

- Uses a master key derived from a password (argon2) or OS-provided keyring.
- Encrypt with XChaCha20-Poly1305/AES-GCM.
- Stores files:

  - `identity.json.enc`
  - `device-<deviceid>.json.enc`

- Provides atomic write (write-temp → rename).

#### `memory_keystore.rs` (for tests)

- Stores keys in-memory only.

---

# 6) `metadata.rs`

### Purpose

CRDT-backed user metadata (display name, avatar, devices). Exposed as CRDT so peers can see each other's public metadata.

```rust
struct DeviceMetadata {
    device_id: DeviceId,
    device_name: LWWRegister<String>,
    last_seen: LWWRegister<Timestamp>,
    key_package_ref: LWWRegister<Option<Hash>>, // reference to DHT-stored KeyPackage
    capabilities: LWWRegister<HashMap<String, String>>,
}

struct UserMetadata {
    user_id: UserId,
    display_name: LWWRegister<String>,
    avatar_hash: LWWRegister<Option<Hash>>,
    devices: ORMap<DeviceId, DeviceMetadata>, // nested CRDT
}
```

### API

```rust
impl UserMetadata {
    fn new(user_id: UserId) -> UserMetadata
    fn set_display_name(&mut self, name: String, ts: Timestamp, node: &str)
    fn add_device(&mut self, meta: DeviceMetadata, add_id: AddId, vc: VectorClock)
    fn remove_device(&mut self, device_id: &DeviceId, vc: VectorClock)
    fn merge(&mut self, other: &UserMetadata)
}
```

### Inputs/Outputs

- Inputs: local set operations; remote CRDT deltas
- Outputs: CRDT deltas to be persisted/published to DHT

---

# 7) `bundles.rs`

### Purpose

Create MLS KeyPackage and identity bundles to be published / advertised.

```rust
struct KeyPackage {
    cipher_suite: String,
    init_key: Bytes, // X25519 public key for the device
    leaf_secret_encryption: Option<Bytes>, // provider-specific
    credential: Bytes, // identity certificate (signed by identity key)
    // plus extensions (capabilities, version, etc.)
}

impl KeyPackage {
    fn new(device_kp: &Keypair, identity_kp: &Keypair, device_metadata: &DeviceMetadata) -> KeyPackage {
        // create credential: sign(KeyPackageBody, identity_kp)
    }
    fn to_bytes(&self) -> Bytes
    fn hash(&self) -> Hash
    fn verify(&self) -> bool // verify signature using identity pubkey
}
```

### Additional Bundles

- `DeviceBundle` – `{ keypackage, device_metadata, signature }`
- `IdentityBundle` – `{ user_id, public_key, devices[], signature }`

### Inputs/Outputs

- Inputs: device keys, identity keys, metadata
- Output: serializable bundles for DHT publish

---

# 8) `signatures.rs`

### Purpose

Helpers and canonical formats for signed statements.

```rust
enum SignedStatement {
    DeviceOwnership { device_id, user_id, sig },
    IdentityProof { user_id, pubkey, sig },
    KeyPackageBinding { keypackage_hash, device_id, sig },
}

fn sign_statement(kp: &Keypair, payload: &Bytes) -> Bytes
fn verify_statement(pub: &Bytes, payload: &Bytes, sig: &Bytes) -> bool
```

### Format

- Use COSE or simple `{payload, sig}` with explicit algorithm field.
- Always include `timestamp` and `nonce` in payload to prevent replay misuse.

---

# 9) `validation.rs`

### Purpose

Stateless validators for incoming identity artifacts.

```rust
enum ValidationError { InvalidSignature, TimestampOutOfRange, UnknownUser, BadFormat }

fn validate_keypackage(kp_bytes: &Bytes) -> Result<KeyPackage, ValidationError>
fn validate_device_bundle(bundle: &DeviceBundle, expected_user: &UserId) -> Result<(), ValidationError>
fn validate_identity_bundle(bundle: &IdentityBundle) -> Result<(), ValidationError>
```

### Checks performed

- Signature correctness
- Key formats and curve checks
- Timestamp sanity (within allowed skew)
- Credential chain checks (if present)
- Duplicates or replay detection (optionally via store of seen signatures)

---

# 10) `dht_sync.rs`

### Purpose

Publish / fetch identity-related bundles and metadata to/from DHT. Also, optionally, cache lookups.

```rust
struct DhtSync {
    dht_adapter: DhtAdapterHandle,
    local_store: LocalStoreHandle,
}

impl DhtSync {
    fn publish_user_metadata(&self, metadata: &UserMetadata) -> Result<Hash, Error> {
        // encode metadata deltas -> bytes
        // sign a compact record (identity proof signed)
        // put to DHT key: "identity/user/<user_id_hash>"
    }

    fn fetch_user_metadata(&self, user_id: &UserId) -> Result<UserMetadata, Error> {
        // DHT.get -> decode -> verify signature -> return
    }

    fn publish_keypackage(&self, kp: &KeyPackage) -> Result<Hash, Error> {
        // DHT.put("keypackage/<kp_hash>", kp_bytes)
    }

    fn fetch_keypackage(&self, kp_hash: &Hash) -> Result<KeyPackage, Error>
}
```

### Notes

- DHT keys should be namespaced and hashed to avoid enumeration (e.g., HMAC or keyed hash).
- All DHT records must be authenticated (signed) to prevent poisoning.

---

# 11) `identity_manager.rs`

### Purpose

High-level orchestrator that exposes friendly APIs to the rest of your runtime.

```rust
struct IdentityManager {
    keystore: Box<dyn Keystore>,
    dht_sync: DhtSync,
    local_store: LocalStoreHandle, // for CRDT metadata storage
    current_user: Option<UserId>,
    current_device: Option<DeviceId>,
    identity_kp: Option<Keypair>,
    device_kp: Option<Keypair>,
}

impl IdentityManager {
    // --- initialization ---
    fn load_or_create_identity(&mut self, password: Option<&str>) -> Result<(UserId, DeviceId), Error> {
        // try keystore.load_identity_keypair()
        // if not found -> generate new identity keypair -> save via keystore
        // generate device keypair -> save
        // compute ids
        // load metadata from local_store or create fresh UserMetadata CRDT
        // publish minimal identity bundle to DHT
        // return ids
    }

    fn create_new_device(&mut self, device_name: &str) -> Result<DeviceId, Error> {
        let new_kp = Keypair::generate(Ed25519);
        let dev_id = DeviceId::from_pubkey(&new_kp.public);
        keystore.save_device_keypair(&dev_id, &new_kp)?;
        // create DeviceMetadata, put into local UserMetadata CRDT (apply_local -> generate delta)
        // create KeyPackage for MLS (bundles::KeyPackage::new(...))
        // publish key package via dht_sync.publish_keypackage(...)
        return Ok(dev_id)
    }

    fn sign(&self, payload: &Bytes) -> Result<Bytes, Error> {
        // sign with identity_kp
    }

    fn rotate_identity_keypair(&mut self, new_password: &str) -> Result<(), Error> {
        // re-encrypt keystore
    }

    fn get_user_metadata(&self) -> UserMetadata { /* load from local_store */ }

    fn set_display_name(&self, name: &str) -> Result<(), Error> {
        // update local CRDT: user_metadata.display_name.set(...)
        // persist to local_store
        // publish minimal change via dht_sync.publish_user_metadata
    }

    fn publish_all_keypackages(&self) -> Result<(), Error> {
        // enumerate devices, for each device: create keypackage, publish
    }
}
```

### Inputs

- user actions (create device, set name)
- keystore, DHT, local store handles

### Outputs

- signed bundles, DHT publishes, CRDT deltas

---

# Example flows (pseudocode)

### Startup (first-run)

```rust
// main.rs
let keystore = FileKeystore::open(path, maybe_password)?;
let local_store = LocalStore::open(path)?;
let dht = DhtAdapter::new(...);

let dht_sync = DhtSync::new(dht, local_store.clone());
let mut idm = IdentityManager::new(keystore, dht_sync, local_store);

let (user_id, device_id) = idm.load_or_create_identity(password)?;
print!("Hello {}", user_id.to_string());
```

### Add new device flow

```rust
let new_device_id = idm.create_new_device("My Laptop")?;
let keypackage = bundles::KeyPackage::new(&kp, &identity_kp, &dev_meta);
dht_sync.publish_keypackage(&keypackage)?;
local_store.apply_crdt_op(user_metadata_crdt_delta);
```

### Publish metadata flow

```rust
idm.set_display_name("alice") -> updates local CRDT
local_store persists delta
dht_sync.publish_user_metadata(&updated_metadata)
```

### Remote lookup flow

```rust
// someone wants to open DM with user U
let meta = dht_sync.fetch_user_metadata(&U)?;
validate(meta) -> validation.validate_identity_bundle(...)
if ok:
    use meta.devices to fetch keypackages -> dht_sync.fetch_keypackage(hash)
    verify keypackage.signature
```

---

# Error cases & security notes

- Keystore compromise: require password or OS keyring; encourage hardware-backed keys (TPM/Keychain).
- DHT poisoning: always validate signatures before consuming records.
- Replay protection: include timestamps and keep small window or signature nonce set.
- Device revocation: removing a device from `UserMetadata.devices` CRDT and publishing removal + revocation CRDT entry. MLS will need a remove proposal to remove leaf.
- Enumeration: store DHT keys hashed (HMAC with server-wide salt or per-user salt) if you want limited discoverability.

---

# Unit test ideas (pseudocode)

Create `tests/core_identity_tests.rs`:

1. `test_user_id_derivation_is_deterministic`

   - generate keypair -> user_id
   - derive again -> equal

2. `test_create_device_and_keystore_roundtrip`

   - init memory keystore
   - create device -> save
   - load device -> verify public key equals

3. `test_keypackage_sign_and_verify`

   - generate identity & device KP
   - KeyPackage::new(...) -> sign -> verify()

4. `test_publish_and_fetch_metadata_via_dht_mock`

   - mock DHT adapter that stores bytes
   - call dht_sync.publish_user_metadata
   - fetch -> validate signature -> compare fields

5. `test_identity_manager_set_display_name_generates_crdt_delta`

   - memory local_store
   - call set_display_name -> ensure local_store got delta and user metadata LWW value set

6. `test_device_add_remove_crdt_roundtrip`

   - add device metadata locally
   - remove device metadata -> merge -> check remove applied

7. `test_offline_keypackage_use_case`

   - simulate creating keypackage, publish, delete local copy, fetch from DHT and validate

---

# Integration tests (higher level)

1. `identity_to_mls_join_flow`

   - create identity & device + keypackage
   - create a group state skeleton and add device keypackage as Add proposal (simulate)
   - apply commit locally (simulate tree math) -> ensure group_state contains device leaf

2. `device_revocation_flow`

   - add device -> publish
   - remove device -> publish CRDT removal + MLS remove proposal -> ensure fetch reveals device no longer present

---

# Implementation tips & practical decisions

- **Serialization formats**: Use compact binary (CBOR, MessagePack) for DHT records; use JSON only for local dev/debug.
- **Hashing**: Use BLAKE3 for speed + keyed hashing.
- **Signature wrappers**: Use COSE or a tiny envelope `{alg, payload, signature}`.
- **Key rotation**: Offer rotate_identity that creates new keypair and issues signed proof linking old -> new (useful for migration).
- **Device discovery**: Use DHT to advertise only KeyPackage hashes, not full keypackages (the DHT stores the payload anyway).
- **Expiration**: consider TTL on DHT entries but rely on CRDT as source-of-truth for re-introduction.
- **CRDT storage**: put `UserMetadata` under document key `core_store::user_meta::<user_id>`.

---

# Final checklist before MLS

Before you start core_mls, ensure:

- identity_manager correctly returns stable `UserId` and `DeviceId`
- keypairs can be produced and loaded from keystore reliably
- DHT can publish/fetch signed keypackages and metadata
- local store holds CRDT UserMetadata and can produce deltas
- validation verifies signatures and prevents DHT poisoning

---
