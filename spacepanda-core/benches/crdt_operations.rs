use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use spacepanda_core::core_store::crdt::lww_register::LWWRegister;
use spacepanda_core::core_store::crdt::vector_clock::VectorClock;
use std::time::{SystemTime, UNIX_EPOCH};

// Helper to get current timestamp
fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_micros() as u64
}

fn bench_lww_register_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("crdt_lww_creation");
    
    group.bench_function("new_empty", |b| {
        b.iter(|| {
            let register: LWWRegister<String> = LWWRegister::new();
            black_box(register)
        });
    });
    
    group.bench_function("with_value", |b| {
        b.iter(|| {
            let register = LWWRegister::with_value(
                "test_value".to_string(),
                "node_1".to_string()
            );
            black_box(register)
        });
    });
    
    // Batch creation
    for batch_size in [10, 100, 1_000].iter() {
        group.throughput(Throughput::Elements(*batch_size as u64));
        group.bench_with_input(BenchmarkId::new("batch", batch_size), batch_size, |b, &n| {
            b.iter(|| {
                let registers: Vec<LWWRegister<String>> = (0..n)
                    .map(|i| LWWRegister::with_value(
                        format!("value_{}", i),
                        format!("node_{}", i % 5)
                    ))
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
                    vc
                );
                black_box(register)
            },
            criterion::BatchSize::SmallInput
        );
    });
    
    // Multiple updates to same register
    for update_count in [10, 100, 1_000].iter() {
        group.throughput(Throughput::Elements(*update_count as u64));
        group.bench_with_input(BenchmarkId::new("sequential_updates", update_count), update_count, |b, &n| {
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
                            vc
                        );
                    }
                    black_box(register)
                },
                criterion::BatchSize::SmallInput
            );
        });
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
                register.set(
                    "old_value".to_string(),
                    1000,
                    "node_1".to_string(),
                    vc
                );
                register
            },
            |mut register| {
                let mut vc = VectorClock::new();
                vc.increment("node_2");
                register.set(
                    "new_value".to_string(),
                    2000,  // Later timestamp
                    "node_2".to_string(),
                    vc
                );
                black_box(register)
            },
            criterion::BatchSize::SmallInput
        );
    });
    
    // Test tiebreaker with same timestamp
    group.bench_function("node_id_tiebreaker", |b| {
        b.iter_batched(
            || {
                let mut register = LWWRegister::new();
                let mut vc = VectorClock::new();
                vc.increment("node_a");
                register.set(
                    "value_a".to_string(),
                    1000,
                    "node_a".to_string(),
                    vc
                );
                register
            },
            |mut register| {
                let mut vc = VectorClock::new();
                vc.increment("node_z");
                register.set(
                    "value_z".to_string(),
                    1000,  // Same timestamp
                    "node_z".to_string(),
                    vc
                );
                black_box(register)
            },
            criterion::BatchSize::SmallInput
        );
    });
    
    // Benchmark conflict resolution rate
    for conflict_count in [10, 100, 1_000].iter() {
        group.throughput(Throughput::Elements(*conflict_count as u64));
        group.bench_with_input(BenchmarkId::new("conflicts", conflict_count), conflict_count, |b, &n| {
            b.iter_batched(
                || LWWRegister::<String>::new(),
                |mut register| {
                    for i in 0..n {
                        let mut vc = VectorClock::new();
                        let node_id = format!("node_{}", i % 5);
                        vc.increment(&node_id);
                        register.set(
                            format!("value_{}", i),
                            1000 + (i % 100) as u64,  // Create timestamp collisions
                            node_id,
                            vc
                        );
                    }
                    black_box(register)
                },
                criterion::BatchSize::SmallInput
            );
        });
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
            criterion::BatchSize::SmallInput
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
                criterion::BatchSize::SmallInput
            );
        });
    }
    
    group.finish();
}

fn bench_lww_register_get(c: &mut Criterion) {
    let mut group = c.benchmark_group("crdt_lww_get");
    
    group.bench_function("get_value", |b| {
        let register = LWWRegister::with_value(
            "test_value".to_string(),
            "node_1".to_string()
        );
        
        b.iter(|| {
            let value = register.get();
            black_box(value)
        });
    });
    
    group.finish();
}

criterion_group!(
    benches,
    bench_lww_register_creation,
    bench_lww_register_set,
    bench_lww_register_conflict_resolution,
    bench_vector_clock_operations,
    bench_lww_register_get
);
criterion_main!(benches);

