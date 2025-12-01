use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use spacepanda_core::core_dht::{RoutingTable, DhtKey, PeerContact, DhtValue};
use std::net::SocketAddr;

fn bench_dht_key_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("dht_key_generation");
    
    // Benchmark single key generation
    group.bench_function("single_key", |b| {
        b.iter(|| {
            black_box(DhtKey::random())
        });
    });
    
    // Benchmark batch key generation
    for batch_size in [10, 100, 500, 1000].iter() {
        group.throughput(Throughput::Elements(*batch_size as u64));
        group.bench_with_input(BenchmarkId::new("batch_size", batch_size), batch_size, |b, &n| {
            b.iter(|| {
                let keys: Vec<DhtKey> = (0..n).map(|_| DhtKey::random()).collect();
                black_box(keys)
            });
        });
    }
    
    group.finish();
}

fn bench_routing_table_init(c: &mut Criterion) {
    let mut group = c.benchmark_group("dht_routing_table_init");
    
    // Benchmark routing table initialization with varying k values
    for k in [8, 16, 20, 32].iter() {
        group.bench_with_input(BenchmarkId::new("k_value", k), k, |b, &k_val| {
            b.iter(|| {
                let local_id = DhtKey::random();
                black_box(RoutingTable::new(local_id, k_val))
            });
        });
    }
    
    group.finish();
}

fn bench_routing_table_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("dht_routing_insert");
    
    // Benchmark insertions with varying table sizes
    for size in [10, 100, 500, 1000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::new("table_size", size), size, |b, &n| {
            b.iter(|| {
                let local_peer = DhtKey::random();
                let mut table = RoutingTable::new(local_peer.clone(), 20);
                
                for i in 0..n {
                    let peer_id = DhtKey::random();
                    let addr: SocketAddr = format!("127.0.0.1:{}", 10000 + (i % 55000)).parse().unwrap();
                    let contact = PeerContact::new(peer_id, addr);
                    table.insert(contact);
                }
                
                black_box(table)
            });
        });
    }
    
    group.finish();
}

fn bench_dht_value_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("dht_value_serialization");
    
    // Benchmark serializing DHT values of varying sizes
    for size in [100, 1_000, 10_000, 50_000].iter() {
        let data = vec![0u8; *size];
        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(BenchmarkId::new("data_size", size), &data, |b, data| {
            b.iter(|| {
                let value = DhtValue::new(data.clone());
                let serialized = bincode::serialize(&value).unwrap();
                black_box(serialized)
            });
        });
    }
    
    group.finish();
}

fn bench_peer_contact_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("dht_peer_contact");
    
    // Benchmark creating peer contacts
    for batch_size in [10, 100, 500, 1000].iter() {
        group.throughput(Throughput::Elements(*batch_size as u64));
        group.bench_with_input(BenchmarkId::new("batch_size", batch_size), batch_size, |b, &n| {
            b.iter(|| {
                let contacts: Vec<PeerContact> = (0..n)
                    .map(|i| {
                        let peer_id = DhtKey::random();
                        let addr: SocketAddr = format!("127.0.0.1:{}", 10000 + (i % 55000)).parse().unwrap();
                        PeerContact::new(peer_id, addr)
                    })
                    .collect();
                black_box(contacts)
            });
        });
    }
    
    group.finish();
}

criterion_group!(
    benches,
    bench_dht_key_generation,
    bench_routing_table_init,
    bench_routing_table_insert,
    bench_dht_value_serialization,
    bench_peer_contact_creation
);
criterion_main!(benches);

