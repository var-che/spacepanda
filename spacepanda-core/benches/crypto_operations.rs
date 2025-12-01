use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use ed25519_dalek::{Signer, Verifier, SigningKey, VerifyingKey};
use x25519_dalek::{StaticSecret, PublicKey};
use rand::rngs::OsRng;
use std::time::Duration;

fn bench_ed25519_keypair_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("crypto_ed25519_keygen");
    
    group.bench_function("generate_keypair", |b| {
        b.iter(|| {
            let signing_key = SigningKey::generate(&mut OsRng);
            black_box(signing_key)
        });
    });
    
    // Benchmark batch generation
    for batch_size in [10, 50, 100, 500].iter() {
        group.throughput(Throughput::Elements(*batch_size as u64));
        group.bench_with_input(BenchmarkId::new("batch_generation", batch_size), batch_size, |b, &n| {
            b.iter(|| {
                let keys: Vec<SigningKey> = (0..n).map(|_| SigningKey::generate(&mut OsRng)).collect();
                black_box(keys)
            });
        });
    }
    
    group.finish();
}

fn bench_ed25519_signing(c: &mut Criterion) {
    let mut group = c.benchmark_group("crypto_ed25519_signing");
    
    let signing_key = SigningKey::generate(&mut OsRng);
    
    // Benchmark signing with varying message sizes
    for size in [32, 256, 1024, 4096, 16384].iter() {
        let message = vec![0u8; *size];
        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(BenchmarkId::new("message_size", size), &message, |b, msg| {
            b.iter(|| {
                let signature = signing_key.sign(black_box(msg));
                black_box(signature)
            });
        });
    }
    
    group.finish();
}

fn bench_ed25519_verification(c: &mut Criterion) {
    let mut group = c.benchmark_group("crypto_ed25519_verification");
    
    let signing_key = SigningKey::generate(&mut OsRng);
    let verifying_key = signing_key.verifying_key();
    
    // Benchmark verification with varying message sizes
    for size in [32, 256, 1024, 4096, 16384].iter() {
        let message = vec![0u8; *size];
        let signature = signing_key.sign(&message);
        
        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(
            BenchmarkId::new("message_size", size),
            &(message, signature, verifying_key),
            |b, (msg, sig, vkey)| {
                b.iter(|| {
                    let result = vkey.verify(black_box(msg), black_box(sig));
                    black_box(result)
                });
            },
        );
    }
    
    group.finish();
}

fn bench_x25519_key_exchange(c: &mut Criterion) {
    let mut group = c.benchmark_group("crypto_x25519_key_exchange");
    
    group.bench_function("dh_exchange", |b| {
        b.iter(|| {
            let alice_secret = StaticSecret::random_from_rng(OsRng);
            let bob_secret = StaticSecret::random_from_rng(OsRng);
            
            let alice_public = PublicKey::from(&alice_secret);
            let bob_public = PublicKey::from(&bob_secret);
            
            let alice_shared = alice_secret.diffie_hellman(&bob_public);
            let bob_shared = bob_secret.diffie_hellman(&alice_public);
            
            black_box((alice_shared, bob_shared))
        });
    });
    
    // Benchmark batch exchanges
    for batch_size in [10, 50, 100, 500].iter() {
        group.throughput(Throughput::Elements(*batch_size as u64));
        group.bench_with_input(BenchmarkId::new("batch_exchanges", batch_size), batch_size, |b, &n| {
            b.iter(|| {
                let mut shared_secrets = Vec::new();
                
                for _ in 0..n {
                    let alice_secret = StaticSecret::random_from_rng(OsRng);
                    let bob_secret = StaticSecret::random_from_rng(OsRng);
                    
                    let alice_public = PublicKey::from(&alice_secret);
                    let bob_public = PublicKey::from(&bob_secret);
                    
                    let shared = alice_secret.diffie_hellman(&bob_public);
                    shared_secrets.push(shared);
                }
                
                black_box(shared_secrets)
            });
        });
    }
    
    group.finish();
}

fn bench_noise_handshake(c: &mut Criterion) {
    let mut group = c.benchmark_group("crypto_noise_handshake");
    group.measurement_time(Duration::from_secs(10));
    
    use snow::Builder;
    
    let params = "Noise_XX_25519_ChaChaPoly_BLAKE2s";
    
    group.bench_function("full_handshake", |b| {
        b.iter(|| {
            // Initiator
            let mut initiator = Builder::new(params.parse().unwrap())
                .build_initiator()
                .unwrap();
            
            // Responder
            let mut responder = Builder::new(params.parse().unwrap())
                .build_responder()
                .unwrap();
            
            // -> e
            let mut buffer = vec![0u8; 65535];
            let len = initiator.write_message(&[], &mut buffer).unwrap();
            let _ = responder.read_message(&buffer[..len], &mut buffer).unwrap();
            
            // <- e, ee, s, es
            let len = responder.write_message(&[], &mut buffer).unwrap();
            let _ = initiator.read_message(&buffer[..len], &mut buffer).unwrap();
            
            // -> s, se
            let len = initiator.write_message(&[], &mut buffer).unwrap();
            let _ = responder.read_message(&buffer[..len], &mut buffer).unwrap();
            
            let initiator_transport = initiator.into_transport_mode().unwrap();
            let responder_transport = responder.into_transport_mode().unwrap();
            
            black_box((initiator_transport, responder_transport))
        });
    });
    
    group.finish();
}

fn bench_noise_transport(c: &mut Criterion) {
    let mut group = c.benchmark_group("crypto_noise_transport");
    
    use snow::Builder;
    
    let params = "Noise_XX_25519_ChaChaPoly_BLAKE2s";
    
    // Setup handshake first
    let mut initiator = Builder::new(params.parse().unwrap())
        .build_initiator()
        .unwrap();
    let mut responder = Builder::new(params.parse().unwrap())
        .build_responder()
        .unwrap();
    
    let mut buffer = vec![0u8; 65535];
    let len = initiator.write_message(&[], &mut buffer).unwrap();
    let _ = responder.read_message(&buffer[..len], &mut buffer).unwrap();
    let len = responder.write_message(&[], &mut buffer).unwrap();
    let _ = initiator.read_message(&buffer[..len], &mut buffer).unwrap();
    let len = initiator.write_message(&[], &mut buffer).unwrap();
    let _ = responder.read_message(&buffer[..len], &mut buffer).unwrap();
    
    let mut initiator_transport = initiator.into_transport_mode().unwrap();
    let mut responder_transport = responder.into_transport_mode().unwrap();
    
    // Benchmark transport mode encryption/decryption
    for size in [64, 256, 1024, 4096, 16384].iter() {
        let plaintext = vec![0u8; *size];
        group.throughput(Throughput::Bytes(*size as u64));
        
        group.bench_with_input(BenchmarkId::new("encrypt_decrypt", size), &plaintext, |b, msg| {
            b.iter(|| {
                let mut buffer = vec![0u8; 65535];
                
                // Encrypt
                let len = initiator_transport.write_message(black_box(msg), &mut buffer).unwrap();
                
                // Decrypt
                let mut recv_buffer = vec![0u8; 65535];
                let _ = responder_transport
                    .read_message(&buffer[..len], &mut recv_buffer)
                    .unwrap();
                
                black_box(recv_buffer)
            });
        });
    }
    
    group.finish();
}

fn bench_hkdf_key_derivation(c: &mut Criterion) {
    let mut group = c.benchmark_group("crypto_hkdf_derivation");
    
    use hkdf::Hkdf;
    use sha2::Sha256;
    
    let ikm = b"input key material";
    let salt = b"salt value";
    let info = b"application info";
    
    group.bench_function("derive_key", |b| {
        b.iter(|| {
            let hk = Hkdf::<Sha256>::new(Some(black_box(salt)), black_box(ikm));
            let mut okm = [0u8; 32];
            hk.expand(black_box(info), &mut okm).unwrap();
            black_box(okm)
        });
    });
    
    // Benchmark batch derivation
    for num_keys in [10, 50, 100, 500].iter() {
        group.throughput(Throughput::Elements(*num_keys as u64));
        group.bench_with_input(BenchmarkId::new("batch_derivation", num_keys), num_keys, |b, &n| {
            b.iter(|| {
                let mut keys = Vec::new();
                
                for i in 0..n {
                    let info_with_index = format!("application info {}", i);
                    let hk = Hkdf::<Sha256>::new(Some(salt), ikm);
                    let mut okm = [0u8; 32];
                    hk.expand(info_with_index.as_bytes(), &mut okm).unwrap();
                    keys.push(okm);
                }
                
                black_box(keys)
            });
        });
    }
    
    group.finish();
}

fn bench_sha256_hashing(c: &mut Criterion) {
    let mut group = c.benchmark_group("crypto_sha256_hash");
    
    use sha2::{Sha256, Digest};
    
    // Benchmark hashing with varying input sizes
    for size in [32, 256, 1024, 4096, 16384, 65536].iter() {
        let data = vec![0u8; *size];
        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(BenchmarkId::new("data_size", size), &data, |b, data| {
            b.iter(|| {
                let mut hasher = Sha256::new();
                hasher.update(black_box(data));
                let hash = hasher.finalize();
                black_box(hash)
            });
        });
    }
    
    group.finish();
}

fn bench_concurrent_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("crypto_concurrent_ops");
    group.measurement_time(Duration::from_secs(15));
    
    use tokio::runtime::Runtime;
    
    let rt = Runtime::new().unwrap();
    
    // Benchmark concurrent signing operations
    for concurrency in [10, 50, 100, 500].iter() {
        group.throughput(Throughput::Elements(*concurrency as u64));
        group.bench_with_input(BenchmarkId::new("concurrent_signing", concurrency), concurrency, |b, &n| {
            b.iter(|| {
                rt.block_on(async {
                    let mut handles = Vec::new();
                    
                    for i in 0..n {
                        handles.push(tokio::spawn(async move {
                            let signing_key = SigningKey::generate(&mut OsRng);
                            let message = format!("message_{}", i);
                            let signature = signing_key.sign(message.as_bytes());
                            black_box(signature)
                        }));
                    }
                    
                    for handle in handles {
                        let _ = handle.await;
                    }
                })
            });
        });
    }
    
    group.finish();
}

criterion_group!(
    benches,
    bench_ed25519_keypair_generation,
    bench_ed25519_signing,
    bench_ed25519_verification,
    bench_x25519_key_exchange,
    bench_noise_handshake,
    bench_noise_transport,
    bench_hkdf_key_derivation,
    bench_sha256_hashing,
    bench_concurrent_operations
);
criterion_main!(benches);
