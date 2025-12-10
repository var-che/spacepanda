//! Storage layer for Spaces and Channels
//!
//! Provides SQL-based persistence for the Space/Channel system.

pub mod migrations;
pub mod sql_store;

pub use migrations::{migrate, CURRENT_SPACE_SCHEMA_VERSION};
pub use sql_store::SpaceSqlStore;
