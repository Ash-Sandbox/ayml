use ayml::from_str;
use serde::Deserialize;
use serde_json::Value;

#[test]
fn triple_quoted_in_flow_mapping() {
    let input = "- {name: \"hello\", bio: \"\"\"\n    some text\n    \"\"\"}\n";
    let result: Vec<Value> = from_str(input).unwrap();
    assert_eq!(result[0]["name"], "hello");
    assert_eq!(result[0]["bio"], "some text");
}

#[test]
fn triple_quoted_in_flow_sequence() {
    let input = "[\"\"\"\n  hello\n  \"\"\", 42]\n";
    let result: Vec<Value> = from_str(input).unwrap();
    assert_eq!(result[0], "hello");
    assert_eq!(result[1], 42);
}

#[test]
fn triple_quoted_before_comma_in_flow_mapping() {
    let input = "{a: \"\"\"\n  text\n  \"\"\", b: 1}\n";
    #[derive(Deserialize, Debug)]
    struct T {
        a: String,
        b: i32,
    }
    let result: T = from_str(input).unwrap();
    assert_eq!(result.a, "text");
    assert_eq!(result.b, 1);
}

#[test]
fn triple_quoted_last_in_flow_mapping() {
    let input = "{a: 1, b: \"\"\"\n  text\n  \"\"\"}\n";
    #[derive(Deserialize, Debug)]
    struct T {
        a: i32,
        b: String,
    }
    let result: T = from_str(input).unwrap();
    assert_eq!(result.a, 1);
    assert_eq!(result.b, "text");
}

#[test]
fn triple_quoted_last_in_flow_sequence() {
    let input = "[1, \"\"\"\n  text\n  \"\"\"]\n";
    let result: Vec<Value> = from_str(input).unwrap();
    assert_eq!(result[0], 1);
    assert_eq!(result[1], "text");
}
