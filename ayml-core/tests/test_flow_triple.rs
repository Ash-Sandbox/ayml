use ayml_core::{MapKey, Value, parse};

#[test]
fn triple_quoted_in_flow_mapping() {
    let input = "- {name: \"hello\", bio: \"\"\"\n    some text\n    \"\"\"}\n";
    let node = parse(input).unwrap();
    let seq = node.value.as_sequence().unwrap();
    let map = seq[0].value.as_mapping().unwrap();
    let bio = &map[&MapKey::String("bio".into())];
    assert_eq!(bio.value, Value::Str("some text".into()));
}

#[test]
fn triple_quoted_in_flow_sequence() {
    let input = "[\"\"\"\n  hello\n  \"\"\", 42]\n";
    let node = parse(input).unwrap();
    let seq = node.value.as_sequence().unwrap();
    assert_eq!(seq[0].value, Value::Str("hello".into()));
    assert_eq!(seq[1].value, Value::Int(42));
}

#[test]
fn triple_quoted_before_comma_in_flow_mapping() {
    let input = "{a: \"\"\"\n  text\n  \"\"\", b: 1}\n";
    let node = parse(input).unwrap();
    let map = node.value.as_mapping().unwrap();
    assert_eq!(
        map[&MapKey::String("a".into())].value,
        Value::Str("text".into())
    );
    assert_eq!(map[&MapKey::String("b".into())].value, Value::Int(1));
}

#[test]
fn triple_quoted_last_in_flow_mapping() {
    let input = "{a: 1, b: \"\"\"\n  text\n  \"\"\"}\n";
    let node = parse(input).unwrap();
    let map = node.value.as_mapping().unwrap();
    assert_eq!(map[&MapKey::String("a".into())].value, Value::Int(1));
    assert_eq!(
        map[&MapKey::String("b".into())].value,
        Value::Str("text".into())
    );
}

#[test]
fn triple_quoted_last_in_flow_sequence() {
    let input = "[1, \"\"\"\n  text\n  \"\"\"]\n";
    let node = parse(input).unwrap();
    let seq = node.value.as_sequence().unwrap();
    assert_eq!(seq[0].value, Value::Int(1));
    assert_eq!(seq[1].value, Value::Str("text".into()));
}

#[test]
fn policy_file() {
    let input = include_str!("../../files/policy.ayml");
    let result = parse(input);
    assert!(result.is_ok(), "parse failed: {}", result.unwrap_err());
}
