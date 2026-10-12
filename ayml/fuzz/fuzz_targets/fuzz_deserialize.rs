#![no_main]

use libfuzzer_sys::fuzz_target;

// Deserialize arbitrary bytes — must never panic.
fuzz_target!(|data: &[u8]| {
    let _ = ayml::from_slice::<ayml::Value>(data);
});
