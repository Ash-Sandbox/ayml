use ayml_core::{
    error::Span,
    parse,
    value::{MapKey, Value},
};

// ── Helper ──────────────────────────────────────────────────────

/// Assert that a node's span covers exactly the expected substring.
fn assert_span_text(input: &str, span: Span, expected: &str) {
    let actual = &input[span.start..span.end];
    assert_eq!(
        actual, expected,
        "span {}..{} is {:?}, expected {:?}",
        span.start, span.end, actual, expected
    );
}

// ── Scalars ─────────────────────────────────────────────────────

#[test]
fn bare_scalar_span() {
    let input = "hello";
    let node = parse(input).unwrap();
    assert_span_text(input, node.span, "hello");
}

#[test]
fn bare_null_span() {
    let input = "null";
    let node = parse(input).unwrap();
    assert!(node.value.is_null());
    assert_span_text(input, node.span, "null");
}

#[test]
fn bare_bool_span() {
    let input = "true";
    let node = parse(input).unwrap();
    assert_span_text(input, node.span, "true");
}

#[test]
fn bare_integer_span() {
    let input = "42";
    let node = parse(input).unwrap();
    assert_span_text(input, node.span, "42");
}

#[test]
fn bare_float_span() {
    let input = "3.14";
    let node = parse(input).unwrap();
    assert_span_text(input, node.span, "3.14");
}

#[test]
fn double_quoted_span() {
    let input = "\"hello world\"";
    let node = parse(input).unwrap();
    assert_span_text(input, node.span, "\"hello world\"");
}

#[test]
fn triple_quoted_span() {
    let input = "\"\"\"\n  line one\n  line two\n  \"\"\"";
    let node = parse(input).unwrap();
    assert_span_text(input, node.span, input);
}

// ── Mappings ────────────────────────────────────────────────────

#[test]
fn mapping_root_span() {
    let input = "a: 1\nb: 2";
    let node = parse(input).unwrap();
    // Root span covers the entire document.
    assert_span_text(input, node.span, input);
}

#[test]
fn mapping_value_spans() {
    let input = "name: Alice\nage: 30";
    let node = parse(input).unwrap();
    let map = node.value.as_mapping().unwrap();

    let name = &map[&MapKey::String("name".into())];
    assert_eq!(name.value, Value::Str("Alice".into()));
    assert_span_text(input, name.span, "Alice");

    let age = &map[&MapKey::String("age".into())];
    assert_eq!(age.value, Value::Int(30));
    assert_span_text(input, age.span, "30");
}

#[test]
fn nested_mapping_value_span() {
    let input = "outer:\n  inner: hello";
    let node = parse(input).unwrap();
    let outer_map = node.value.as_mapping().unwrap();
    let inner_node = &outer_map[&MapKey::String("outer".into())];
    let inner_map = inner_node.value.as_mapping().unwrap();
    let hello = &inner_map[&MapKey::String("inner".into())];
    assert_span_text(input, hello.span, "hello");
}

// ── Sequences ───────────────────────────────────────────────────

#[test]
fn sequence_root_span() {
    let input = "- a\n- b\n- c";
    let node = parse(input).unwrap();
    assert_span_text(input, node.span, input);
}

#[test]
fn sequence_element_spans() {
    let input = "- foo\n- 42\n- true";
    let node = parse(input).unwrap();
    let seq = node.value.as_sequence().unwrap();

    assert_eq!(seq[0].value, Value::Str("foo".into()));
    assert_span_text(input, seq[0].span, "foo");

    assert_eq!(seq[1].value, Value::Int(42));
    assert_span_text(input, seq[1].span, "42");

    assert_eq!(seq[2].value, Value::Bool(true));
    assert_span_text(input, seq[2].span, "true");
}

// ── Flow Collections ────────────────────────────────────────────

#[test]
fn flow_sequence_span() {
    let input = "[1, 2, 3]";
    let node = parse(input).unwrap();
    assert_span_text(input, node.span, "[1, 2, 3]");
}

#[test]
fn flow_sequence_element_spans() {
    let input = "[10, 20, 30]";
    let node = parse(input).unwrap();
    let seq = node.value.as_sequence().unwrap();
    assert_span_text(input, seq[0].span, "10");
    assert_span_text(input, seq[1].span, "20");
    assert_span_text(input, seq[2].span, "30");
}

#[test]
fn flow_mapping_span() {
    let input = "{a: 1, b: 2}";
    let node = parse(input).unwrap();
    assert_span_text(input, node.span, "{a: 1, b: 2}");
}

#[test]
fn flow_mapping_value_spans() {
    let input = "{x: hello, y: 99}";
    let node = parse(input).unwrap();
    let map = node.value.as_mapping().unwrap();

    let x = &map[&MapKey::String("x".into())];
    assert_span_text(input, x.span, "hello");

    let y = &map[&MapKey::String("y".into())];
    assert_span_text(input, y.span, "99");
}

// ── With Comments ───────────────────────────────────────────────

#[test]
fn span_with_leading_comment() {
    let input = "# comment\nvalue: 42";
    let node = parse(input).unwrap();
    // The root node's span includes the comment.
    assert_span_text(input, node.span, input);
}

#[test]
fn span_with_inline_comment() {
    let input = "port: 8080 # default";
    let node = parse(input).unwrap();
    let map = node.value.as_mapping().unwrap();
    let port = &map[&MapKey::String("port".into())];
    // The value span covers just the value, not the comment.
    assert_span_text(input, port.span, "8080");
}

// ── Compact Mapping in Sequence ─────────────────────────────────

#[test]
fn compact_mapping_span() {
    let input = "- name: Alice\n  age: 30";
    let node = parse(input).unwrap();
    let seq = node.value.as_sequence().unwrap();
    // The compact mapping node should span both entries.
    let map_node = &seq[0];
    let map = map_node.value.as_mapping().unwrap();
    assert_eq!(map.len(), 2);
    // The individual value spans should be correct.
    let name = &map[&MapKey::String("name".into())];
    assert_span_text(input, name.span, "Alice");
    let age = &map[&MapKey::String("age".into())];
    assert_span_text(input, age.span, "30");
}

// ── Span Invariants ─────────────────────────────────────────────

#[test]
fn span_start_before_end() {
    let input = "key: value";
    let node = parse(input).unwrap();
    assert!(node.span.start <= node.span.end);
}

#[test]
fn span_within_input() {
    let input = "items:\n  - one\n  - two\n  - three";
    let node = parse(input).unwrap();
    assert!(node.span.end <= input.len());

    let map = node.value.as_mapping().unwrap();
    let items = &map[&MapKey::String("items".into())];
    assert!(items.span.end <= input.len());

    let seq = items.value.as_sequence().unwrap();
    for item in seq {
        assert!(item.span.start <= item.span.end);
        assert!(item.span.end <= input.len());
    }
}
