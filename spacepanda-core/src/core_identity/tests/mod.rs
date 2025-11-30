//! Core identity test suite
//!
//! Organized into unit tests, integration tests, security tests, and edge cases

mod unit_tests;
mod integration_tests;
mod security_tests;
mod crdt_edge_cases;
mod identity_edge_cases;

// Test helpers and fixtures
pub mod helpers;
