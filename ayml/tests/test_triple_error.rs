use ayml::from_str;

#[test]
fn triple_quoted_misaligned_content_rejected() {
    let input = "key: \"\"\"\n  aligned\nmisaligned\n  \"\"\"";
    let err = from_str::<serde_json::Value>(input);
    assert!(err.is_err(), "misaligned content should be rejected");
    let msg = format!("{}", err.unwrap_err());
    assert!(
        msg.contains("indentation"),
        "expected indentation error, got: {msg}"
    );
}

#[test]
fn triple_quoted_blank_lines_allowed() {
    let input = "key: \"\"\"\n  content\n\n  more\n  \"\"\"";
    let result: Result<serde_json::Value, _> = from_str(input);
    assert!(
        result.is_ok(),
        "blank lines should be allowed: {}",
        result.unwrap_err()
    );
}
