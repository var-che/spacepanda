//! HTTP Test Harness for SpacePanda MLS
//!
//! This module provides an HTTP API for testing the MLS implementation.
//! It's useful for manual testing, integration tests, and demonstrations.

pub mod api;
pub mod handlers;
pub mod server;
pub mod state;
pub mod types;

// Re-export the start_server function for convenience
pub use server::start_server;
