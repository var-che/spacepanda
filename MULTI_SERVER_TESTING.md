# Multi-Server P2P Testing Guide

## Overview

P2P network layer is now fully wired and ready for multi-server testing. This guide shows how to test P2P message distribution across multiple server instances.

## Quick Start

### Option 1: Automated Test Script (Recommended)

```bash
cd spacepanda
./scripts/run_multi_server_test.sh
```

This will:

1. Start 3 API servers on ports 50051, 50052, 50053
2. Run the multi-server Flutter test
3. Clean up servers when done

### Option 2: Manual Testing

#### Step 1: Start Multiple Servers

```bash
# Terminal 1 - Server on port 50051
cd spacepanda
nix develop --command cargo run --bin spacepanda-api -- 50051

# Terminal 2 - Server on port 50052
cd spacepanda
nix develop --command cargo run --bin spacepanda-api -- 50052

# Terminal 3 - Server on port 50053
cd spacepanda
nix develop --command cargo run --bin spacepanda-api -- 50053
```

#### Step 2: Run Multi-Server Tests

```bash
cd spacepanda_flutter
flutter test test/p2p_multi_server_test.dart
```

## Current Status

### ✅ What's Working

- **P2P Infrastructure:** NetworkLayer instantiated per session
- **Router:** Each session has its own P2P router listening
- **Multiple Servers:** Can run on different ports
- **MLS Encryption:** End-to-end encryption working
- **Local Storage:** Messages stored in each user's database

### ⚠️ What's Next

**Server Connection Required:**

Currently, each server has its own isolated P2P network. To enable message distribution, servers need to be connected:

```dart
// Connect server 1 to server 2
await client1.network.connectPeer(
  ConnectPeerRequest(
    sessionToken: alice.token,
    peerAddress: "/ip4/127.0.0.1/tcp/<server2_p2p_port>",
  ),
);
```

**Challenge:** We need to:

1. Get the actual P2P listen port from each server
2. Expose it via NetworkService.GetNetworkStatus()
3. Connect servers before testing

## Test Scenarios

### Scenario 1: Local Storage (Current)

**Setup:**

- 3 servers running
- 3 users, each connected to different server
- All users join same channel

**Expected Result:**

- Each user sees only their own messages
- Messages stored locally in each user's database
- ✅ This works now

### Scenario 2: P2P Distribution (After Connecting Servers)

**Setup:**

- 3 servers running and **connected** via NetworkService.ConnectPeer()
- 3 users, each connected to different server
- All users join same channel

**Expected Result:**

- User sends message → encrypted with MLS
- Message broadcast via P2P to all servers
- All users see all messages
- Full P2P distribution working

## Connecting Servers

### Get Network Status

```dart
final status = await client.network.getNetworkStatus(
  NetworkStatusRequest(sessionToken: token),
);

print('PeerId: ${status.peerId}');
print('Listen: ${status.listenAddress}');
print('Peers: ${status.connectedPeers}');
```

### Connect to Peer

```dart
final response = await client.network.connectPeer(
  ConnectPeerRequest(
    sessionToken: token,
    peerAddress: "/ip4/127.0.0.1/tcp/50052",
  ),
);

if (response.success) {
  print('Connected!');
}
```

## Architecture

```text
┌─────────────────────────────────────────────────┐
│              Flutter Test Client                 │
│  ┌───────────┐  ┌───────────┐  ┌───────────┐    │
│  │  Alice    │  │    Bob    │  │  Charlie  │    │
│  │(client1)  │  │(client2)  │  │(client3)  │    │
│  └─────┬─────┘  └─────┬─────┘  └─────┬─────┘    │
└────────┼──────────────┼──────────────┼──────────┘
         │gRPC          │gRPC          │gRPC
         ▼              ▼              ▼
┌────────────┐  ┌────────────┐  ┌────────────┐
│  Server 1  │  │  Server 2  │  │  Server 3  │
│ :50051     │  │ :50052     │  │ :50053     │
│            │  │            │  │            │
│ Alice's    │  │ Bob's      │  │ Charlie's  │
│ Session    │  │ Session    │  │ Session    │
│  ├─Router  │  │  ├─Router  │  │  ├─Router  │
│  ├─Network │  │  ├─Network │  │  ├─Network │
│  └─Manager │  │  └─Manager │  │  └─Manager │
└─────┬──────┘  └──────┬─────┘  └──────┬─────┘
      │                │                │
      └────────────────┴────────────────┘
            P2P Network (TCP)
        (After ConnectPeer() calls)
```

## Implementation TODOs

### 1. Get Actual Listen Address

**File:** `spacepanda-core/src/core_mvp/network.rs`

Add method to store and retrieve the actual listen address after binding:

```rust
impl NetworkLayer {
    pub fn listen_address(&self) -> Option<String> {
        // Store actual address after listen() succeeds
        self.listen_addr.clone()
    }
}
```

### 2. Track Connected Peers

**File:** `spacepanda-core/src/core_mvp/network.rs`

Add field to track connected peer IDs:

```rust
pub struct NetworkLayer {
    connected_peers: Arc<RwLock<HashSet<PeerId>>>,
    // ... existing fields
}
```

### 3. Update NetworkService.GetNetworkStatus()

**File:** `spacepanda-api/src/services/network_service.rs`

Return real data instead of placeholders:

```rust
Ok(Response::new(NetworkStatusResponse {
    peer_id: format!("{:?}", network.local_peer_id()),
    listen_address: network.listen_address().unwrap_or_default(),
    connected_peers: network.connected_peers()
        .iter()
        .map(|p| format!("{:?}", p))
        .collect(),
}))
```

### 4. Enhanced Multi-Server Test

**File:** `spacepanda_flutter/test/p2p_multi_server_test.dart`

Add server connection step:

```dart
// Get listen addresses
final status1 = await client1.network.getNetworkStatus(...);
final status2 = await client2.network.getNetworkStatus(...);
final status3 = await client3.network.getNetworkStatus(...);

// Connect servers
await client1.network.connectPeer(
  ConnectPeerRequest(peerAddress: status2.listenAddress),
);
await client2.network.connectPeer(
  ConnectPeerRequest(peerAddress: status3.listenAddress),
);

// Now test P2P distribution
// All users should see all messages!
```

## Troubleshooting

### Servers Don't Connect

**Check:**

- Firewall rules (allow TCP on P2P ports)
- Correct multiaddr format: `/ip4/127.0.0.1/tcp/PORT`
- Server logs for connection errors

### Messages Not Distributed

**Check:**

- Are servers actually connected? (Check GetNetworkStatus)
- Are channel members registered? (Check logs)
- Is background task running? (Check incoming message handler)

### Permission Denied on Port

**Solution:**

- Use ports above 1024
- Check if port is already in use: `lsof -i :50051`

## Next Steps

1. ✅ P2P wired into API
2. ✅ Multi-server test created
3. ⏳ Implement listen_address() getter
4. ⏳ Implement connected_peers tracker
5. ⏳ Update GetNetworkStatus() with real data
6. ⏳ Test full P2P distribution

## Success Criteria

✅ **Phase 1 Complete:** Infrastructure ready

- Network layer wired
- Multiple servers can run
- Tests created

🔄 **Phase 2 In Progress:** Server Connection

- Need to get actual listen addresses
- Need to track connected peers
- Need to test ConnectPeer()

⏳ **Phase 3 Planned:** Full P2P

- Servers connected
- Messages distributed
- All users see all messages

---

**Related Documentation:**

- [P2P_STATUS.md](../P2P_STATUS.md) - Architecture overview
- [P2P_WIRING_COMPLETE.md](../P2P_WIRING_COMPLETE.md) - Implementation details
- [P2P_TEST_RESULTS.md](../P2P_TEST_RESULTS.md) - Single-server test results
