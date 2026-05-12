use ayml_core::parse;

#[test]
fn no_comment_multikey_mapping() {
    let input = "schema_version: 1\nnetwork:\n  host: x";
    let result = parse(input);
    assert!(result.is_ok(), "parse failed: {}", result.unwrap_err());
}

#[test]
fn comment_blank_then_mapping() {
    let input = "# comment\n\nschema_version: 1";
    let result = parse(input);
    assert!(result.is_ok(), "parse failed: {}", result.unwrap_err());
}

#[test]
fn comment_blank_then_multikey_mapping() {
    let input = "# comment\n\nschema_version: 1\nnetwork:\n  host: x";
    let result = parse(input);
    assert!(result.is_ok(), "parse failed: {}", result.unwrap_err());
}

#[test]
fn blank_line_then_mapping() {
    let input = "\nschema_version: 1";
    let result = parse(input);
    assert!(result.is_ok(), "parse failed: {}", result.unwrap_err());
}

#[test]
fn inner_comment_above_mapping_value() {
    // Minimal: mapping value that has a comment before the next key
    let input = "network:\n  # comment\n  rules: ok";
    let result = parse(input);
    assert!(result.is_ok(), "A: {}", result.unwrap_err());
}

#[test]
fn mapping_with_trailing_newline() {
    let input = "network:\n  rules: ok\n";
    let result = parse(input);
    assert!(result.is_ok(), "E: {}", result.unwrap_err());
}

#[test]
fn inner_comment_above_seq_entry() {
    let input = "rules:\n    # comment\n    - host: x\n";
    let result = parse(input);
    assert!(result.is_ok(), "B: {}", result.unwrap_err());
}

#[test]
fn inner_comment_between_seq_entries() {
    let input = "rules:\n    - host: x\n      ports:\n        - 443\n        - 22\n        # comment\n    - host: y\n";
    let result = parse(input);
    assert!(result.is_ok(), "C: {}", result.unwrap_err());
}

#[test]
fn two_keys_with_inner_comment() {
    let input = "schema_version: 1\nnetwork:\n  # comment\n  rules: ok\n";
    let result = parse(input);
    assert!(result.is_ok(), "D: {}", result.unwrap_err());
}

#[test]
fn policy_file() {
    let input = include_str!("../../files/policy.ayml");
    let result = parse(input);
    assert!(result.is_ok(), "parse failed: {}", result.unwrap_err());
}
