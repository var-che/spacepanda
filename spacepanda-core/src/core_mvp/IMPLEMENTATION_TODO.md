# Core MVP Implementation TODO

**Last Updated**: December 3, 2025  
**Sprint**: Week 1 - Foundation

## Priority System

- 🔴 **P0**: Blocking - Must complete for MVP
- 🟡 **P1**: Important - Complete in Week 1-2
- 🟢 **P2**: Nice to have - Complete if time allows
- ⚪ **P3**: Future - Defer to v0.2

## Week 1: Foundation & Core Orchestration

### 🔴 P0: Module Setup (Day 1)

- [x] Create directory structure
- [x] Write README.md
- [x] Write IMPLEMENTATION_TODO.md (this file)
- [ ] Create lib.rs with public exports
- [ ] Create types.rs with core data models
- [ ] Create errors.rs with error types
- [ ] Add core_mvp to workspace Cargo.toml
- [ ] Add dependencies (axum, serde, tokio)

### 🔴 P0: ChannelManager Core (Day 1-2)

- [ ] Create channel_manager.rs skeleton
- [ ] Implement `ChannelManager::new()`
- [ ] Implement `create_channel()`
  - [ ] Create MLS group via MlsService
  - [ ] Create CRDT Channel model
  - [ ] Store in LocalStore
  - [ ] Publish to DHT (if public)
- [ ] Implement `create_invite()`
  - [ ] Get MLS handle for channel
  - [ ] Add member via add_members()
  - [ ] Export ratchet tree
  - [ ] Return Welcome + ratchet tree
- [ ] Implement `join_channel()`
  - [ ] Join MLS group from Welcome
  - [ ] Register in MlsService
  - [ ] Fetch channel metadata
  - [ ] Update local store
- [ ] Implement `send_message()`
  - [ ] Get MLS handle
  - [ ] Encrypt message
  - [ ] Return ciphertext
- [ ] Implement `receive_message()`
  - [ ] Decrypt via MLS
  - [ ] Parse ChatMessage
  - [ ] Return plaintext
- [ ] Unit tests for each method

### 🟡 P1: Types & Errors (Day 2)

- [ ] Define `ChannelDescriptor`
- [ ] Define `InviteToken`
- [ ] Define `ChatMessage`
- [ ] Define `MvpError` enum
- [ ] Implement Display for errors
- [ ] Implement From conversions (MlsError, StoreError)
- [ ] Add error documentation

### 🟡 P1: Basic Integration Test (Day 2-3)

- [ ] Create tests/integration/mod.rs
- [ ] Create tests/fixtures/mod.rs
- [ ] Write fixture: create_test_identity()
- [ ] Write fixture: create_test_manager()
- [ ] Test: create_channel → verify MLS group exists
- [ ] Test: create_invite → verify Welcome generated
- [ ] Test: join_channel → verify member added
- [ ] Test: send/receive → verify E2E encryption

## Week 2: HTTP API & Routing

### 🔴 P0: HTTP Server Setup (Day 3-4)

- [ ] Create api/mod.rs
- [ ] Create api/routes.rs
- [ ] Set up axum Router
- [ ] Add middleware: logging, metrics
- [ ] Create api/handlers/identity.rs
  - [ ] POST /identity/create
  - [ ] GET /identity/me
- [ ] Create api/handlers/channels.rs
  - [ ] POST /channels/create
  - [ ] GET /channels/:id
  - [ ] POST /channels/:id/invite
  - [ ] POST /channels/:id/join
- [ ] Create api/handlers/messages.rs
  - [ ] POST /channels/:id/send
  - [ ] GET /channels/:id/messages
- [ ] Add request/response types
- [ ] Add validation

### 🟡 P1: API Integration Tests (Day 4)

- [ ] Create tests/integration/api_tests.rs
- [ ] Test: Start server, create identity
- [ ] Test: Create channel via HTTP
- [ ] Test: Invite flow (2 servers)
- [ ] Test: Send message via HTTP
- [ ] Test: Error handling (404, 400, 500)
- [ ] Test: Concurrent requests

### 🟡 P1: Message Router (Day 5)

- [ ] Create message_router.rs
- [ ] Define MessageEnvelope
- [ ] Implement route_to_group()
- [ ] Integrate with core_router
- [ ] Add retry logic
- [ ] Add sequence numbers
- [ ] Add deduplication
- [ ] Unit tests for routing

### 🟢 P2: Permissions Module (Day 5-6)

- [ ] Create permissions.rs
- [ ] Implement check_permission()
- [ ] Integrate with ChannelManager
- [ ] Add permission validation tests
- [ ] Test role changes
- [ ] Test unauthorized operations

## Week 3: Polish & Demo

### 🟡 P1: End-to-End Integration Tests (Day 6-7)

- [ ] Test: 3-party scenario (Alice, Bob, Charlie)
- [ ] Test: Member removal + forward secrecy
- [ ] Test: Role changes
- [ ] Test: Permission enforcement
- [ ] Test: Concurrent channel operations
- [ ] Test: Error recovery

### 🟡 P1: Demo Script (Day 7)

- [ ] Create examples/demo.rs
- [ ] Script: Start 2 servers
- [ ] Script: Create channel
- [ ] Script: Invite + join
- [ ] Script: Send messages
- [ ] Script: Show decryption
- [ ] Add CLI output formatting
- [ ] Record demo video

### 🟢 P2: Documentation (Day 8)

- [ ] Add rustdoc comments to all public APIs
- [ ] Create sequence diagrams
- [ ] Write API reference
- [ ] Create troubleshooting guide
- [ ] Add configuration examples

### 🟢 P2: Observability (Day 8-9)

- [ ] Add metrics to ChannelManager
- [ ] Add trace spans
- [ ] Add structured logging
- [ ] Create Grafana dashboard config
- [ ] Add health check endpoint

### ⚪ P3: Advanced Features (Future)

- [ ] Persistence layer (save/restore state)
- [ ] Bootstrap helpers (DHT discovery)
- [ ] Message history pagination
- [ ] User profile management
- [ ] Channel search/discovery
- [ ] Invite link generation
- [ ] Member kick/ban
- [ ] Channel deletion
- [ ] Backup/export

## Testing Checklist

### Unit Tests

- [ ] ChannelManager methods (100% coverage)
- [ ] Message router (100% coverage)
- [ ] Permission checks (100% coverage)
- [ ] Type conversions
- [ ] Error handling

### Integration Tests

- [ ] 2-party channel lifecycle
- [ ] 3-party channel lifecycle
- [ ] HTTP API endpoints
- [ ] Permission enforcement
- [ ] Message delivery
- [ ] Error scenarios

### Performance Tests

- [ ] Benchmark channel creation
- [ ] Benchmark message encryption
- [ ] Benchmark invite generation
- [ ] Load test: 100 messages/sec
- [ ] Load test: 50 concurrent channels

### Security Tests

- [ ] Forward secrecy (removed member)
- [ ] Permission violations
- [ ] Invalid Welcome message
- [ ] Tampered ciphertext
- [ ] Replay attacks

## Dependencies to Add

```toml
[dependencies]
# HTTP Server
axum = "0.7"
tower = "0.4"
tower-http = "0.5"

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Async
tokio = { version = "1.0", features = ["full"] }

# Error handling
thiserror = "1.0"
anyhow = "1.0"

# Logging
tracing = "0.1"

# Testing
reqwest = { version = "0.11", features = ["json"] }
tempfile = "3.0"

# Internal
spacepanda-core = { path = "../" }
```

## Metrics to Track

- `mvp.channel.created.total`
- `mvp.channel.joined.total`
- `mvp.invite.generated.total`
- `mvp.message.sent.total`
- `mvp.message.received.total`
- `mvp.message.encrypted.duration_ms`
- `mvp.message.decrypted.duration_ms`
- `mvp.permission.check.duration_ms`
- `mvp.permission.denied.total`
- `mvp.api.request.duration_ms`
- `mvp.api.request.total`
- `mvp.api.error.total`

## Success Criteria

### Week 1

- ✅ ChannelManager can create channels
- ✅ Can generate invites (Welcome messages)
- ✅ Can join from Welcome
- ✅ Can send/receive encrypted messages
- ✅ Basic integration test passes

### Week 2

- ✅ HTTP API server running
- ✅ Can create channel via API
- ✅ Can invite/join via API
- ✅ Can send messages via API
- ✅ 2-server integration test passes

### Week 3

- ✅ Demo script shows full flow
- ✅ 3-party test with member removal
- ✅ Permission enforcement works
- ✅ Documentation complete
- ✅ Ready for manager demo

## Current Status

**Phase**: Week 1 - Day 1  
**Progress**: 15% (Structure created, starting implementation)  
**Blocked**: None  
**Next Action**: Implement ChannelManager::new() and create_channel()

---

## Daily Updates

### December 3, 2025

- ✅ Created module structure
- ✅ Wrote comprehensive README
- ✅ Created implementation TODO
- 🚧 Starting ChannelManager implementation
- **Next**: Complete types.rs and errors.rs

---

## Notes

### Design Decisions

1. **HTTP API First**: Enables rapid testing without building UI
2. **LocalStore Integration**: Reuse existing CRDT persistence
3. **MLS Service Wrapper**: Don't reimplement MLS, orchestrate existing code
4. **Defer Networking**: Use localhost for MVP, add P2P later
5. **Metrics from Day 1**: Production-ready mindset

### Risks

- ⚠️ MlsService API might need adjustments for multi-group management
- ⚠️ CRDT sync across joins not fully specified
- ⚠️ Message delivery without proper P2P is simplified

### Mitigations

- Start with single-group tests to validate approach
- Use in-memory message queue for MVP
- Document limitations clearly
