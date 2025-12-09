# Space Visibility & Architecture Specification

**Version**: 1.0  
**Date**: December 9, 2025  
**Status**: Design Document

---

## Overview

SpacePanda provides Discord-like functionality with enhanced privacy and security. Users create **Spaces** (similar to Discord servers), which contain multiple **Channels**. Each Space has visibility settings controlling discovery and access.

**Key Design Principles**:
- Privacy-first architecture with end-to-end encryption
- Scalable design supporting 100-1000+ members per Space
- Channel-scoped MLS groups for optimal security and performance
- Simple permission model initially, expanding to Discord-level granularity

---

## Terminology

- **Space**: A container for channels, similar to a Discord server
- **Channel**: A communication primitive with its own MLS group (one per channel)
- **MLS Group**: End-to-end encrypted group for a specific channel
- **Space Member**: User with access to the Space and its public channels
- **Channel Member**: User participating in a channel's MLS group (can read & write)

---

## Architecture Overview

### Hierarchical Structure

```
Space (Metadata Container)
├── Space ID, Name, Description, Icon
├── Space Visibility (Public / Private)
├── Space Members (UserID → Role mapping)
│   ├── Owner (1 per space)
│   ├── Admins (0-N)
│   └── Members (0-N)
│
└── Channels (1-N channels per space)
    ├── Channel #1
    │   ├── MLS Group #1 (separate encryption domain)
    │   ├── Channel Visibility (Public / Private)
    │   ├── Channel Members (all Space members if public)
    │   └── Channel Permissions (optional overrides)
    │
    ├── Channel #2 → MLS Group #2
    └── Channel #3 → MLS Group #3
```

### Key Architectural Decisions

1. **MLS Scope**: Each channel has its own MLS group
   - **Rationale**: Reduces churn from Space-level member changes
   - **Benefit**: Channel access control independent of Space membership
   - **Trade-off**: More MLS groups to manage, but better isolation

2. **Member Access Model**: All channel members are full MLS participants
   - **Rationale**: Seamless read-write transition, simpler UX
   - **Benefit**: No "promotion" flow needed
   - **Trade-off**: Larger MLS groups, but acceptable for 100-1000 members

3. **Scalability Target**: Design for 1000+ members
   - **Initial**: 100 members per Space/Channel
   - **Future**: Support for 1000+ members
   - **Optimization**: Channel-scoped MLS keeps groups manageable

---

## Space Visibility

### Space Visibility Modes

| Mode | Discovery | Join Process | Use Case |
|------|-----------|--------------|----------|
| **Public** | Listed in global directory | Anyone can join | Communities, open groups |
| **Private** | Not listed, invite-only | Requires invite link/code | Private groups, teams |

### Space Discovery (Public Spaces)

**Public Space Directory**:
- Global searchable listing of all public Spaces
- Search by: Name, topic, tags, member count
- Browse by: Category, popularity, recently active
- Preview: Space name, description, icon, member count

**Implementation**:
```rust
struct SpaceDirectory {
    // Indexed for fast search
    spaces: BTreeMap<SpaceId, PublicSpaceInfo>,
    search_index: SearchIndex, // Name/topic search
}

struct PublicSpaceInfo {
    id: SpaceId,
    name: String,
    description: String,
    icon_url: Option<String>,
    tags: Vec<String>,
    member_count: usize,
    created_at: Timestamp,
}
```

### Space Invitation (Private Spaces)

**Invite Methods**:
1. **Invite Link**: One-time or permanent links
2. **Direct Invite**: Sent to specific user by UserID
3. **Invite Code**: Short alphanumeric code (e.g., `abc-xyz-123`)

**Invite Flow**:
```
1. Admin generates invite (link/code)
2. Invitee receives invite
3. Invitee sees Space preview (name, icon, member count)
4. Invitee accepts → becomes Space Member
5. Invitee auto-joins all public channels
6. Each channel adds user to its MLS group
```

---

## Channel Architecture

### Channel Visibility Modes

| Mode | Visibility | Auto-Join | Use Case |
|------|-----------|-----------|----------|
| **Public** | All Space members see it | Yes, all Space members | Default channels (#general) |
| **Private** | Only channel members see it | No, explicit invite required | Admin-only, sub-groups |

### Channel Defaults

When creating a new channel:
- **Default Visibility**: Public
- **Default Behavior**: All Space members auto-join
- **Override**: Creator can set to Private during creation

### Channel ↔ MLS Group Mapping

**One Channel = One MLS Group**

```
Channel: #general
├── MLS Group ID: group_abc123
├── Channel Members: [user1, user2, user3, ...]
├── MLS Group Members: [user1, user2, user3, ...] (same list)
└── Group Exporter Secret: Used for sealed sender

Channel: #admin-only
├── MLS Group ID: group_def456
├── Channel Members: [admin1, admin2] (only admins)
├── MLS Group Members: [admin1, admin2]
└── Group Exporter Secret: Separate from #general
```

**Benefits**:
- Channel isolation (compromise of one doesn't affect others)
- Independent key rotation per channel
- Granular access control

---

## Membership Model

### Space Membership

**Lifecycle**:
```
User → Receives Invite → Accepts → Space Member → Auto-joins Public Channels
```

**Space Member Attributes**:
```rust
struct SpaceMember {
    user_id: UserId,
    role: SpaceRole,
    joined_at: Timestamp,
    invited_by: Option<UserId>,
}

enum SpaceRole {
    Owner,   // 1 per space, full control
    Admin,   // Can manage channels, members, roles
    Member,  // Default role, can participate in channels
}
```

### Channel Membership

**Public Channel**:
- All Space members are automatically channel members
- Added to channel's MLS group on Space join
- Receive Welcome message with group keys

**Private Channel**:
- Only explicitly invited users (or admins) can join
- Must have required Space role (if set)
- Added to channel's MLS group on invitation acceptance

**Channel Member Operations**:
```rust
impl Channel {
    // Add user to channel (and MLS group)
    async fn add_member(&mut self, user_id: UserId) -> Result<()>;
    
    // Remove user from channel (and MLS group)
    async fn remove_member(&mut self, user_id: UserId) -> Result<()>;
    
    // Check if user can access channel
    fn can_access(&self, user: &SpaceMember) -> bool;
}
```

---

## Permission System

### Phase 1: MVP Roles (Current)

**Space-Level Roles**:

| Role | Create Channels | Delete Channels | Manage Members | Send Messages | Manage Space |
|------|----------------|----------------|----------------|---------------|-------------|
| **Owner** | ✅ | ✅ | ✅ | ✅ | ✅ (+ delete Space, transfer ownership) |
| **Admin** | ✅ | ✅ | ✅ | ✅ | ✅ |
| **Member** | ❌ | ❌ | ❌ | ✅ | ❌ |

**Channel-Level Overrides** (Future):
- Not implemented in Phase 1
- Will allow per-channel permission customization
- Example: #announcements (Members can read only, Admins can send)

### Phase 2: Extended Permissions (Future)

**Additional Roles**:
- **Moderator**: Manage messages, timeout users
- **Guest**: Temporary read-only access

**Permission Granularity**:
```rust
bitflags! {
    struct PermissionSet: u64 {
        // Channel Permissions
        const VIEW_CHANNEL = 1 << 0;
        const SEND_MESSAGES = 1 << 1;
        const MANAGE_MESSAGES = 1 << 2;
        const READ_HISTORY = 1 << 3;
        
        // Space Permissions
        const MANAGE_CHANNELS = 1 << 10;
        const MANAGE_ROLES = 1 << 11;
        const MANAGE_MEMBERS = 1 << 12;
        const KICK_MEMBERS = 1 << 13;
        const BAN_MEMBERS = 1 << 14;
        
        // Advanced
        const ADMINISTRATOR = 1 << 30; // All permissions
    }
}
```

---

## Key Distribution & Security

### Channel Join Flow

**User Joins Space → Auto-joins Public Channels**:

```
1. User accepts Space invite
2. User becomes Space Member with default role
3. For each public channel:
   a. User added to channel's member list
   b. User added to channel's MLS group
   c. MLS group generates Welcome message
   d. Welcome message contains:
      - Group keys (symmetric encryption)
      - Group exporter secret (sealed sender)
      - Current epoch number
   e. User can now decrypt all messages
```

**MLS Welcome Message Contents**:
```rust
struct WelcomeMessage {
    group_id: GroupId,
    epoch: u64,
    group_secrets: GroupSecrets, // Symmetric keys
    group_exporter_secret: [u8; 32], // For sealed sender
    tree_hash: TreeHash, // Ratchet tree state
}
```

### Member Removal & Key Rotation

**User Kicked from Space**:

```
1. Admin kicks user from Space
2. For each channel user is a member of:
   a. User removed from channel's MLS group
   b. MLS group advances epoch (generates new keys)
   c. All remaining members get new keys
   d. Kicked user cannot decrypt new messages
3. Forward secrecy maintained
```

**Key Rotation Triggers**:
- Member removal (kick/ban)
- Member leaves voluntarily
- Scheduled rotation (optional, e.g., monthly)
- Suspected key compromise

### Private Channel Access

**User Invited to Private Channel**:

```
1. Admin invites user to #admin-only
2. System checks: Is user a Space member? 
   - If yes: Continue
   - If no: Auto-add to Space first
3. System checks: Does user have required role?
   - If yes: Continue
   - If no: Deny access
4. User added to channel's MLS group
5. User receives Welcome message
6. User can now see and participate in channel
```

---

## Scalability Considerations

### Member Count Targets

| Phase | Space Size | Channel Size | MLS Group Size | Notes |
|-------|-----------|-------------|----------------|-------|
| **MVP** | 10-100 members | 10-100 members | 10-100 members | Initial deployment |
| **Phase 2** | 100-500 members | 50-500 members | 50-500 members | Community growth |
| **Phase 3** | 500-1000+ members | 100-1000+ members | 100-1000+ members | Large communities |

### Optimization Strategies

**For Large Channels (1000+ members)**:

1. **Read-Only Channels** (Future Optimization):
   - Announcements-only channels
   - Only admins are MLS members
   - Regular members get group exporter secret (read-only)
   - Reduces MLS group size by 100x

2. **Lazy Channel Joining**:
   - Don't auto-join all public channels immediately
   - Join on first access (user clicks channel)
   - Reduces initial Welcome message overhead

3. **Channel Archival**:
   - Inactive channels can be archived
   - MLS group dissolved, messages stored encrypted
   - Re-activate channel creates new MLS group

4. **Batched Operations**:
   - Batch member additions (one epoch advancement)
   - Batch member removals (one epoch advancement)
   - Reduces MLS overhead

---

## Data Models

### Space

```rust
pub struct Space {
    pub id: SpaceId,
    pub name: String,
    pub description: Option<String>,
    pub icon_url: Option<String>,
    pub visibility: SpaceVisibility,
    pub owner_id: UserId,
    pub members: HashMap<UserId, SpaceMember>,
    pub channels: Vec<ChannelId>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

pub enum SpaceVisibility {
    Public,  // Listed in directory, anyone can join
    Private, // Invite-only, not listed
}

pub struct SpaceMember {
    pub user_id: UserId,
    pub role: SpaceRole,
    pub joined_at: Timestamp,
    pub invited_by: Option<UserId>,
}

pub enum SpaceRole {
    Owner,
    Admin,
    Member,
}
```

### Channel

```rust
pub struct Channel {
    pub id: ChannelId,
    pub space_id: SpaceId,
    pub name: String,
    pub description: Option<String>,
    pub visibility: ChannelVisibility,
    pub mls_group_id: GroupId,
    pub members: HashSet<UserId>,
    pub required_role: Option<SpaceRole>, // None = all, Some(Admin) = admins only
    pub created_at: Timestamp,
    pub last_message_at: Option<Timestamp>,
}

pub enum ChannelVisibility {
    Public,  // Visible to all Space members, auto-join
    Private, // Only visible to explicit members
}
```

### Space Invite

```rust
pub struct SpaceInvite {
    pub id: InviteId,
    pub space_id: SpaceId,
    pub created_by: UserId,
    pub invite_type: InviteType,
    pub max_uses: Option<usize>, // None = unlimited
    pub uses: usize,
    pub expires_at: Option<Timestamp>,
    pub created_at: Timestamp,
}

pub enum InviteType {
    Link(String),      // Permanent or one-time link
    Code(String),      // Short alphanumeric code (e.g., "abc-xyz")
    Direct(UserId),    // Sent to specific user
}
```

---

## API Operations

### Space Management

```rust
trait SpaceManager {
    // Create new Space
    async fn create_space(
        &self,
        name: String,
        visibility: SpaceVisibility,
        owner: UserId,
    ) -> Result<Space>;

    // Delete Space (and all channels)
    async fn delete_space(&self, space_id: SpaceId) -> Result<()>;

    // Update Space metadata
    async fn update_space(&self, space_id: SpaceId, updates: SpaceUpdate) -> Result<()>;

    // List public Spaces (for directory)
    async fn list_public_spaces(&self, filter: SearchFilter) -> Result<Vec<PublicSpaceInfo>>;
}
```

### Membership Management

```rust
trait MembershipManager {
    // Generate invite
    async fn create_invite(
        &self,
        space_id: SpaceId,
        invite_type: InviteType,
        max_uses: Option<usize>,
    ) -> Result<SpaceInvite>;

    // Join Space via invite
    async fn join_space(&self, invite: &SpaceInvite, user_id: UserId) -> Result<()>;

    // Leave Space
    async fn leave_space(&self, space_id: SpaceId, user_id: UserId) -> Result<()>;

    // Kick member
    async fn kick_member(&self, space_id: SpaceId, user_id: UserId) -> Result<()>;

    // Update member role
    async fn update_role(
        &self,
        space_id: SpaceId,
        user_id: UserId,
        new_role: SpaceRole,
    ) -> Result<()>;
}
```

### Channel Management

```rust
trait ChannelManager {
    // Create channel in Space
    async fn create_channel(
        &self,
        space_id: SpaceId,
        name: String,
        visibility: ChannelVisibility,
    ) -> Result<Channel>;

    // Delete channel
    async fn delete_channel(&self, channel_id: ChannelId) -> Result<()>;

    // Add member to private channel
    async fn add_channel_member(
        &self,
        channel_id: ChannelId,
        user_id: UserId,
    ) -> Result<()>;

    // Remove member from channel
    async fn remove_channel_member(
        &self,
        channel_id: ChannelId,
        user_id: UserId,
    ) -> Result<()>;
}
```

---

## Implementation Phases

### Phase 1: MVP (Current Sprint)

**Deliverables**:
- ✅ Basic Space CRUD operations
- ✅ Public/Private Space visibility
- ✅ Space invite system (links/codes)
- ✅ Channel creation (public only)
- ✅ One MLS group per channel
- ✅ Auto-join public channels on Space join
- ✅ Simple roles: Owner, Admin, Member

**Out of Scope**:
- Private channels
- Permission overrides
- Space directory/search
- Lazy channel joining

### Phase 2: Discovery & Privacy

**Deliverables**:
- Public Space directory
- Search & browse functionality
- Private channels
- Channel-level access control
- Invitation analytics

### Phase 3: Advanced Permissions

**Deliverables**:
- Moderator role
- Custom roles
- Per-channel permission overrides
- Permission templates

### Phase 4: Scale Optimizations

**Deliverables**:
- Lazy channel joining
- Read-only channels (non-MLS members)
- Batched MLS operations
- Channel archival

---

## Security Considerations

### Threat Model

**Protected Against**:
- ✅ Server cannot read messages (E2EE via MLS)
- ✅ Removed members cannot read new messages (forward secrecy)
- ✅ New members cannot read old messages (post-compromise security via epochs)
- ✅ Sender anonymity (sealed sender)
- ✅ Metadata privacy (encrypted channel names via metadata encryption)

**Not Protected Against** (By Design):
- ⚠️ Space metadata visible to server (name, member count)
- ⚠️ Channel existence visible to Space members
- ⚠️ Membership changes visible to server (for group operations)

### Privacy Guarantees

**Space Level**:
- Space name/description: Encrypted if private, public if public Space
- Member list: Visible to all Space members, not to non-members
- Invite links: One-time codes with rate limiting to prevent enumeration

**Channel Level**:
- Channel name: Encrypted in MLS metadata
- Message content: End-to-end encrypted via MLS
- Sender identity: Hidden via sealed sender (optional)

---

## Open Questions & Future Work

### Decisions Deferred to Future Phases

1. **DM Channels**: Separate concept from Spaces (not part of this spec)

2. **Bots/Webhooks**: Not implemented initially, future consideration

3. **Voice/Video**: Out of scope for Space/Channel architecture

4. **Threading**: Should threaded conversations share parent channel's MLS group?

5. **Cross-Space Channels**: Can a channel exist in multiple Spaces? (Likely no)

6. **Space Categories**: Should Spaces have subcategories/folders? (Discord-style)

---

## Appendix: Example Scenarios

### Scenario 1: Creating a Public Gaming Community

```
1. Alice creates Space "Gamers Unite"
   - Visibility: Public
   - Default channel: #general (public)

2. Alice creates additional channels:
   - #announcements (public)
   - #strategy (public)
   - #admin-chat (private, Admin role required)

3. Bob discovers Space in public directory
   - Sees: Name, description, member count
   - Joins immediately (no invite needed)
   - Auto-added to #general and #announcements
   - Cannot see #admin-chat (not admin)

4. Alice promotes Carol to Admin
   - Carol gains access to #admin-chat
   - Carol added to #admin-chat MLS group
   - Carol receives Welcome message
```

### Scenario 2: Private Team Space

```
1. Company creates Space "Engineering Team"
   - Visibility: Private
   - Channels: #general, #backend, #frontend

2. Manager generates invite link
   - Link: https://spacepanda.io/invite/abc-xyz-123
   - Max uses: 50
   - Expires: 7 days

3. New employee receives link
   - Clicks link → sees Space preview
   - Accepts → becomes Member
   - Auto-joins all public channels

4. Employee leaves company
   - Admin kicks from Space
   - Employee removed from all channels
   - All channels advance epoch (new keys)
   - Ex-employee cannot decrypt new messages
```

---

**End of Specification**
