use ayml_core::{
    error::ErrorKind,
    parse, parse_with_max_depth,
    value::{MapKey, Value},
};

#[test]
fn duplicate_key() {
    let input = "a: 1\na: 2";
    let err = parse(input).unwrap_err();
    assert!(matches!(err.kind, ErrorKind::DuplicateKey(_)));
}

#[test]
fn null_mapping_key() {
    let input = "null: value";
    let err = parse(input).unwrap_err();
    assert!(matches!(err.kind, ErrorKind::NullKey));
}

#[test]
fn float_mapping_key() {
    let input = "3.14: value";
    let err = parse(input).unwrap_err();
    assert!(matches!(err.kind, ErrorKind::FloatKey));
}

#[test]
fn invalid_escape_in_double_quoted() {
    let input = r#"s: "\q""#;
    let err = parse(input).unwrap_err();
    assert!(matches!(err.kind, ErrorKind::InvalidEscape(_)));
}

#[test]
fn line_continuation_in_triple_quoted() {
    // `\` at end of line suppresses the line break, joining the next line
    let input = "s: \"\"\"\n  hello \\\n  world\n  \"\"\"";
    let node = parse(input).unwrap();
    let map = node.value.as_mapping().unwrap();
    assert_eq!(
        map[&MapKey::String("s".into())].value,
        Value::Str("hello world".into())
    );
}

#[test]
fn backslash_eof_in_triple_quoted_is_error() {
    // A lone `\` followed immediately by EOF (malformed — no closing `"""`)
    let input = "s: \"\"\"\n  hello\\";
    let err = parse(input).unwrap_err();
    // Should error (unclosed triple-quote), not silently emit a backslash
    assert!(matches!(
        err.kind,
        ErrorKind::UnexpectedEof | ErrorKind::Expected(_)
    ),);
}

#[test]
fn unclosed_double_quote() {
    let input = r#"s: "hello"#;
    let err = parse(input).unwrap_err();
    assert!(matches!(err.kind, ErrorKind::UnexpectedEof));
}

#[test]
fn unclosed_flow_sequence() {
    let input = "items: [a, b";
    let err = parse(input).unwrap_err();
    assert!(matches!(err.kind, ErrorKind::Expected(_)));
}

#[test]
fn unclosed_flow_mapping() {
    let input = "m: {a: 1";
    let err = parse(input).unwrap_err();
    assert!(matches!(err.kind, ErrorKind::Expected(_)));
}

#[test]
fn bom_rejected() {
    let input = "\u{FEFF}key: value";
    let err = parse(input).unwrap_err();
    assert!(matches!(err.kind, ErrorKind::ByteOrderMark));
}

#[test]
fn deeply_nested_flow_sequences_rejected() {
    // 200 nested opening brackets — well past the default limit of 128.
    let input: String = "[".repeat(200) + &"]".repeat(200);
    let err = parse(&input).unwrap_err();
    assert!(matches!(err.kind, ErrorKind::RecursionLimit));
}

#[test]
fn deeply_nested_flow_mappings_rejected() {
    // {a: {a: {a: ... }}}
    let mut input = String::new();
    for _ in 0..200 {
        input.push_str("{a: ");
    }
    input.push('1');
    for _ in 0..200 {
        input.push('}');
    }
    let err = parse(&input).unwrap_err();
    assert!(matches!(err.kind, ErrorKind::RecursionLimit));
}

#[test]
fn deeply_nested_block_mappings_rejected() {
    // a:\n  b:\n    c:\n ... (200 levels deep)
    let mut input = String::new();
    for i in 0..200 {
        for _ in 0..(i * 2) {
            input.push(' ');
        }
        input.push_str(&format!("k{i}:\n"));
    }
    for _ in 0..(200 * 2) {
        input.push(' ');
    }
    input.push('1');
    let err = parse(&input).unwrap_err();
    assert!(matches!(err.kind, ErrorKind::RecursionLimit));
}

#[test]
fn custom_max_depth_is_respected() {
    // 5 levels of nesting: [[[[[ 1 ]]]]]
    let input = "[".repeat(5) + "1" + &"]".repeat(5);
    // Limit of 4 should reject it.
    let err = parse_with_max_depth(&input, 4).unwrap_err();
    assert!(matches!(err.kind, ErrorKind::RecursionLimit));
    // Limit of 10 should accept it.
    let node = parse_with_max_depth(&input, 10).unwrap();
    assert!(node.value.as_sequence().is_some());
}

#[test]
fn moderate_nesting_within_default_limit_ok() {
    // 50 levels — well within the default 128.
    let input = "[".repeat(50) + "1" + &"]".repeat(50);
    let node = parse(&input).unwrap();
    assert!(node.value.as_sequence().is_some());
}

#[test]
fn error_has_line_column() {
    let input = "a: 1\nb: 2\na: 3";
    let err = parse(input).unwrap_err();
    // The duplicate key error should have a meaningful line/column
    assert!(err.line >= 1);
    assert!(err.column >= 1);
}

#[test]
fn tab_indent_in_mapping_value() {
    let input = "outer:\n\tinner: 1";
    let err = parse(input).unwrap_err();
    assert!(matches!(err.kind, ErrorKind::TabIndent));
    assert_eq!(err.line, 2);
    assert_eq!(err.column, 1);
}

#[test]
fn tab_indent_in_sequence() {
    let input = "items:\n\t- one";
    let err = parse(input).unwrap_err();
    assert!(matches!(err.kind, ErrorKind::TabIndent));
}

#[test]
fn tab_indent_at_top_level() {
    let input = "\tkey: val";
    let err = parse(input).unwrap_err();
    assert!(matches!(err.kind, ErrorKind::TabIndent));
}

#[test]
fn tab_after_spaces_in_indent() {
    let input = "outer:\n  \tinner: 1";
    let err = parse(input).unwrap_err();
    assert!(matches!(err.kind, ErrorKind::TabIndent));
}

// ── Non-printable character rejection ────────────────────────────

#[test]
fn null_byte_in_bare_string_rejected() {
    let input = "key: hel\x00lo";
    let err = parse(input).unwrap_err();
    assert!(matches!(err.kind, ErrorKind::NonPrintable('\x00')));
}

#[test]
fn form_feed_in_bare_string_rejected() {
    let input = "key: hel\x0Clo";
    let err = parse(input).unwrap_err();
    assert!(matches!(err.kind, ErrorKind::NonPrintable('\x0C')));
}

#[test]
fn bel_in_bare_string_rejected() {
    let input = "key: hel\x07lo";
    let err = parse(input).unwrap_err();
    assert!(matches!(err.kind, ErrorKind::NonPrintable('\x07')));
}

// ── Deeply nested block sequences ────────────────────────────────

#[test]
fn deeply_nested_block_sequences_rejected() {
    // Nested via seq-in-mapping-in-seq pattern:
    // - s:
    //   - s:
    //     - s:
    //       ...
    let mut input = String::new();
    for i in 0..200 {
        let indent = i * 2;
        for _ in 0..indent {
            input.push(' ');
        }
        input.push_str("- s:\n");
    }
    let indent = 200 * 2;
    for _ in 0..indent {
        input.push(' ');
    }
    input.push_str("- 1");
    let err = parse(&input).unwrap_err();
    assert!(matches!(err.kind, ErrorKind::RecursionLimit));
}
