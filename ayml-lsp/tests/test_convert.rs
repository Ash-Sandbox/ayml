#[path = "../src/convert.rs"]
mod convert;

use ayml_core::{Node, Value};

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
    assert_eq!(json.as_f64().unwrap(), f64::MAX);
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
