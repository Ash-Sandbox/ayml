#![no_main]

use ayml_testgen::tree::{canonicalize, diff};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    let Ok(node) = ayml_core::parse(text) else {
        return;
    };
    let emitted = ayml_core::emit(&node);
    let reparsed = ayml_core::parse(&emitted)
        .unwrap_or_else(|e| panic!("emitted text does not parse: {e}\n--- emitted ---\n{emitted}"));
    if let Some(d) = diff(&canonicalize(&node), &reparsed) {
        panic!("{d}\n--- input ---\n{text}\n--- emitted ---\n{emitted}");
    }
    assert_eq!(
        ayml_core::emit(&reparsed),
        emitted,
        "emitting is not idempotent"
    );
});
