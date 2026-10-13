//! Golden output of the canonical emitter.

mod common;

use ayml_core::{Key, Mapping, Node, Value, emit, emit_flow, parse};

fn st(s: &str) -> Node {
    Node::new(Value::Str(s.into()))
}

fn int(i: i64) -> Node {
    Node::new(Value::Int(i))
}

fn map(entries: Vec<(&str, Node)>) -> Node {
    Node::new(Value::Map(
        entries
            .into_iter()
            .map(|(k, v)| (Key::new(k), v))
            .collect::<Mapping>(),
    ))
}

fn seq(items: Vec<Node>) -> Node {
    Node::new(Value::Seq(items))
}

/// Emit, check the golden text, and check that it parses back to the node.
#[track_caller]
fn golden(node: &Node, expected: &str) {
    let text = emit(node);
    assert_eq!(text, expected);
    assert_eq!(
        parse(&text).unwrap().value,
        node.value,
        "round trip of {text:?}"
    );
}

#[test]
fn scalars() {
    golden(&Node::new(Value::Null), "null\n");
    golden(&Node::new(Value::Bool(true)), "true\n");
    golden(&int(-17), "-17\n");
    golden(&int(i64::MIN), "-9223372036854775808\n");
}

#[test]
fn floats_use_shortest_round_trip_form() {
    let cases = [
        (1.0, "1.0"),
        (-0.0, "-0.0"),
        (0.1, "0.1"),
        (1e300, "1e300"),
        (1e-7, "1e-7"),
        (1e16, "1e16"),
        (123_456.789, "123456.789"),
        (f64::INFINITY, "inf"),
        (f64::NEG_INFINITY, "-inf"),
    ];
    for (f, text) in cases {
        golden(&Node::new(Value::Float(f)), &format!("{text}\n"));
    }
    assert_eq!(emit(&Node::new(Value::Float(f64::NAN))), "nan\n");
}

#[test]
fn bare_strings() {
    for s in [
        "hello world",
        "007",
        "yes",
        "é",
        "a,b",
        "say \"hi\"",
        "back\\slash",
        "-a",
        "a:b",
        "**/*.rs",
    ] {
        golden(&st(s), &format!("{s}\n"));
    }
}

#[test]
fn strings_that_would_resolve_are_quoted() {
    for s in [
        "null", "true", "false", "inf", "+inf", "-inf", "nan", "42", "-7", "1.5", "1e5", "0x10",
        "0b1", "0o7",
    ] {
        golden(&st(s), &format!("\"{s}\"\n"));
    }
}

#[test]
fn strings_that_are_not_valid_bare_are_quoted() {
    for s in [
        "", "-", ":", "- a", "a: b", "a #b", "a#b", "a:", " lead", "trail ", "'q'", "[a", "{a",
        "\"q", "\\x", "#c",
    ] {
        let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
        golden(&st(s), &format!("\"{escaped}\"\n"));
    }
}

#[test]
fn invisible_edges_are_quoted() {
    golden(&st("\u{a0}x"), "\"\u{a0}x\"\n");
    golden(&st("x\u{3000}"), "\"x\u{3000}\"\n");
}

#[test]
fn control_and_confusable_characters_are_escaped() {
    golden(&st("\0"), "\"\\0\"\n");
    golden(&st("\t"), "\"\\t\"\n");
    golden(&st("a\u{7f}"), "\"a\\x7F\"\n");
    golden(&st("a\u{85}b"), "\"a\\x85b\"\n");
    golden(&st("\u{feff}a"), "\"\\uFEFFa\"\n");
    golden(&st("a\u{2028}b"), "\"a\\u2028b\"\n");
    golden(&st("\u{fffe}"), "\"\\uFFFE\"\n");
    golden(&st("a\u{202E}b"), "\"a\\u202Eb\"\n");
    golden(&st("\x07\x08\x0b\x0c\x1b\r"), "\"\\a\\b\\v\\f\\e\\r\"\n");
}

#[test]
fn multi_line_strings_are_triple_quoted() {
    golden(&st("a\nb"), "\"\"\"\n  a\n  b\n  \"\"\"\n");
    golden(
        &map(vec![("k", st("a\nb"))]),
        "k: \"\"\"\n  a\n  b\n  \"\"\"\n",
    );
    golden(&seq(vec![st("a\nb")]), "- \"\"\"\n  a\n  b\n  \"\"\"\n");
}

#[test]
fn nested_triple_quoted_indentation() {
    golden(
        &map(vec![("outer", map(vec![("k", st("x\ny"))]))]),
        "outer:\n  k: \"\"\"\n    x\n    y\n    \"\"\"\n",
    );
}

#[test]
fn triple_quoted_trailing_newlines_use_empty_lines() {
    golden(&st("a\n"), "\"\"\"\n  a\n\n  \"\"\"\n");
    golden(&st("\n"), "\"\"\"\n\n\n  \"\"\"\n");
}

#[test]
fn triple_quoted_escapes() {
    golden(&st("a\\b\nc"), "\"\"\"\n  a\\\\b\n  c\n  \"\"\"\n");
    golden(&st("a\r\nb"), "\"\"\"\n  a\\r\n  b\n  \"\"\"\n");
}

#[test]
fn triple_quoted_breaks_delimiter_runs() {
    golden(
        &st("x\n\"\"\"\ny"),
        "\"\"\"\n  x\n  \"\"\\\"\n  y\n  \"\"\"\n",
    );
    golden(
        &st("q\"\"\"\"\"\"q\n"),
        "\"\"\"\n  q\"\"\\\"\"\"\\\"q\n\n  \"\"\"\n",
    );
}

#[test]
fn triple_quoted_trailing_whitespace_is_escaped() {
    golden(&st("a  \nb\t"), "\"\"\"\n  a \\ \n  b\\t\n  \"\"\"\n");
}

#[test]
fn triple_quoted_leading_tab_is_literal() {
    golden(&st("\tx\ny"), "\"\"\"\n  \tx\n  y\n  \"\"\"\n");
}

#[test]
fn multi_line_keys_are_double_quoted() {
    golden(&map(vec![("multi\nline", int(1))]), "\"multi\\nline\": 1\n");
}

#[test]
fn keys_are_quoted_only_when_needed() {
    golden(
        &map(vec![
            ("true", int(1)),
            ("42", int(2)),
            ("a: b", int(3)),
            ("", int(4)),
            ("key two", int(5)),
        ]),
        "true: 1\n42: 2\n\"a: b\": 3\n\"\": 4\nkey two: 5\n",
    );
}

#[test]
fn block_collections_with_regular_indentation() {
    golden(
        &map(vec![
            ("name", st("app")),
            ("inner", map(vec![("host", st("localhost"))])),
            ("ports", seq(vec![int(80), int(443)])),
        ]),
        "name: app\ninner:\n  host: localhost\nports:\n  - 80\n  - 443\n",
    );
}

#[test]
fn compact_collections_in_sequences() {
    golden(
        &seq(vec![
            map(vec![("name", st("Widget")), ("qty", int(5))]),
            seq(vec![int(1), int(2)]),
        ]),
        "- name: Widget\n  qty: 5\n- - 1\n  - 2\n",
    );
}

#[test]
fn empty_collections_use_flow_style() {
    golden(
        &map(vec![("a", seq(vec![])), ("b", map(vec![]))]),
        "a: []\nb: {}\n",
    );
    golden(&seq(vec![seq(vec![]), map(vec![])]), "- []\n- {}\n");
    golden(&seq(vec![]), "[]\n");
}

#[test]
fn empty_collection_trailing_comment_goes_inside() {
    golden(
        &map(vec![("e", map(vec![]).with_trailing_comment("todo"))]),
        "e: {\n  # todo\n}\n",
    );
}

#[test]
fn empty_root_collection_trailing_comment_goes_after() {
    golden(&seq(vec![]).with_trailing_comment("none"), "[]\n# none\n");
    let node = parse("[\n  # inside\n]\n# after\n").unwrap();
    assert_eq!(node.trailing_comment.as_deref(), Some("inside\nafter"));
    assert_eq!(emit(&node), "[]\n# inside\n# after\n");
}

#[test]
fn comments_in_every_position() {
    let node = map(vec![
        (
            "server",
            map(vec![(
                "host",
                st("localhost")
                    .with_comment("the host")
                    .with_inline_comment("default"),
            )])
            .with_comment("server settings")
            .with_trailing_comment("end of server"),
        ),
        (
            "ports",
            seq(vec![int(80), int(443).with_inline_comment("tls")])
                .with_inline_comment("open ports")
                .with_trailing_comment("end of ports"),
        ),
    ])
    .with_trailing_comment("the end");
    let expected = "\
# server settings
server:
  # the host
  host: localhost # default
  # end of server
ports: # open ports
  - 80
  - 443 # tls
  # end of ports
# the end
";
    assert_eq!(emit(&node), expected);
    let mut canonical = node.clone();
    canonical.comment = Some("server settings".to_owned());
    if let Value::Map(m) = &mut canonical.value {
        m["server"].comment = None;
    }
    assert_eq!(parse(expected).unwrap(), canonical);
}

#[test]
fn root_collection_comments_move_to_first_entry() {
    let node = map(vec![("a", int(1).with_comment("first")), ("b", int(2))])
        .with_comment("root")
        .with_inline_comment("root inline")
        .with_trailing_comment("end");
    assert_eq!(
        emit(&node),
        "# root\n# root inline\n# first\na: 1\nb: 2\n# end\n"
    );
}

#[test]
fn sequence_entry_collection_comments_move_above_dash() {
    let item = map(vec![
        ("host", st("x").with_comment("host comment")),
        ("port", int(1).with_comment("port comment")),
    ])
    .with_comment("item")
    .with_inline_comment("item inline")
    .with_trailing_comment("item end");
    let node = map(vec![(
        "rules",
        seq(vec![item, int(3)]).with_trailing_comment("rules end"),
    )]);
    let expected = "\
rules:
  # item
  # item inline
  # host comment
  - host: x
    # port comment
    port: 1
    # item end
  - 3
  # rules end
";
    assert_eq!(emit(&node), expected);
}

#[test]
fn compact_sequence_comments_move_above_dash() {
    let node = seq(vec![
        seq(vec![
            int(1).with_comment("inner"),
            int(2).with_inline_comment("two"),
        ])
        .with_comment("outer"),
    ]);
    assert_eq!(emit(&node), "# outer\n# inner\n- - 1\n  - 2 # two\n");
}

#[test]
fn scalar_trailing_comment_is_written_after_entry() {
    let node = map(vec![
        ("a", int(1).with_trailing_comment("after a")),
        ("b", int(2)),
    ]);
    let text = emit(&node);
    assert_eq!(text, "a: 1\n  # after a\nb: 2\n");
    assert_eq!(
        parse(&text).unwrap().get("b").unwrap().comment.as_deref(),
        Some("after a")
    );
}

#[test]
fn root_scalar_comments() {
    assert_eq!(
        emit(
            &int(1)
                .with_comment("head")
                .with_inline_comment("side")
                .with_trailing_comment("tail")
        ),
        "# head\n1 # side\n# tail\n"
    );
}

#[test]
fn empty_comment_lines() {
    let node = int(1).with_comment("\na\n\nb\n").with_trailing_comment("");
    let text = emit(&node);
    assert_eq!(text, "#\n# a\n\n# b\n#\n1\n#\n");
    assert_eq!(parse(&text).unwrap(), node);
}

#[test]
fn comment_text_is_sanitized() {
    let node = int(1)
        .with_comment("bad\u{0}char\r\nline2  ")
        .with_inline_comment("x\ny ");
    assert_eq!(emit(&node), "# bad\u{FFFD}char\n# line2\n1 # x y\n");
}

#[test]
fn comments_keep_leading_whitespace() {
    let node = int(1).with_comment("  indented\n\ttab");
    let text = emit(&node);
    assert_eq!(text, "#   indented\n# \ttab\n1\n");
    assert_eq!(parse(&text).unwrap(), node);
}

#[test]
fn emit_flow_writes_one_line() {
    let value = map(vec![
        ("true", st("v")),
        ("a: b", st("v")),
        ("multi\nline", st("x\ny")),
        ("", seq(vec![])),
        ("e", map(vec![]).with_trailing_comment("dropped")),
    ])
    .value;
    assert_eq!(
        emit_flow(&value),
        "{true: v, \"a: b\": v, \"multi\\nline\": \"x\\ny\", \"\": [], e: {}}"
    );
}

#[test]
fn emit_flow_quotes_flow_indicators() {
    let value = seq(vec![
        st("a,b"),
        st("c"),
        Node::new(Value::Float(1.0)),
        st("x y"),
        st("[z]"),
    ])
    .value;
    let text = emit_flow(&value);
    assert_eq!(text, "[\"a,b\", c, 1.0, x y, \"[z]\"]");
    assert_eq!(parse(&text).unwrap().value, value);
}

#[test]
fn spec_invoice_reformats_canonically() {
    let input = "bill-to:\n  given: Chris\n  address:\n    lines: \"\"\"\n      458 Walkman Dr.\n      Suite #292\n      \"\"\"\n    city    : Royal Oak\nproduct:\n  - sku         : BL394D\n    price       : 450.00\n";
    let expected = "bill-to:\n  given: Chris\n  address:\n    lines: \"\"\"\n      458 Walkman Dr.\n      Suite #292\n      \"\"\"\n    city: Royal Oak\nproduct:\n  - sku: BL394D\n    price: 450.0\n";
    assert_eq!(emit(&common::ok(input)), expected);
}
