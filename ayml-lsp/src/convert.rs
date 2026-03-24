use ayml_core::value::{Node, Value};
use serde_json::json;

/// Convert an AYML [`Node`] into a [`serde_json::Value`], discarding comments.
pub fn node_to_json(node: &Node) -> serde_json::Value {
    value_to_json(&node.value)
}

fn value_to_json(value: &Value) -> serde_json::Value {
    match value {
        Value::Null => serde_json::Value::Null,
        Value::Bool(b) => json!(b),
        Value::Int(i) => json!(i),
        Value::Float(f) => {
            // JSON has no representation for inf/nan. Convert to the AYML
            // bare-word form so schema validation sees a string rather than null.
            if f.is_nan() {
                json!("nan")
            } else if *f == f64::INFINITY {
                json!("inf")
            } else if *f == f64::NEG_INFINITY {
                json!("-inf")
            } else {
                json!(f)
            }
        }
        Value::Str(s) => json!(s),
        Value::Seq(items) => serde_json::Value::Array(items.iter().map(node_to_json).collect()),
        Value::Map(map) => {
            let obj = map
                .iter()
                .map(|(k, v)| (k.to_string(), node_to_json(v)))
                .collect();
            serde_json::Value::Object(obj)
        }
    }
}
