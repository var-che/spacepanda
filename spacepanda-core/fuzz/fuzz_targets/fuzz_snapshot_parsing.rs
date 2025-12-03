#![no_main]

use libfuzzer_sys::fuzz_target;
use spacepanda_core::core_mls::state::snapshot::GroupSnapshot;

fuzz_target!(|data: &[u8]| {
    // Attempt to parse arbitrary bytes as a group snapshot
    // This tests resilience against malformed snapshot data
    let _ = GroupSnapshot::from_bytes(data);
});
