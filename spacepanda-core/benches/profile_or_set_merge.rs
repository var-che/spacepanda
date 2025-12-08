/// Profiling target for ORSet 3-way merge
/// Run with: cargo flamegraph --bench profile_or_set_merge
use spacepanda_core::core_store::crdt::or_set::{AddId, ORSet};
use spacepanda_core::core_store::crdt::traits::Crdt;
use spacepanda_core::core_store::crdt::vector_clock::VectorClock;

fn main() {
    let vc = VectorClock::new();

    // Run the 3-way merge scenario that showed lower throughput (200 elements/node)
    for _ in 0..1000 {
        let mut set1 = ORSet::<u64>::new();
        let mut set2 = ORSet::<u64>::new();
        let mut set3 = ORSet::<u64>::new();

        // Each set adds 200 unique elements
        for i in 0..200 {
            let add_id1 = AddId::new("node1".to_string(), i as u64);
            set1.add(i as u64, add_id1, vc.clone());

            let add_id2 = AddId::new("node2".to_string(), i as u64);
            set2.add((i + 200) as u64, add_id2, vc.clone());

            let add_id3 = AddId::new("node3".to_string(), i as u64);
            set3.add((i + 400) as u64, add_id3, vc.clone());
        }

        // Perform 3-way merge (this is the hot path we want to profile)
        let _ = set1.merge(&set2);
        let _ = set1.merge(&set3);
    }
}
