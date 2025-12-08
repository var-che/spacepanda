#![no_main]

use libfuzzer_sys::fuzz_target;
use spacepanda_core::core_mls::state::snapshot::GroupSnapshot;

fuzz_target!(|data: &[u8]| {
    // Attempt to parse arbitrary bytes as a group snapshot
    // This tests resilience against malformed snapshot data
    
    // Test both bincode and JSON parsing paths
    let _ = GroupSnapshot::from_bytes(data);
    
    // Also test JSON parsing if input is valid UTF-8
    if let Ok(json_str) = std::str::from_utf8(data) {
        let _ = serde_json::from_str::<GroupSnapshot>(json_str);
    }
});
