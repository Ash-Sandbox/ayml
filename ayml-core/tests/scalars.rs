use ayml_core::{
    error::ErrorKind,
    parse,
    value::{MapKey, Value},
};

// ── Null ─────────────────────────────────────────────────────────

#[test]
fn null_value() {
    let node = parse("key: null").unwrap();
    let map = node.value.as_mapping().unwrap();
    let val = &map[&MapKey::String("key".into())];
    assert!(val.value.is_null());
}

// ── Booleans ─────────────────────────────────────────────────────

#[test]
fn bool_true() {
    let node = parse("flag: true").unwrap();
    let map = node.value.as_mapping().unwrap();
    assert_eq!(map[&MapKey::String("flag".into())].value, Value::Bool(true));
}

#[test]
fn bool_false() {
    let node = parse("flag: false").unwrap();
    let map = node.value.as_mapping().unwrap();
    assert_eq!(
        map[&MapKey::String("flag".into())].value,
        Value::Bool(false)
    );
}

// ── Integers ─────────────────────────────────────────────────────

#[test]
fn integer_decimal() {
    let node = parse("n: 12345").unwrap();
    let map = node.value.as_mapping().unwrap();
    assert_eq!(map[&MapKey::String("n".into())].value, Value::Int(12345));
}

#[test]
fn integer_negative() {
    let node = parse("n: -9876").unwrap();
    let map = node.value.as_mapping().unwrap();
    assert_eq!(map[&MapKey::String("n".into())].value, Value::Int(-9876));
}

#[test]
fn integer_positive_sign() {
    let node = parse("n: +42").unwrap();
    let map = node.value.as_mapping().unwrap();
    assert_eq!(map[&MapKey::String("n".into())].value, Value::Int(42));
}

#[test]
fn integer_binary() {
    let node = parse("n: 0b10101010").unwrap();
    let map = node.value.as_mapping().unwrap();
    assert_eq!(
        map[&MapKey::String("n".into())].value,
        Value::Str("0b10101010".into())
    );
}

#[test]
fn integer_octal() {
    let node = parse("n: 0o14").unwrap();
    let map = node.value.as_mapping().unwrap();
    assert_eq!(map[&MapKey::String("n".into())].value, Value::Str("0o14".into()));
}

#[test]
fn integer_hex() {
    let node = parse("n: 0xC").unwrap();
    let map = node.value.as_mapping().unwrap();
    assert_eq!(map[&MapKey::String("n".into())].value, Value::Str("0xC".into()));
}

#[test]
fn integer_zero() {
    let node = parse("n: 0").unwrap();
    let map = node.value.as_mapping().unwrap();
    assert_eq!(map[&MapKey::String("n".into())].value, Value::Int(0));
}

#[test]
fn integer_i64_max() {
    let node = parse("n: 9223372036854775807").unwrap();
    let map = node.value.as_mapping().unwrap();
    assert_eq!(map[&MapKey::String("n".into())].value, Value::Int(i64::MAX));
}

#[test]
fn integer_i64_min() {
    let node = parse("n: -9223372036854775808").unwrap();
    let map = node.value.as_mapping().unwrap();
    assert_eq!(map[&MapKey::String("n".into())].value, Value::Int(i64::MIN));
}

#[test]
fn integer_overflow_is_error() {
    let err = parse("n: 9223372036854775808").unwrap_err();
    assert!(matches!(err.kind, ErrorKind::IntegerOverflow));
}

#[test]
fn integer_negative_overflow_is_error() {
    let err = parse("n: -9223372036854775809").unwrap_err();
    assert!(matches!(err.kind, ErrorKind::IntegerOverflow));
}

#[test]
fn integer_hex_i64_min() {
    let node = parse("n: -0x8000000000000000").unwrap();
    let map = node.value.as_mapping().unwrap();
    assert_eq!(map[&MapKey::String("n".into())].value, Value::Str("-0x8000000000000000".into()));
}

#[test]
fn integer_octal_i64_min() {
    let node = parse("n: -0o1000000000000000000000").unwrap();
    let map = node.value.as_mapping().unwrap();
    assert_eq!(map[&MapKey::String("n".into())].value, Value::Str("-0o1000000000000000000000".into()));
}

#[test]
fn integer_binary_i64_min() {
    let node =
        parse("n: -0b1000000000000000000000000000000000000000000000000000000000000000").unwrap();
    let map = node.value.as_mapping().unwrap();
    assert_eq!(map[&MapKey::String("n".into())].value, Value::Str("-0b1000000000000000000000000000000000000000000000000000000000000000".into()));
}

// ── Floats ───────────────────────────────────────────────────────

#[test]
fn float_fixed() {
    let node = parse("n: 1230.15").unwrap();
    let map = node.value.as_mapping().unwrap();
    assert_eq!(
        map[&MapKey::String("n".into())].value,
        Value::Float(1230.15)
    );
}

#[test]
fn float_exponential() {
    let node = parse("n: 12.3015e+02").unwrap();
    let map = node.value.as_mapping().unwrap();
    if let Value::Float(f) = map[&MapKey::String("n".into())].value {
        assert!((f - 1230.15).abs() < 0.001);
    } else {
        panic!("expected float");
    }
}

#[test]
fn float_canonical() {
    let node = parse("n: 1.23015e+3").unwrap();
    let map = node.value.as_mapping().unwrap();
    if let Value::Float(f) = map[&MapKey::String("n".into())].value {
        assert!((f - 1230.15).abs() < 0.001);
    } else {
        panic!("expected float");
    }
}

#[test]
fn float_pure_exponential() {
    let node = parse("n: 5e10").unwrap();
    let map = node.value.as_mapping().unwrap();
    if let Value::Float(f) = map[&MapKey::String("n".into())].value {
        assert!((f - 5e10).abs() < 1.0);
    } else {
        panic!("expected float");
    }
}

#[test]
fn float_inf() {
    let node = parse("n: inf").unwrap();
    let map = node.value.as_mapping().unwrap();
    assert_eq!(
        map[&MapKey::String("n".into())].value,
        Value::Float(f64::INFINITY)
    );
}

#[test]
fn float_neg_inf() {
    let node = parse("n: -inf").unwrap();
    let map = node.value.as_mapping().unwrap();
    assert_eq!(
        map[&MapKey::String("n".into())].value,
        Value::Float(f64::NEG_INFINITY)
    );
}

#[test]
fn float_pos_inf() {
    let node = parse("n: +inf").unwrap();
    let map = node.value.as_mapping().unwrap();
    assert_eq!(
        map[&MapKey::String("n".into())].value,
        Value::Float(f64::INFINITY)
    );
}

#[test]
fn float_nan() {
    let node = parse("n: nan").unwrap();
    let map = node.value.as_mapping().unwrap();
    if let Value::Float(f) = map[&MapKey::String("n".into())].value {
        assert!(f.is_nan());
    } else {
        panic!("expected float");
    }
}

// ── Bare Strings ─────────────────────────────────────────────────

#[test]
fn bare_string_simple() {
    let node = parse("name: Mark McGwire").unwrap();
    let map = node.value.as_mapping().unwrap();
    assert_eq!(
        map[&MapKey::String("name".into())].value,
        Value::Str("Mark McGwire".into())
    );
}

#[test]
fn bare_string_with_url() {
    let node = parse("url: https://example.com/path").unwrap();
    let map = node.value.as_mapping().unwrap();
    assert_eq!(
        map[&MapKey::String("url".into())].value,
        Value::Str("https://example.com/path".into())
    );
}

#[test]
fn bare_string_date_is_string() {
    // Dates are NOT special in AYML — they're just strings
    let node = parse("date: 2001-01-23").unwrap();
    let map = node.value.as_mapping().unwrap();
    assert_eq!(
        map[&MapKey::String("date".into())].value,
        Value::Str("2001-01-23".into())
    );
}

#[test]
fn bare_string_yes_is_string() {
    let node = parse("answer: yes").unwrap();
    let map = node.value.as_mapping().unwrap();
    assert_eq!(
        map[&MapKey::String("answer".into())].value,
        Value::Str("yes".into())
    );
}

#[test]
fn bare_string_no_is_string() {
    let node = parse("answer: no").unwrap();
    let map = node.value.as_mapping().unwrap();
    assert_eq!(
        map[&MapKey::String("answer".into())].value,
        Value::Str("no".into())
    );
}

// ── Double-Quoted Strings ────────────────────────────────────────

#[test]
fn double_quoted_simple() {
    let node = parse(r#"s: "hello world""#).unwrap();
    let map = node.value.as_mapping().unwrap();
    assert_eq!(
        map[&MapKey::String("s".into())].value,
        Value::Str("hello world".into())
    );
}

#[test]
fn double_quoted_escapes() {
    let node = parse(r#"s: "\t\n\r\\\"\/""#).unwrap();
    let map = node.value.as_mapping().unwrap();
    assert_eq!(
        map[&MapKey::String("s".into())].value,
        Value::Str("\t\n\r\\\"/".into())
    );
}

#[test]
fn double_quoted_unicode_escape() {
    let node = parse(r#"s: "Sosa did fine.\u263A""#).unwrap();
    let map = node.value.as_mapping().unwrap();
    assert_eq!(
        map[&MapKey::String("s".into())].value,
        Value::Str("Sosa did fine.\u{263A}".into())
    );
}

#[test]
fn double_quoted_hex_escape() {
    let node = parse(r#"s: "\x0d\x0a""#).unwrap();
    let map = node.value.as_mapping().unwrap();
    assert_eq!(
        map[&MapKey::String("s".into())].value,
        Value::Str("\r\n".into())
    );
}

#[test]
fn double_quoted_null_escape() {
    let node = parse(r#"s: "\0""#).unwrap();
    let map = node.value.as_mapping().unwrap();
    assert_eq!(
        map[&MapKey::String("s".into())].value,
        Value::Str("\0".into())
    );
}

#[test]
fn double_quoted_empty() {
    let node = parse(r#"s: """#).unwrap();
    let map = node.value.as_mapping().unwrap();
    assert_eq!(
        map[&MapKey::String("s".into())].value,
        Value::Str("".into())
    );
}

#[test]
fn double_quoted_reserved_word() {
    // "null" in quotes is a string, not null
    let node = parse(r#"s: "null""#).unwrap();
    let map = node.value.as_mapping().unwrap();
    assert_eq!(
        map[&MapKey::String("s".into())].value,
        Value::Str("null".into())
    );
}

#[test]
fn double_quoted_true_is_string() {
    let node = parse(r#"s: "true""#).unwrap();
    let map = node.value.as_mapping().unwrap();
    assert_eq!(
        map[&MapKey::String("s".into())].value,
        Value::Str("true".into())
    );
}

// ── Triple-Quoted Strings ────────────────────────────────────────

#[test]
fn triple_quoted_simple() {
    let input = "s: \"\"\"\n  This string\n  spans many lines.\n  \"\"\"";
    let node = parse(input).unwrap();
    let map = node.value.as_mapping().unwrap();
    assert_eq!(
        map[&MapKey::String("s".into())].value,
        Value::Str("This string\nspans many lines.".into())
    );
}

#[test]
fn triple_quoted_with_escape() {
    let input = "s: \"\"\"\n  Line one\\nhas an escape.\n  Line two is normal.\n  \"\"\"";
    let node = parse(input).unwrap();
    let map = node.value.as_mapping().unwrap();
    assert_eq!(
        map[&MapKey::String("s".into())].value,
        Value::Str("Line one\nhas an escape.\nLine two is normal.".into())
    );
}

#[test]
fn triple_quoted_line_continuation() {
    let input = "s: \"\"\"\n  This is all \\\n  on one line.\n  \"\"\"";
    let node = parse(input).unwrap();
    let map = node.value.as_mapping().unwrap();
    assert_eq!(
        map[&MapKey::String("s".into())].value,
        Value::Str("This is all on one line.".into())
    );
}

#[test]
fn triple_quoted_with_hash() {
    // # inside triple-quoted strings is literal, not a comment
    let input = "s: \"\"\"\n  x = foo(bar) # this is not a comment\n  \"\"\"";
    let node = parse(input).unwrap();
    let map = node.value.as_mapping().unwrap();
    assert_eq!(
        map[&MapKey::String("s".into())].value,
        Value::Str("x = foo(bar) # this is not a comment".into())
    );
}

#[test]
fn triple_quoted_with_quotes() {
    let input = "s: \"\"\"\n  You can add \"quotes\" and 'quotes' here.\n  \"\"\"";
    let node = parse(input).unwrap();
    let map = node.value.as_mapping().unwrap();
    assert_eq!(
        map[&MapKey::String("s".into())].value,
        Value::Str("You can add \"quotes\" and 'quotes' here.".into())
    );
}

#[test]
fn triple_quoted_deeper_indent() {
    // Closing """ at 4 spaces, content at 4+ spaces
    let input = "s: \"\"\"\n    line one\n    line two\n    \"\"\"";
    let node = parse(input).unwrap();
    let map = node.value.as_mapping().unwrap();
    assert_eq!(
        map[&MapKey::String("s".into())].value,
        Value::Str("line one\nline two".into())
    );
}

// ── Scalar Resolution ────────────────────────────────────────────

#[test]
fn resolution_order() {
    // null beats string
    let node = parse("v: null").unwrap();
    let map = node.value.as_mapping().unwrap();
    assert!(map[&MapKey::String("v".into())].value.is_null());

    // true beats string
    let node = parse("v: true").unwrap();
    let map = node.value.as_mapping().unwrap();
    assert_eq!(map[&MapKey::String("v".into())].value, Value::Bool(true));

    // 42 is int, not string
    let node = parse("v: 42").unwrap();
    let map = node.value.as_mapping().unwrap();
    assert_eq!(map[&MapKey::String("v".into())].value, Value::Int(42));

    // 3.25 is float, not string
    let node = parse("v: 3.25").unwrap();
    let map = node.value.as_mapping().unwrap();
    assert_eq!(map[&MapKey::String("v".into())].value, Value::Float(3.25));
}
