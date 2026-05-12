//! Regression tests for bugs found during code review.
//!
//! Each test is marked with `#[should_panic]` or asserts the buggy behavior
//! BEFORE the fix is applied. After fixing, the attribute/assertion is
//! updated to verify the correct behavior.

use ayml::{
    commented::Commented, commented_value::CommentedValueKind, from_str, to_string, value::Value,
};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

#[test]
fn triple_quoted_nul_at_end_of_line() {
    // A triple-quoted string with \0 at end of a line, followed by more content
    let input = "\"\"\"\n  hello\\0\n  world\n  \"\"\"";
    let result: String = from_str(input).unwrap();
    // After fix: \0 should be preserved and newline should remain
    assert_eq!(result, "hello\0\nworld");
}

#[test]
fn ser_tab_before_hash_needs_quoting() {
    let input = "hello\t#world";
    let serialized = to_string(&input).unwrap();
    // After fix: must be quoted because \t# would start a comment
    assert!(
        serialized.starts_with('"'),
        "expected quoted output for tab-before-#, got: {serialized}"
    );
    let back: String = from_str(&serialized).unwrap();
    assert_eq!(back, input);
}

#[test]
fn ser_c1_control_char_needs_quoting() {
    let input = "hello\u{0080}world"; // U+0080 is a C1 control char
    let serialized = to_string(&input).unwrap();
    assert!(
        serialized.starts_with('"'),
        "expected quoted output for C1 control char, got: {serialized}"
    );
    let back: String = from_str(&serialized).unwrap();
    assert_eq!(back, input);
}

#[test]
fn ser_c1_control_char_u009f() {
    let input = "test\u{009F}value";
    let serialized = to_string(&input).unwrap();
    assert!(
        serialized.starts_with('"'),
        "expected quoted output for U+009F, got: {serialized}"
    );
    let back: String = from_str(&serialized).unwrap();
    assert_eq!(back, input);
}

#[test]
fn ser_c1_nel_u0085_allowed_bare() {
    // U+0085 (NEL) IS in c-printable and is nb-char (not a line break per spec).
    // It should be allowed in bare strings without quoting.
    let input = "test\u{0085}value";
    let serialized = to_string(&input).unwrap();
    let back: String = from_str(&serialized).unwrap();
    assert_eq!(back, input, "NEL should round-trip correctly");
}

#[test]
fn ser_triple_quote_in_content() {
    let input = "line1\nfoo\"\"\"bar";
    let serialized = to_string(&input).unwrap();
    let back: String = from_str(&serialized).unwrap();
    assert_eq!(
        back, input,
        "roundtrip failed for string containing \"\"\"\nayml:\n{serialized}"
    );
}

#[test]
fn ser_triple_quote_alone_on_line() {
    let input = "before\n\"\"\"\nafter";
    let serialized = to_string(&input).unwrap();
    let back: String = from_str(&serialized).unwrap();
    assert_eq!(
        back, input,
        "roundtrip failed for string with \"\"\" on its own line\nayml:\n{serialized}"
    );
}

#[test]
fn ser_struct_field_renamed_to_reserved() {
    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct S {
        #[serde(rename = "null")]
        field: i32,
    }
    let val = S { field: 42 };
    let serialized = to_string(&val).unwrap();
    let back: S = from_str(&serialized).unwrap();
    assert_eq!(
        back, val,
        "roundtrip failed for struct with 'null' field\nayml:\n{serialized}"
    );
}

#[test]
fn ser_struct_field_with_colon_space() {
    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct S {
        #[serde(rename = "key: value")]
        field: i32,
    }
    let val = S { field: 42 };
    let serialized = to_string(&val).unwrap();
    let back: S = from_str(&serialized).unwrap();
    assert_eq!(
        back, val,
        "roundtrip failed for struct with 'key: value' field\nayml:\n{serialized}"
    );
}

#[test]
fn de_option_quoted_null_key_as_map() {
    let input = "\"null\": 5";
    let result: Option<HashMap<String, i32>> = from_str(input).unwrap();
    assert!(
        result.is_some(),
        "expected Some for '\"null\": 5' as Option<Map>"
    );
    let map = result.unwrap();
    assert_eq!(map.get("null"), Some(&5));
}

#[test]
fn de_option_plain_null_is_none() {
    let result: Option<i32> = from_str("null").unwrap();
    assert_eq!(result, None);
}

#[test]
fn ser_unit_struct_as_key_rejected() {
    #[derive(Serialize, PartialEq, Eq, Hash)]
    struct Marker;

    let mut map = HashMap::new();
    map.insert(Marker, "value");
    let result = to_string(&map);
    assert!(
        result.is_err(),
        "expected error when using unit struct as map key"
    );
}

#[test]
fn ser_flow_indicator_in_string_roundtrip() {
    let cases = vec!["a,b", "a]b", "a}b", "[hello", "{hello"];
    for input in cases {
        let serialized = to_string(&input).unwrap();
        let back: String = from_str(&serialized).unwrap();
        assert_eq!(
            back, input,
            "roundtrip failed for string with flow indicator: {input}\nayml:\n{serialized}"
        );
    }
}

#[test]
fn ser_flow_indicators_in_map_values() {
    let mut map = BTreeMap::new();
    map.insert("key".to_string(), "a,b".to_string());
    let serialized = to_string(&map).unwrap();
    let back: BTreeMap<String, String> = from_str(&serialized).unwrap();
    assert_eq!(
        back, map,
        "roundtrip failed for map with flow indicator in value\nayml:\n{serialized}"
    );
}

#[test]
fn ser_flow_indicator_in_seq_element() {
    let items = vec!["a,b".to_string(), "c]d".to_string()];
    let serialized = to_string(&items).unwrap();
    let back: Vec<String> = from_str(&serialized).unwrap();
    assert_eq!(
        back, items,
        "roundtrip failed for seq with flow indicators\nayml:\n{serialized}"
    );
}

#[test]
fn value_display_str_null_ambiguity() {
    let v = Value::Str("null".to_string());
    let display = format!("{v}");
    assert_ne!(
        display, "null",
        "Str(\"null\") should not display as bare 'null'"
    );
}

#[test]
fn value_display_float_int_ambiguity() {
    let v = Value::Float(1.0);
    let display = format!("{v}");
    assert!(
        display.contains('.'),
        "Float(1.0) should display with decimal point, got: {display}"
    );
}

#[test]
fn commented_value_display_str_null_ambiguity() {
    let v = Commented {
        top_comment: None,
        value: CommentedValueKind::Str("null".to_string()),
        inline_comment: None,
    };
    let display = format!("{v}");
    assert_ne!(
        display, "null",
        "Str(\"null\") should not display as bare 'null'"
    );
}

#[test]
fn commented_value_display_float_int_ambiguity() {
    let v = Commented {
        top_comment: None,
        value: CommentedValueKind::Float(1.0),
        inline_comment: None,
    };
    let display = format!("{v}");
    assert!(
        display.contains('.'),
        "Float(1.0) should display with decimal point, got: {display}"
    );
}

#[test]
fn de_reject_unquoted_null_key() {
    let input = "null: 5";
    // When target is Value (untyped), null key should be rejected
    let result: Result<Value, _> = from_str(input);
    assert!(
        result.is_err(),
        "expected error for unquoted null mapping key"
    );
}

#[test]
fn de_reject_unquoted_nan_key() {
    let input = "nan: 5";
    let result: Result<Value, _> = from_str(input);
    assert!(
        result.is_err(),
        "expected error for unquoted nan mapping key"
    );
}

#[test]
fn de_reject_unquoted_inf_key() {
    let input = "inf: 5";
    let result: Result<Value, _> = from_str(input);
    assert!(
        result.is_err(),
        "expected error for unquoted inf mapping key"
    );
}

#[test]
fn de_reject_unquoted_float_key() {
    let input = "3.14: 5";
    let result: Result<Value, _> = from_str(input);
    assert!(
        result.is_err(),
        "expected error for unquoted float mapping key"
    );
}

#[test]
fn de_allow_quoted_null_key() {
    let input = "\"null\": 5";
    let result: Result<HashMap<String, i32>, _> = from_str(input);
    assert!(result.is_ok(), "quoted null key should be allowed");
}

#[test]
fn de_allow_quoted_nan_key() {
    let input = "\"nan\": 5";
    let result: Result<HashMap<String, i32>, _> = from_str(input);
    assert!(result.is_ok(), "quoted nan key should be allowed");
}

#[test]
fn ser_trailing_newline_roundtrip() {
    let input = "hello\n";
    let serialized = to_string(&input).unwrap();
    let back: String = from_str(&serialized).unwrap();
    assert_eq!(
        back, input,
        "trailing newline roundtrip failed\nayml:\n{serialized}"
    );
}

#[test]
fn ser_double_trailing_newline_roundtrip() {
    let input = "hello\n\n";
    let serialized = to_string(&input).unwrap();
    let back: String = from_str(&serialized).unwrap();
    assert_eq!(
        back, input,
        "double trailing newline roundtrip failed\nayml:\n{serialized}"
    );
}

#[test]
fn ser_only_newline_roundtrip() {
    let input = "\n";
    let serialized = to_string(&input).unwrap();
    let back: String = from_str(&serialized).unwrap();
    assert_eq!(
        back, input,
        "single newline roundtrip failed\nayml:\n{serialized}"
    );
}

#[test]
fn value_display_leading_space() {
    let v = Value::Str(" hello".to_string());
    let display = format!("{v}");
    assert!(
        display.starts_with('"'),
        "Str(\" hello\") should be quoted, got: {display}"
    );
}

#[test]
fn value_display_trailing_space() {
    let v = Value::Str("hello ".to_string());
    let display = format!("{v}");
    assert!(
        display.starts_with('"'),
        "Str(\"hello \") should be quoted, got: {display}"
    );
}

#[test]
fn value_display_dash_space_prefix() {
    let v = Value::Str("- item".to_string());
    let display = format!("{v}");
    assert!(
        display.starts_with('"'),
        "Str(\"- item\") should be quoted, got: {display}"
    );
}

#[test]
fn looks_like_number_infinity_not_ayml_float() {
    // "infinity" is accepted by Rust's f64::parse but is NOT an AYML float.
    // The spec only recognizes "inf", "+inf", "-inf", "nan".
    // "infinity" should display as a bare string, not quoted.
    let v = Value::Str("infinity".into());
    let display = format!("{v}");
    assert_eq!(
        display, "infinity",
        "\"infinity\" is not an AYML keyword and should not be quoted, got: {display}"
    );
}

#[test]
fn looks_like_number_nan_caps_not_ayml_float() {
    let v = Value::Str("NaN".into());
    let display = format!("{v}");
    assert_eq!(
        display, "NaN",
        "\"NaN\" is not an AYML keyword (only \"nan\" is) and should not be quoted, got: {display}"
    );
}

#[test]
fn looks_like_number_dot_prefix_not_ayml_float() {
    // ".5" is accepted by Rust's f64::parse but AYML requires digits before the dot
    let v = Value::Str(".5".into());
    let display = format!("{v}");
    assert_eq!(
        display, ".5",
        "\".5\" is not an AYML float and should not be quoted, got: {display}"
    );
}

#[test]
fn looks_like_number_trailing_dot_not_ayml_float() {
    // "5." is accepted by Rust's f64::parse but AYML requires digits after the dot
    let v = Value::Str("5.".into());
    let display = format!("{v}");
    assert_eq!(
        display, "5.",
        "\"5.\" is not an AYML float and should not be quoted, got: {display}"
    );
}

#[test]
fn bare_dash_string_roundtrips() {
    // Fuzz crash: input "-#V@" parses as Str("-") because # starts a comment.
    // The serializer must quote "-" since a bare `-` requires a following ns-char.
    let v: Value = from_str("-#V@").unwrap();
    assert_eq!(v, Value::Str("-".into()));
    let s = to_string(&v).unwrap();
    assert_eq!(s, "\"-\"\n");
    let rt: Value = from_str(&s).unwrap();
    assert_eq!(v, rt);
}

#[test]
fn bare_colon_string_roundtrips() {
    // Same class of bug: a bare `:` also requires a following ns-char.
    let s = to_string(&Value::Str(":".into())).unwrap();
    assert_eq!(s, "\":\"\n");
    let rt: Value = from_str(&s).unwrap();
    assert_eq!(rt, Value::Str(":".into()));
}

#[test]
fn colon_tab_mid_string_quoted() {
    // `:\t` mid-string would be parsed as a mapping value indicator.
    let v = Value::Str("foo:\tbar".into());
    let s = to_string(&v).unwrap();
    assert!(s.starts_with('"'), "should be quoted: {s}");
    let rt: Value = from_str(&s).unwrap();
    assert_eq!(v, rt);
}

#[test]
fn mapping_key_with_newline_roundtrips() {
    // Fuzz crash: a mapping key containing \n was triple-quoted by the serializer,
    // but triple-quoted strings span multiple lines and can't be mapping keys.
    // The serializer must use single-line double-quoting for keys with \n.
    let mut inner = IndexMap::new();
    inner.insert("v".to_string(), Value::Int(1));
    let mut outer = IndexMap::new();
    outer.insert("a\nb".to_string(), Value::Map(inner));
    let v = Value::Map(outer);
    let s = to_string(&v).unwrap();
    assert!(
        s.starts_with("\"a\\nb\":"),
        "key should be double-quoted: {s}"
    );
    let rt: Value = from_str(&s).unwrap();
    assert_eq!(v, rt);
}

#[test]
fn bare_numeric_prefix_strings_roundtrip() {
    // Bare prefixes like "0b" are not valid integers (no digits after prefix).
    // The deserializer correctly treats them as strings, so the serializer
    // does not need to quote them.
    for s in ["0b", "0o", "0x", "+0b", "-0x"] {
        let v = Value::Str(s.into());
        let ser = to_string(&v).unwrap();
        let rt: Value = from_str(&ser).unwrap();
        assert_eq!(v, rt);
    }
}

#[test]
fn overflowing_digit_string_roundtrips() {
    // Fuzz crash: "888...888" (30 digits) exceeds i64 range. The serializer
    // must quote it so it roundtrips as a string, not fail as integer overflow.
    let digits = "8".repeat(30);
    let v = Value::Str(digits.clone());
    let s = to_string(&v).unwrap();
    assert!(
        s.starts_with('"'),
        "overflowing digits should be quoted: {s}"
    );
    let rt: Value = from_str(&s).unwrap();
    assert_eq!(v, rt);
}

#[test]
fn key_deserialize_any_does_not_spuriously_dedup() {
    // Issue #11: with the old byte-recording mechanism, any key whose
    // `Deserialize` impl called `deserialize_any` (custom impls, serde's
    // `Content`-buffer replay for `#[serde(untagged)]`, etc.) was observed
    // as an empty string by the outer `MapAccess`. The second such key then
    // tripped the duplicate-key check with a spurious `duplicate key \`\``.
    use std::fmt;
    #[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
    struct AnyKey(String);
    impl<'de> serde::Deserialize<'de> for AnyKey {
        fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            struct V;
            impl<'de> serde::de::Visitor<'de> for V {
                type Value = AnyKey;
                fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                    write!(f, "any key")
                }
                fn visit_str<E: serde::de::Error>(self, s: &str) -> Result<AnyKey, E> {
                    Ok(AnyKey(s.into()))
                }
                fn visit_string<E: serde::de::Error>(self, s: String) -> Result<AnyKey, E> {
                    Ok(AnyKey(s))
                }
            }
            d.deserialize_any(V)
        }
    }
    let input = "alpha: 1\nbeta: 2\ngamma: 3\n";
    let parsed: BTreeMap<AnyKey, i32> = from_str(input).unwrap();
    assert_eq!(parsed.len(), 3);
    assert_eq!(parsed[&AnyKey("alpha".into())], 1);
    assert_eq!(parsed[&AnyKey("beta".into())], 2);
    assert_eq!(parsed[&AnyKey("gamma".into())], 3);
}

#[test]
fn quoted_and_bare_same_key_is_duplicate() {
    // Dedup is now done on the parsed key text, not raw source bytes, so a
    // quoted key and a bare key that resolve to the same string collide as
    // they should. (Old behaviour: hashed on raw bytes, missed the dup.)
    let input = "\"foo\": 1\nfoo: 2\n";
    let result: Result<HashMap<String, i32>, _> = from_str(input);
    assert!(
        result
            .as_ref()
            .is_err_and(|e| e.to_string().contains("duplicate key")),
        "expected duplicate-key error, got: {result:?}"
    );
}

#[test]
fn non_printable_unicode_escaped_in_serializer() {
    // Fuzz crash: U+FFFF is excluded from c-printable but the serializer
    // emitted it as a literal character. It must be escaped as \uffff.
    let v = Value::Str("\u{FFFF}".into());
    let s = to_string(&v).unwrap();
    assert!(s.contains("\\uffff"), "U+FFFF should be escaped: {s:?}");
    let rt: Value = from_str(&s).unwrap();
    assert_eq!(v, rt);

    // U+FFFE is also excluded from c-printable
    let v2 = Value::Str("\u{FFFE}".into());
    let s2 = to_string(&v2).unwrap();
    assert!(s2.contains("\\ufffe"), "U+FFFE should be escaped: {s2:?}");
    let rt2: Value = from_str(&s2).unwrap();
    assert_eq!(v2, rt2);
}
