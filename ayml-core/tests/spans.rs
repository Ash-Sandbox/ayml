//! Node and key spans.

mod common;

use ayml_core::{Node, Span};
use common::{get, items, ok};

#[track_caller]
fn text_of(input: &str, span: Span) -> &str {
    &input[span.start..span.end]
}

#[track_caller]
fn key_span(node: &Node, key: &str) -> Span {
    node.value
        .as_mapping()
        .expect("mapping")
        .get_key_value(key)
        .expect("key")
        .0
        .span
}

#[test]
fn bare_scalar_spans() {
    for input in [
        "hello world",
        "null",
        "true",
        "-42",
        "0x1F",
        "3.25e2",
        "nan",
    ] {
        assert_eq!(text_of(input, ok(input).span), input);
    }
}

#[test]
fn bare_scalar_span_excludes_trailing_whitespace_and_comment() {
    let input = "k: value   # comment\n";
    assert_eq!(text_of(input, get(&ok(input), "k").span), "value");
}

#[test]
fn double_quoted_span_includes_quotes() {
    let input = "k: \"a\\tb\"\n";
    assert_eq!(text_of(input, get(&ok(input), "k").span), "\"a\\tb\"");
}

#[test]
fn triple_quoted_span_includes_delimiters() {
    let input = "k: \"\"\"\n  line\n  \"\"\" # c\n";
    assert_eq!(
        text_of(input, get(&ok(input), "k").span),
        "\"\"\"\n  line\n  \"\"\""
    );
}

#[test]
fn key_spans() {
    let input = "plain: 1\n\"quoted key\" : 2\n";
    let root = ok(input);
    assert_eq!(text_of(input, key_span(&root, "plain")), "plain");
    assert_eq!(
        text_of(input, key_span(&root, "quoted key")),
        "\"quoted key\""
    );
}

#[test]
fn flow_key_spans() {
    let input = "{a: 1, \"b\": 2}";
    let root = ok(input);
    assert_eq!(text_of(input, key_span(&root, "a")), "a");
    assert_eq!(text_of(input, key_span(&root, "b")), "\"b\"");
}

#[test]
fn block_mapping_span_runs_from_first_key_to_last_value() {
    let input = "# lead\nname: John\nage: 30 # c\n# trailing\n";
    let root = ok(input);
    assert_eq!(text_of(input, root.span), "name: John\nage: 30");
    assert_eq!(text_of(input, get(&root, "age").span), "30");
}

#[test]
fn nested_mapping_span() {
    let input = "outer:\n  inner: 42\n  other: x\nnext: 1\n";
    let root = ok(input);
    assert_eq!(
        text_of(input, get(&root, "outer").span),
        "inner: 42\n  other: x"
    );
}

#[test]
fn block_sequence_spans() {
    let input = "- apple\n- banana\n";
    let root = ok(input);
    assert_eq!(text_of(input, root.span), "- apple\n- banana");
    let entries = items(&root);
    assert_eq!(text_of(input, entries[0].span), "apple");
    assert_eq!(text_of(input, entries[1].span), "banana");
}

#[test]
fn compact_collection_spans() {
    let input = "- name: Widget\n  qty: 5\n- - a\n  - b\n";
    let root = ok(input);
    let entries = items(&root);
    assert_eq!(text_of(input, entries[0].span), "name: Widget\n  qty: 5");
    assert_eq!(text_of(input, get(&entries[0], "qty").span), "5");
    assert_eq!(text_of(input, entries[1].span), "- a\n  - b");
}

#[test]
fn flow_collection_spans() {
    let input = "k: [1, {a: b}, \"c\"] # c\n";
    let root = ok(input);
    let seq = get(&root, "k");
    assert_eq!(text_of(input, seq.span), "[1, {a: b}, \"c\"]");
    let entries = items(seq);
    assert_eq!(text_of(input, entries[0].span), "1");
    assert_eq!(text_of(input, entries[1].span), "{a: b}");
    assert_eq!(text_of(input, get(&entries[1], "a").span), "b");
    assert_eq!(text_of(input, entries[2].span), "\"c\"");
}

#[test]
fn multi_line_flow_span() {
    let input = "k: {\n  a: 1,\n}\n";
    assert_eq!(text_of(input, get(&ok(input), "k").span), "{\n  a: 1,\n}");
}

#[test]
fn spans_are_byte_offsets_with_unicode() {
    let input = "ключ: значение\n";
    let root = ok(input);
    assert_eq!(text_of(input, key_span(&root, "ключ")), "ключ");
    assert_eq!(text_of(input, get(&root, "ключ").span), "значение");
}

#[test]
fn built_nodes_have_empty_spans() {
    assert_eq!(Node::default().span, Span::new(0, 0));
}
