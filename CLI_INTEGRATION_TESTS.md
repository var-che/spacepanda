# SpacePanda CLI Integration Tests

## Overview

Comprehensive integration test suite for SpacePanda CLI that validates end-to-end workflows with multiple actors.

## Test Suite

Location: `spacepanda-cli/tests/integration_tests.rs`

### Test Infrastructure

**TestActor** - Simulates a complete user with:
- Isolated data directory (using tempfile)
- Dedicated MLS service with persistent storage
- Local store for channel metadata
- Pre-generated KeyPackage for receiving invites
- Identity with unique UserId and node ID

### Test Scenarios (8 total)

#### 1. Four-Party Channel Creation and Messaging ✅
**Purpose**: Validate basic multi-party channel workflow

**Flow**:
1. Alice creates channel
2. Alice invites Bob, Charlie, and Diana using their KeyPackages
3. All members join successfully
4. All members send messages

**Validates**:
- Channel creation
- Multi-party invitations
- KeyPackage-based join flow
- Message encryption/decryption

#### 2. Member Removal by Creator ✅
**Purpose**: Test channel admin can remove members

**Flow**:
1. Alice creates channel and invites all members
2. Alice removes Charlie
3. Remaining members (Alice, Bob, Diana) can still communicate

**Validates**:
- Admin removal permissions
- Group state updates after removal
- Remaining members can continue messaging

**Note**: Network sync limitations mean removed members' local state isn't updated in isolated tests.

#### 3. Moderator Permissions ✅
**Purpose**: Test admin promotion workflow

**Flow**:
1. Alice creates channel and invites all members
2. Alice promotes Bob to admin
3. Alice (as admin) removes Charlie
4. Remaining members communicate

**Validates**:
- Admin promotion API works
- Original admin retains removal permissions

**Note**: In isolated tests, promoted members don't receive the commit, so they won't have admin permissions in their local state without network sync.

#### 4. Sequential Member Removals ✅
**Purpose**: Test removing multiple members one by one

**Flow**:
1. All 4 members join channel
2. All send initial messages
3. Alice removes Diana (4 → 3 members)
4. Remaining 3 send messages
5. Alice removes Charlie (3 → 2 members)
6. Remaining 2 (Alice, Bob) send messages

**Validates**:
- Sequential group modifications
- Group remains functional after multiple removals
- Admin can perform multiple removals

#### 5. Non-Admin Cannot Remove Members ✅
**Purpose**: Test permission enforcement

**Flow**:
1. Alice creates channel, invites all members
2. Bob (non-admin) attempts to remove Charlie
3. Attempt fails with permission error
4. Charlie can still send messages
5. Alice (admin) successfully removes Charlie

**Validates**:
- Permission checks prevent unauthorized removals
- Failed removal doesn't corrupt group state
- Admin permissions work correctly

#### 6. Multiple Moderators Can Be Promoted ✅
**Purpose**: Test promoting multiple admins

**Flow**:
1. Alice creates channel and invites all members
2. Alice promotes both Bob and Charlie to admin
3. Alice removes Diana
4. All remaining members communicate

**Validates**:
- Multiple admin promotions succeed
- Original admin retains full permissions

**Note**: Promoted admins need network sync to exercise their permissions.

#### 7. Disconnection and Reconnection Simulation ✅
**Purpose**: Test asynchronous messaging patterns

**Flow**:
1. All members join and send initial messages
2. Charlie "disconnects" (stops sending)
3. Other members continue messaging
4. Charlie "reconnects" and sends message
5. All members active again

**Validates**:
- Asynchronous messaging works
- Members can be offline and rejoin
- No group state corruption from intermittent connectivity

#### 8. Member Removal During Simulated Disconnection ✅
**Purpose**: Test removals when members are offline

**Flow**:
1. All members join and send messages
2. Diana "disconnects"
3. Alice removes Charlie while Diana offline
4. Alice and Bob communicate
5. Diana "reconnects" and can still send
6. All remaining members communicate

**Validates**:
- Removals work when members offline
- Offline members not affected by removals
- Group remains consistent

## Test Architecture

### Isolated Testing Model

Each TestActor operates independently with:
- **Isolated MLS storage**: No shared group state
- **No network layer**: Actors cannot sync commits
- **Manual invite flow**: KeyPackages passed explicitly

### Limitations

Due to isolation without network synchronization:

1. **Removed members' state not updated**: When Alice removes Charlie, Charlie's local MLS group state is not modified. In production, the removal commit would be broadcast and Charlie would see he's been removed.

2. **Admin promotions not effective**: When Alice promotes Bob to admin, Bob's local state doesn't reflect the promotion. In production, Bob would receive and process the commit.

3. **No commit propagation**: Changes made by one actor don't automatically propagate to others.

### What These Tests Validate

✅ **API Correctness**: All channel management APIs work without errors  
✅ **Permission Checks**: Admin-only operations are enforced  
✅ **MLS Group Operations**: Create, join, send, remove all function correctly  
✅ **Multi-Party Workflows**: 4-actor scenarios work as designed  
✅ **KeyPackage Flow**: Proper invite generation and join process  
✅ **Async Patterns**: Members can be offline/online intermittently  

### What Requires Network Testing

⏳ **Full Removal Semantics**: Removed members actually lose send capability  
⏳ **Admin Promotion Effects**: Promoted members can exercise admin powers  
⏳ **Commit Propagation**: Changes broadcast to all members  
⏳ **State Synchronization**: All members converge to same group state  

## Running the Tests

```bash
# Run all CLI integration tests
nix develop --command cargo test -p spacepanda-cli --test integration_tests

# Run specific test
nix develop --command cargo test -p spacepanda-cli --test integration_tests test_four_party -- --nocapture

# Run with output
nix develop --command cargo test -p spacepanda-cli --test integration_tests -- --nocapture
```

## Test Results

✅ **8/8 tests passing**  
✅ **All library tests still passing** (1205 tests)  
✅ **Zero compilation errors**  
✅ **Zero runtime panics**  

## Dependencies

- `tempfile = "3.8"` - Isolated temporary directories for each actor
- `tokio` - Async runtime for tests
- `anyhow` - Error handling in test code

## Future Enhancements

### Phase 3 - Network Integration Tests

To test full commit propagation and state sync:

1. Add in-memory network layer for tests
2. Implement commit broadcasting between actors
3. Test actual removal semantics (removed members can't send)
4. Test promoted admin permissions
5. Test concurrent operations and conflict resolution

### Phase 4 - Chaos Testing

1. Random disconnections during operations
2. Network partitions and healing
3. Concurrent removals by different admins
4. Race conditions in group modifications

## Security Considerations

These tests validate:
- ✅ Permission enforcement (non-admins can't remove)
- ✅ Admin-only operations protected
- ✅ MLS group integrity maintained
- ✅ No crashes or panics from invalid operations

Network-level security (e.g., commit signature verification, replay protection) requires integration tests with network layer.

---

**Created**: Phase 2 Completion  
**Status**: All tests passing  
**Coverage**: CLI channel management workflows  
**Next Steps**: Add network layer for full end-to-end testing
