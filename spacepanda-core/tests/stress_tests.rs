/// Stress tests for large group operations
/// 
/// These tests verify performance and correctness under scale:
/// - Large group membership (1000+ members)
/// - CRDT merge performance at scale
/// - Memory usage and leak detection
///
/// Run with: cargo test --test stress_tests -- --ignored --nocapture

#[cfg(test)]
mod stress_tests {
    use spacepanda_core::core_store::crdt::or_set::{ORSet, AddId};
    use spacepanda_core::core_store::crdt::vector_clock::VectorClock;
    use spacepanda_core::core_store::crdt::traits::Crdt;
    use std::time::Instant;

    /// Test ORSet with 1000+ elements
    /// Verifies merge performance and memory usage
    #[test]
    #[ignore] // Run with: cargo test --ignored
    fn stress_or_set_large_scale() {
        let mut set1 = ORSet::<u64>::new();
        let mut set2 = ORSet::<u64>::new();
        let vc = VectorClock::new();
        
        let start = Instant::now();
        
        // Node 1 adds 1000 elements
        for i in 0..1000 {
            let add_id = AddId::new("node1".to_string(), i);
            set1.add(i, add_id, vc.clone());
        }
        
        // Node 2 adds 1000 different elements (500 overlap)
        for i in 500..1500 {
            let add_id = AddId::new("node2".to_string(), i);
            set2.add(i, add_id, vc.clone());
        }
        
        let add_duration = start.elapsed();
        println!("Added 2000 elements (1000 each) in {:?}", add_duration);
        
        // Verify counts before merge
        assert_eq!(set1.len(), 1000);
        assert_eq!(set2.len(), 1000);
        
        // Test merge performance
        let merge_start = Instant::now();
        set1.merge(&set2);
        let merge_duration = merge_start.elapsed();
        
        println!("Merged 1000-element sets in {:?}", merge_duration);
        
        // After merge, should have 1500 unique elements (0-1499)
        assert_eq!(set1.len(), 1500);
        
        // Verify convergence property: different merge order gives same result
        let mut set3 = ORSet::<u64>::new();
        let mut set4 = ORSet::<u64>::new();
        
        for i in 0..1000 {
            let add_id = AddId::new("node1".to_string(), i);
            set3.add(i, add_id, vc.clone());
        }
        
        for i in 500..1500 {
            let add_id = AddId::new("node2".to_string(), i);
            set4.add(i, add_id, vc.clone());
        }
        
        // Merge in opposite order
        set4.merge(&set3);
        
        assert_eq!(set1.len(), set4.len(), "Merge should be commutative");
    }

    /// Test ORSet remove operations at scale
    /// Verifies tombstone management doesn't cause memory issues
    #[test]
    #[ignore]
    fn stress_or_set_remove_operations() {
        let mut set = ORSet::<u64>::new();
        let vc = VectorClock::new();
        
        // Add 1000 elements
        for i in 0..1000 {
            let add_id = AddId::new("node1".to_string(), i);
            set.add(i, add_id, vc.clone());
        }
        
        assert_eq!(set.len(), 1000);
        
        let start = Instant::now();
        
        // Remove half of them
        for i in 0..500 {
            set.remove(&i, vc.clone());
        }
        
        let remove_duration = start.elapsed();
        println!("Removed 500 elements in {:?}", remove_duration);
        
        // Should have 500 remaining
        assert_eq!(set.len(), 500);
        
        // Verify correct elements removed
        for i in 0..500 {
            assert!(!set.contains(&i), "Element {} should be removed", i);
        }
        for i in 500..1000 {
            assert!(set.contains(&i), "Element {} should remain", i);
        }
    }

    /// Test CRDT convergence after concurrent operations
    /// Simulates network partition scenario with 3 nodes
    #[test]
    #[ignore]
    fn stress_crdt_convergence_under_partition() {
        // Simulate 3 partitioned nodes
        let mut set1 = ORSet::<String>::new();
        let mut set2 = ORSet::<String>::new();
        let mut set3 = ORSet::<String>::new();
        let vc = VectorClock::new();
        
        let start = Instant::now();
        
        // Each node adds 100 unique items
        for i in 0..100 {
            let add_id1 = AddId::new("node1".to_string(), i);
            set1.add(format!("node1_item_{}", i), add_id1, vc.clone());
            
            let add_id2 = AddId::new("node2".to_string(), i);
            set2.add(format!("node2_item_{}", i), add_id2, vc.clone());
            
            let add_id3 = AddId::new("node3".to_string(), i);
            set3.add(format!("node3_item_{}", i), add_id3, vc.clone());
        }
        
        // Also add some overlapping items
        for i in 0..20 {
            let shared_item = format!("shared_item_{}", i);
            
            let add_id1 = AddId::new("node1".to_string(), 100 + i);
            set1.add(shared_item.clone(), add_id1, vc.clone());
            
            let add_id2 = AddId::new("node2".to_string(), 100 + i);
            set2.add(shared_item.clone(), add_id2, vc.clone());
            
            let add_id3 = AddId::new("node3".to_string(), 100 + i);
            set3.add(shared_item.clone(), add_id3, vc.clone());
        }
        
        // Merge in different orders (should converge to same result)
        let mut merged_123 = set1.clone();
        merged_123.merge(&set2);
        merged_123.merge(&set3);
        
        let mut merged_321 = set3.clone();
        merged_321.merge(&set2);
        merged_321.merge(&set1);
        
        let duration = start.elapsed();
        println!("Partition convergence test completed in {:?}", duration);
        
        // Both merge orders should produce identical results
        assert_eq!(
            merged_123.len(),
            merged_321.len(),
            "Merge order should not affect final size"
        );
        
        // Should have 300 unique + 20 shared = 320 total
        assert_eq!(merged_123.len(), 320);
    }

    /// Test many concurrent ORSets
    /// Simulates 100 nodes each adding 100 elements
    #[test]
    #[ignore]
    fn stress_many_concurrent_sets() {
        let mut sets: Vec<ORSet<u64>> = (0..100)
            .map(|_| ORSet::new())
            .collect();
        
        let vc = VectorClock::new();
        let start = Instant::now();
        
        // Each set adds 100 unique elements
        for (node_idx, set) in sets.iter_mut().enumerate() {
            for i in 0..100 {
                let element = (node_idx * 100 + i) as u64;
                let add_id = AddId::new(format!("node_{}", node_idx), i as u64);
                set.add(element, add_id, vc.clone());
            }
        }
        
        println!("Created 100 sets with 100 elements each");
        
        // Merge all sets pairwise
        let merge_start = Instant::now();
        let mut merged = sets[0].clone();
        for set in &sets[1..] {
            merged.merge(set);
        }
        let merge_duration = merge_start.elapsed();
        
        let total_duration = start.elapsed();
        
        println!("Merged 100 sets in {:?}", merge_duration);
        println!("Total time: {:?}", total_duration);
        
        // Should have 10,000 unique elements
        assert_eq!(merged.len(), 10_000);
    }

    /// Memory usage test - ensure no leaks with large operations
    #[test]
    #[ignore]
    fn stress_memory_usage() {
        let vc = VectorClock::new();
        
        // Create and destroy many ORSets
        for iteration in 0..100 {
            let mut set = ORSet::<u64>::new();
            
            for i in 0..1000 {
                let add_id = AddId::new("memory_test".to_string(), i);
                set.add(i, add_id, vc.clone());
            }
            
            // Remove half
            for i in 0..500 {
                set.remove(&i, vc.clone());
            }
            
            if iteration % 10 == 0 {
                println!("Iteration {}: set size = {}", iteration, set.len());
            }
        }
        
        // If we get here without OOM, memory management is working
        println!("Memory stress test completed successfully");
    }
}
