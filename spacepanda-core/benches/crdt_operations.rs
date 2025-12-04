use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use spacepanda_core::core_store::crdt::lww_register::LWWRegister;
use spacepanda_core::core_store::crdt::vector_clock::VectorClock;
use spacepanda_core::core_store::crdt::or_set::{ORSet, AddId};
use spacepanda_core::core_store::crdt::traits::Crdt;
use std::time::{SystemTime, UNIX_EPOCH};

mod bench_config;
use bench_config::{create_rng, BenchConfig};
use rand::Rng;

// Load or create benchmark configuration for reproducibility
fn get_bench_config() -> BenchConfig {
    let config_path = "target/bench_config.json";
    let mut config = BenchConfig::load_or_default(config_path);

    // Set benchmark-specific parameters
    config.set_param("benchmark_suite", "crdt_operations");
    config.set_param("criterion_version", "0.5");

    // Save for reference
    let _ = config.save(config_path);

    config
}

// Helper to get deterministic timestamp from RNG
fn deterministic_timestamp(rng: &mut rand::rngs::StdRng) -> u64 {
    // Use RNG to generate deterministic timestamps for benchmarks
    rng.gen_range(1_000_000..10_000_000)
}

// Helper to get current timestamp (fallback)
fn current_timestamp() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_micros() as u64
}

fn bench_lww_register_creation(c: &mut Criterion) {
    let config = get_bench_config();
    let _rng = create_rng(&config);
    let mut group = c.benchmark_group("crdt_lww_creation");

    group.bench_function("new_empty", |b| {
        b.iter(|| {
            let register: LWWRegister<String> = LWWRegister::new();
            black_box(register)
        });
    });

    group.bench_function("with_value", |b| {
        b.iter(|| {
            let register = LWWRegister::with_value("test_value".to_string(), "node_1".to_string());
            black_box(register)
        });
    });

    // Batch creation
    for batch_size in [10, 100, 1_000].iter() {
        group.throughput(Throughput::Elements(*batch_size as u64));
        group.bench_with_input(BenchmarkId::new("batch", batch_size), batch_size, |b, &n| {
            b.iter(|| {
                let registers: Vec<LWWRegister<String>> = (0..n)
                    .map(|i| {
                        LWWRegister::with_value(format!("value_{}", i), format!("node_{}", i % 5))
                    })
                    .collect();
                black_box(registers)
            });
        });
    }

    group.finish();
}

fn bench_lww_register_set(c: &mut Criterion) {
    let mut group = c.benchmark_group("crdt_lww_set");

    group.bench_function("single_set", |b| {
        b.iter_batched(
            || LWWRegister::<String>::new(),
            |mut register| {
                let mut vc = VectorClock::new();
                vc.increment("node_1");
                register.set(
                    "new_value".to_string(),
                    current_timestamp(),
                    "node_1".to_string(),
                    vc,
                );
                black_box(register)
            },
            criterion::BatchSize::SmallInput,
        );
    });

    // Multiple updates to same register
    for update_count in [10, 100, 1_000].iter() {
        group.throughput(Throughput::Elements(*update_count as u64));
        group.bench_with_input(
            BenchmarkId::new("sequential_updates", update_count),
            update_count,
            |b, &n| {
                b.iter_batched(
                    || LWWRegister::<String>::new(),
                    |mut register| {
                        for i in 0..n {
                            let mut vc = VectorClock::new();
                            vc.increment("node_1");
                            register.set(
                                format!("value_{}", i),
                                current_timestamp() + i as u64,
                                "node_1".to_string(),
                                vc,
                            );
                        }
                        black_box(register)
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

fn bench_lww_register_conflict_resolution(c: &mut Criterion) {
    let mut group = c.benchmark_group("crdt_lww_conflicts");

    // Test conflict resolution with different timestamps
    group.bench_function("timestamp_wins", |b| {
        b.iter_batched(
            || {
                let mut register = LWWRegister::new();
                let mut vc = VectorClock::new();
                vc.increment("node_1");
                register.set("old_value".to_string(), 1000, "node_1".to_string(), vc);
                register
            },
            |mut register| {
                let mut vc = VectorClock::new();
                vc.increment("node_2");
                register.set(
                    "new_value".to_string(),
                    2000, // Later timestamp
                    "node_2".to_string(),
                    vc,
                );
                black_box(register)
            },
            criterion::BatchSize::SmallInput,
        );
    });

    // Test tiebreaker with same timestamp
    group.bench_function("node_id_tiebreaker", |b| {
        b.iter_batched(
            || {
                let mut register = LWWRegister::new();
                let mut vc = VectorClock::new();
                vc.increment("node_a");
                register.set("value_a".to_string(), 1000, "node_a".to_string(), vc);
                register
            },
            |mut register| {
                let mut vc = VectorClock::new();
                vc.increment("node_z");
                register.set(
                    "value_z".to_string(),
                    1000, // Same timestamp
                    "node_z".to_string(),
                    vc,
                );
                black_box(register)
            },
            criterion::BatchSize::SmallInput,
        );
    });

    // Benchmark conflict resolution rate
    for conflict_count in [10, 100, 1_000].iter() {
        group.throughput(Throughput::Elements(*conflict_count as u64));
        group.bench_with_input(
            BenchmarkId::new("conflicts", conflict_count),
            conflict_count,
            |b, &n| {
                b.iter_batched(
                    || LWWRegister::<String>::new(),
                    |mut register| {
                        for i in 0..n {
                            let mut vc = VectorClock::new();
                            let node_id = format!("node_{}", i % 5);
                            vc.increment(&node_id);
                            register.set(
                                format!("value_{}", i),
                                1000 + (i % 100) as u64, // Create timestamp collisions
                                node_id,
                                vc,
                            );
                        }
                        black_box(register)
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

fn bench_vector_clock_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("crdt_vector_clock");

    group.bench_function("new", |b| {
        b.iter(|| {
            let vc = VectorClock::new();
            black_box(vc)
        });
    });

    group.bench_function("increment", |b| {
        b.iter_batched(
            || VectorClock::new(),
            |mut vc| {
                vc.increment("node_1");
                black_box(vc)
            },
            criterion::BatchSize::SmallInput,
        );
    });

    // Benchmark merge operations
    for node_count in [2, 5, 10, 20].iter() {
        group.bench_with_input(BenchmarkId::new("merge", node_count), node_count, |b, &n| {
            b.iter_batched(
                || {
                    let mut vc1 = VectorClock::new();
                    let mut vc2 = VectorClock::new();
                    for i in 0..n {
                        vc1.increment(&format!("node_{}", i));
                        vc2.increment(&format!("node_{}", i));
                    }
                    (vc1, vc2)
                },
                |(mut vc1, vc2)| {
                    vc1.merge(&vc2);
                    black_box(vc1)
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

fn bench_lww_register_get(c: &mut Criterion) {
    let mut group = c.benchmark_group("crdt_lww_get");

    group.bench_function("get_value", |b| {
        let register = LWWRegister::with_value("test_value".to_string(), "node_1".to_string());

        b.iter(|| {
            let value = register.get();
            black_box(value)
        });
    });

    group.finish();
}

fn bench_or_set_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("crdt_or_set");
    let vc = VectorClock::new();

    // Benchmark single add operation
    group.bench_function("add_single", |b| {
        let mut counter = 0u64;
        b.iter(|| {
            let mut set = ORSet::<u64>::new();
            let add_id = AddId::new("node1".to_string(), counter);
            counter += 1;
            set.add(42, add_id, vc.clone());
            black_box(set)
        });
    });

    // Benchmark batch adds at different scales
    for size in [10, 100, 1000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::new("add_batch", size), size, |b, &size| {
            b.iter(|| {
                let mut set = ORSet::<u64>::new();
                for i in 0..size {
                    let add_id = AddId::new("node1".to_string(), i as u64);
                    set.add(i as u64, add_id, vc.clone());
                }
                black_box(set)
            });
        });
    }

    // Benchmark contains operation
    group.bench_function("contains", |b| {
        let mut set = ORSet::<u64>::new();
        for i in 0..100 {
            let add_id = AddId::new("node1".to_string(), i);
            set.add(i, add_id, vc.clone());
        }
        
        b.iter(|| {
            let contains = set.contains(&50);
            black_box(contains)
        });
    });

    // Benchmark remove operation
    group.bench_function("remove", |b| {
        b.iter_batched(
            || {
                let mut set = ORSet::<u64>::new();
                for i in 0..100 {
                    let add_id = AddId::new("node1".to_string(), i);
                    set.add(i, add_id, vc.clone());
                }
                set
            },
            |mut set| {
                set.remove(&50, vc.clone());
                black_box(set)
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.finish();
}

fn bench_or_set_merge(c: &mut Criterion) {
    let mut group = c.benchmark_group("crdt_or_set_merge");
    let vc = VectorClock::new();

    // Benchmark merge at different scales
    for size in [10, 100, 500, 1000].iter() {
        group.throughput(Throughput::Elements(*size as u64 * 2));
        group.bench_with_input(BenchmarkId::new("merge", size), size, |b, &size| {
            b.iter_batched(
                || {
                    let mut set1 = ORSet::<u64>::new();
                    let mut set2 = ORSet::<u64>::new();
                    
                    // Set 1: elements 0 to size
                    for i in 0..size {
                        let add_id = AddId::new("node1".to_string(), i as u64);
                        set1.add(i as u64, add_id, vc.clone());
                    }
                    
                    // Set 2: elements size/2 to size*3/2 (50% overlap)
                    for i in (size/2)..(size * 3 / 2) {
                        let add_id = AddId::new("node2".to_string(), i as u64);
                        set2.add(i as u64, add_id, vc.clone());
                    }
                    
                    (set1, set2)
                },
                |(mut set1, set2)| {
                    let _ = set1.merge(&set2);
                    black_box(set1)
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

fn bench_or_set_convergence(c: &mut Criterion) {
    let mut group = c.benchmark_group("crdt_or_set_convergence");
    let vc = VectorClock::new();

    // Benchmark convergence scenario (3 nodes merging)
    for size in [50, 100, 200].iter() {
        group.throughput(Throughput::Elements(*size as u64 * 3));
        group.bench_with_input(
            BenchmarkId::new("three_way_merge", size),
            size,
            |b, &size| {
                b.iter_batched(
                    || {
                        let mut set1 = ORSet::<u64>::new();
                        let mut set2 = ORSet::<u64>::new();
                        let mut set3 = ORSet::<u64>::new();
                        
                        // Each set adds unique elements
                        for i in 0..size {
                            let add_id1 = AddId::new("node1".to_string(), i as u64);
                            set1.add(i as u64, add_id1, vc.clone());
                            
                            let add_id2 = AddId::new("node2".to_string(), i as u64);
                            set2.add((i + size) as u64, add_id2, vc.clone());
                            
                            let add_id3 = AddId::new("node3".to_string(), i as u64);
                            set3.add((i + size * 2) as u64, add_id3, vc.clone());
                        }
                        
                        (set1, set2, set3)
                    },
                    |(mut set1, set2, set3)| {
                        let _ = set1.merge(&set2);
                        let _ = set1.merge(&set3);
                        black_box(set1)
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_lww_register_creation,
    bench_lww_register_set,
    bench_lww_register_conflict_resolution,
    bench_vector_clock_operations,
    bench_lww_register_get,
    bench_or_set_operations,
    bench_or_set_merge,
    bench_or_set_convergence
);
criterion_main!(benches);
