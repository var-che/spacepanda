#![no_main]

use libfuzzer_sys::fuzz_target;
use spacepanda_core::core_mls::messages::EncryptedEnvelope;

fuzz_target!(|data: &[u8]| {
    // Attempt to parse arbitrary bytes as an MLS encrypted envelope
    // This tests resilience against malformed messages
    let _ = EncryptedEnvelope::from_bytes(data);
});
