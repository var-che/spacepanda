# Priority 6: Feature Options & Recommendations

## Completed So Far

- ✅ Priority 1: Core MLS Implementation
- ✅ Priority 2: Channel Manager
- ✅ Priority 3: Two-way & Multi-party Messaging
- ✅ Priority 4: HTTP Test Harness
- ✅ Priority 5: Member Removal

## Next Feature Options

### Option A: **Channel Admin/Roles System** ⭐ RECOMMENDED

**Why This is Important:**

- Member removal needs permission checks
- Foundation for future access control
- Critical for real-world usage
- Builds on existing member management

**What to Implement:**

1. Add `role` field to group members (Admin, Member, ReadOnly)
2. Add `is_admin()` method to ChannelManager
3. Enforce permissions in `remove_member()`
4. Add `promote_member()` and `demote_member()` methods
5. Creator is automatically admin
6. HTTP endpoints for role management

**Effort:** 4-6 hours  
**Impact:** High - Enables secure channel management  
**Dependencies:** None - builds on existing code

---

### Option B: **Message Threading/Replies**

**Why This is Useful:**

- Essential for group conversations
- Better UX than flat messages
- Frequently requested feature

**What to Implement:**

1. Add `reply_to` field to ChatMessage
2. Thread ID generation
3. Query messages by thread
4. HTTP endpoints for threaded messages

**Effort:** 3-4 hours  
**Impact:** Medium - UX improvement  
**Dependencies:** None

---

### Option C: **Message Reactions/Emoji**

**Why This is Fun:**

- Modern messaging feature
- Low complexity, high user delight
- Good for demos

**What to Implement:**

1. Add reactions as metadata
2. Store reactions per message
3. Query reactions
4. HTTP endpoints

**Effort:** 2-3 hours  
**Impact:** Low - Nice-to-have  
**Dependencies:** None

---

### Option D: **File Attachments**

**Why This is Complex:**

- Very useful feature
- Requires binary handling
- Chunking for large files

**What to Implement:**

1. Binary message support (already have Vec<u8>)
2. Attachment metadata (filename, size, mime-type)
3. Chunking for large files
4. HTTP multipart upload
5. Download endpoints

**Effort:** 6-8 hours  
**Impact:** High - Major feature  
**Dependencies:** Message metadata system

---

### Option E: **Typing Indicators**

**Why This is Real-time:**

- Improves UX
- Shows who's typing
- Requires different architecture

**What to Implement:**

1. Ephemeral state (not in MLS)
2. Separate signaling channel
3. WebSocket support
4. HTTP Server-Sent Events alternative

**Effort:** 8-10 hours  
**Impact:** Medium - UX feature  
**Dependencies:** Real-time transport layer

---

### Option F: **Message Read Receipts**

**Why This is Privacy-Sensitive:**

- Shows who read messages
- Privacy implications
- Requires tracking

**What to Implement:**

1. Read status per user per message
2. Update mechanism
3. Privacy controls (opt-in/out)
4. HTTP endpoints

**Effort:** 4-5 hours  
**Impact:** Medium - Expected feature  
**Dependencies:** None

---

### Option G: **Channel Discovery/Search**

**Why This is Useful:**

- Find channels by name/topic
- Public channel directory
- User search

**What to Implement:**

1. Channel metadata indexing
2. Search API
3. Public/private filtering
4. HTTP search endpoint

**Effort:** 3-4 hours  
**Impact:** Medium - Usability  
**Dependencies:** Existing metadata

---

## Recommendation: **Option A - Channel Admin/Roles** ⭐

### Why This Should Be Next

1. **Security Critical**: Currently anyone can remove anyone
2. **Builds on Recent Work**: Extends member removal feature
3. **Foundation for Future**: Many features need permissions
4. **Low Hanging Fruit**: Straightforward implementation
5. **High Value**: Essential for production use

### Implementation Plan

**Phase 1: Core Role System (2 hours)**

- Add `MemberRole` enum (Admin, Member, ReadOnly)
- Store roles in group metadata CRDT
- Add permission check methods

**Phase 2: Permission Enforcement (1 hour)**

- Update `remove_member()` to check permissions
- Add `promote_member()` method
- Add `demote_member()` method

**Phase 3: HTTP API (1 hour)**

- Add role management endpoints
- Update member list to show roles
- Add permission denied error responses

**Phase 4: Testing (2 hours)**

- Permission check tests
- Role promotion/demotion tests
- Error handling tests
- HTTP endpoint tests

### What You'll Have After

- ✅ Secure member removal (only admins)
- ✅ Role-based access control
- ✅ Foundation for future permissions
- ✅ Admin promotion/demotion
- ✅ Comprehensive permission testing

---

## Alternative: **Quick Wins Bundle**

If you prefer multiple small features over one large one:

1. **Message Threading** (3-4 hours) - Better conversations
2. **Message Reactions** (2-3 hours) - User engagement
3. **Read Receipts** (4-5 hours) - Expected feature

Total: 9-12 hours for 3 features vs 6 hours for roles system

---

## What Should We Build Next?

**My Recommendation:** Start with **Channel Admin/Roles (Option A)**

It's the most important for security, builds directly on what we just completed, and unblocks future features that need permissions.

**Would you like to:**

1. ✅ **Implement Channel Admin/Roles** (recommended)
2. Choose a different option from above
3. Suggest something else entirely
4. Take a break and revisit the module visibility issues first

What sounds most valuable to you?
