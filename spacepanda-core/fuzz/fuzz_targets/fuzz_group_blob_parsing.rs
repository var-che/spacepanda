#![no_main]

use libfuzzer_sys::fuzz_target;
use spacepanda_core::core_mls::persistence::{EncryptedGroupBlob, decrypt_group_state};

fuzz_target!(|data: &[u8]| {
    // Test 1: Parse EncryptedGroupBlob structure
    // This tests resilience against malformed persistent data format
    if let Ok(blob) = EncryptedGroupBlob::from_bytes(data) {
        // Test 2: Attempt decryption with arbitrary passphrase
        // This tests resilience against decryption of malformed/tampered data
        let _ = decrypt_group_state(&blob, Some("fuzz_passphrase"));
    }
});
