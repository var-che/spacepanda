# Core MVP Layer

**Status**: 🚧 In Development  
**Owner**: SpacePanda Core Team  
**Version**: 0.1.0  
**Last Updated**: December 3, 2025

## Overview

The `core_mvp` module is the **orchestration layer** that integrates all SpacePanda subsystems to provide a complete, testable, end-to-end encrypted chat application. It sits above `core_identity`, `core_mls`, `core_store` (CRDT), and `core_dht` to provide high-level operations.

### Purpose

This module bridges the gap between low-level primitives and a working product by:

- **Orchestrating** MLS group creation with CRDT channel metadata
- **Managing** invite flows (Welcome message creation → delivery → join)
- **Routing** encrypted messages through the network layer
- **Enforcing** permissions and role-based access control
- **Providing** HTTP API for testing and integration

### Architecture Principles

1. **Separation of Concerns**: Each module has a single, clear responsibility
2. **Testability**: All components have integration tests with HTTP endpoints
3. **Production Ready**: Includes metrics, tracing, error handling from day 1
4. **Incremental**: Can be used in stages (local-only → P2P → distributed)

## Module Structure

```
core_mvp/
├── README.md                   # This file
├── IMPLEMENTATION_TODO.md      # Detailed implementation checklist
├── lib.rs                      # Public API exports
├── types.rs                    # Core types (ChannelDescriptor, envelopes)
├── errors.rs                   # Error types
│
├── channel_manager.rs          # PRIORITY 1: Main orchestrator
├── invite.rs                   # Invite creation/processing
├── permissions.rs              # Role enforcement logic
├── message_router.rs           # Message routing/delivery
├── persistence.rs              # State snapshots
├── bootstrap.rs                # DHT discovery helpers
│
├── api/
│   ├── mod.rs                  # HTTP server setup
│   ├── routes.rs               # Route definitions
│   ├── handlers/
│   │   ├── identity.rs         # Identity endpoints
│   │   ├── channels.rs         # Channel endpoints
│   │   └── messages.rs         # Message endpoints
│   └── middleware.rs           # Auth, logging, metrics
│
└── tests/
    ├── integration/
    │   ├── mod.rs
    │   ├── channel_lifecycle.rs    # Create→Invite→Join→Send
    │   ├── permissions.rs          # Role enforcement
    │   └── message_delivery.rs     # Routing tests
    └── fixtures/
        └── mod.rs                  # Test helpers
```

## Core Components

### 1. ChannelManager (Priority 1) 🎯

**File**: `channel_manager.rs`  
**Status**: 🚧 In Progress

The main orchestrator that coordinates:

- **Channel Creation**: MLS group + CRDT metadata + DHT publication
- **Invite Flow**: Generate Welcome message, handle join
- **Message Operations**: Encrypt, route, decrypt
- **Member Management**: Add/remove with permission checks

**Key Methods**:

```rust
impl ChannelManager {
    async fn create_channel(&self, name: String, is_public: bool) -> Result<ChannelId>
    async fn create_invite(&self, channel_id: &ChannelId, invitee_kp: &[u8]) -> Result<Vec<u8>>
    async fn join_channel(&self, welcome: &[u8], ratchet_tree: Option<&[u8]>) -> Result<ChannelId>
    async fn send_message(&self, channel_id: &ChannelId, plaintext: &[u8]) -> Result<MessageId>
    async fn receive_message(&self, ciphertext: &[u8]) -> Result<ChatMessage>
}
```

### 2. HTTP API Server (Priority 2)

**Directory**: `api/`  
**Status**: 📋 Planned

REST API for testing and integration:

| Endpoint                 | Method | Purpose                 |
| ------------------------ | ------ | ----------------------- |
| `/identity/create`       | POST   | Create local identity   |
| `/identity/me`           | GET    | Get current identity    |
| `/channels/create`       | POST   | Create new channel      |
| `/channels/:id`          | GET    | Get channel metadata    |
| `/channels/:id/invite`   | POST   | Generate invite/Welcome |
| `/channels/:id/join`     | POST   | Join from Welcome       |
| `/channels/:id/send`     | POST   | Send encrypted message  |
| `/channels/:id/messages` | GET    | Get message history     |
| `/channels/:id/members`  | GET    | List members            |
| `/channels/:id/roles`    | POST   | Manage roles            |

### 3. Message Router (Priority 4)

**File**: `message_router.rs`  
**Status**: 📋 Planned

Handles message delivery:

- MLS envelope serialization
- Integration with `core_router`
- Retry logic
- Sequence numbering and deduplication

### 4. Permissions Module (Priority 3)

**File**: `permissions.rs`  
**Status**: 📋 Planned

Runtime permission enforcement:

- Check capabilities before operations
- Validate role changes via MLS signatures
- CRDT-based role resolution

## Data Models

### ChannelDescriptor

```rust
pub struct ChannelDescriptor {
    pub channel_id: ChannelId,
    pub owner: UserId,
    pub name: String,
    pub is_public: bool,
    pub mls_group_id: GroupId,
    pub created_at: Timestamp,
    pub bootstrap_peers: Vec<PeerId>,
}
```

### InviteToken

```rust
pub struct InviteToken {
    pub channel_id: ChannelId,
    pub welcome_blob: Vec<u8>,
    pub ratchet_tree: Option<Vec<u8>>,
    pub created_at: Timestamp,
    pub expires_at: Option<Timestamp>,
}
```

### ChatMessage

```rust
pub struct ChatMessage {
    pub message_id: MessageId,
    pub channel_id: ChannelId,
    pub sender: UserId,
    pub timestamp: Timestamp,
    pub body: Vec<u8>,
    pub reply_to: Option<MessageId>,
}
```

## Integration Points

### Dependencies on Core Subsystems

```
core_mvp
├── core_identity      # Identity management, signatures
├── core_mls           # Group encryption (OpenMLS)
├── core_store         # CRDT state, persistence
├── core_dht           # Discovery, bootstrap
├── core_router        # Message transport
└── Infrastructure     # Metrics, tracing, shutdown
```

### Flow Example: Create Channel

```
1. User calls ChannelManager::create_channel()
   ↓
2. Create MLS group via MlsService
   ↓
3. Create CRDT Channel model
   ↓
4. Store in LocalStore
   ↓
5. (If public) Publish ChannelDescriptor to DHT
   ↓
6. Return ChannelId to user
```

## Testing Strategy

### Unit Tests

- Each module has `#[cfg(test)] mod tests`
- Test individual functions in isolation
- Mock dependencies where needed

### Integration Tests

- `tests/integration/` directory
- Multi-party scenarios (2-3 participants)
- Full end-to-end flows
- HTTP API tests

### Test Scenarios

1. **Basic Flow**: Create → Invite → Join → Send → Receive
2. **Permission Tests**: Role enforcement, unauthorized operations
3. **Member Management**: Add, remove, role changes
4. **Forward Secrecy**: Removed member can't decrypt
5. **Concurrent Operations**: Race conditions, epoch sync
6. **Error Handling**: Invalid Welcome, missing ratchet tree

### Test Fixtures

Located in `tests/fixtures/`:

- Test identities
- Pre-created channels
- Sample messages
- Mock DHT/Router

## Development Phases

### Phase 1: Foundation (Week 1) ✅ CURRENT

- ✅ Module structure created
- ✅ Documentation written
- 🚧 ChannelManager core implementation
- 🚧 Basic types and errors

### Phase 2: API (Week 1-2)

- 📋 HTTP server setup
- 📋 Channel endpoints
- 📋 Identity endpoints
- 📋 Integration tests

### Phase 3: Routing (Week 2)

- 📋 Message router
- 📋 MLS ↔ Router integration
- 📋 Delivery tests

### Phase 4: Polish (Week 3)

- 📋 Permission enforcement
- 📋 Demo script
- 📋 Documentation
- 📋 Manager presentation

## Metrics & Observability

### Recorded Metrics

- `mvp.channel.created` - Channel creation count
- `mvp.invite.generated` - Invite generation count
- `mvp.message.sent` - Messages sent
- `mvp.message.received` - Messages received
- `mvp.permission.denied` - Permission violations

### Trace Spans

- `mvp::create_channel`
- `mvp::join_channel`
- `mvp::send_message`
- `mvp::process_invite`

## Security Considerations

1. **E2E Encryption**: All messages encrypted via MLS
2. **Forward Secrecy**: Ratcheting keys, removed members excluded
3. **Permission Validation**: Operations checked against CRDT roles
4. **Signature Verification**: MLS commits/proposals verified
5. **Rate Limiting**: (Future) Prevent invite spam

## API Examples

### Create Channel

```bash
curl -X POST http://localhost:8080/channels/create \
  -H "Content-Type: application/json" \
  -d '{"name": "general", "is_public": false}'

# Response:
# {"channel_id": "ch_abc123", "mls_group_id": "..."}
```

### Invite Member

```bash
curl -X POST http://localhost:8080/channels/ch_abc123/invite \
  -H "Content-Type: application/json" \
  -d '{"key_package": "0x1234..."}'

# Response:
# {"welcome": "0xabcd...", "ratchet_tree": "0xef01..."}
```

### Send Message

```bash
curl -X POST http://localhost:8080/channels/ch_abc123/send \
  -H "Content-Type: application/json" \
  -d '{"plaintext": "Hello, world!"}'

# Response:
# {"message_id": "msg_xyz789", "timestamp": 1733234567}
```

## Configuration

```toml
[mvp]
# HTTP server settings
http_port = 8080
http_bind = "127.0.0.1"

# Channel settings
max_channels_per_user = 100
max_members_per_channel = 1000

# Invite settings
invite_expiry_seconds = 86400  # 24 hours

# Message settings
max_message_size_bytes = 65536  # 64KB
```

## Known Limitations (v0.1)

- **No P2P networking**: Uses localhost/hardcoded peers
- **No DHT discovery**: Bootstrap peers are configured
- **No offline sync**: Assumes all participants online
- **No message persistence**: Messages in-memory only
- **No relay logic**: Direct connections only
- **No NAT traversal**: Local network only

These will be addressed in future versions.

## Contributing

See `IMPLEMENTATION_TODO.md` for the detailed task breakdown and current priorities.

## References

- [MLS RFC 9420](https://datatracker.ietf.org/doc/rfc9420/)
- [OpenMLS Documentation](https://openmls.tech/)
- [CRDT Literature](https://crdt.tech/)
- SpacePanda Project Overview (docs/SPACEPANDA_PROJECT_OVERVIEW.md)
