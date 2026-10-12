//! Property tests for the serde layer.
//!
//! * Typed values are round-tripped correctly
//! * Untyped values are round-tripped correctly
//! * comments on `Commented<T>` fields are round-tripped correctly
//! * JSON values are round-tripped correctly
//! * deserialization errors contain locations
//! * malformed input does not cause a panic

use std::collections::BTreeMap;

use ayml::{Commented, Node, Value};
use ayml_testgen::config;
use ayml_testgen::mutate::arb_mutated_doc;
use ayml_testgen::render::arb_doc;
use ayml_testgen::strings::{arb_comment, arb_comment_line, arb_f64, arb_i64, arb_key, arb_string};
use ayml_testgen::tree::{arb_plain_node, diff_values_only};
use proptest::prelude::*;
use proptest_derive::Arbitrary;
use serde::{Deserialize, Serialize};

fn finite_f64() -> impl Strategy<Value = f64> {
    arb_f64().prop_filter("finite", |f| f.is_finite())
}

fn finite_f32() -> impl Strategy<Value = f32> {
    any::<f32>().prop_filter("finite", |f| f.is_finite())
}

fn string_map() -> impl Strategy<Value = BTreeMap<String, i64>> {
    prop::collection::btree_map(arb_key(), arb_i64(), 0..4)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Arbitrary)]
struct Marker;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Arbitrary)]
struct Meters(#[proptest(strategy = "finite_f64()")] f64);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Arbitrary)]
struct Pair(i32, #[proptest(strategy = "arb_string()")] String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Arbitrary)]
enum Color {
    Red,
    Green,
    #[serde(rename = "true")]
    True,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Arbitrary)]
enum Shape {
    Empty,
    Circle(#[proptest(strategy = "finite_f64()")] f64),
    Label(#[proptest(strategy = "arb_string()")] String),
    Move(i32, i32),
    Rect {
        w: u32,
        h: u32,
        #[proptest(strategy = "proptest::option::of(arb_string())")]
        label: Option<String>,
    },
    Many(Vec<Pair>),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Arbitrary)]
struct Inner {
    #[proptest(strategy = "arb_string()")]
    name: String,
    #[proptest(strategy = "proptest::option::of(arb_i64())")]
    count: Option<i64>,
    #[proptest(strategy = "prop::collection::vec(arb_string(), 0..3)")]
    tags: Vec<String>,
}

/// Every serde data model shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Arbitrary)]
struct Everything {
    b: bool,
    i8: i8,
    i16: i16,
    i32: i32,
    i64: i64,
    #[proptest(strategy = "i128::from(i64::MIN)..=i128::from(i64::MAX)")]
    i128: i128,
    u8: u8,
    u16: u16,
    u32: u32,
    #[proptest(strategy = "0..=(i64::MAX as u64)")]
    u64: u64,
    #[proptest(strategy = "finite_f32()")]
    f32: f32,
    #[proptest(strategy = "finite_f64()")]
    f64: f64,
    ch: char,
    #[proptest(strategy = "arb_string()")]
    s: String,
    #[proptest(strategy = "proptest::option::of(arb_string())")]
    opt: Option<String>,
    unit: (),
    marker: Marker,
    meters: Meters,
    pair: Pair,
    tuple: (i8, bool, Color),
    array: [u8; 3],
    #[proptest(
        strategy = "prop::collection::vec(prop::collection::vec(arb_string(), 0..3), 0..3)"
    )]
    nested: Vec<Vec<String>>,
    #[proptest(strategy = "string_map()")]
    map: BTreeMap<String, i64>,
    int_keys: BTreeMap<i32, bool>,
    bool_keys: BTreeMap<bool, u8>,
    enum_keys: BTreeMap<Color, i16>,
    shapes: Vec<Shape>,
    inner: Inner,
    maybe_inner: Option<Inner>,
    #[proptest(
        strategy = "prop::collection::vec(any::<u8>(), 0..5).prop_map(serde_bytes::ByteBuf::from)"
    )]
    bytes: serde_bytes::ByteBuf,
}

fn commented<T: std::fmt::Debug + Clone>(
    value: impl Strategy<Value = T>,
    trailing: bool,
) -> impl Strategy<Value = Commented<T>> {
    let trailing = if trailing {
        arb_comment().boxed()
    } else {
        Just(None).boxed()
    };
    (
        arb_comment(),
        proptest::option::of(arb_comment_line()),
        trailing,
        value,
    )
        .prop_map(
            |(comment, inline_comment, trailing_comment, value)| Commented {
                comment,
                inline_comment,
                trailing_comment,
                value,
            },
        )
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Arbitrary)]
struct CommentedConfig {
    #[proptest(strategy = "commented(any::<u16>(), false)")]
    port: Commented<u16>,
    #[proptest(strategy = "commented(arb_string(), false)")]
    host: Commented<String>,
    #[proptest(
        strategy = "commented(prop::collection::vec(commented(arb_string(), false), 0..4), true)"
    )]
    tags: Commented<Vec<Commented<String>>>,
    #[proptest(strategy = "commented(any::<Inner>(), true)")]
    inner: Commented<Inner>,
    #[proptest(strategy = "prop::collection::vec(any::<Rule>(), 0..3)")]
    rules: Vec<Rule>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Arbitrary)]
struct Rule {
    #[proptest(strategy = "commented(arb_string(), false)")]
    host: Commented<String>,
    port: u16,
}

fn node_to_value(node: &Node) -> Value {
    Value::from(node)
}

fn values_equal_strict(a: &Value, b: &Value) -> Result<(), TestCaseError> {
    match diff_values_only(&Node::from(a), &Node::from(b)) {
        None => Ok(()),
        Some(d) => Err(TestCaseError::fail(d)),
    }
}

proptest! {
    #![proptest_config(config(1000))]

    #[test]
    fn typed_round_trip(value in any::<Everything>()) {
        let text = ayml::to_string(&value).map_err(|e| TestCaseError::fail(e.to_string()))?;
        let back: Everything = ayml::from_str(&text)
            .map_err(|e| TestCaseError::fail(format!("{e}\n--- text ---\n{text}")))?;
        prop_assert_eq!(&value, &back, "\n--- text ---\n{}", text);
    }

    #[test]
    fn serde_layer_matches_core(value in any::<Everything>()) {
        let node = ayml::to_node(&value).unwrap();
        let text = ayml::to_string(&value).unwrap();
        prop_assert_eq!(&text, &ayml_core::emit(&node));
        let parsed = ayml_core::parse(&text).unwrap();
        prop_assert_eq!(ayml::from_node::<Everything>(&parsed).unwrap(), value);
    }

    #[test]
    fn value_round_trip(node in arb_plain_node()) {
        let value = node_to_value(&node);
        let text = ayml::to_string(&value).unwrap();
        let back: Value = ayml::from_str(&text)
            .map_err(|e| TestCaseError::fail(format!("{e}\n--- text ---\n{text}")))?;
        values_equal_strict(&value, &back)?;
    }

    #[test]
    fn value_display_is_flow_ayml(node in arb_plain_node()) {
        let value = node_to_value(&node);
        let shown = value.to_string();
        prop_assert!(!shown.contains('\n') && !shown.contains('\r'));
        let back: Value = ayml::from_str(&shown)
            .map_err(|e| TestCaseError::fail(format!("{e}\n--- display ---\n{shown}")))?;
        values_equal_strict(&value, &back)?;
    }

    #[test]
    fn value_agrees_with_core(doc in arb_doc()) {
        let value: Value = ayml::from_str(&doc.text)
            .map_err(|e| TestCaseError::fail(format!("{e}\n--- text ---\n{}", doc.text)))?;
        values_equal_strict(&node_to_value(&doc.expected), &value)?;
    }

    #[test]
    fn commented_round_trip(config in any::<CommentedConfig>()) {
        let text = ayml::to_string(&config).unwrap();
        let back: CommentedConfig = ayml::from_str(&text)
            .map_err(|e| TestCaseError::fail(format!("{e}\n--- text ---\n{text}")))?;
        prop_assert_eq!(&config, &back, "\n--- text ---\n{}", text);
    }

    #[test]
    fn json_round_trip(json in arb_json()) {
        let text = ayml::to_string(&json).unwrap();
        let back: serde_json::Value = ayml::from_str(&text)
            .map_err(|e| TestCaseError::fail(format!("{e}\n--- text ---\n{text}")))?;
        prop_assert_eq!(&json, &back, "\n--- text ---\n{}", text);
    }

    #[test]
    fn errors_point_at_the_offending_value(
        keys in prop::collection::btree_set("[a-z]{1,6}", 1..6),
        bad in any::<prop::sample::Index>(),
        bad_value in prop_oneof![Just("\"80\""), Just("1.5"), Just("true"), Just("[1]"), Just("null"), Just("-1"), Just("70000")],
    ) {
        let keys: Vec<String> = keys.into_iter().collect();
        let bad = bad.index(keys.len());
        let text: String = keys
            .iter()
            .enumerate()
            .map(|(i, k)| if i == bad { format!("{k}: {bad_value}\n") } else { format!("{k}: 1\n") })
            .collect();
        let err = ayml::from_str::<BTreeMap<String, u16>>(&text).unwrap_err();
        let location = err.location().expect("location");
        prop_assert_eq!(location.line, bad + 1, "{}", err);
        prop_assert_eq!(location.column, keys[bad].len() + 3, "{}", err);
    }

    #[test]
    fn streaming_matches_tree(text in prop_oneof![
        arb_doc().prop_map(|d| d.text),
        arb_mutated_doc(),
    ]) {
        let streamed = ayml::from_str::<Value>(&text);
        match ayml_core::parse(&text) {
            Ok(tree) => {
                let from_tree = ayml::from_node::<Value>(&tree).unwrap();
                let streamed = streamed.map_err(|e| TestCaseError::fail(format!("{e}\n{text:?}")))?;
                values_equal_strict(&from_tree, &streamed)?;
            }
            Err(e) => {
                let streamed_err = streamed.err().map(|e| e.to_string());
                prop_assert_eq!(streamed_err, Some(e.to_string()), "{:?}", text);
            }
        }
    }

    #[test]
    fn typed_streaming_matches_tree(config in any::<CommentedConfig>(), value in any::<Everything>()) {
        let text = ayml::to_string(&config).unwrap();
        let tree = ayml_core::parse(&text).unwrap();
        prop_assert_eq!(ayml::from_str::<CommentedConfig>(&text).unwrap(), ayml::from_node::<CommentedConfig>(&tree).unwrap());
        let text = ayml::to_string(&value).unwrap();
        let tree = ayml_core::parse(&text).unwrap();
        prop_assert_eq!(ayml::from_str::<Everything>(&text).unwrap(), ayml::from_node::<Everything>(&tree).unwrap());
    }

    #[test]
    fn typed_errors_match_tree(text in arb_mutated_doc()) {
        let streamed = ayml::from_str::<Everything>(&text).map(drop).map_err(|e| e.to_string());
        let from_tree = ayml_core::parse(&text)
            .map_err(|e| ayml::Error::from(e).to_string())
            .and_then(|tree| ayml::from_node::<Everything>(&tree).map(drop).map_err(|e| e.to_string()));
        prop_assert_eq!(streamed.is_ok(), from_tree.is_ok(), "{:?} vs {:?}", streamed, from_tree);
    }

    #[test]
    fn mutated_documents_never_panic(text in arb_mutated_doc()) {
        let _ = ayml::from_str::<Value>(&text);
        let _ = ayml::from_str::<Everything>(&text);
        let _ = ayml::from_str::<CommentedConfig>(&text);
        let _ = ayml::from_str::<serde_json::Value>(&text);
    }
}

fn arb_json() -> impl Strategy<Value = serde_json::Value> {
    use serde_json::Value as J;
    let leaf = prop_oneof![
        Just(J::Null),
        any::<bool>().prop_map(J::Bool),
        arb_i64().prop_map(J::from),
        finite_f64().prop_map(J::from),
        arb_string().prop_map(J::String),
    ];
    leaf.prop_recursive(4, 32, 5, |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..5).prop_map(J::Array),
            prop::collection::vec((arb_key(), inner), 0..5)
                .prop_map(|entries| J::Object(entries.into_iter().collect())),
        ]
    })
}

#[test]
fn nan_and_infinity_round_trip_typed() {
    for v in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -0.0] {
        let back: f64 = ayml::from_str(&ayml::to_string(&v).unwrap()).unwrap();
        if v.is_nan() {
            assert!(back.is_nan());
        } else {
            assert_eq!(back.to_bits(), v.to_bits());
        }
    }
    let back: f32 = ayml::from_str(&ayml::to_string(&f32::NAN).unwrap()).unwrap();
    assert!(back.is_nan());
}
