use ayml_core::parse;

#[test]
fn triple_quoted_no_linebreak_block() {
    let input = "key: \"\"\"content\n  \"\"\"";
    let err = parse(input).unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("content must start on the next line"),
        "expected clear error about missing line break, got: {msg}"
    );
}

#[test]
fn triple_quoted_no_linebreak_flow() {
    let input = "{key: \"\"\"content\n  \"\"\"}";
    let err = parse(input).unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("content must start on the next line"),
        "expected clear error about missing line break, got: {msg}"
    );
}

#[test]
fn triple_quoted_no_linebreak_in_nested_document() {
    let input = "network:\n  rules:\n    - host: x\n      action: deny\n    - {host: y, ports: \"\"\"all\n    \"\"\"}";
    let err = parse(input).unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("content must start on the next line"),
        "expected clear error about missing line break, got: {msg}"
    );
}

#[test]
fn triple_quoted_misaligned_content_rejected() {
    let input = "key: \"\"\"\n  aligned\nmisaligned\n  \"\"\"";
    let err = parse(input).unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("indentation"),
        "expected indentation error for misaligned content, got: {msg}"
    );
}

#[test]
fn triple_quoted_misaligned_in_flow_rejected() {
    let input = "- {host: \"x\", ports: \"\"\"\n    all asdkofj\n alsdkfja\n      \"\"\"}";
    let err = parse(input).unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("indentation"),
        "expected indentation error for misaligned content in flow, got: {msg}"
    );
}

#[test]
fn triple_quoted_blank_lines_allowed_at_any_indent() {
    // Blank lines (whitespace-only) are allowed per the spec.
    let input = "key: \"\"\"\n  content\n\n  more content\n  \"\"\"";
    let result = parse(input);
    assert!(
        result.is_ok(),
        "blank lines should be allowed: {}",
        result.unwrap_err()
    );
}
