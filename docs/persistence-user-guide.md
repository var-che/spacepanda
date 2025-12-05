# SpacePanda Persistence User Guide

**Status:** Alpha/MVP  
**Last Updated:** December 4, 2025

## Overview

SpacePanda saves your identity and channel metadata to disk, but currently has a known limitation with MLS group state persistence. This guide explains what persists, what doesn't, and how to work with the current system.

## What Persists

### ✅ Between CLI Sessions

1. **User Identity** - Stored in `~/.spacepanda/identity.json`

   - User ID (UUID)
   - Display name
   - Identity fingerprint

2. **MLS Group Snapshots** - Stored in `~/.spacepanda/mls_groups/`

   - Group metadata (ID, epoch number)
   - Public state (ratchet tree, group context)
   - Member list and join times
   - Encrypted at rest with AES-256-GCM

3. **CRDT Event Log** - Stored in `~/.spacepanda/commit_log/`
   - Channel metadata operations
   - Message history (future feature)
   - Conflict-free replicated data

### ❌ Does NOT Persist (Current Limitation)

1. **Active MLS Groups** - Groups don't restore to working state
   - Can't send messages to groups from previous session
   - Must re-create or re-join channels after restart
2. **Channel List** - Channels don't show in `channel list` after restart
   - CRDT store restoration issue (separate from MLS)
   - Will be fixed in future release

## Current User Experience

### First Session

```bash
# Initialize identity
$ spacepanda init --name "Alice"
✅ SpacePanda initialized successfully!
   User ID: 8e505b2e-8216-47da-b3a4-97bdc3f8054a

# Create a channel
$ spacepanda channel create "general"
✅ Channel created successfully!
   Channel ID: cf8e15fc-92d4-4d23-bc82-aa02e82e8771

# Snapshot automatically saved to disk
INFO: Successfully saved group cf8e15fc... at epoch 0
```

### Subsequent Sessions

```bash
# CLI automatically detects saved snapshots
$ spacepanda channel list

INFO: Loading persisted groups from storage
INFO: Found 1 persisted group snapshot(s)
INFO: Loaded snapshot for group cf8e15fc...: epoch=0, size=809 bytes
⚠️  Loaded 1 snapshot(s), but full group restoration is not yet implemented.
⚠️  Groups must be re-created or re-joined after restart.

No channels found.
```

**What this means:**

- Your snapshot files are safe on disk
- The CLI acknowledges their presence
- But groups aren't in active memory yet
- You need to re-create the channel or re-join via invite

## Workarounds

### For Channel Creators

**Option 1: Re-create channel with same name**

```bash
$ spacepanda channel create "general"
# Creates a new MLS group with a new ID
# Previous snapshot remains on disk but unused
```

**Option 2: Keep channel running**

```bash
# Use a terminal multiplexer to keep CLI alive
$ tmux new -s spacepanda
$ spacepanda channel create "general"
# Detach: Ctrl+B, D
# Reattach later: tmux attach -t spacepanda
```

### For Channel Members

**Save invite codes for re-joining:**

```bash
# Creator generates invite
$ spacepanda channel invite cf8e15fc-92d4-4d23-bc82-aa02e82e8771
Invite code: W3siaWRlbn...

# Member saves invite code to file
$ echo "W3siaWRlbn..." > ~/spacepanda-general-invite.txt

# After restart, re-join
$ spacepanda channel join $(cat ~/spacepanda-general-invite.txt)
```

## Data Directory Structure

```
~/.spacepanda/
├── identity.json                    # Your user identity (✅ persists)
├── commit_log/                      # CRDT event log
│   └── 000001.log                  # Operation history
├── snapshots/                       # CRDT snapshots
│   └── snapshot_0.dat              # Compacted state
└── mls_groups/                      # MLS group snapshots (✅ persists)
    ├── group-{hex-id}.snapshot     # Encrypted group state
    └── ...
```

### Snapshot File Format

Each `group-*.snapshot` file contains:

- **Header:** Magic bytes (`MLSS0001`) + version
- **Salt:** 16 bytes for Argon2 key derivation
- **Nonce:** 12 bytes for AES-GCM
- **Ciphertext:** Encrypted group state
  - Group ID
  - Epoch number
  - Serialized ratchet tree
  - Group context
  - Member information

**Size:** ~800-900 bytes per group (very efficient)

## Security Notes

### What's Protected

1. **At Rest:**

   - MLS snapshots encrypted with AES-256-GCM
   - Password-based encryption optional (currently disabled for MVP)
   - File permissions: 0600 (user read/write only)

2. **In Transit:**
   - Identity never leaves your machine
   - Snapshots are local-only
   - Invite codes use base64 encoding (not encryption)

### What to Backup

**Critical Files:**

```bash
# Backup your identity
cp ~/.spacepanda/identity.json ~/backup/

# Backup MLS snapshots (optional, for debugging)
cp -r ~/.spacepanda/mls_groups/ ~/backup/

# Backup CRDT state (future: contains message history)
cp -r ~/.spacepanda/commit_log/ ~/backup/
cp -r ~/.spacepanda/snapshots/ ~/backup/
```

**Recovery:**

```bash
# Restore identity
cp ~/backup/identity.json ~/.spacepanda/

# Note: Even with snapshots restored, groups won't be active
# You'll still need to re-join channels
```

## Troubleshooting

### "No channels found" after restart

**This is expected behavior.** Channels don't persist in the current MVP.

**Solutions:**

1. Re-create the channel (new group ID)
2. Re-join using saved invite code
3. Keep CLI running in tmux/screen

### Snapshot files exist but not loading

Check the logs:

```bash
$ spacepanda -l debug channel list 2>&1 | grep -i snapshot
```

Expected output:

```
INFO: Found 1 persisted group snapshot(s)
INFO: Loaded snapshot for group ...: epoch=0, size=809 bytes
WARN: Full group restoration is not yet implemented
```

If you see errors, the snapshot file might be corrupted.

### Removing old snapshots

Snapshots accumulate as you create channels:

```bash
# List snapshots
$ ls -lh ~/.spacepanda/mls_groups/

# Remove specific snapshot
$ rm ~/.spacepanda/mls_groups/group-{hex-id}.snapshot

# Clean all snapshots (caution!)
$ rm ~/.spacepanda/mls_groups/*.snapshot
```

**Note:** Removing snapshots doesn't affect active sessions, only prevents detection on restart.

## Roadmap

### Phase 2: Full Persistence (Planned)

**Goal:** Groups restore to working state after restart

**Implementation:** Use OpenMLS native provider storage

- Requires implementing `StorageProvider<VERSION>` trait
- ~30 methods for complete group state management
- Estimated effort: 2-4 days development

**User Impact:**

- ✅ Multi-session CLI usage
- ✅ `channel list` shows persisted channels
- ✅ `send` works with groups from previous session
- ✅ Production-ready persistence

### Phase 3: Channel Metadata (Parallel)

**Goal:** Fix CRDT LocalStore persistence

**Tasks:**

- Verify snapshot triggers
- Implement channel metadata restoration
- Link restored channels to restored MLS groups

**Estimated Effort:** 1-2 days

### Timeline

- **Current (Dec 2025):** Snapshot awareness + auto-loading ✅
- **Q1 2026:** Full MLS group restoration
- **Q1 2026:** Channel metadata persistence
- **Q2 2026:** Network layer integration + message sync

## FAQ

### Q: Will my messages be lost when I restart?

**A:** In the current MVP, there is no network layer, so messages are only in memory. When network sync is added, messages will be recovered from peers. For now, treat each session as ephemeral.

### Q: Can I migrate my snapshots to another machine?

**A:** Technically yes (they're just files), but without group restoration, they won't be useful. Wait for Phase 2 before attempting this.

### Q: Why not implement full persistence now?

**A:** The OpenMLS `StorageProvider` trait requires implementing ~30 methods correctly, with complex state management. For MVP, we prioritized getting the CLI functional with a clear limitation rather than risking buggy half-implemented persistence.

### Q: Is this production-ready?

**A:** No. The current persistence limitation makes this unsuitable for production use. It's perfect for:

- Testing the MLS protocol
- Evaluating the CLI interface
- Developing on SpacePanda
- Short-lived demo sessions

### Q: How do I know which version I'm running?

```bash
$ spacepanda --version
spacepanda-cli 0.1.0

# Check for persistence features
$ spacepanda channel create test
# Look for "Successfully saved group" in logs
```

## Getting Help

- **Documentation:** See `docs/mls-persistence-status.md` for technical details
- **Issues:** File bugs at github.com/var-che/spacepanda/issues
- **Logs:** Run with `-l debug` for detailed output

## Summary

**Current State:**

- ✅ Snapshots save automatically
- ✅ Snapshots load on startup
- ✅ Clear user warnings
- ❌ Groups don't restore (known limitation)

**Recommended Usage:**

- Use tmux/screen for persistent sessions
- Save invite codes for re-joining
- Treat as ephemeral for now
- Update when Phase 2 ships

**Bottom Line:**
SpacePanda is in active development. The current persistence implementation provides a solid foundation for future full restoration support. For alpha testing and development, the current state is acceptable with proper user expectations.
