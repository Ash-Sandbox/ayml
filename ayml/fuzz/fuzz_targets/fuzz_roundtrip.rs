#![no_main]

use libfuzzer_sys::fuzz_target;

// Parse arbitrary bytes into a `Value`. If it succeeds then a round trip
// must return the same value, in the same order.
fuzz_target!(|data: &[u8]| {
    let Ok(value) = ayml::from_slice::<ayml::Value>(data) else {
        return;
    };
    let serialized =
        ayml::to_string(&value).expect("serializing a successfully-parsed Value should not fail");
    let roundtripped: ayml::Value = ayml::from_str(&serialized)
        .expect("deserializing our own serialized output should not fail");
    assert_eq!(value, roundtripped, "roundtrip mismatch\n{serialized}");
    let reserialized = ayml::to_string(&roundtripped).expect("reserializing should not fail");
    assert_eq!(serialized, reserialized, "key order or formatting changed");
});
