use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use spacepanda_core::core_mls::storage::SqlStorageProvider;
use std::sync::Arc;
use std::time::Duration;
use tempfile::NamedTempFile;

mod bench_config;
use bench_config::{create_rng, BenchConfig};
use rand::Rng;

fn get_bench_config() -> BenchConfig {
    let config_path = "target/bench_config.json";
    let mut config = BenchConfig::load_or_default(config_path);
    config.set_param("benchmark_suite", "storage_operations");
    config.set_param("criterion_version", "0.5");
    let _ = config.save(config_path);
    config
}

fn bench_channel_metadata_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("storage_metadata");
    group.measurement_time(Duration::from_secs(10));

    // Create a temp database
    let temp_file = NamedTempFile::new().unwrap();
    let db_path = temp_file.path().to_str().unwrap();
    let storage = Arc::new(SqlStorageProvider::new(db_path).unwrap());

    // Benchmark save operations with encrypted metadata
    group.bench_function("save_channel_metadata", |b| {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let storage_ref = storage.clone();
        
        b.to_async(&runtime).iter(|| {
            let storage = storage_ref.clone();
            let group_id = {
                use rand::Rng;
                let mut rng = rand::thread_rng();
                let counter: u64 = rng.gen();
                format!("group_{}", counter).into_bytes()
            };
            
            async move {
                let name = b"Test Channel Name";
                let topic = Some(b"Test Topic".as_slice());
                let members = b"member1,member2,member3";
                
                storage
                    .save_channel_metadata(&group_id, name, topic, members, 1)
                    .await
                    .unwrap();
                
                black_box(group_id)
            }
        });
    });

    // Benchmark load operations with decryption
    group.bench_function("load_channel_metadata", |b| {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let storage_ref = storage.clone();
        
        // Pre-populate some data
        runtime.block_on(async {
            for i in 0..100 {
                let group_id = format!("bench_group_{}", i).into_bytes();
                storage_ref
                    .save_channel_metadata(&group_id, b"name", Some(b"topic"), b"members", 1)
                    .await
                    .unwrap();
            }
        });

        b.to_async(&runtime).iter(|| {
            let storage = storage_ref.clone();
            let group_id = {
                use rand::Rng;
                let mut rng = rand::thread_rng();
                let idx: u64 = rng.gen_range(0..100);
                format!("bench_group_{}", idx).into_bytes()
            };
            
            async move {
                let result = storage.load_channel_metadata(&group_id).await.unwrap();
                black_box(result)
            }
        });
    });

    // Benchmark round-trip (save + load with encryption/decryption)
    group.bench_function("roundtrip_channel_metadata", |b| {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let storage_ref = storage.clone();
        
        b.to_async(&runtime).iter(|| {
            let storage = storage_ref.clone();
            let group_id = {
                use rand::Rng;
                let mut rng = rand::thread_rng();
                let counter: u64 = rng.gen();
                format!("roundtrip_{}", counter).into_bytes()
            };
            
            async move {
                // Save (with encryption)
                storage
                    .save_channel_metadata(&group_id, b"name", Some(b"topic"), b"members", 1)
                    .await
                    .unwrap();
                
                // Load (with decryption)
                let result = storage.load_channel_metadata(&group_id).await.unwrap();
                black_box(result)
            }
        });
    });

    // Benchmark different payload sizes
    for size in [10, 100, 1000, 10_000].iter() {
        group.throughput(Throughput::Bytes(*size as u64));
        let storage_ref = storage.clone();
        let large_data = vec![b'x'; *size];
        
        group.bench_with_input(
            BenchmarkId::new("save_varying_size", size),
            size,
            |b, _| {
                let runtime = tokio::runtime::Runtime::new().unwrap();
                
                b.to_async(&runtime).iter(|| {
                    let storage = storage_ref.clone();
                    let data = large_data.clone();
                    let group_id = {
                        use rand::Rng;
                        let mut rng = rand::thread_rng();
                        let counter: u64 = rng.gen();
                        format!("large_{}", counter).into_bytes()
                    };
                    
                    async move {
                        storage
                            .save_channel_metadata(&group_id, &data, Some(&data), &data, 1)
                            .await
                            .unwrap();
                        
                        black_box(group_id)
                    }
                });
            },
        );
    }

    group.finish();
}

fn bench_key_package_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("storage_key_packages");
    group.measurement_time(Duration::from_secs(10));

    let temp_file = NamedTempFile::new().unwrap();
    let db_path = temp_file.path().to_str().unwrap();
    let storage = Arc::new(SqlStorageProvider::new(db_path).unwrap());

    group.bench_function("store_key_package", |b| {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let storage_ref = storage.clone();
        
        b.to_async(&runtime).iter(|| {
            let storage = storage_ref.clone();
            let kp_id = {
                use rand::Rng;
                let mut rng = rand::thread_rng();
                let counter: u64 = rng.gen();
                format!("kp_{}", counter).into_bytes()
            };
            let kp_data = vec![0u8; 1024];
            
            async move {
                storage
                    .store_key_package(&kp_id, &kp_data, b"cred", None)
                    .await
                    .unwrap();
                
                black_box(kp_id)
            }
        });
    });

    group.bench_function("load_key_package", |b| {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let storage_ref = storage.clone();
        
        // Pre-populate
        runtime.block_on(async {
            for i in 0..100 {
                let kp_id = format!("bench_kp_{}", i).into_bytes();
                storage_ref
                    .store_key_package(&kp_id, &vec![0u8; 1024], b"cred", None)
                    .await
                    .unwrap();
            }
        });

        b.to_async(&runtime).iter(|| {
            let storage = storage_ref.clone();
            let kp_id = {
                use rand::Rng;
                let mut rng = rand::thread_rng();
                let idx: u64 = rng.gen_range(0..100);
                format!("bench_kp_{}", idx).into_bytes()
            };
            
            async move {
                let result = storage.load_key_package(&kp_id).await.unwrap();
                black_box(result)
            }
        });
    });

    group.finish();
}

fn bench_encryption_overhead(c: &mut Criterion) {
    use spacepanda_core::core_mls::storage::metadata_encryption::{encrypt_metadata, decrypt_metadata};
    
    let mut group = c.benchmark_group("encryption_overhead");
    
    let group_id = b"bench_group";
    
    // Benchmark pure encryption/decryption performance
    for size in [10, 100, 1000, 10_000].iter() {
        group.throughput(Throughput::Bytes(*size as u64));
        
        let data = vec![b'x'; *size];
        
        group.bench_with_input(
            BenchmarkId::new("encrypt", size),
            &data,
            |b, data| {
                b.iter(|| {
                    let encrypted = encrypt_metadata(group_id, data).unwrap();
                    black_box(encrypted)
                });
            },
        );
        
        // Pre-encrypt for decryption benchmark
        let encrypted = encrypt_metadata(group_id, &data).unwrap();
        
        group.bench_with_input(
            BenchmarkId::new("decrypt", size),
            &encrypted,
            |b, encrypted| {
                b.iter(|| {
                    let decrypted = decrypt_metadata(group_id, encrypted).unwrap();
                    black_box(decrypted)
                });
            },
        );
        
        // Round-trip
        group.bench_with_input(
            BenchmarkId::new("roundtrip", size),
            &data,
            |b, data| {
                b.iter(|| {
                    let encrypted = encrypt_metadata(group_id, data).unwrap();
                    let decrypted = decrypt_metadata(group_id, &encrypted).unwrap();
                    black_box(decrypted)
                });
            },
        );
    }
    
    group.finish();
}

criterion_group!(
    benches,
    bench_channel_metadata_operations,
    bench_key_package_operations,
    bench_encryption_overhead
);
criterion_main!(benches);
