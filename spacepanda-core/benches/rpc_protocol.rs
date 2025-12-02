use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use spacepanda_core::core_router::rpc_protocol::RpcProtocol;
use spacepanda_core::core_router::session_manager::SessionCommand;
use serde_json::json;
use std::time::Duration;
use tokio::runtime::Runtime;
use tokio::sync::mpsc;

fn bench_protocol_initialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("rpc_protocol_init");
    
    let rt = Runtime::new().unwrap();
    
    // Benchmark protocol creation with default settings
    group.bench_function("default_config", |b| {
        b.iter(|| {
            rt.block_on(async {
                let (session_tx, _session_rx) = mpsc::channel::<SessionCommand>(100);
                black_box(RpcProtocol::new(session_tx))
            })
        });
    });
    
    // Benchmark protocol creation with custom config
    group.bench_function("custom_config", |b| {
        b.iter(|| {
            rt.block_on(async {
                let (session_tx, _session_rx) = mpsc::channel::<SessionCommand>(100);
                black_box(RpcProtocol::new_with_config(
                    session_tx,
                    Duration::from_secs(30),
                    100_000,
                ))
            })
        });
    });
    
    group.finish();
}

fn bench_seen_requests_tracking(c: &mut Criterion) {
    let mut group = c.benchmark_group("rpc_protocol_lifecycle");
    
    let rt = Runtime::new().unwrap();
    
    // Benchmark creating and shutting down protocol instances
    for capacity in [1_000, 10_000, 50_000, 100_000].iter() {
        group.throughput(Throughput::Elements(*capacity as u64));
        group.bench_with_input(BenchmarkId::new("capacity", capacity), capacity, |b, &n| {
            b.iter(|| {
                rt.block_on(async {
                    let (session_tx, _session_rx) = mpsc::channel::<SessionCommand>(100);
                    let protocol = RpcProtocol::new_with_config(
                        session_tx,
                        Duration::from_secs(30),
                        n,
                    );
                    // No shutdown needed - no background task in LRU version
                    black_box(protocol)
                })
            });
        });
    }
    
    group.finish();
}

fn bench_json_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("rpc_json_serialization");
    
    // Benchmark JSON request serialization with varying payload sizes
    for size in [100, 1_000, 10_000, 50_000].iter() {
        let payload = "x".repeat(*size);
        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(BenchmarkId::new("payload_size", size), &payload, |b, payload| {
            b.iter(|| {
                let request = json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "method": "test_method",
                    "params": {"data": payload.clone()}
                });
                let serialized = serde_json::to_vec(&request).unwrap();
                black_box(serialized)
            });
        });
    }
    
    group.finish();
}

fn bench_json_deserialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("rpc_json_deserialization");
    
    // Benchmark JSON response deserialization with varying sizes
    for size in [100, 1_000, 10_000, 50_000].iter() {
        let payload = "x".repeat(*size);
        let request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "test_method",
            "params": {"data": payload}
        });
        let serialized = serde_json::to_vec(&request).unwrap();
        
        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(BenchmarkId::new("payload_size", size), &serialized, |b, data| {
            b.iter(|| {
                let deserialized: serde_json::Value = serde_json::from_slice(black_box(data)).unwrap();
                black_box(deserialized)
            });
        });
    }
    
    group.finish();
}

fn bench_frame_size_validation(c: &mut Criterion) {
    let mut group = c.benchmark_group("rpc_frame_validation");
    
    const MAX_FRAME_SIZE: usize = 64 * 1024;
    
    // Benchmark frame size checks
    for size in [1_024, 16_384, 32_768, 65_536, 131_072].iter() {
        let data = vec![0u8; *size];
        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(BenchmarkId::new("frame_size", size), &data, |b, data| {
            b.iter(|| {
                let is_valid = data.len() <= MAX_FRAME_SIZE;
                black_box(is_valid)
            });
        });
    }
    
    group.finish();
}

fn bench_protocol_drop(c: &mut Criterion) {
    let mut group = c.benchmark_group("rpc_protocol_drop");
    
    let rt = Runtime::new().unwrap();
    
    // Benchmark protocol drop (no background task to shutdown in LRU version)
    group.bench_function("drop", |b| {
        b.iter(|| {
            rt.block_on(async {
                let (session_tx, _session_rx) = mpsc::channel::<SessionCommand>(100);
                let protocol = RpcProtocol::new(session_tx);
                black_box(protocol)
                // Protocol drops here - LRU cache cleaned up automatically
            })
        });
    });
    
    group.finish();
}

criterion_group!(
    benches,
    bench_protocol_initialization,
    bench_seen_requests_tracking,
    bench_json_serialization,
    bench_json_deserialization,
    bench_frame_size_validation,
    bench_protocol_drop
);
criterion_main!(benches);

