//! Core MVP Layer - Orchestration for E2E Encrypted Chat
//!
//! This module provides the high-level API for SpacePanda's MVP by coordinating
//! `core_identity`, `core_mls`, `core_store`, and `core_dht` subsystems.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────┐
//! │   UI/CLI    │
//! └──────┬──────┘
//!        │
//! ┌──────▼──────────────────────┐
//! │     core_mvp (this)         │
//! │  ChannelManager, Router     │
//! └──┬────┬────┬────┬──────────┘
//!    │    │    │    │
//!    ▼    ▼    ▼    ▼
//!  MLS  CRDT DHT Identity
//! ```
//!
//! # Quick Start
//!
//! ```ignore
//! use core_mvp::ChannelManager;
//!
//! // Create manager
//! let manager = ChannelManager::new(config, services).await?;
//!
//! // Create channel
//! let channel_id = manager.create_channel("general", false).await?;
//!
//! // Invite member
//! let welcome = manager.create_invite(&channel_id, &bob_key_package).await?;
//!
//! // Join channel (Bob's side)
//! let channel_id = manager.join_channel(&welcome, Some(&ratchet_tree)).await?;
//!
//! // Send message
//! let msg_id = manager.send_message(&channel_id, b"Hello!").await?;
//! ```
//!
//! # Modules
//!
//! - [`channel_manager`] - Main orchestrator for channel operations
//! - [`types`] - Core data types (ChannelDescriptor, ChatMessage, etc.)
//! - [`errors`] - Error types for MVP operations
//! - [`api`] - HTTP API server for testing (optional)

pub mod channel_manager;
pub mod errors;
pub mod types;

// Re-exports for convenience
pub use channel_manager::ChannelManager;
pub use errors::{MvpError, MvpResult};
pub use types::{ChannelDescriptor, ChatMessage, InviteToken};
