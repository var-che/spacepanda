//! Space & Channel Management
//!
//! This module provides the core data structures and operations for Spaces and Channels,
//! implementing the architecture specified in `SPACE_VISIBILITY.md`.
//!
//! ## Architecture
//!
//! - **Space**: A container for channels (like Discord servers)
//! - **Channel**: Communication primitive with its own MLS group
//! - One MLS group per channel for optimal security and scalability
//!
//! ## Key Design Principles
//!
//! 1. Channel-scoped MLS groups (not Space-scoped)
//! 2. Auto-join public channels on Space membership
//! 3. Simple role model (Owner, Admin, Member)
//! 4. Scalable to 1000+ members per Space

pub mod channel;
pub mod invite;
pub mod manager;
pub mod space;
pub mod types;

pub use channel::{Channel, ChannelError, ChannelVisibility};
pub use invite::{InviteError, InviteType, SpaceInvite};
pub use manager::{ChannelManager, MembershipError, MembershipManager, SpaceManager};
pub use space::{Space, SpaceError, SpaceMember, SpaceRole, SpaceVisibility};
pub use types::{ChannelId, SpaceId};
