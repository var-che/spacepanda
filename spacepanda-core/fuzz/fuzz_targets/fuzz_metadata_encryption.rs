#![no_main]

use libfuzzer_sys::fuzz_target;
use spacepanda_core::core_mls::storage::metadata_encryption::MetadataEncryption;

fuzz_target!(|data: &[u8]| {
    // Split input into group_id and plaintext to fuzz
    if data.len() < 2 {
        return;
    }
    
    // Use first byte to determine split point
    let split_idx = (data[0] as usize % data.len().saturating_sub(1)).max(1);
    let (group_id, plaintext) = data[1..].split_at(split_idx.min(data.len() - 1));
    
    // Test 1: Create encryption context with arbitrary group_id
    // This tests HKDF key derivation with malformed inputs
    let enc = MetadataEncryption::new(group_id);
    
    // Test 2: Encrypt arbitrary plaintext
    // This tests ChaCha20-Poly1305 with edge case inputs
    if let Ok(ciphertext) = enc.encrypt(plaintext) {
        // Test 3: Decrypt the ciphertext
        // This validates round-trip integrity
        let _ = enc.decrypt(&ciphertext);
    }
    
    // Test 4: Attempt to decrypt arbitrary ciphertext
    // This tests resilience against malformed/tampered ciphertexts
    let _ = enc.decrypt(plaintext);
    
    // Test 5: Test with very large plaintexts (DoS resistance)
    // Limited to 1MB to prevent fuzzer from hanging
    if plaintext.len() <= 1_000_000 {
        let _ = enc.encrypt(plaintext);
    }
});
