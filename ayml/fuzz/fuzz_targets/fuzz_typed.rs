#![no_main]

use std::collections::BTreeMap;

use ayml::Commented;
use libfuzzer_sys::fuzz_target;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
enum Shape {
    Empty,
    Circle(f64),
    Move(i32, i32),
    Rect { w: u32, h: Option<u32> },
}

#[derive(Debug, Serialize, Deserialize)]
struct Inner {
    name: Commented<String>,
    tags: Vec<Commented<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Config {
    #[serde(default)]
    flag: bool,
    #[serde(default)]
    small: Option<u8>,
    #[serde(default)]
    big: i128,
    #[serde(default)]
    ratio: f32,
    #[serde(default)]
    ch: Option<char>,
    #[serde(default)]
    text: String,
    #[serde(default)]
    list: Vec<Vec<i64>>,
    #[serde(default)]
    tuple: Option<(u16, String)>,
    #[serde(default)]
    int_keys: BTreeMap<i32, Shape>,
    #[serde(default)]
    shapes: Vec<Shape>,
    #[serde(default)]
    inner: Option<Commented<Inner>>,
    #[serde(default)]
    bytes: Option<serde_bytes_like::Bytes>,
}

mod serde_bytes_like {
    #[derive(Debug, serde::Serialize, serde::Deserialize)]
    pub struct Bytes(pub Vec<u8>);
}

// Deserialize arbitrary input into a type with many field kinds.
// Re-serialization must succeed and the output must deserialize again.
fuzz_target!(|data: &[u8]| {
    let Ok(config) = ayml::from_slice::<Config>(data) else {
        return;
    };
    let text = ayml::to_string(&config).expect("serializing a deserialized Config");
    ayml::from_str::<Config>(&text)
        .unwrap_or_else(|e| panic!("re-deserializing failed: {e}\n--- text ---\n{text}"));
});
