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

// Mission-critical tests before MLS integration
mod crypto_sanity_tests;
mod crdt_mission_critical_tests;

// Production-grade TDD tests (real cryptography)
mod identity_crypto_tests;

// Test helpers and fixtures
pub mod helpers;
