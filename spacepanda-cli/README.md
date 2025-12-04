# SpacePanda CLI

Privacy-first encrypted chat - Command-line interface for SpacePanda.

## Installation

```bash
# From source
nix develop
cargo build --release --bin spacepanda

# Binary location
./target/release/spacepanda
```

## Quick Start

### 1. Initialize SpacePanda

```bash
spacepanda init --name "Your Name"
```

This creates:

- `~/.spacepanda/` - Data directory
- `~/.spacepanda/identity.json` - Your encrypted identity
- `~/.spacepanda/commit_log` - CRDT operation log
- `~/.spacepanda/snapshots/` - Store snapshots

### 2. Create a Channel

```bash
spacepanda channel create "general"
```

Output:

```
✅ Channel created successfully!
   Channel ID: 22163aed-8c95-4c9e-8e19-8ec07617400d
   Name: general
   Public: no

To invite others:
  spacepanda channel invite 22163aed-8c95-4c9e-8e19-8ec07617400d
```

### 3. Invite Others

```bash
spacepanda channel invite <channel-id>
```

This generates a base64-encoded invite code containing:

- MLS Welcome message (encrypted)
- Ratchet tree for group state
- Channel metadata
- Inviter's peer ID (for P2P connection)

**Share this code with the person you want to invite.**

### 4. Join from Invite

```bash
spacepanda channel join <invite-code>
```

### 5. List Your Channels

```bash
spacepanda channel list
```

Output:

```
Your channels:

  📁 general (22163aed-8c95-4c9e-8e19-8ec07617400d)
     Owner: 4a64d642-4e1c-4308-9c81-b9b0d85b3eee
     Public: no
```

### 6. Send Messages

```bash
spacepanda send <channel-id> "Hello from SpacePanda!"
```

### 7. Listen for Messages (Future)

```bash
spacepanda listen <channel-id>
```

_(Interactive message receiving not yet implemented - requires network layer)_

## Commands

### `init`

Initialize SpacePanda (create identity and storage).

```bash
spacepanda init --name <name>
```

**Options:**

- `--name <name>` - Your display name

### `channel`

Channel management commands.

#### `channel create`

Create a new encrypted channel.

```bash
spacepanda channel create <name> [--public]
```

**Options:**

- `<name>` - Channel name
- `--public` - Make channel publicly discoverable (default: false)

#### `channel join`

Join a channel from an invite code.

```bash
spacepanda channel join <invite>
```

**Arguments:**

- `<invite>` - Base64-encoded invite token

#### `channel invite`

Generate an invite code for a channel.

```bash
spacepanda channel invite <channel-id>
```

**Arguments:**

- `<channel-id>` - Channel ID to create invite for

#### `channel list`

List all your channels.

```bash
spacepanda channel list
```

### `send`

Send an encrypted message to a channel.

```bash
spacepanda send <channel-id> <message>
```

**Arguments:**

- `<channel-id>` - Channel ID to send to
- `<message>` - Message text

### `listen`

Listen for incoming messages (interactive mode).

```bash
spacepanda listen <channel-id>
```

**Arguments:**

- `<channel-id>` - Channel ID to listen on

## Global Options

- `-l, --log-level <LEVEL>` - Set log level (trace, debug, info, warn, error) [default: info]
- `--json-logs` - Enable JSON formatted logging
- `-d, --data-dir <DIR>` - Data directory for storage [default: ~/.spacepanda]

## Architecture

```
┌─────────────────┐
│  SpacePanda CLI │
└────────┬────────┘
         │
         ▼
┌─────────────────────────────┐
│    ChannelManager (MVP)     │
│  • create_channel()         │
│  • join_channel()           │
│  • send_message()           │
└────────┬────────────────────┘
         │
    ┌────┴────┬────────┬───────┐
    ▼         ▼        ▼       ▼
┌────────┬─────────┬─────┬──────────┐
│  MLS   │  CRDT   │ DHT │ Network  │
│ (E2EE) │ (State) │(Disc│  (P2P)   │
└────────┴─────────┴─────┴──────────┘
```

## Security Features

✅ **End-to-End Encryption** - MLS (RFC 9420) protocol  
✅ **Perfect Forward Secrecy** - New keys per epoch  
✅ **Privacy-First Peer Discovery** - Invite-based (no DHT metadata leakage)  
✅ **Onion Routing** - 3-hop circuits for message delivery  
✅ **Local-First Storage** - CRDT-based state sync

## Current Limitations

⚠️ **MVP Status** - This is an early prototype:

1. **No MLS State Persistence** - Groups don't persist between CLI invocations

   - **Impact**: Can only send/receive messages within a single CLI session
   - **Workaround**: Keep CLI running for interactive sessions
   - **Fix**: Implement MLS state serialization (planned)

2. **No Network Layer in CLI** - P2P networking not integrated

   - **Impact**: Cannot actually communicate with other users yet
   - **Workaround**: Use test harness for multi-user testing
   - **Fix**: Add network layer initialization in CLI (next priority)

3. **Invite Key Package Issue** - CLI generates temporary key packages

   - **Impact**: Invitees can't properly join yet
   - **Workaround**: Invitee generates their own key package first
   - **Fix**: Implement proper key package exchange flow

4. **No Message Receiving UI** - `listen` command is a placeholder
   - **Impact**: Can't see incoming messages interactively
   - **Workaround**: Poll channel state
   - **Fix**: Implement message queue and display loop

## Development Roadmap

### Next Steps (Priority Order)

1. **MLS State Persistence** (P0 - Blocking)

   - Store group state to disk
   - Load groups on manager initialization
   - Enable multi-session usage

2. **Network Layer Integration** (P0 - Blocking)

   - Initialize P2P router in CLI
   - Connect `listen` command to incoming message queue
   - Enable actual multi-user chat

3. **Interactive Message UI** (P1)

   - Real-time message display
   - Typing indicators
   - Read receipts

4. **Key Package Exchange** (P1)

   - Proper out-of-band key package sharing
   - QR code generation for invites
   - Import key packages from files

5. **TUI (Terminal UI)** (P2)
   - Full-screen interface with `ratatui`
   - Channel switcher
   - Message history scrolling
   - Status bar with connection info

## Testing

```bash
# Run all tests
cargo test

# Test CLI build
cargo build --bin spacepanda

# Test CLI commands
spacepanda --help
spacepanda -d /tmp/test-sp init --name "Test"
spacepanda -d /tmp/test-sp channel create "test"
```

## Contributing

See [../DEVELOPMENT.md](../DEVELOPMENT.md) for development setup.

## License

MIT - See [../LICENSE-MIT](../LICENSE-MIT)
