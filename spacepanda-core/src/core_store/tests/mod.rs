/*
    Integration tests for core_store subsystem
    
    Test suite covering:
    - CRDT convergence and correctness
    - Model merging (Channel, Space, Role)
    - Storage persistence
    - Sync layer operations
    - Permission validation
    - Identity metadata
    - Edge cases and tricky scenarios
*/

pub mod crdt_tests;
pub mod model_tests;
pub mod convergence_tests;

// Edge case tests
pub mod lww_edge_cases;
pub mod orset_edge_cases;
pub mod ormap_edge_cases;
pub mod vector_clock_edge_cases;
pub mod replica_merge_edge_cases;

// Advanced edge case tests
pub mod vector_clock_advanced;
pub mod orset_advanced;
pub mod lww_advanced;
pub mod channel_full_merge;
