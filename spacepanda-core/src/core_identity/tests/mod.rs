//! Core identity test suite
//!
//! Organized into unit tests, integration tests, security tests, edge cases,
//! CRDT algebraic laws, multi-replica tests, and adversarial security tests

mod unit_tests;
mod integration_tests;
mod security_tests;
mod crdt_edge_cases;
mod identity_edge_cases;
mod crdt_laws;
mod crdt_replica_tests;
mod adversarial_tests;
mod crdt_advanced_tests;

// Test helpers and fixtures
pub mod helpers;
