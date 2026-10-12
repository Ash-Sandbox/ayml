//! Error kinds, messages, locations, and limits.

mod common;

use ayml_core::{DEFAULT_MAX_DEPTH, ErrorKind, Location, Span, parse, parse_with_max_depth};
use common::{err, ok};

#[test]
fn display_includes_line_and_column() {
    let e = err("a: 1\nb: \"\\q\"\n");
    assert_eq!(
        e.to_string(),
        "2:5: invalid escape sequence: unknown escape `\\q`"
    );
    assert_eq!((e.line(), e.column()), (2, 5));
}

#[test]
fn columns_count_characters_not_bytes() {
    let e = err("héllo: wörld#x\n");
    assert_eq!(
        e.location,
        Location {
            line: 1,
            column: 13
        }
    );
    let e = err("k: \"☺☺\\q\"");
    assert_eq!(e.column(), 7);
}

#[test]
fn lines_count_cr_lf_and_crlf() {
    assert_eq!(err("a: 1\r\nb: 1\rc: 1\nd: \"\\q\"").line(), 4);
    assert_eq!(err("a: 1\r\n\r\nb: 9223372036854775808").line(), 3);
}

#[test]
fn error_spans_point_into_input() {
    let input = "a: 1\nb: 0x8000000000000000\n";
    let e = err(input);
    assert_eq!(e.kind, ErrorKind::IntegerOverflow);
    assert_eq!(&input[e.span.start..e.span.end], "0x8000000000000000");
}

#[test]
fn duplicate_key_span_is_the_second_key() {
    let input = "name: a\n\"name\": b\n";
    let e = err(input);
    assert_eq!(e.kind, ErrorKind::DuplicateKey("name".into()));
    assert_eq!(e.span, Span::new(8, 14));
    assert_eq!(e.location, Location { line: 2, column: 1 });
}

#[test]
fn messages_explain_common_mistakes() {
    let cases = [
        ("password: abc#123", "quote the string"),
        ("k: 'v'", "use double quotes"),
        (
            "a:\n  b",
            "write scalars and flow collections on the same line",
        ),
        ("a:\nb: 1", "write `null`"),
        ("- a\n-", "write `null`"),
        ("a: b\n  c", "use a triple-quoted string"),
        ("a: b: c", "a nested mapping must start on a new line"),
        ("{a:1}", "must be followed by a space"),
        ("\"a\nb\"", "use a triple-quoted string for multiple lines"),
        ("k: \"\"\"x\n\"\"\"", "must start on the line after"),
    ];
    for (input, hint) in cases {
        let message = err(input).to_string();
        assert!(
            message.contains(hint),
            "{input:?}: {message:?} lacks {hint:?}"
        );
    }
}

#[test]
fn byte_order_mark_is_rejected() {
    let e = err("\u{FEFF}a: 1");
    assert_eq!(e.kind, ErrorKind::ByteOrderMark);
    assert_eq!(e.location, Location { line: 1, column: 1 });
}

#[test]
fn byte_order_mark_inside_a_string_is_allowed() {
    assert!(parse("a: x\u{FEFF}y").is_ok());
}

#[test]
fn non_printable_characters_are_rejected_everywhere() {
    for (input, ch) in [
        ("a: x\u{0}y", '\u{0}'),
        ("a: \"\u{7}\"", '\u{7}'),
        ("a: 1 # \u{1B}[0m", '\u{1B}'),
        ("# \u{7F}\na: 1", '\u{7F}'),
        ("a\u{C}b: 1", '\u{C}'),
        ("a: \"\"\"\n  \u{80}\n  \"\"\"", '\u{80}'),
        ("a: \u{FFFE}", '\u{FFFE}'),
    ] {
        assert_eq!(err(input).kind, ErrorKind::NonPrintable(ch), "{input:?}");
    }
}

#[test]
fn printable_unusual_characters_are_allowed() {
    assert!(parse("a: x\u{85}y # \u{2028}").is_ok());
    assert!(parse("a: \u{A0}x").is_ok());
}

#[test]
fn tabs_in_indentation() {
    for input in [
        "a:\n\tb: 1",
        "a:\n  \tb: 1",
        "\ta: 1",
        "- a\n\t- b",
        "a: 1\n \t# c",
    ] {
        assert_eq!(err(input).kind, ErrorKind::TabIndent, "{input:?}");
    }
}

#[test]
fn tabs_on_blank_lines_are_allowed() {
    assert!(parse("a: 1\n\t\n \t \nb: 2").is_ok());
}

#[test]
fn unexpected_end_of_input() {
    for input in ["\"abc", "[1, 2", "{a: 1", "k: \"\"\"\n  x"] {
        assert!(
            matches!(err(input).kind, ErrorKind::UnexpectedEof(_)),
            "{input:?}"
        );
    }
}

#[test]
fn empty_document() {
    assert_eq!(err("").kind, ErrorKind::EmptyDocument);
    assert_eq!(
        err("# only\n").to_string(),
        "2:1: document is empty (expected a root value)"
    );
}

#[test]
fn default_depth_limit() {
    let depth = DEFAULT_MAX_DEPTH;
    let at_limit = format!("{}1{}", "[".repeat(depth), "]".repeat(depth));
    assert!(parse(&at_limit).is_ok());
    let over = format!("{}1{}", "[".repeat(depth + 1), "]".repeat(depth + 1));
    assert_eq!(err(&over).kind, ErrorKind::RecursionLimit);
}

#[test]
fn depth_limit_applies_to_every_collection_kind() {
    let n = DEFAULT_MAX_DEPTH + 1;
    let flow_map = format!("{}1{}", "{a: ".repeat(n), "}".repeat(n));
    let compact = format!("{}x", "- ".repeat(n));
    let mut block = String::new();
    for i in 0..n {
        block.push_str(&" ".repeat(i));
        block.push_str("k:\n");
    }
    block.push_str(&" ".repeat(n));
    block.push_str("k: 1");
    for input in [flow_map, compact, block] {
        assert_eq!(err(&input).kind, ErrorKind::RecursionLimit);
    }
}

#[test]
fn deep_nesting_beyond_limit_does_not_overflow_the_stack() {
    let input = "[".repeat(100_000);
    assert_eq!(err(&input).kind, ErrorKind::RecursionLimit);
}

#[test]
fn custom_depth_limit() {
    assert!(parse_with_max_depth("[[1]]", 2).is_ok());
    let e = parse_with_max_depth("[[[1]]]", 2).unwrap_err();
    assert_eq!(e.kind, ErrorKind::RecursionLimit);
    assert!(parse_with_max_depth("a:\n  b: 1", 2).is_ok());
    assert!(parse_with_max_depth("a:\n  b:\n    c: 1", 2).is_err());
    assert!(parse_with_max_depth("42", 0).is_ok());
    assert!(parse_with_max_depth("[]", 0).is_err());
}

#[test]
fn moderate_nesting_is_fine() {
    let input = format!("{}1{}", "[".repeat(50), "]".repeat(50));
    assert!(ok(&input).value.is_collection());
}

#[test]
fn location_of_offsets() {
    let text = "ab\r\ncd\ref\ngh";
    assert_eq!(Location::of(text, 0), Location { line: 1, column: 1 });
    assert_eq!(Location::of(text, 4), Location { line: 2, column: 1 });
    assert_eq!(Location::of(text, 7), Location { line: 3, column: 1 });
    assert_eq!(Location::of(text, 10), Location { line: 4, column: 1 });
    assert_eq!(Location::of(text, 999), Location { line: 4, column: 3 });
    assert_eq!(Location::of("é", 1), Location { line: 1, column: 1 });
}
