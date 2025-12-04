# HTTP Test Harness for MLS MVP

## Overview

The HTTP Test Harness provides a simple REST API for testing and demonstrating the SpacePanda MLS (Message Layer Security) functionality. It allows multiple clients to connect via HTTP and perform encrypted messaging operations without needing to build a full P2P infrastructure.

## Purpose

- **Development Testing**: Quick manual testing during development
- **Integration Testing**: HTTP-based end-to-end tests
- **Demonstrations**: Show encrypted messaging to stakeholders
- **Debugging**: Inspect MLS operations via simple HTTP calls

## Architecture

```
┌─────────────┐
│ HTTP Client │ (curl, Postman, browser)
└──────┬──────┘
       │
       │ HTTP REST API
       ▼
┌──────────────────┐
│  Axum Web Server │
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│ ChannelManager   │ ← Core MLS MVP logic
└────────┬─────────┘
         │
         ├─────► MlsService (encryption)
         ├─────► LocalStore (persistence)
         └─────► Identity (user context)
```

## API Endpoints

### Identity Management

#### Create Identity

```bash
POST /identity/create
```

Creates a new user identity for the server instance.

**Response**:

```json
{
  "identity_id": "uuid-string",
  "public_key": [...]
}
```

#### Get Current Identity

```bash
GET /identity/me
```

Returns the current identity information.

**Response**:

```json
{
  "identity_id": "uuid-string",
  "public_key": [...]
}
```

### Channel Operations

#### Create Channel

```bash
POST /channels/create
Content-Type: application/json

{
  "name": "general",
  "is_public": true
}
```

Creates a new encrypted channel (MLS group).

**Response**:

```json
{
  "channel_id": "channel-uuid",
  "name": "general",
  "is_public": true
}
```

#### Get Channel Info

```bash
GET /channels/{channel_id}
```

Retrieves channel metadata.

**Response**:

```json
{
  "channel_id": "channel-uuid",
  "name": "general",
  "is_public": true,
  "member_count": 2
}
```

#### Create Invite

```bash
POST /channels/{channel_id}/invite
Content-Type: application/json

{
  "key_package": [...]  // Serialized KeyPackage from invitee
}
```

Creates an MLS Welcome message for a new member.

**Response**:

```json
{
  "invite_token": [...],  // Serialized InviteToken
  "commit": [...] | null  // Optional commit for existing members
}
```

#### Join Channel

```bash
POST /channels/{channel_id}/join
Content-Type: application/json

{
  "invite_token": [...]  // Received from inviter
}
```

Joins a channel using a Welcome message.

**Response**:

```json
{
  "channel_id": "channel-uuid",
  "channel_name": "general",
  "is_public": true,
  "success": true
}
```

#### List Members

```bash
GET /channels/{channel_id}/members
```

Lists current channel members.

**Response**:

```json
{
  "members": [
    {
      "identity_id": "hex-encoded",
      "public_key": [...]
    }
  ]
}
```

#### Process Commit

```bash
POST /channels/{channel_id}/process-commit
Content-Type: application/json

[...]  // Raw commit bytes
```

Processes an MLS commit from another member (for epoch synchronization).

**Response**: `200 OK` or error

### Messaging

#### Send Message

```bash
POST /channels/{channel_id}/send
Content-Type: application/json

{
  "plaintext": "Hello, world!"
}
```

Sends an encrypted message to the channel.

**Response**:

```json
{
  "message_id": "msg-uuid",
  "encrypted_bytes": 145
}
```

#### Get Messages

```bash
GET /channels/{channel_id}/messages
```

Retrieves message history for a channel.

**Response**:

```json
{
  "messages": [
    {
      "message_id": "msg-uuid",
      "sender_id": "user-id",
      "plaintext": "Hello, world!",
      "timestamp": 1234567890
    }
  ]
}
```

## Usage Examples

### Starting the Server

```bash
# Start on default port (3000)
cargo run --example http_demo

# Or programmatically:
use spacepanda_core::core_mvp::test_harness;

#[tokio::main]
async fn main() {
    test_harness::start_server("127.0.0.1:3000").await.unwrap();
}
```

### Full Encrypted Messaging Flow

```bash
# Terminal 1: Start Alice's server
cargo run --example http_demo -- 3001

# Terminal 2: Start Bob's server
cargo run --example http_demo -- 3002

# Terminal 3: Run the demo script
./scripts/test_http_harness.sh
```

### Manual Testing with curl

```bash
# Alice creates a channel
CHANNEL=$(curl -X POST http://localhost:3001/channels/create \
  -H "Content-Type: application/json" \
  -d '{"name":"secret-chat","is_public":false}')

CHANNEL_ID=$(echo $CHANNEL | jq -r '.channel_id')

# Bob generates a key package (would need Bob's server to implement this)
# Then Alice creates an invite with Bob's key package
# Bob joins, and messages can be exchanged
```

## Testing

The test harness includes a basic unit test:

```bash
cargo test --lib core_mvp::test_harness::server::tests
```

## Current Limitations

1. **Single Identity per Server**: Each server instance represents one user
2. **In-Memory State**: Message history not persisted across restarts
3. **No Key Package Generation**: Clients must implement this separately
4. **No Receive Message Processing**: Incoming messages not automatically decrypted
5. **Member Count Placeholder**: Returns 0 (MLS group introspection needed)

## Future Enhancements

- [ ] Multi-user support (session management)
- [ ] WebSocket support for real-time messages
- [ ] Key package generation endpoint
- [ ] Automatic message reception and decryption
- [ ] Member list from MLS group state
- [ ] Remove member endpoint
- [ ] Group admin operations
- [ ] Message history persistence
- [ ] Rate limiting and authentication
- [ ] OpenAPI/Swagger documentation

## Security Considerations

⚠️ **This is a development/testing tool only!**

- No authentication or authorization
- No rate limiting
- No TLS (uses plain HTTP)
- Stores secrets in memory
- Should NOT be used in production

For production deployment, implement:

- TLS/HTTPS
- User authentication (JWT, OAuth)
- Rate limiting
- Input validation
- Audit logging
- Secure key storage

## Architecture Integration

The test harness sits on top of the existing MVP layer:

```
HTTP Test Harness
    ↓
ChannelManager (core_mvp)
    ↓
├─ MlsService (core_mls)
├─ LocalStore (core_store)
└─ Identity (core_identity)
```

It uses the same `ChannelManager` that would be used by a full P2P client, making it an accurate representation of production behavior.

## Related Documentation

- [Core MVP README](../spacepanda-core/src/core_mvp/README.md)
- [MLS Integration Guide](../docs/mls-production-integration.md)
- [SpacePanda Project Overview](../spacepanda-core/docs/SPACEPANDA_PROJECT_OVERVIEW.md)

## License

MIT OR Apache-2.0
