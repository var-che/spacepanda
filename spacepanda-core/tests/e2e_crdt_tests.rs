/// End-to-end CRDT integration tests
/// 
/// These tests verify complete CRDT flows across scenarios:
/// - Multi-node synchronization
/// - Conflict resolution
/// - Eventual consistency
/// - Network partition healing
///
/// Run with: cargo test --test e2e_crdt_tests

#[cfg(test)]
mod e2e_crdt_tests {
    use spacepanda_core::core_store::crdt::or_set::{ORSet, AddId};
    use spacepanda_core::core_store::crdt::vector_clock::VectorClock;
    use spacepanda_core::core_store::crdt::traits::Crdt;
    use std::time::Duration;
    use tokio::time::sleep;

    /// Test multi-node CRDT synchronization
    /// Simulates 3 nodes making concurrent changes and synchronizing
    #[tokio::test]
    async fn test_multi_node_sync() {
        // Create 3 nodes
        let mut node1 = ORSet::<String>::new();
        let mut node2 = ORSet::<String>::new();
        let mut node3 = ORSet::<String>::new();
        let vc = VectorClock::new();

        // Node 1 adds messages
        for i in 0..10 {
            let add_id = AddId::new("node1".to_string(), i);
            let _ = node1.add(format!("msg_from_node1_{}", i), add_id, vc.clone());
        }

        // Node 2 adds messages
        for i in 0..10 {
            let add_id = AddId::new("node2".to_string(), i);
            let _ = node2.add(format!("msg_from_node2_{}", i), add_id, vc.clone());
        }

        // Node 3 adds messages
        for i in 0..10 {
            let add_id = AddId::new("node3".to_string(), i);
            let _ = node3.add(format!("msg_from_node3_{}", i), add_id, vc.clone());
        }

        // Simulate network delay
        sleep(Duration::from_millis(10)).await;

        // Synchronize: Node 1 receives updates from Node 2 and Node 3
        let _ = node1.merge(&node2);
        let _ = node1.merge(&node3);

        // Synchronize: Node 2 receives updates from Node 1 and Node 3
        let _ = node2.merge(&node1);
        let _ = node2.merge(&node3);

        // Synchronize: Node 3 receives updates from Node 1 and Node 2
        let _ = node3.merge(&node1);
        let _ = node3.merge(&node2);

        // All nodes should converge to the same state
        assert_eq!(node1.len(), 30, "Node 1 should have all 30 messages");
        assert_eq!(node2.len(), 30, "Node 2 should have all 30 messages");
        assert_eq!(node3.len(), 30, "Node 3 should have all 30 messages");

        // Verify specific messages
        assert!(node1.contains(&"msg_from_node2_5".to_string()));
        assert!(node2.contains(&"msg_from_node3_7".to_string()));
        assert!(node3.contains(&"msg_from_node1_9".to_string()));
    }

    /// Test CRDT with conflicting operations
    /// Two nodes add and remove same element concurrently
    #[tokio::test]
    async fn test_conflicting_operations() {
        let mut node1 = ORSet::<String>::new();
        let mut node2 = ORSet::<String>::new();
        let vc = VectorClock::new();

        let element = "conflicted_element".to_string();

        // Node 1: Add element
        let add_id1 = AddId::new("node1".to_string(), 1);
        let _ = node1.add(element.clone(), add_id1, vc.clone());

        // Node 2: Add same element (concurrent)
        let add_id2 = AddId::new("node2".to_string(), 1);
        let _ = node2.add(element.clone(), add_id2, vc.clone());

        // Node 1: Remove element (only knows about its own add)
        let _ = node1.remove(&element, vc.clone());

        // Before sync:
        // - Node 1 doesn't have element (removed it)
        // - Node 2 has element (added it)
        assert!(!node1.contains(&element));
        assert!(node2.contains(&element));

        // Synchronize
        let _ = node1.merge(&node2);
        let _ = node2.merge(&node1);

        // After sync: Element should exist because Node 2's add
        // was concurrent with Node 1's remove
        // (add-wins semantics for concurrent ops)
        assert!(node1.contains(&element), "Element should exist after merge (add-wins)");
        assert!(node2.contains(&element), "Element should exist after merge (add-wins)");
    }

    /// Test eventual consistency
    /// Network partitions heal and nodes converge
    #[tokio::test]
    async fn test_eventual_consistency() {
        // Create 4 nodes
        let mut node0 = ORSet::<u64>::new();
        let mut node1 = ORSet::<u64>::new();
        let mut node2 = ORSet::<u64>::new();
        let mut node3 = ORSet::<u64>::new();
        let vc = VectorClock::new();

        // Phase 1: Each node adds unique elements
        for i in 0..5 {
            let add_id0 = AddId::new("node0".to_string(), i);
            let _ = node0.add(i, add_id0, vc.clone());

            let add_id1 = AddId::new("node1".to_string(), i);
            let _ = node1.add(i + 10, add_id1, vc.clone());

            let add_id2 = AddId::new("node2".to_string(), i);
            let _ = node2.add(i + 20, add_id2, vc.clone());

            let add_id3 = AddId::new("node3".to_string(), i);
            let _ = node3.add(i + 30, add_id3, vc.clone());
        }

        // Simulate network partition: nodes 0-1 can sync, nodes 2-3 can sync
        let _ = node0.merge(&node1);
        let _ = node1.merge(&node0);
        
        let _ = node2.merge(&node3);
        let _ = node3.merge(&node2);

        // After partition sync:
        assert_eq!(node0.len(), 10); // 0-1 partition has 10 elements
        assert_eq!(node1.len(), 10);
        assert_eq!(node2.len(), 10); // 2-3 partition has 10 elements
        assert_eq!(node3.len(), 10);

        // Phase 2: Partition heals - all nodes sync
        let _ = node0.merge(&node2);
        let _ = node0.merge(&node3);
        
        let _ = node1.merge(&node2);
        let _ = node1.merge(&node3);
        
        let _ = node2.merge(&node0);
        let _ = node2.merge(&node1);
        
        let _ = node3.merge(&node0);
        let _ = node3.merge(&node1);

        // All nodes should converge to 20 elements
        assert_eq!(
            node0.len(),
            20,
            "Node 0 should have all 20 elements after partition heals"
        );
        assert_eq!(
            node1.len(),
            20,
            "Node 1 should have all 20 elements after partition heals"
        );
        assert_eq!(
            node2.len(),
            20,
            "Node 2 should have all 20 elements after partition heals"
        );
        assert_eq!(
            node3.len(),
            20,
            "Node 3 should have all 20 elements after partition heals"
        );
    }

    /// Test concurrent add/remove waves
    /// Simulates chat with rapid message adds and deletes
    #[tokio::test]
    async fn test_concurrent_waves() {
        let mut node1 = ORSet::<String>::new();
        let mut node2 = ORSet::<String>::new();
        let vc = VectorClock::new();

        // Wave 1: Both nodes add messages
        for i in 0..5 {
            let add_id1 = AddId::new("node1".to_string(), i);
            let _ = node1.add(format!("msg_{}", i), add_id1, vc.clone());

            let add_id2 = AddId::new("node2".to_string(), i);
            let _ = node2.add(format!("msg_{}", i), add_id2, vc.clone());
        }

        // Wave 2: Node 1 removes some messages
        for i in 0..3 {
            let _ = node1.remove(&format!("msg_{}", i), vc.clone());
        }

        // Sync
        let _ = node1.merge(&node2);
        let _ = node2.merge(&node1);

        // After sync: Messages 0-2 should exist (Node 2's concurrent adds win)
        // Messages 3-4 should exist (not removed)
        assert_eq!(node1.len(), 5);
        assert_eq!(node2.len(), 5);

        for i in 0..5 {
            let msg = format!("msg_{}", i);
            assert!(node1.contains(&msg), "Message {} should exist", i);
            assert!(node2.contains(&msg), "Message {} should exist", i);
        }
    }

    /// Test idempotent operations
    /// Applying same operation multiple times has same effect
    #[tokio::test]
    async fn test_idempotent_operations() {
        let mut node = ORSet::<String>::new();
        let vc = VectorClock::new();

        let element = "test_element".to_string();
        let add_id = AddId::new("node1".to_string(), 1);

        // Add element once
        let _ = node.add(element.clone(), add_id.clone(), vc.clone());
        assert_eq!(node.len(), 1);

        // Add same element with same add_id again (idempotent)
        let _ = node.add(element.clone(), add_id.clone(), vc.clone());
        assert_eq!(node.len(), 1, "Duplicate add should be idempotent");

        // Verify element is still present
        assert!(node.contains(&element));
    }

    /// Test merge is commutative
    /// A.merge(B) should equal B.merge(A)
    #[tokio::test]
    async fn test_merge_commutativity() {
        let mut set_a = ORSet::<u64>::new();
        let mut set_b = ORSet::<u64>::new();
        let vc = VectorClock::new();

        // Set A adds elements 0-9
        for i in 0..10 {
            let add_id = AddId::new("nodeA".to_string(), i);
            let _ = set_a.add(i, add_id, vc.clone());
        }

        // Set B adds elements 5-14
        for i in 5..15 {
            let add_id = AddId::new("nodeB".to_string(), i);
            let _ = set_b.add(i, add_id, vc.clone());
        }

        // Create copies for different merge orders
        let mut ab = set_a.clone();
        let _ = ab.merge(&set_b);

        let mut ba = set_b.clone();
        let _ = ba.merge(&set_a);

        // Both should have same size (15 elements: 0-14)
        assert_eq!(ab.len(), ba.len(), "Merge should be commutative");
        assert_eq!(ab.len(), 15);
    }

    /// Test associativity of merge
    /// (A.merge(B)).merge(C) should equal A.merge(B.merge(C))
    #[tokio::test]
    async fn test_merge_associativity() {
        let mut set_a = ORSet::<u64>::new();
        let mut set_b = ORSet::<u64>::new();
        let mut set_c = ORSet::<u64>::new();
        let vc = VectorClock::new();

        // Each set adds unique elements
        for i in 0..5 {
            let add_id_a = AddId::new("nodeA".to_string(), i);
            let _ = set_a.add(i, add_id_a, vc.clone());

            let add_id_b = AddId::new("nodeB".to_string(), i);
            let _ = set_b.add(i + 5, add_id_b, vc.clone());

            let add_id_c = AddId::new("nodeC".to_string(), i);
            let _ = set_c.add(i + 10, add_id_c, vc.clone());
        }

        // Test (A ∪ B) ∪ C
        let mut abc_left = set_a.clone();
        let _ = abc_left.merge(&set_b);
        let _ = abc_left.merge(&set_c);

        // Test A ∪ (B ∪ C)
        let mut bc = set_b.clone();
        let _ = bc.merge(&set_c);
        let mut abc_right = set_a.clone();
        let _ = abc_right.merge(&bc);

        // Should have same result
        assert_eq!(
            abc_left.len(),
            abc_right.len(),
            "Merge should be associative"
        );
        assert_eq!(abc_left.len(), 15);
    }

    /// Test graceful degradation
    /// Ensures system can handle continued operation after sync
    #[tokio::test]
    async fn test_graceful_degradation() {
        let mut node1 = ORSet::<String>::new();
        let mut node2 = ORSet::<String>::new();
        let vc = VectorClock::new();

        // Node 1 adds elements
        for i in 0..10 {
            let add_id = AddId::new("node1".to_string(), i);
            let _ = node1.add(format!("msg_{}", i), add_id, vc.clone());
        }

        // Simulate successful sync
        let _ = node2.merge(&node1);
        assert_eq!(node2.len(), 10);

        // Both nodes continue operating independently
        let add_id1 = AddId::new("node1".to_string(), 10);
        let _ = node1.add("new_msg_1".to_string(), add_id1, vc.clone());

        let add_id2 = AddId::new("node2".to_string(), 10);
        let _ = node2.add("new_msg_2".to_string(), add_id2, vc.clone());

        // Final sync
        let _ = node1.merge(&node2);
        let _ = node2.merge(&node1);

        // Should have all messages
        assert_eq!(node1.len(), 12);
        assert_eq!(node2.len(), 12);
    }
}
