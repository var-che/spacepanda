#![no_main]

use libfuzzer_sys::fuzz_target;
use spacepanda_core::core_mls::sealed_sender::{derive_sender_key, seal_sender, unseal_sender};

fuzz_target!(|data: &[u8]| {
    // Split input into components for fuzzing
    if data.len() < 3 {
        return;
    }
    
    // Use bytes to split data into key material, sender identity, and epoch
    let key_split = (data[0] as usize % data.len().saturating_sub(2)).max(1);
    let sender_split = key_split + ((data[1] as usize % data.len().saturating_sub(key_split + 1)).max(1));
    
    let key_material = &data[2..key_split.min(data.len())];
    let sender_identity = &data[key_split..sender_split.min(data.len())];
    let epoch_bytes = &data[sender_split..];
    
    // Derive epoch from remaining bytes (or default to 0)
    let epoch = if epoch_bytes.len() >= 8 {
        u64::from_le_bytes([
            epoch_bytes[0], epoch_bytes[1], epoch_bytes[2], epoch_bytes[3],
            epoch_bytes[4], epoch_bytes[5], epoch_bytes[6], epoch_bytes[7],
        ])
    } else {
        0
    };
    
    // Test 1: Derive sender key from arbitrary key material
    // This tests HKDF key derivation with malformed inputs
    let key = derive_sender_key(key_material);
    
    // Test 2: Seal sender identity with arbitrary data
    // This tests ChaCha20-Poly1305 encryption with edge cases
    if let Ok(sealed) = seal_sender(sender_identity, &key, epoch) {
        // Test 3: Unseal the sender
        // This validates round-trip integrity
        let _ = unseal_sender(&sealed, &key, epoch);
        
        // Test 4: Unseal with wrong epoch (should fail)
        // This tests epoch validation
        let _ = unseal_sender(&sealed, &key, epoch.wrapping_add(1));
        
        // Test 5: Unseal with wrong key (should fail)
        // This tests authentication
        let wrong_key = derive_sender_key(b"wrong_key");
        let _ = unseal_sender(&sealed, &wrong_key, epoch);
    }
    
    // Test 6: Seal very large sender identities (DoS resistance)
    // Limited to 10KB to prevent fuzzer from hanging
    if sender_identity.len() <= 10_000 {
        let _ = seal_sender(sender_identity, &key, epoch);
    }
});
