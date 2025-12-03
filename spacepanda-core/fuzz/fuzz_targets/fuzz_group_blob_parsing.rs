#![no_main]

use libfuzzer_sys::fuzz_target;
use spacepanda_core::core_mls::persistence::EncryptedGroupBlob;

fuzz_target!(|data: &[u8]| {
    // Attempt to parse arbitrary bytes as an encrypted group blob
    // This tests resilience against malformed persistent data
    let _ = EncryptedGroupBlob::from_bytes(data);
});
