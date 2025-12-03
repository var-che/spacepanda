use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use spacepanda_core::core_dht::{DhtKey, DhtValue, PeerContact, RoutingTable};

mod bench_config;
use bench_config::{create_rng, BenchConfig};
use rand::Rng;

// Load or create benchmark configuration for reproducibility
fn get_bench_config() -> BenchConfig {
    let config_path = "target/bench_config.json";
    let mut config = BenchConfig::load_or_default(config_path);

    // Set benchmark-specific parameters
    config.set_param("benchmark_suite", "dht_operations");
    config.set_param("criterion_version", "0.5");

    // Save for reference
    let _ = config.save(config_path);

    config
}

// Helper to create deterministic DhtKeys using config seed
fn deterministic_dht_key(rng: &mut rand::rngs::StdRng) -> DhtKey {
    use rand::Rng;
    let seed: u64 = rng.random();
    DhtKey::hash(&seed.to_le_bytes())
}

// Helper to create random DhtKeys for benchmarking (fallback)
fn random_dht_key(seed: u64) -> DhtKey {
    DhtKey::hash(&seed.to_le_bytes())
}

fn bench_dht_key_generation(c: &mut Criterion) {
    let config = get_bench_config();
    let mut rng = create_rng(&config);
    let mut group = c.benchmark_group("dht_key_generation");

    // Benchmark key generation from hashing
    group.bench_function("single_key", |b| {
        b.iter(|| black_box(deterministic_dht_key(&mut rng)));
    });

    // Benchmark batch key generation
    for batch_size in [10, 100, 500, 1000].iter() {
        group.throughput(Throughput::Elements(*batch_size as u64));
        group.bench_with_input(BenchmarkId::new("batch_size", batch_size), batch_size, |b, &n| {
            b.iter(|| {
                let keys: Vec<DhtKey> = (0..n).map(|i| random_dht_key(i as u64)).collect();
                black_box(keys)
            });
        });
    }

    group.finish();
}

fn bench_dht_key_hashing(c: &mut Criterion) {
    let mut group = c.benchmark_group("dht_key_hashing");

    // Benchmark hashing different data sizes
    for size in [32, 256, 1024, 4096, 16384].iter() {
        let data = vec![0u8; *size];
        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(BenchmarkId::new("data_size", size), &data, |b, data| {
            b.iter(|| black_box(DhtKey::hash(black_box(data))));
        });
    }

    group.finish();
}

fn bench_dht_key_distance(c: &mut Criterion) {
    let mut group = c.benchmark_group("dht_key_distance");

    // Benchmark XOR distance calculation
    for num_comparisons in [10, 100, 1000, 10000].iter() {
        group.throughput(Throughput::Elements(*num_comparisons as u64));
        group.bench_with_input(
            BenchmarkId::new("comparisons", num_comparisons),
            num_comparisons,
            |b, &n| {
                let target = random_dht_key(99999);
                let keys: Vec<DhtKey> = (0..n).map(|i| random_dht_key(i as u64)).collect();

                b.iter(|| {
                    for key in &keys {
                        let distance = key.distance(&target);
                        black_box(distance);
                    }
                });
            },
        );
    }

    group.finish();
}

fn bench_routing_table_init(c: &mut Criterion) {
    let mut group = c.benchmark_group("dht_routing_table_init");

    // Benchmark routing table initialization with varying k values
    for k in [8, 16, 20, 32].iter() {
        group.bench_with_input(BenchmarkId::new("k_value", k), k, |b, &k_val| {
            let mut counter = 0u64;
            b.iter(|| {
                counter += 1;
                let local_id = random_dht_key(counter);
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
                let local_peer = random_dht_key(0);
                let mut table = RoutingTable::new(local_peer.clone(), 20);

                for i in 0..n {
                    let peer_id = random_dht_key(i as u64 + 1);
                    let addr = format!("127.0.0.1:{}", 10000 + (i % 55000));
                    let contact = PeerContact::new(peer_id, addr);
                    let _ = table.insert(contact);
                }

                black_box(table)
            });
        });
    }

    group.finish();
}

fn bench_routing_table_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("dht_routing_lookup");

    // Benchmark lookups with varying table sizes
    for size in [10, 100, 500, 1000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::new("table_size", size), size, |b, &n| {
            // Pre-populate table
            let local_peer = random_dht_key(0);
            let mut table = RoutingTable::new(local_peer.clone(), 20);

            for i in 0..n {
                let peer_id = random_dht_key(i as u64 + 1);
                let addr = format!("127.0.0.1:{}", 10000 + (i % 55000));
                let contact = PeerContact::new(peer_id, addr);
                let _ = table.insert(contact);
            }

            b.iter(|| {
                let target = random_dht_key(99999);
                black_box(table.find_closest(&target, 20))
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
                        let peer_id = random_dht_key(i as u64);
                        let addr = format!("127.0.0.1:{}", 10000 + (i % 55000));
                        PeerContact::new(peer_id, addr)
                    })
                    .collect();
                black_box(contacts)
            });
        });
    }

    group.finish();
}

fn bench_bucket_distribution(c: &mut Criterion) {
    let mut group = c.benchmark_group("dht_bucket_distribution");

    // Benchmark how peers distribute across buckets
    for num_peers in [100, 500, 1000, 5000].iter() {
        group.throughput(Throughput::Elements(*num_peers as u64));
        group.bench_with_input(BenchmarkId::new("num_peers", num_peers), num_peers, |b, &n| {
            b.iter(|| {
                let local_peer = random_dht_key(0);
                let mut table = RoutingTable::new(local_peer.clone(), 20);

                // Insert many peers to see bucket distribution
                for i in 0..n {
                    let peer_id = random_dht_key(i as u64 + 1);
                    let addr = format!("127.0.0.1:{}", 10000 + (i % 55000));
                    let contact = PeerContact::new(peer_id, addr);
                    let _ = table.insert(contact);
                }

                black_box(table)
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_dht_key_generation,
    bench_dht_key_hashing,
    bench_dht_key_distance,
    bench_routing_table_init,
    bench_routing_table_insert,
    bench_routing_table_lookup,
    bench_dht_value_serialization,
    bench_peer_contact_creation,
    bench_bucket_distribution
);
criterion_main!(benches);
