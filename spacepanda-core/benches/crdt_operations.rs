use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use serde_json::json;

fn bench_json_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("crdt_json_operations");
    
    // Benchmark JSON serialization for CRDT values
    for size in [10, 100, 1_000, 10_000].iter() {
        let value = "x".repeat(*size);
        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(BenchmarkId::new("serialize", size), &value, |b, val| {
            b.iter(|| {
                let json_val = json!({"value": val});
                let serialized = serde_json::to_vec(&json_val).unwrap();
                black_box(serialized)
            });
        });
    }
    
    group.finish();
}

fn bench_timestamp_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("crdt_timestamp");
    
    use std::time::{SystemTime, UNIX_EPOCH};
    
    group.bench_function("system_time", |b| {
        b.iter(|| {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_micros() as u64;
            black_box(timestamp)
        });
    });
    
    // Benchmark batch timestamp generation
    for batch_size in [10, 100, 1_000, 10_000].iter() {
        group.throughput(Throughput::Elements(*batch_size as u64));
        group.bench_with_input(BenchmarkId::new("batch", batch_size), batch_size, |b, &n| {
            b.iter(|| {
                let timestamps: Vec<u64> = (0..n)
                    .map(|_| {
                        SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap()
                            .as_micros() as u64
                    })
                    .collect();
                black_box(timestamps)
            });
        });
    }
    
    group.finish();
}

fn bench_hash_map_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("crdt_hashmap");
    
    use std::collections::HashMap;
    
    // Benchmark HashMap operations (used internally by CRDTs)
    for size in [100, 1_000, 10_000, 100_000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::new("insert", size), size, |b, &n| {
            b.iter(|| {
                let mut map = HashMap::new();
                for i in 0..n {
                    map.insert(format!("key_{}", i), i);
                }
                black_box(map)
            });
        });
    }
    
    group.finish();
}

criterion_group!(
    benches,
    bench_json_operations,
    bench_timestamp_generation,
    bench_hash_map_operations
);
criterion_main!(benches);

