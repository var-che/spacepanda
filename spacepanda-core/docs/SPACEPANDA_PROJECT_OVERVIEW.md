Below is a focused, practical, developer-facing design and implementation plan you can use as the single-source-of-truth for the SpacePanda MVP layer that sits on top of your existing core_identity, core_dht, core_crdt, and core_mls. It includes APIs you can test immediately, data models, module boundaries, message flows, test harness ideas, security & threat notes, and suggestions for hybrid deployment later.

I assume your existing subsystems provide at least these primitives:
• core_identity — create identities, sign/verify, key material access
• core_dht — publish / lookup arbitrary blobs or small records (peer discovery, channel directory)
• core_crdt — conflict-free replicated data type store for channel metadata (members, permissions)
• core_mls — group membership, commit/proposal flow, encrypt/decrypt, welcome messages, persistent state

If any of those are missing features I mention, you can treat them as TODO items in the doc. OK — let’s go.

⸻

SpacePanda — MVP Layer Design (overview)

Goal: Build a decentralized, P2P, E2EE text chat application (mobile + desktop) with features akin to Discord (channels, servers, channel permissions, invites, membership management), using MLS for group encryption, DHT for discovery, CRDT for shared metadata, and local identity for authentication.

Primary constraints / principles:
• E2EE: messages encrypted with MLS group keys.
• P2P-first: peers communicate directly when possible. Use relays only where direct connectivity fails (NAT traversal / TURN / optionally relays).
• Decentralized discovery: small “directory” objects in DHT represent public channel metadata. Private channels use invitation-only welcome blobs.
• CRDT for eventually-consistent metadata: membership lists, permission sets, channel topic, pinned messages metadata.
• Testability: local test harness with HTTP/CLI endpoints that exercise flows (create identity, create channel, invite, join, send message).

Target MVP features: 1. Create Identity (local). 2. Create Server/Channel (public/private). 3. Invite users to channel (Welcome/HPKE/MLS flow). 4. Add / remove / update member permissions. 5. Send encrypted messages to channel (MLS sealed messages). 6. Offline / reconnect sync; basic history. 7. Test harness endpoints / CLI for all flows.

⸻

High-level module responsibility & interactions

                        +--------------------+
                        |  UI (mobile/desktop)|
                        +----------+---------+
                                   |
                            Application API (HTTP/IPC)
                                   |

+-------------------+ +----------v------------+ +------------------+
| core_identity | | core_mvp_layer | | core_networking |
| - keys, sign | | (this document) | | (peer connect, |
| - DID-ish support | | - channel manager | | NAT traversal) |
+-------------------+ | - invites/welcome | +------------------+
| - permission manager |
+-------------------+ | - message routing | +------------------+
| core_mls | | - persistence / APIs | | core_dht |
| - group mgmt | | - test harness | +-- discovery & |
| - encryption | +----------+------------+ storage for meta
+-------------------+ |
v
core_crdt (channel metadata)

Roles:
• core_identity: create/restore user identity, produce signing keys used in MLS member authentication or for OT/CRDT operations, provide public identity descriptors (username, avatar hash, optional PKI cert).
• core_mls: ultimate source of truth for group encryption keys. Each channel has an MLS group; MLS handles adding/removing, welcomes, commits, ratcheting encryption secrets.
• core_crdt: stores mutable, public (or encrypted) channel metadata that needs eventual consistency — permissions sets, role bindings, topics, pinned message indices, message index metadata (not message bodies).
• core_dht: store channel directory entries, optional public channel metadata and bootstrap info (e.g., current set of relay nodes, public channel descriptors, hashed invitation tokens).
• core_networking (not listed originally but required): peer connectivity, NAT/traversal, transport layer for SessionManager / Router. Responsible for delivering MLS envelopes and message envelopes (can wrap with onion / relay when necessary).
• core_mvp_layer (new): orchestrates flows between MLS, CRDT, DHT, Identity, networking and exposes a simple test API for UI/CLI.

⸻

Data model (structs / persisted blobs)

These are compact models; choose names/types to match your Rust project.

Identity

struct Identity {
id: String, // local stable id (DID or user-chosen UUID)
display_name: String,
public_encryption_key: Vec<u8>, // X25519 pubkey for HPKE if used
public_sign_key: Vec<u8>, // Ed25519 or similar
avatar_hash: Option<Vec<u8>>,
// optionally: verifiable credentials / certificate
}

Channel descriptor (DHT / public directory)

struct ChannelDescriptor {
channel_id: ChannelId, // e.g. sha256(name || owner_pubkey) or UUID
owner: IdentityId,
name: String,
is_public: bool,
visibility: Option<String>, // optional short description
bootstrap_relay_nodes: Vec<PeerAddr>, // peers/relays to reach participants
mls_group_info_hash: Vec<u8>, // hash of GroupInfo (used to fetch GroupInfo from DHT or via on-chain)
created_at: i64,
}

Invite token (out-of-band)
• Implementation choices:
• Welcome blob (standard MLS): an encrypted Welcome message containing secrets for a new member. That is the recommended approach for MLS: the inviter produces Welcome, and sends the Welcome envelope to invitee.
• Additionally: an invitation descriptor stored in DHT for public invites (hash points to encrypted welcome available for that identity or for any holder of secret), or as a one-time token you share.

Channel CRDT state (persisted via core_crdt)

Used to track metadata that must converge:

struct ChannelStateCRDT {
members: ORSet<MemberEntry>, // holds member list (leaf index, identity id, roles)
roles: MapCRDT<RoleId, RoleDefinition>, // permission definitions
member_roles: MapCRDT<MemberId, Set<RoleId>>,
pinned_msgs: MapCRDT<MessageId, PinInfo>,
topic: LWWString, // last-writer-wins for topic
}

Message Envelope (routed via networking)

Two layers: 1. Transport envelope — contains routing info, optional onion wrapping. 2. MLS envelope — contains MLS sealed message (ciphertext + header). 3. Application payload (after open): contains message body and optionally CRDT updates.

Simplified:

struct RoutedEnvelope {
sender_peer: PeerId,
target_peer: Option<PeerId>, // None => broadcast to group members via route
payload: Vec<u8>, // contains MlsEnvelope or other control frames
metadata: ...
}

Inside MLS:

// after decrypt
struct ChatMessage {
sequence: u64,
sender_leaf: u32,
timestamp: i64,
body: Vec<u8>, // application-level plaintext (JSON or binary)
message_id: MessageId, // uuid/sha256
attributes: Option<...> // e.g. ephemeral reply-to
}

⸻

Channel lifecycle & flow (sequence of operations)

I’ll describe flows as step-by-step sequences with interactions between modules.

1. Channel creation (private)
   • Actor: Creator (Alice)
   • Steps:

   1. Alice calls core_identity to create or use existing Identity.
   2. core_mvp_layer.create_channel(name, is_public=false, initial_members=[alice]):
      • Creates a ChannelId (UUID or hash).
      • Invokes core_mls.create_group to create an MLS group for the channel. The MLS GroupInfo and initial state are created.
      • Stores ChannelDescriptor in DHT if the channel is public; for private channels, DHT entry may be minimal or absent.
      • Create initial ChannelStateCRDT with members set (Alice).
      • Persist channel state locally.
   3. Return ChannelId and initial GroupInfo / Welcome (Alice already in group, no welcome needed).

2. Invite flow (out-of-band)
   • Option A: Direct (Alice → Bob) via external channel (email / messaging app).
   • Alice calls core_mls.create_welcome(target_identity) or MlsGroup::create_welcome that yields a Welcome object (HPKE-encrypted secrets targeted at Bob’s public key).
   • Alice serializes the Welcome and sends it to Bob (out-of-band) or posts to DHT along with ACL that identifies Bob.
   • Option B: Invite via DHT (public invite)
   • Alice stores an encrypted welcome blob in DHT under a key like invite:<channel_id>:<invite_token_hash>.
   • Invitee fetches the blob and attempts to decrypt with their secret key.
   • On receive:
   • Bob calls core_mls.join_from_welcome(welcome_blob), which validates and creates local MLS group state.
   • Bob now becomes a member; the MLS protocol ensures others see his Add proposal (Alice must apply the Add commit if she initiated offline; or if the welcome included the commit, the group state is consistent).
   • CRDT membership state updated to include Bob.
   • Networking: peers exchange updated GroupInfo or commit envelope.

3. Add/Remove/Update members (in-band)
   • Members propose add/remove/update proposals via core_mls.propose_add/propose_remove/propose_update.
   • Any member can commit proposals; after commit is generated:
   • Updated GroupInfo/Commit is broadcast to group members using core_networking.
   • Each recipient validates commit (confirmation tags, etc.) and applies it. core_crdt should be updated accordingly (member list role mapping).
   • For remove: removed member’s state cannot decrypt future MLS messages after the commit.

4. Sending messages
   • Sender creates an application payload ChatMessage.
   • Call core_mls.seal_message(chat_message) -> returns MlsCiphertext (encrypted for group).
   • Wrap in transport envelope and send to group via core_networking:
   • Strategy: broadcast push to known peers, or route via DHT/relays for offline members.
   • Recipient receives envelope, delivers to core_mls.open_message() -> resulting ChatMessage.
   • After decrypting, update local message store and CRDT metadata (for pinned flags, reactions).

5. Permissions model
   • Permissions should be enforced at the application layer (CRDT) and optionally at the transport layer:
   • Define roles and capabilities:
   • Admin: invite/kick/create-channel/modify-roles
   • Moderator: moderate messages, remove messages, pin
   • Member: send messages, edit own messages
   • Guest: read-only
   • roles defined in ChannelStateCRDT.roles: a role -> set of capabilities.
   • member_roles map assigns roles to members.
   • Any operation that changes membership or roles is stored as a CRDT operation and as an MLS commit (for ops that need confidentiality/integrity). For example:
   • Changing role is a CRDT-proposed operation signed by the actor and then broadcast through MLS as an encrypted proposal (so confidentiality of role changes in private channels).
   • Enforcement:
   • At the sender: UI + client library checks whether the local identity has capability to perform the action.
   • At each recipient: upon receiving an action (e.g., commit that removed a member), the recipient must validate operation according to CRDT and MLS validation rules. If an unprivileged party attempts a privileged operation, the commit/proposal is rejected by validators.

⸻

API (for testing and UI) — functions and minimal REST endpoints

Design both an in-process API (Rust function calls) and a test REST API (HTTP) for rapid testing. The REST API can be implemented with an embedded HTTP server in test harness.

Core RPC / HTTP endpoints (examples)

/identity
• POST /identity/create -> create local identity; returns IdentityId, public keys.
• GET /identity/me -> return current identity descriptor.

/channels
• POST /channels/create body: { name, is_public } -> returns channel_id
• GET /channels/:id -> returns ChannelDescriptor
• POST /channels/:id/invite body: { target_pubkey, via: "dht" | "oob" } -> returns Welcome blob or DHT key
• POST /channels/:id/join body: { welcome_blob } -> join using welcome
• POST /channels/:id/commit body: { commit_envelope } -> apply commit (used for tests)
• GET /channels/:id/members -> CRDT-based member list

/messages
• POST /channels/:id/send body: { plaintext } -> sends encrypted message (returns envelope id)
• GET /channels/:id/messages -> returns local history (for test harness)

/roles
• POST /channels/:id/roles -> create/update roles
• POST /channels/:id/members/:member_id/roles -> assign roles

/debug
• POST /shutdown, POST /reset etc.

These are minimal and sufficient to script flows from the CLI or integration tests.

⸻

Storage & Persistence
• Each user persists:
• Identities (private keys) encrypted with OS keystore or passphrase.
• MLS group state per channel (encrypted on disk with passphrase).
• CRDT local store snapshot.
• Local message store (ciphertexts only optionally with indexing).
• Backup: allow exporting encrypted stateblob to a file (for tests).

⸻

Test harness & developer tools

To get confidence before UI integration, provide: 1. Single-process test harness: an HTTP server that exposes the endpoints above. Run several instances on different ports to simulate clients. 2. Integration tests: orchestrate several harness instances:
• Create identity A, create channel, invite B, B joins, send messages, assert decrypts.
• Run tests for add/remove/update, permissions enforcement, stale-state rejection. 3. Fuzz tests / property tests: random sequences of join/add/remove/update and assert convergence (CRDT) and MLS consistency. 4. Benchmarks: measure latency of invite flow, commit apply times for groups of size N (10, 50, 100). 5. Interoperability test: (if you migrate to OpenMLS) same harness but instantiate either native MLS or OpenMLS provider.

⸻

Edge cases & important invariants (must be covered by tests) 1. Out-of-order commits: reject commits that skip epochs. 2. Replayed messages: detect and reject replay attempts. 3. Removed member: cannot decrypt messages produced after removal commit. 4. Tampered Welcome/Commit: reject corrupted HPKE ciphertext or broken confirmation tags. 5. Missing proposals: validator must reject commits referencing proposals not present. 6. Conflicting role updates: CRDT guarantees converge — ensure the role assignment CRDT is commutative/idempotent. 7. Network partition: divergent proposals from different partitions — test merge behavior and conflict resolution. 8. Capacity / resource limits: protect against huge invites or spam (rate-limits). 9. Key rotation: member key updates propagate and old keys cannot decrypt future messages. 10. Persistence rollback: loaded stale state must be validated against cryptographic checks.

⸻

How MLS, CRDT, DHT coordinate (practical mapping)
• MLS holds confidential group state and encryption secrets. Use MLS for:
• Sealing application messages,
• Propagating group-level membership changes (commits),
• Providing Welcome blobs for invites.
• CRDT holds non-secret but mutable metadata that must converge across replicas, such as:
• Roles, role mappings, pinned message ids, topic, unread counters, reaction counts.
• Depending on privacy, you can keep some CRDT fields encrypted (CRDT over ciphertext) — or store the CRDT state inside MLS-protected payloads for private channels.
• DHT holds public discovery:
• Public channel descriptor (ChannelDescriptor).
• Optionally: encrypted welcome(s) (if you support DHT-based invites).
• Index: channel id → bootstrap peers / relay list.

Design pattern:
• Use MLS to exchange critical messages (commits, encrypted CRDT deltas).
• Option A: Broadcast CRDT deltas inside MLS-encrypted messages: simplifies confidentiality (only group members can decode deltas) and still allows CRDT convergence.
• Option B: Keep CRDT deltas as plaintext and store in CRDT store — useful for public channels.

Tradeoffs:
• Putting CRDT deltas inside MLS messages: simpler confidentiality but loses ability to merge deltas from offline members unless they re-synchronize via welcome or pull history.
• Keeping CRDT deltas outside MLS: easier for offline replication but may expose metadata.

⸻

Permission design suggestions (practical model)
• Roles: a role has a set of capabilities. Make capabilities small and composable strings: invite, kick, send, pin, manage_roles.
• Role assignment: member_roles is a CRDT Map<member, ORSet>.
• Constraints:
• Only Admin can assign Admin role.
• Role-change proposals should carry the signer’s identity and be validated by recipients.
• Enforcement:
• Local enforcement: UI + client lib checks.
• Remote enforcement: recipients validate that the commit/proposal that applied the role change was produced by a member that had the right capability at the time of signing. If signature proves permission violation, reject.

⸻

Transport / networking notes
• Use a session manager that guarantees:
• Delivery of envelopes to peers (retries).
• Option to route via relays if direct connectivity fails.
• Persistent peer contacts & bootstrapping via DHT.
• Envelope types:
• Control (MLS commit, Welcome)
• Data (MLS application ciphertext)
• Use sequence numbers and dedup caches to avoid replays.

⸻

Logging, metrics & observability
• Log (structured) security events: welcome-decrypt-fail, commit-apply-fail, replay-detected.
• Track metrics: commits/sec, invites/sec, latency of join flow.

⸻

Security & threat model summary
• Threats:
• Malicious network actor injecting garbage or replaying messages — handle via signature/nonce and sequence checking.
• Malicious group member trying to escalate privileges — verify role-change proposals are signed by authorized roles.
• Denial-of-Service by flooding invites or unique request IDs — rate-limit DHT and invitations; bound internal maps.
• Compromised peer: the peer’s MLS secrets are lost to attacker — rotation & forward secrecy (MLS) mitigates future messages.
• Defenses:
• MLS for confidentiality and forward secrecy.
• Signatures for authenticity of proposals/commits.
• CRDT + validation for metadata consistency.
• Capacity limits for state-tracking maps.

⸻

File layout suggestions for core_mvp_layer (concrete)

core_mvp/
├─ src/
│ ├─ lib.rs // Public API: create_channel, invite, join, send_message...
│ ├─ channel_manager.rs // Channel lifecycle orchestrator (creates MLS group, CRDT state)
│ ├─ invite.rs // Invite creation/processing (Welcome producer / consumer)
│ ├─ permissions.rs // Roles, capability checks, CRDT role integration
│ ├─ message_router.rs // Encapsulate sealing/transport, retry, delivery
│ ├─ api_server.rs // HTTP/IPC test API harness
│ ├─ persistence.rs // local disk persistence & snapshot helpers
│ ├─ bootstrap.rs // dht registration / discovery helpers
│ ├─ tests/ // integration & e2e tests using HTTP harness
│ └─ types.rs // ChannelId, Envelope, ChatMessage, etc.

⸻

Test plan & immediate checklist (what to implement first)

Minimum viable testable skeleton: 1. Implement channel_manager.create_channel() — uses core_mls to create group and persists it. 2. Implement invite.create_welcome() — produce Welcome for invitee and serialize. 3. Implement invite.join_from_welcome() — consumer can join and create local group. 4. Implement message_router.send() — uses MLS to seal and send envelope via core_networking (a mock transport for unit tests). 5. Implement message_router.receive() — calls core_mls.open_message() and returns ChatMessage. 6. Test flow: create channel → produce welcome → join → send message → decrypt message. This is the most important end-to-end flow.

Then add:
• CRDT integration for roles and membership metadata.
• REST test harness exposing create/invite/join/send endpoints.
• Tests for out-of-order commits, tampered welcome, removed member decryption failure.
• Performance benchmarks for groups of size 10/50/100.

⸻

Migration note: using OpenMLS vs custom core_mls
• If you choose to switch to OpenMLS later:
• Keep your core_mvp_layer APIs stable and isolate MLS usage behind a GroupProvider trait in core_mvp. Implement two adapters: LocalCoreMlsAdapter (your current core_mls) and OpenMlsAdapter.
• This allows swapping the backend with minimal changes to higher-level code and test suite.

Trait example (Rust-style pseudocode):

pub trait GroupProvider {
fn create_group(&self, channel_id: ChannelId, owner: Identity, config: GroupConfig) -> GroupHandle;
fn create_welcome(&self, handle: &GroupHandle, recipient_pubkey: &[u8]) -> Result<Welcome>;
fn join_from_welcome(&self, welcome: &Welcome, my_secret: &[u8]) -> Result<GroupHandle>;
fn seal_message(&self, handle: &GroupHandle, plaintext: &[u8]) -> Result<Vec<u8>>;
fn open_message(&self, handle: &GroupHandle, ciphertext: &[u8]) -> Result<Vec<u8>>;
fn propose_add(&self, ...);
fn commit(&self, ...);
// ...
}

⸻

Final checklist before implementation
• Define core_mvp_layer Rust crate with the layout above.
• Define GroupProvider trait and implement adapter for current core_mls.
• Implement channel_manager flows with unit tests.
• Implement HTTP CLI test harness that scripts flows (used for demos).
• Add CRDT-based permissions module and write property tests (convergence).
• Add rate-limits and resource bounds for invites and DHT entries.
• Add observability: structured logging for security-relevant events.
• Create an integration test suite that exercises full lifecycle end-to-end.
• (Optional) Design support for relays/TURN if direct P2P fails.

⸻

Example quick pseudocode for create-invite-join-send test (for the test harness)

// 1. Alice creates identity and channel
alice = harness.create_identity("alice")
channel_id = harness.create_channel(alice, "spacepanda-room", private=true)

// 2. Alice creates welcome for Bob (using Bob's public key)
bob_pk = test_keypair("bob").public
welcome_blob = harness.create_invite(channel_id, alice, bob_pk)

// 3. Bob joins using welcome_blob
bob = harness.create_identity("bob")
bob_join_result = harness.join_channel(bob, welcome_blob)
assert bob_join_result.ok()

// 4. Alice sends an encrypted message
payload = b"hello bob"
envelope = harness.send_message(alice, channel_id, payload)

// 5. Bob receives and decrypts
received = harness.receive_message(bob, envelope)
assert received == payload

⸻

Closing notes & priorities
• Priority 1: implement the minimal end-to-end path (create channel, create welcome, join, send message). This validates MLS + transport.
• Priority 2: add CRDT roles & permissions and test their enforcement.
• Priority 3: prepare adapters (GroupProvider trait) for swapping MLS implementation later (OpenMLS).
• Security: implement strict validations on commits and welcome messages (confirmation tags + HPKE checks) before trusting any state.
• Testing: run the test harness in-process with multiple instances to reduce external network flakiness during development.

⸻

### DOC 2:

Below is a prioritized, developer-ready backlog (epics → stories → tasks) tailored to SpacePanda’s current state and your goals (P2P, E2EE, MLS, CRDT, DHT). Each item has: a short description, why it matters, acceptance criteria, suggested files to change/implement, tests to add, and a rough complexity tag (Small / Medium / Large). Use this as your sprint board — pick top items for the next sprint.

Priority 1 — Blockers / Security / E2E critical path (do these first)

These unblock everything and must pass before MLS-heavy features.

1. End-to-End Join & Messaging Golden Path
   • What: Implement and harden “create-channel → invite (Welcome) → join → send message → decrypt” flow using current core_mls, core_networking (or mocked transport).
   • Why: This is the MVP flow that proves MLS integration and transport.
   • Acceptance criteria:
   • Unit test or harness script that spins up two instances, performs flow, and asserts messages decrypt.
   • No silent failures; errors are returned and logged.
   • Files: core_mvp/channel_manager.rs, core_mvp/invite.rs, tests in core_mvp/tests/e2e_join_message.rs.
   • Tests: end-to-end harness test, edge-case tests (invalid welcome, wrong key).
   • Complexity: Medium

2. Strict Commit & Welcome Validation
   • What: Ensure commit confirmation tags, sender identity, epoch checks, and Welcome HPKE decrypt errors are handled and cause rejection.
   • Why: Prevents state corruption and security bypass.
   • Acceptance criteria:
   • Tests for out-of-order commit rejection, tampered commit confirmation, corrupted Welcome (HPKE decrypt fails).
   • Logs include clear security messages.
   • Files: core_mls/commit_validator.rs, core_mls/welcome.rs, core_mvp/tests/security.
   • Tests: Add or strengthen tests you already have (you had many—make them fail for bad input).
   • Complexity: Small → Medium

3. Persisted Group State Encryption & Safe Restore
   • What: Ensure group state saved to disk is encrypted, authenticated, and fails cleanly on wrong passphrase or tampering.
   • Why: Prevents leakage of secrets and ensures recoverability.
   • Acceptance criteria:
   • Save/load roundtrip tests (encrypted).
   • Wrong passphrase test fails with specific error; tampering causes validation to fail.
   • Files: core_mls/persistence.rs, core_mvp/persistence.rs, tests in core_mls/tests.
   • Complexity: Small

Priority 2 — Core MVP features & developer ergonomics

Build features that make the system testable and robust.

4. GroupProvider Trait & Adapter (abstraction over MLS impl)
   • What: Define GroupProvider trait that hides MLS backend; implement adapter for current core_mls.
   • Why: Enables later swap to OpenMLS with minimal changes.
   • Acceptance criteria:
   • Trait defined with methods: create_group, create_welcome, join_from_welcome, seal, open, propose_add/remove/update, commit, epoch, member_count.
   • Adapter implemented and used by core_mvp.
   • Unit tests using trait mock.
   • Files: core_mvp/group_provider.rs, core_mvp/adapters/core_mls_adapter.rs.
   • Complexity: Medium

5. Lightweight HTTP Test Harness
   • What: Small embedded HTTP server exposing create/join/send endpoints to script flows (helps manual testing).
   • Why: Makes multi-process integration tests trivial and demoable.
   • Acceptance criteria:
   • Endpoints described previously (/identity, /channels, /messages).
   • A simple CLI script (or curl examples) that runs the golden path.
   • Files: core_mvp/api_server.rs, core_mvp/examples/cli.
   • Complexity: Small

6. CRDT → MLS Integration Strategy (prototype)
   • What: Design and implement how CRDT deltas are propagated (inside MLS messages vs plaintext CRDT sync). Implement one approach as prototype.
   • Why: Decides how metadata is kept private/public and affects offline behavior.
   • Acceptance criteria:
   • One working mode implemented (recommended: CRDT deltas inside MLS-encrypted payloads for private channels).
   • Tests for convergence after offline edits + rejoin.
   • Files: core_crdt/sync.rs, core_mvp/message_router.rs.
   • Complexity: Medium → Large

Priority 3 — Reliability, Scale & security hardening

Make the system robust and production-aware.

7. Rate-limiting & Resource Bounds
   • What: Add per-peer rate limits for invites, commit acceptance, and request caches; bound seen-messages maps to avoid memory exhaustion.
   • Why: Prevent DoS attacks and runaway memory usage.
   • Acceptance criteria:
   • Limits configurable; tests simulate flood and show graceful rejection.
   • Files: core_mls/rate_limiter.rs, core_networking/session_manager.rs.
   • Complexity: Medium

8. Replay Protection & Sequence Checks
   • What: Ensure per-member sequence numbers and anti-replay caches are bounded and pruneable.
   • Why: Security invariant; prevents replays and bounds memory.
   • Acceptance criteria:
   • Replay detection tests pass.
   • TTL-configurable pruning test passes.
   • Files: core_mls/replay_protection.rs.
   • Complexity: Small

9. Transport Robustness: Relay / Retry / Offline Sync
   • What: Implement retry/backoff for delivering envelopes; optional relay path for NATed peers; offline sync via DHT or relay store.
   • Why: Real-world networks need resilience.
   • Acceptance criteria:
   • Tests: sender sends while recipient offline; recipient syncs and receives missed commits/messages when online.
   • Files: core_networking/router.rs, core_networking/relay.rs.
   • Complexity: Large

Priority 4 — UX features & developer polish

These make the product usable and testable for demos.

10. Roles & Permission CRDTs + Enforcement
    • What: Implement roles and member_roles as CRDTs and enforce checks on client ops and on commit validation.
    • Why: Required for admin/moderation functionality.
    • Acceptance criteria:
    • Create role, assign role, attempt unauthorized action -> rejected.
    • Tests proving enforcement across nodes.
    • Files: core_mvp/permissions.rs, core_crdt/roles.rs.
    • Complexity: Medium

11. Message History & Indexing (encrypted)
    • What: Local encrypted message store with indexed metadata (message id, sequence, sender, timestamp), enabling search and sync.
    • Why: Usability (history, scrollback).
    • Acceptance criteria:
    • Save/retrieve tests for message history; encryption ensures ciphertexts on disk.
    • Files: core_mvp/storage/messages.rs.
    • Complexity: Medium

12. Test Coverage / CI Integration
    • What: Add CI pipeline (GitHub Actions) to run unit + integration tests and run benchmarks; enforce cargo fmt and clippy.
    • Why: Maintain code quality and catch regressions early.
    • Acceptance criteria:
    • ci.yml that runs cargo test --workspace, cargo clippy, and cargo fmt -- --check.
    • Files: .github/workflows/ci.yml
    • Complexity: Small

Priority 5 — Optional / Future / Migration

Things to do as you approach release or swap MLS backend.

13. Adapter for OpenMLS & Provider Bridge
    • What: Implement OpenMlsAdapter that implements GroupProvider trait (requires StorageProvider and CryptoProvider bridges).
    • Why: Strongly recommended for production security and to avoid rolling your own MLS primitives.
    • Acceptance criteria:
    • Adapter compiles and passes the same test suite.
    • Files: core_mvp/adapters/openmls_adapter.rs, core_mvp/adapters/storage_bridge.rs, core_mvp/adapters/crypto_bridge.rs
    • Complexity: Large

14. Relay Network / Onion Routing (privacy)
    • What: Implement onion routing / mix for hiding metadata (optional).
    • Why: Advanced privacy; not required for MVP but desirable.
    • Acceptance criteria:
    • Experimental prototype and tests that show intermediate relays cannot read final destination.
    • Complexity: Large / Research

15. Metrics, Telemetry & Audit Trails
    • What: Collect metrics for joins, commits, invites, failures for diagnostics; logging of security events.
    • Why: Observability for production.
    • Files: core_mvp/telemetry.rs and integrate prometheus/tracing.
    • Complexity: Medium

Top 3 Immediate Tasks (for the next 1–2 sprints) 1. Golden path E2E (Item 1) — get create-invite-join-send passing via harness. (Medium) 2. Commit & Welcome validation tests (Item 2) — ensure security tests pass and are non-flaky. (Small) 3. GroupProvider trait + Adapter (Item 4) — create the abstraction so later OpenMLS adapter plugs in. (Medium)

Test/QA Matrix pointers

For every feature add:
• Unit tests for small pieces (key derivation, HPKE decrypt).
• Integration tests for flows (multi-instance harness).
• Property tests where applicable (CRDT convergence, monotonic epoch).
• Stress tests for scale (mass members/messages).
• Fuzz/negative tests for corrupted messages/commit tampering.

Suggested branch/PR workflow
• main stable: always green CI.
• Feature branches per item: feature/e2e-golden-path, feature/group-provider.
• Pull request checklist: tests added, CI green, clippy/fmt passes, short description of security guarantees and attack scenarios covered.

Helpful small snippets / acceptance-check patterns
• For critical failures (tampered commit), assert both: apply_commit() returns Err AND that no state change occurred (epoch unchanged, member_count unchanged).
• For concurrency tests: use deterministic nonces or seeded RNG so test results are deterministic.
• For resource caps: test both rejection (when full) and eviction (after TTL/prune).

Final notes & risks
• If you keep custom MLS primitives, be prepared for long-term maintenance and hard security reviews. OpenMLS is strongly recommended for production in the long run.
• Prioritize reproducible tests and harness before adding complexity like relays.
• Make the GroupProvider abstraction early — it reduces refactor cost when switching MLS backends.

⸻
