use ayml_core::{Key, Node, Value};

use ayml_lsp::convert;

#[test]
fn i64_max() {
    let node = Node::new(Value::Int(i64::MAX));
    let json = convert::node_to_json(&node);
    assert_eq!(json.as_i64(), Some(i64::MAX));
}

#[test]
fn i64_min() {
    let node = Node::new(Value::Int(i64::MIN));
    let json = convert::node_to_json(&node);
    assert_eq!(json.as_i64(), Some(i64::MIN));
}

#[test]
fn f64_max() {
    let node = Node::new(Value::Float(f64::MAX));
    let json = convert::node_to_json(&node);
    assert!(json.is_f64());
    assert_eq!(json.as_f64().unwrap().to_bits(), f64::MAX.to_bits());
}

#[test]
fn f64_infinity_becomes_string() {
    let node = Node::new(Value::Float(f64::INFINITY));
    let json = convert::node_to_json(&node);
    assert_eq!(json.as_str(), Some("inf"));
}

#[test]
fn f64_neg_infinity_becomes_string() {
    let node = Node::new(Value::Float(f64::NEG_INFINITY));
    let json = convert::node_to_json(&node);
    assert_eq!(json.as_str(), Some("-inf"));
}

#[test]
fn f64_nan_becomes_string() {
    let node = Node::new(Value::Float(f64::NAN));
    let json = convert::node_to_json(&node);
    assert_eq!(json.as_str(), Some("nan"));
}

#[test]
fn large_i64_survives_json_roundtrip() {
    let node = Node::new(Value::Int(i64::MAX));
    let json = convert::node_to_json(&node);
    let serialized = serde_json::to_string(&json).unwrap();
    let deserialized: serde_json::Value = serde_json::from_str(&serialized).unwrap();
    assert_eq!(deserialized.as_i64(), Some(i64::MAX));
}

#[test]
fn keys_are_strings() {
    let node = ayml_core::parse("1: a\ntrue: b\n\"x y\": c\n").unwrap();
    let json = convert::node_to_json(&node);
    assert_eq!(json, serde_json::json!({"1": "a", "true": "b", "x y": "c"}));
}

#[test]
fn nested_values_convert() {
    let map: Value = [(
        Key::new("a"),
        Node::new(Value::Seq(vec![Node::new(Value::Null)])),
    )]
    .into_iter()
    .collect();
    let json = convert::node_to_json(&Node::new(map));
    assert_eq!(json, serde_json::json!({"a": [null]}));
}
