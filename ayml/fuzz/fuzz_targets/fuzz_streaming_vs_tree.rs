#![no_main]

use libfuzzer_sys::fuzz_target;

// The streaming deserializer and parsed tree deserializer must agree on every input.
fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    let streamed = ayml::from_str::<ayml::Value>(text);
    match ayml_core::parse(text) {
        Ok(tree) => {
            let from_tree: ayml::Value = ayml::from_node(&tree).expect("tree deserializes");
            let streamed = streamed.expect("streaming accepts what the parser accepts");
            assert_eq!(
                ayml::to_string(&streamed).unwrap(),
                ayml::to_string(&from_tree).unwrap(),
            );
        }
        Err(e) => {
            let streamed = streamed.expect_err("streaming rejects what the parser rejects");
            assert_eq!(streamed.to_string(), e.to_string());
        }
    }
});
