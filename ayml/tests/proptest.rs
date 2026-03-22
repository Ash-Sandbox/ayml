//! Property-based tests for ayml roundtrip and robustness.

use ayml::{Value, from_str, to_string};
use indexmap::IndexMap;
use proptest::prelude::*;

// ── Generators ───────────────────────────────────────────────────

/// Generate a random printable Unicode character, including non-ASCII.
fn arb_unicode_char() -> impl Strategy<Value = char> {
    // Sample from interesting Unicode ranges, excluding surrogates and
    // non-characters.
    prop_oneof![
        // Basic Latin (ASCII printable)
        (0x20_u32..=0x7E_u32),
        // Latin-1 Supplement (accented characters, symbols)
        (0xA0_u32..=0xFF_u32),
        // Latin Extended-A (Ā-ſ)
        (0x100_u32..=0x17F_u32),
        // Greek and Coptic
        (0x370_u32..=0x3FF_u32),
        // Cyrillic
        (0x400_u32..=0x4FF_u32),
        // Arabic
        (0x600_u32..=0x6FF_u32),
        // Devanagari
        (0x900_u32..=0x97F_u32),
        // CJK Unified Ideographs (subset)
        (0x4E00_u32..=0x9FFF_u32),
        // Hiragana
        (0x3040_u32..=0x309F_u32),
        // Katakana
        (0x30A0_u32..=0x30FF_u32),
        // Hangul Syllables (subset)
        (0xAC00_u32..=0xD7A3_u32),
        // Emoji (Miscellaneous Symbols and Pictographs)
        (0x1F300_u32..=0x1F5FF_u32),
        // Emoticons
        (0x1F600_u32..=0x1F64F_u32),
    ]
    .prop_filter_map("valid char", char::from_u32)
}

/// Generate a random Unicode string of 1..=max_len printable characters.
fn arb_unicode_string(max_len: usize) -> impl Strategy<Value = String> {
    prop::collection::vec(arb_unicode_char(), 1..=max_len)
        .prop_map(|chars| chars.into_iter().collect())
}

/// Map keys: valid AYML mapping keys that roundtrip through serde's
/// HashMap<String, _>.
fn arb_map_key() -> impl Strategy<Value = String> {
    prop_oneof![
        2 => "[a-zA-Z_][a-zA-Z0-9_]{0,15}",
        2 => arb_unicode_string(16),
        // Keys that need quoting
        1 => Just(String::new()),
        1 => Just("null".into()),
        1 => Just("true".into()),
        1 => Just("false".into()),
        1 => Just("42".into()),
        1 => Just("with: colon".into()),
        1 => Just("has #hash".into()),
        1 => Just("back\\slash".into()),
    ]
}

/// Scalar strings covering bare, quoted, and special cases.
fn arb_scalar_string() -> impl Strategy<Value = String> {
    prop_oneof![
        // Simple alphanumeric
        2 => "[a-zA-Z][a-zA-Z0-9 _-]{0,30}",
        // Strings containing special chars that force quoting
        1 => "[a-zA-Z0-9 :{}\\[\\]#,]{1,20}",
        // Random Unicode strings
        3 => arb_unicode_string(30),
        // Empty string
        1 => Just(String::new()),
        // Reserved words
        1 => Just("null".into()),
        1 => Just("true".into()),
        1 => Just("false".into()),
        1 => Just("inf".into()),
        1 => Just("+inf".into()),
        1 => Just("-inf".into()),
        1 => Just("nan".into()),
        // Numeric-looking
        1 => Just("42".into()),
        1 => Just("3.25".into()),
        1 => Just("-7".into()),
        // Strings with control characters
        1 => Just("tab\there".into()),
        1 => Just("cr\rhere".into()),
        1 => Just("newline\nhere".into()),
        // Trailing newline edge cases
        1 => Just("trailing newline\n".into()),
        1 => Just("end\n\n".into()),
        1 => Just("\n".into()),
        1 => Just("a\nb\n".into()),
        // Leading/trailing whitespace
        1 => Just(" leading".into()),
        1 => Just("trailing ".into()),
        1 => Just(" both ".into()),
        // Dash-space prefix
        1 => Just("- item".into()),
        1 => Just("- ".into()),
    ]
}

/// Generate a random Value tree with bounded depth.
fn arb_value(depth: u32) -> impl Strategy<Value = Value> {
    let leaf = prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::Bool),
        prop_oneof![any::<i64>(), Just(i64::MIN), Just(i64::MAX), Just(0_i64),]
            .prop_map(Value::Int),
        // Finite, non-subnormal floats that roundtrip cleanly via ryu
        (-1e15_f64..1e15_f64)
            .prop_filter("finite and normal", |f| f.is_finite())
            .prop_map(|f| Value::Float((f * 1e6).round() / 1e6)),
        prop_oneof![
            Just(Value::Float(f64::INFINITY)),
            Just(Value::Float(f64::NEG_INFINITY)),
            Just(Value::Float(f64::NAN)),
            Just(Value::Float(0.0)),
            Just(Value::Float(1.0)),
            Just(Value::Float(-1.0)),
            Just(Value::Float(1e20)),
            Just(Value::Float(-1e20)),
        ],
        arb_scalar_string().prop_map(Value::Str),
    ];

    if depth == 0 {
        leaf.boxed()
    } else {
        prop_oneof![
            4 => leaf,
            // Sequences of 0..5 elements
            1 => prop::collection::vec(arb_value(depth - 1), 0..5)
                .prop_map(Value::Seq),
            // Mappings of 0..5 entries (String keys for serde HashMap)
            1 => prop::collection::vec(
                    (arb_map_key(), arb_value(depth - 1)),
                    0..5
                )
                .prop_map(|entries| {
                    let map: IndexMap<String, Value> = entries.into_iter().collect();
                    Value::Map(map)
                }),
        ]
        .boxed()
    }
}

// ── Property tests ───────────────────────────────────────────────

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]

    /// Serialize a random Value tree, deserialize it back, and verify equality.
    #[test]
    fn serde_round_trip(value in arb_value(4)) {
        let serialized = to_string(&value).map_err(|e| {
            TestCaseError::fail(format!(
                "serialize failed: {e}\n--- value ---\n{value:?}"
            ))
        })?;

        let deserialized: Value = from_str(&serialized).map_err(|e| {
            TestCaseError::fail(format!(
                "deserialize failed: {e}\n--- serialized ---\n{serialized}--- value ---\n{value:?}"
            ))
        })?;

        prop_assert!(
            value == deserialized,
            "values differ\n--- original ---\n{:?}\n--- serialized ---\n{}\n--- deserialized ---\n{:?}",
            value,
            serialized,
            deserialized,
        );
    }

    /// Ensure from_str never panics on arbitrary input.
    #[test]
    fn deserializer_no_panic(input in "[\\s\\S]{0,200}") {
        let _ = from_str::<Value>(&input);
    }

    /// Ensure to_string never panics on arbitrary Value trees.
    #[test]
    fn serializer_no_panic(value in arb_value(3)) {
        let _ = to_string(&value);
    }

    /// Multi-line strings roundtrip through triple-quoted serialization.
    #[test]
    fn multiline_string_roundtrip(
        lines in prop::collection::vec("[a-zA-Z0-9 .!?]{0,40}", 1..6),
    ) {
        let s = lines.join("\n");
        let serialized = to_string(&s).unwrap();
        let deserialized: String = from_str(&serialized).unwrap();
        prop_assert_eq!(s, deserialized);
    }

    /// Option<i64> roundtrips through flow sequences (covers null before `,`/`]`).
    #[test]
    fn option_in_flow_seq_roundtrip(
        vals in prop::collection::vec(
            prop_oneof![Just(None), any::<i64>().prop_map(Some)],
            0..6,
        )
    ) {
        let serialized = to_string(&vals).unwrap();
        let deserialized: Vec<Option<i64>> = from_str(&serialized).unwrap();
        prop_assert_eq!(vals, deserialized);
    }

    /// Strings that Rust's f64::parse accepts but are NOT valid AYML floats
    /// should round-trip as bare strings (not quoted) via Value Display.
    #[test]
    fn non_ayml_float_strings_not_over_quoted(
        s in prop_oneof![
            Just(String::from("infinity")),
            Just(String::from("Infinity")),
            Just(String::from("INFINITY")),
            Just(String::from("NaN")),
            Just(String::from("NAN")),
            Just(String::from(".5")),
            Just(String::from("5.")),
            Just(String::from(".1e2")),
            Just(String::from("1.")),
            Just(String::from("1.e5")),
        ]
    ) {
        // These are parseable by Rust's f64::parse but are NOT AYML floats.
        // They should NOT be considered numbers by AYML's display logic.
        // When displayed as Value::Str, they should be bare (unquoted).
        let v = Value::Str(s.clone());
        let display = format!("{v}");
        prop_assert!(
            !display.starts_with('"'),
            "\"{}\" is not an AYML number and should display bare, got: {}",
            s,
            display,
        );
    }

    /// Triple-quoted strings inside flow collections parse correctly via serde.
    #[test]
    fn triple_quoted_in_flow_collections_serde(
        text in "[a-zA-Z0-9 ]{1,20}",
        kind in 0..3u8,
    ) {
        let input = match kind {
            0 => format!("[1, \"\"\"\n  {text}\n  \"\"\"]\n"),
            1 => format!("{{a: \"\"\"\n  {text}\n  \"\"\"}}\n"),
            _ => format!("{{a: \"\"\"\n  {text}\n  \"\"\", b: 1}}\n"),
        };
        let parsed: Result<serde_json::Value, _> = from_str(&input);
        prop_assert!(
            parsed.is_ok(),
            "serde parse failed: {}\n--- input ---\n{}---",
            parsed.unwrap_err(),
            input,
        );
    }
}
