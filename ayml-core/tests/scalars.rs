//! Scalar syntax and resolution.

mod common;

use ayml_core::{ErrorKind, Value, parse};
use common::{assert_syntax, err, get, ok, s, scalar};

// ── Null and booleans ───────────────────────────────────────────────

#[test]
fn null() {
    assert_eq!(scalar("null"), Value::Null);
}

#[test]
fn booleans() {
    assert_eq!(scalar("true"), Value::Bool(true));
    assert_eq!(scalar("false"), Value::Bool(false));
}

#[test]
fn reserved_words_are_case_sensitive() {
    for word in [
        "Null", "NULL", "True", "TRUE", "False", "NaN", "Inf", "INF", "Nan",
    ] {
        assert_eq!(scalar(word), s(word), "{word}");
    }
}

#[test]
fn yaml_booleans_are_strings() {
    for word in ["yes", "no", "on", "off", "y", "n", "~"] {
        assert_eq!(scalar(word), s(word), "{word}");
    }
}

#[test]
fn quoted_reserved_words_are_strings() {
    for word in ["null", "true", "false", "inf", "-inf", "nan", "42", "1.5"] {
        assert_eq!(scalar(&format!("\"{word}\"")), s(word), "{word}");
    }
}

// ── Integers ────────────────────────────────────────────────────────

#[test]
fn decimal_integers() {
    assert_eq!(scalar("0"), Value::Int(0));
    assert_eq!(scalar("12345"), Value::Int(12345));
    assert_eq!(scalar("-9876"), Value::Int(-9876));
    assert_eq!(scalar("+42"), Value::Int(42));
    assert_eq!(scalar("-0"), Value::Int(0));
    assert_eq!(scalar("+0"), Value::Int(0));
}

#[test]
fn binary_octal_hex_integers() {
    assert_eq!(scalar("0b10101010"), Value::Int(170));
    assert_eq!(scalar("0o14"), Value::Int(12));
    assert_eq!(scalar("0xC"), Value::Int(12));
    assert_eq!(scalar("0xff"), Value::Int(255));
    assert_eq!(scalar("0xFf"), Value::Int(255));
    assert_eq!(scalar("-0x10"), Value::Int(-16));
    assert_eq!(scalar("+0o7"), Value::Int(7));
    assert_eq!(scalar("-0b1"), Value::Int(-1));
    assert_eq!(scalar("0x00FF"), Value::Int(255));
}

#[test]
fn integer_extremes() {
    assert_eq!(scalar("9223372036854775807"), Value::Int(i64::MAX));
    assert_eq!(scalar("-9223372036854775808"), Value::Int(i64::MIN));
    assert_eq!(scalar("0x7fffffffffffffff"), Value::Int(i64::MAX));
    assert_eq!(scalar("-0x8000000000000000"), Value::Int(i64::MIN));
    assert_eq!(scalar("-0o1000000000000000000000"), Value::Int(i64::MIN));
    assert_eq!(
        scalar("-0b1000000000000000000000000000000000000000000000000000000000000000"),
        Value::Int(i64::MIN)
    );
}

#[test]
fn integer_overflow_is_an_error() {
    for text in [
        "9223372036854775808",
        "-9223372036854775809",
        "0x8000000000000000",
        "-0x8000000000000001",
        "18446744073709551615",
        "99999999999999999999999999",
        "0o1000000000000000000000",
    ] {
        assert_eq!(err(text).kind, ErrorKind::IntegerOverflow, "{text}");
    }
}

#[test]
fn integer_overflow_in_a_mapping_value_is_an_error() {
    assert_eq!(
        err("a: 9223372036854775808").kind,
        ErrorKind::IntegerOverflow
    );
}

#[test]
fn leading_zeros_make_strings() {
    for text in ["007", "0755", "02134", "-007", "+00", "00"] {
        assert_eq!(scalar(text), s(text), "{text}");
    }
}

#[test]
fn malformed_numbers_are_strings() {
    for text in [
        "0x",
        "0x1G",
        "0o8",
        "0b2",
        "0X10",
        "0B1",
        "0O7",
        "1_000",
        "1,000",
        "+",
        "-1-",
        "1.2.3",
        "2001-01-23",
        "12:30",
    ] {
        assert_eq!(scalar(text), s(text), "{text}");
    }
}

// ── Floats ──────────────────────────────────────────────────────────

#[test]
#[allow(clippy::float_cmp)]
fn float_forms() {
    let f = |t: &str| {
        scalar(t)
            .as_f64()
            .unwrap_or_else(|| panic!("{t} is not a float"))
    };
    assert_eq!(f("1230.15"), 1230.15);
    assert_eq!(f("1.23015e+3"), 1230.15);
    assert_eq!(f("12.3015e+02"), 1230.15);
    assert_eq!(f("1e10"), 1e10);
    assert_eq!(f("1E10"), 1e10);
    assert_eq!(f("1e-7"), 1e-7);
    assert_eq!(f("-0.5"), -0.5);
    assert_eq!(f("+0.5"), 0.5);
    assert_eq!(f("0.0"), 0.0);
    assert!(f("-0.0").is_sign_negative());
    assert_eq!(f("0e0"), 0.0);
    assert_eq!(f("450.00"), 450.0);
}

#[test]
fn special_floats() {
    assert_eq!(scalar("inf"), Value::Float(f64::INFINITY));
    assert_eq!(scalar("+inf"), Value::Float(f64::INFINITY));
    assert_eq!(scalar("-inf"), Value::Float(f64::NEG_INFINITY));
    assert!(scalar("nan").as_f64().unwrap().is_nan());
}

#[test]
fn signed_nan_is_a_string() {
    assert_eq!(scalar("-nan"), s("-nan"));
    assert_eq!(scalar("+nan"), s("+nan"));
}

#[test]
fn float_overflow_is_an_error() {
    for text in [
        "1e400",
        "-1e400",
        "1.5e309",
        "2e308",
        "17976931348623159e292",
    ] {
        assert_eq!(err(text).kind, ErrorKind::FloatOverflow, "{text}");
    }
}

#[test]
fn float_underflow_rounds() {
    assert_eq!(scalar("1e-400"), Value::Float(0.0));
    assert_eq!(scalar("5e-324"), Value::Float(5e-324));
}

#[test]
fn incomplete_floats_are_strings() {
    for text in [
        "1.", ".5", "-.5", "1.e5", "1e", "1e+", "01.5", "00.5", "01e5", "1.5e", "e5",
    ] {
        assert_eq!(scalar(text), s(text), "{text}");
    }
}

// ── Bare strings ────────────────────────────────────────────────────

#[test]
fn bare_strings() {
    assert_eq!(scalar("hello world"), s("hello world"));
    assert_eq!(
        scalar("https://example.com/path?q=1"),
        s("https://example.com/path?q=1")
    );
    assert_eq!(scalar("**/*.rs"), s("**/*.rs"));
    assert_eq!(scalar("2001-01-23"), s("2001-01-23"));
    assert_eq!(
        scalar("2001-11-23T15:03:17-5:00"),
        s("2001-11-23T15:03:17-5:00")
    );
    assert_eq!(scalar("a\\b"), s("a\\b"));
    assert_eq!(scalar("don't"), s("don't"));
    assert_eq!(scalar("a\"b"), s("a\"b"));
    assert_eq!(scalar("^0"), s("^0"));
    assert_eq!(scalar(">= 1.0"), s(">= 1.0"));
    assert_eq!(scalar("*alias"), s("*alias"));
    assert_eq!(scalar("&anchor"), s("&anchor"));
    assert_eq!(scalar("!tag"), s("!tag"));
    assert_eq!(scalar("|"), s("|"));
    assert_eq!(scalar("?"), s("?"));
}

#[test]
fn bare_string_internal_whitespace_is_kept_and_edges_trimmed() {
    assert_eq!(get(&ok("k:   a  \t b   "), "k").value, s("a  \t b"));
}

#[test]
fn bare_strings_may_contain_brackets_in_block_context() {
    assert_eq!(scalar("a[0]"), s("a[0]"));
    assert_eq!(scalar("x{y},z"), s("x{y},z"));
}

#[test]
fn bare_strings_with_colons() {
    assert_eq!(scalar("a:b"), s("a:b"));
    assert_eq!(scalar("::a"), s("::a"));
    // A trailing `:` cannot continue a bare string.
    assert!(parse("::").is_err());
    assert!(parse("k: a:").is_err());
    assert_eq!(scalar(":a"), s(":a"));
    assert_eq!(
        get(&ok("url: http://x.com:80/a"), "url").value,
        s("http://x.com:80/a")
    );
}

#[test]
fn bare_strings_starting_with_dash() {
    assert_eq!(scalar("-a"), s("-a"));
    assert_eq!(scalar("--"), s("--"));
    assert_eq!(scalar("-.5"), s("-.5"));
}

#[test]
fn lone_dash_and_colon_must_be_quoted() {
    assert_syntax("k: -", "bare `-`");
    assert_syntax("k: :", "bare `:`");
    assert_eq!(get(&ok("k: \"-\""), "k").value, s("-"));
}

#[test]
fn bare_string_cannot_start_with_single_quote() {
    assert_syntax("'hello'", "single-quoted strings are not supported");
    assert_syntax("k: 'v'", "single-quoted strings are not supported");
    assert_syntax("['v']", "single-quoted strings are not supported");
}

#[test]
fn bare_string_cannot_start_with_indicator() {
    for text in ["k: ]a", "k: }a", "k: ,a", "k: \\a"] {
        assert!(parse(text).is_err(), "{text}");
    }
}

#[test]
fn hash_without_whitespace_is_an_error() {
    assert_syntax("password: abc#123", "`#` must be preceded by white space");
    assert_syntax("a#b", "`#` must be preceded by white space");
    assert_syntax("\"a\"#b", "`#` must be preceded by white space");
    assert_syntax("[a#b]", "`#` must be preceded by white space");
}

#[test]
fn hash_with_whitespace_starts_a_comment() {
    let node = ok("k: abc #123");
    assert_eq!(get(&node, "k").value, s("abc"));
    assert_eq!(get(&node, "k").inline_comment.as_deref(), Some("123"));
}

#[test]
fn quoted_hash_is_content() {
    assert_eq!(get(&ok("k: \"abc#123\""), "k").value, s("abc#123"));
}

// ── Double-quoted strings ───────────────────────────────────────────

#[test]
fn double_quoted_simple() {
    assert_eq!(scalar("\"hello\""), s("hello"));
    assert_eq!(scalar("\"\""), s(""));
    assert_eq!(scalar("\"  padded  \""), s("  padded  "));
}

#[test]
fn every_named_escape() {
    assert_eq!(
        scalar(r#""\0\a\b\t\n\v\f\r\e\ \"\/\\""#),
        s("\0\x07\x08\t\n\x0B\x0C\r\x1B \"/\\")
    );
}

#[test]
fn hex_escapes() {
    assert_eq!(scalar(r#""\x41\x0d\x0a\xe9""#), s("A\r\né"));
    assert_eq!(scalar(r#""\u263A\u00e9""#), s("☺é"));
    assert_eq!(scalar(r#""\U0001F600""#), s("😀"));
    assert_eq!(scalar(r#""\x85\uFEFF""#), s("\u{85}\u{FEFF}"));
}

#[test]
fn escapes_can_produce_non_printable_characters() {
    assert_eq!(scalar(r#""\x00\x7F\uFFFE""#), s("\0\x7F\u{FFFE}"));
}

#[test]
fn surrogate_escapes_are_errors() {
    for text in [
        r#""\uD800""#,
        r#""\uDFFF""#,
        r#""\U0000D800""#,
        r#""\uD83D\uDE00""#,
    ] {
        assert!(
            matches!(err(text).kind, ErrorKind::InvalidEscape(_)),
            "{text}"
        );
    }
}

#[test]
fn out_of_range_escape_is_an_error() {
    assert!(matches!(
        err(r#""\U00110000""#).kind,
        ErrorKind::InvalidEscape(_)
    ));
    assert!(matches!(
        err(r#""\UFFFFFFFF""#).kind,
        ErrorKind::InvalidEscape(_)
    ));
}

#[test]
fn malformed_escapes_are_errors() {
    for text in [
        r#""\q""#,
        r#""\x4""#,
        r#""\x4g""#,
        r#""\u12""#,
        r#""\U1234567""#,
        r#""\'""#,
    ] {
        assert!(
            matches!(err(text).kind, ErrorKind::InvalidEscape(_)),
            "{text}"
        );
    }
}

#[test]
fn double_quoted_strings_are_single_line() {
    assert_syntax("\"abc\ndef\"", "unterminated double-quoted string");
    assert!(matches!(err("\"abc").kind, ErrorKind::UnexpectedEof(_)));
}

#[test]
fn raw_tab_in_double_quoted_string() {
    assert_eq!(scalar("\"a\tb\""), s("a\tb"));
}

// ── Triple-quoted strings ───────────────────────────────────────────

#[test]
fn triple_quoted_strips_closing_indentation() {
    let node = ok("k: \"\"\"\n  This string\n    spans many lines.\n  \"\"\"\n");
    assert_eq!(get(&node, "k").value, s("This string\n  spans many lines."));
}

#[test]
fn triple_quoted_has_no_trailing_newline() {
    assert_eq!(get(&ok("k: \"\"\"\n  x\n  \"\"\""), "k").value, s("x"));
}

#[test]
fn triple_quoted_trailing_newline_needs_an_empty_line() {
    assert_eq!(get(&ok("k: \"\"\"\n  x\n\n  \"\"\""), "k").value, s("x\n"));
    assert_eq!(
        get(&ok("k: \"\"\"\n  x\n\n\n  \"\"\""), "k").value,
        s("x\n\n")
    );
}

#[test]
fn triple_quoted_empty_content() {
    assert_eq!(get(&ok("k: \"\"\"\n  \"\"\""), "k").value, s(""));
    assert_eq!(get(&ok("k: \"\"\"\n\n  \"\"\""), "k").value, s(""));
}

#[test]
fn triple_quoted_blank_lines() {
    // Lines of fewer than `m` spaces are empty; lines of at least `m`
    // spaces keep the excess.
    let node = ok("k: \"\"\"\n  a\n\n \n     \n  b\n  \"\"\"");
    assert_eq!(get(&node, "k").value, s("a\n\n\n   \nb"));
}

#[test]
fn triple_quoted_hash_and_quotes_are_content() {
    let node = ok("k: \"\"\"\n  x = \"y\" # not a comment\n  'single'\n  \"\"\"");
    assert_eq!(
        get(&node, "k").value,
        s("x = \"y\" # not a comment\n'single'")
    );
}

#[test]
fn triple_quoted_escapes() {
    let node = ok("k: \"\"\"\n  tab\\there\\n\n  \\u263A \\\\\n  \"\"\"");
    assert_eq!(get(&node, "k").value, s("tab\there\n\n☺ \\"));
}

#[test]
fn triple_quoted_line_continuation() {
    let node = ok("k: \"\"\"\n  This is all \\\n  on one line.\n  \"\"\"");
    assert_eq!(get(&node, "k").value, s("This is all on one line."));
}

#[test]
fn triple_quoted_continuation_keeps_extra_indentation() {
    let node = ok("k: \"\"\"\n  a\\\n    b\n  \"\"\"");
    assert_eq!(get(&node, "k").value, s("a  b"));
}

#[test]
fn triple_quoted_continuation_on_last_line() {
    assert_eq!(get(&ok("k: \"\"\"\n  a\\\n  \"\"\""), "k").value, s("a"));
}

#[test]
fn triple_quoted_escaped_backslash_at_line_end_is_not_continuation() {
    assert_eq!(
        get(&ok("k: \"\"\"\n  a\\\\\n  b\n  \"\"\""), "k").value,
        s("a\\\nb")
    );
}

#[test]
fn triple_quoted_escaped_delimiter() {
    let node = ok("k: \"\"\"\n  \\\"\"\" docstring\n  \"\"\\\"\n  \"\"\"");
    assert_eq!(get(&node, "k").value, s("\"\"\" docstring\n\"\"\""));
}

#[test]
fn triple_quote_mid_line_is_content() {
    let node = ok("k: \"\"\"\n  a \"\"\" b\n  \"\"\"");
    assert_eq!(get(&node, "k").value, s("a \"\"\" b"));
}

#[test]
fn trailing_whitespace_after_opening_delimiter() {
    assert_eq!(get(&ok("k: \"\"\"  \t\n  x\n  \"\"\""), "k").value, s("x"));
}

#[test]
fn trailing_whitespace_in_content_is_kept() {
    assert_eq!(get(&ok("k: \"\"\"\n  x  \n  \"\"\""), "k").value, s("x  "));
}

#[test]
fn triple_quoted_crlf_is_normalized() {
    assert_eq!(
        get(&ok("k: \"\"\"\r\n  a\r\n  b\r\n  \"\"\"\r\n"), "k").value,
        s("a\nb")
    );
    assert_eq!(
        get(&ok("k: \"\"\"\r  a\r  b\r  \"\"\"\r"), "k").value,
        s("a\nb")
    );
}

#[test]
fn triple_quoted_tabs_after_indentation_are_content() {
    assert_eq!(get(&ok("k: \"\"\"\n  \tx\n  \"\"\""), "k").value, s("\tx"));
}

#[test]
fn triple_quoted_at_root_may_close_at_column_zero() {
    assert_eq!(scalar("\"\"\"\nline\n\"\"\""), s("line"));
}

#[test]
fn triple_quoted_closing_at_entry_indentation() {
    let node = ok("outer:\n  key: \"\"\"\n  text\n  \"\"\"\n");
    assert_eq!(get(get(&node, "outer"), "key").value, s("text"));
}

#[test]
fn triple_quoted_closing_below_entry_indentation_is_an_error() {
    assert_syntax(
        "a:\n  b: \"\"\"\n  x\n \"\"\"",
        "closing `\"\"\"` must be indented at least 2 spaces",
    );
}

#[test]
fn triple_quoted_content_less_indented_than_closing_is_an_error() {
    assert_syntax("k: \"\"\"\n  a\n b\n  \"\"\"", "indented at least 2 spaces");
}

#[test]
fn triple_quoted_tab_in_indentation_is_an_error() {
    assert!(parse("k: \"\"\"\n  a\n\tb\n  \"\"\"").is_err());
    assert!(parse("k: \"\"\"\n  a\n\t\n  \"\"\"").is_err());
}

#[test]
fn triple_quoted_content_must_start_on_next_line() {
    assert_syntax("k: \"\"\"text\n  \"\"\"", "must start on the line after");
    assert_syntax(
        "k: \"\"\" # c\n  x\n  \"\"\"",
        "must start on the line after",
    );
}

#[test]
fn unterminated_triple_quoted_string() {
    assert!(matches!(
        err("k: \"\"\"\n  x\n").kind,
        ErrorKind::UnexpectedEof(_)
    ));
    assert!(matches!(err("k: \"\"\"").kind, ErrorKind::UnexpectedEof(_)));
}

#[test]
fn content_after_closing_delimiter_is_an_error() {
    assert!(parse("k: \"\"\"\n  x\n  \"\"\"y\n").is_err());
}

#[test]
fn inline_comment_after_closing_delimiter() {
    let node = ok("k: \"\"\"\n  x\n  \"\"\"   # done\nnext: 1");
    assert_eq!(get(&node, "k").value, s("x"));
    assert_eq!(get(&node, "k").inline_comment.as_deref(), Some("done"));
}

#[test]
fn triple_quoted_in_sequence() {
    let node = ok("- \"\"\"\n  a\n  \"\"\"\n- b");
    let items = common::items(&node);
    assert_eq!(items[0].value, s("a"));
    assert_eq!(items[1].value, s("b"));
}

#[test]
fn triple_quoted_in_flow_collections() {
    let node = ok("[1, \"\"\"\n  text\n  \"\"\", 2]");
    let items = common::items(&node);
    assert_eq!(items[1].value, s("text"));
    let node = ok("{a: \"\"\"\n  text\n  \"\"\"}");
    assert_eq!(get(&node, "a").value, s("text"));
}
