use ayml_core::{parse, value::MapKey};

#[test]
fn blank_line_between_top_comments_preserved() {
    let input = "# first\n\n# second\nkey: value";
    let node = parse(input).unwrap();
    assert_eq!(
        node.comment.as_deref(),
        Some("first\n\nsecond"),
        "blank line between comments should be preserved"
    );
}

#[test]
fn multiple_blank_lines_between_comments_preserved() {
    let input = "# first\n\n\n# second\nkey: value";
    let node = parse(input).unwrap();
    assert_eq!(
        node.comment.as_deref(),
        Some("first\n\n\nsecond"),
        "multiple blank lines should be preserved"
    );
}

#[test]
fn no_blank_line_between_comments_unchanged() {
    let input = "# first\n# second\nkey: value";
    let node = parse(input).unwrap();
    assert_eq!(node.comment.as_deref(), Some("first\nsecond"));
}

#[test]
fn blank_line_between_comments_in_mapping() {
    let input = "a: 1\n\n# above b\n\n# directly above b\nb: 2";
    let node = parse(input).unwrap();
    let map = node.value.as_mapping().unwrap();
    let b = &map[&MapKey::String("b".into())];
    assert_eq!(
        b.comment.as_deref(),
        Some("above b\n\ndirectly above b"),
        "blank line between comments before mapping entry should be preserved"
    );
}

#[test]
fn blank_line_between_comments_in_sequence() {
    let input = "- 1\n\n# above two\n\n# directly above two\n- 2";
    let node = parse(input).unwrap();
    let seq = node.value.as_sequence().unwrap();
    assert_eq!(
        seq[1].comment.as_deref(),
        Some("above two\n\ndirectly above two"),
        "blank line between comments before sequence entry should be preserved"
    );
}

#[test]
fn round_trip_preserves_blank_lines_in_comments() {
    let input = "# first\n\n# second\nkey: value\n";
    let node = parse(input).unwrap();
    let emitted = ayml_core::emit(&node);
    let reparsed = parse(&emitted).unwrap();
    assert_eq!(node.comment, reparsed.comment);
}
