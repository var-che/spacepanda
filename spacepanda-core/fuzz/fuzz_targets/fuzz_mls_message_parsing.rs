#![no_main]

use libfuzzer_sys::fuzz_target;
use spacepanda_core::core_mls::messages::EncryptedEnvelope;
use spacepanda_core::core_mls::transport::MlsEnvelope;
use spacepanda_core::core_mls::encryption::SenderData;

fuzz_target!(|data: &[u8]| {
    // Test 1: Parse EncryptedEnvelope (bincode format)
    // This tests resilience against malformed message envelopes
    let _ = EncryptedEnvelope::from_bytes(data);
    
    // Test 2: Parse MlsEnvelope (bincode format)
    // This tests resilience against malformed transport envelopes
    let _ = MlsEnvelope::from_bytes(data);
    
    // Test 3: Parse MlsEnvelope from JSON
    // This tests resilience against malformed JSON input
    if let Ok(json_str) = std::str::from_utf8(data) {
        let _ = MlsEnvelope::from_json(json_str);
    }
    
    // Test 4: Parse SenderData (fixed-length 20 bytes)
    // This tests resilience against malformed sender metadata
    let _ = SenderData::from_bytes(data);
});
