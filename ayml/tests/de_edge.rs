//! Deserialization edge cases — things that don't roundtrip but must parse correctly.

use ayml::from_str;
use serde::Deserialize;
use std::collections::HashMap;

// ── Whitespace and comments ─────────────────────────────────────

#[test]
fn de_leading_whitespace() {
    assert_eq!(from_str::<i32>("  42").unwrap(), 42);
    assert_eq!(from_str::<i32>("\n42").unwrap(), 42);
    assert_eq!(from_str::<i32>("\n\n  42").unwrap(), 42);
}

#[test]
fn de_trailing_whitespace() {
    assert_eq!(from_str::<i32>("42  ").unwrap(), 42);
    assert_eq!(from_str::<i32>("42\n").unwrap(), 42);
    assert_eq!(from_str::<i32>("42\n\n").unwrap(), 42);
}

#[test]
fn de_leading_comment() {
    assert_eq!(from_str::<i32>("# a comment\n42").unwrap(), 42);
    assert_eq!(from_str::<i32>("# line1\n# line2\n42").unwrap(), 42);
}

#[test]
fn de_trailing_comment() {
    assert_eq!(from_str::<i32>("42 # trailing").unwrap(), 42);
}

#[test]
fn de_block_mapping_with_comments() {
    #[derive(Deserialize, Debug, PartialEq)]
    struct Config {
        host: String,
        port: u16,
    }
    let input = "host: localhost # the host\nport: 8080 # the port";
    let c: Config = from_str(input).unwrap();
    assert_eq!(c.host, "localhost");
    assert_eq!(c.port, 8080);
}

// ── Flow collections ────────────────────────────────────────────

#[test]
fn de_flow_sequence_multiline() {
    let input = "[\n  1,\n  2,\n  3\n]";
    let v: Vec<i32> = from_str(input).unwrap();
    assert_eq!(v, vec![1, 2, 3]);
}

#[test]
fn de_flow_mapping_multiline() {
    let input = "{\n  a: 1,\n  b: 2\n}";
    let m: HashMap<String, i32> = from_str(input).unwrap();
    assert_eq!(m["a"], 1);
    assert_eq!(m["b"], 2);
}

#[test]
fn de_flow_trailing_comma_seq() {
    let v: Vec<i32> = from_str("[1, 2, 3,]").unwrap();
    assert_eq!(v, vec![1, 2, 3]);
}

#[test]
fn de_flow_trailing_comma_map() {
    let m: HashMap<String, i32> = from_str("{a: 1, b: 2,}").unwrap();
    assert_eq!(m["a"], 1);
    assert_eq!(m["b"], 2);
}

// ── Block collections ───────────────────────────────────────────

#[test]
fn de_block_sequence_with_blank_lines() {
    let input = "- 1\n\n- 2\n\n- 3";
    let v: Vec<i32> = from_str(input).unwrap();
    assert_eq!(v, vec![1, 2, 3]);
}

#[test]
fn de_block_mapping_with_blank_lines() {
    #[derive(Deserialize, Debug, PartialEq)]
    struct S {
        a: i32,
        b: i32,
    }
    let input = "a: 1\n\nb: 2";
    let s: S = from_str(input).unwrap();
    assert_eq!(s, S { a: 1, b: 2 });
}

#[test]
fn de_block_sequence_with_comments_between() {
    let input = "- 1\n# comment\n- 2\n# another\n- 3";
    let v: Vec<i32> = from_str(input).unwrap();
    assert_eq!(v, vec![1, 2, 3]);
}

// ── Any resolution ──────────────────────────────────────────────

#[test]
fn de_any_null() {
    #[derive(Deserialize, Debug, PartialEq)]
    #[serde(untagged)]
    enum Any {
        Null,
        Int(i64),
    }
    assert_eq!(from_str::<Any>("null").unwrap(), Any::Null);
}

#[test]
fn de_any_bool() {
    #[derive(Deserialize, Debug, PartialEq)]
    #[serde(untagged)]
    enum Any {
        Bool(bool),
        Str(String),
    }
    assert_eq!(from_str::<Any>("true").unwrap(), Any::Bool(true));
    assert_eq!(from_str::<Any>("false").unwrap(), Any::Bool(false));
}

#[test]
fn de_any_int() {
    #[derive(Deserialize, Debug, PartialEq)]
    #[serde(untagged)]
    enum Any {
        Int(i64),
        Str(String),
    }
    assert_eq!(from_str::<Any>("42").unwrap(), Any::Int(42));
}

#[test]
fn de_any_float() {
    #[derive(Deserialize, Debug, PartialEq)]
    #[serde(untagged)]
    enum Any {
        Float(f64),
        Str(String),
    }
    assert_eq!(from_str::<Any>("3.25").unwrap(), Any::Float(3.25));
}

#[test]
fn de_any_string() {
    #[derive(Deserialize, Debug, PartialEq)]
    #[serde(untagged)]
    enum Any {
        Bool(bool),
        Int(i64),
        Float(f64),
        Str(String),
    }
    assert_eq!(from_str::<Any>("hello").unwrap(), Any::Str("hello".into()));
}

#[test]
fn de_any_seq() {
    #[derive(Deserialize, Debug, PartialEq)]
    #[serde(untagged)]
    enum Any {
        Seq(Vec<i32>),
        Int(i32),
    }
    assert_eq!(
        from_str::<Any>("[1, 2, 3]").unwrap(),
        Any::Seq(vec![1, 2, 3])
    );
}

#[test]
fn de_any_map() {
    #[derive(Deserialize, Debug, PartialEq)]
    #[serde(untagged)]
    enum Any {
        Map(HashMap<String, i32>),
        Int(i32),
    }
    let v = from_str::<Any>("a: 1\nb: 2").unwrap();
    if let Any::Map(m) = v {
        assert_eq!(m["a"], 1);
        assert_eq!(m["b"], 2);
    } else {
        panic!("expected Map");
    }
}

// ── Numeric bases ───────────────────────────────────────────────



#[test]
fn de_signed_integer_prefix() {
    assert_eq!(from_str::<i32>("+42").unwrap(), 42);
    assert_eq!(from_str::<i32>("-42").unwrap(), -42);
}

// ── Float special values ────────────────────────────────────────

#[test]
fn de_float_inf() {
    assert_eq!(from_str::<f64>("inf").unwrap(), f64::INFINITY);
    assert_eq!(from_str::<f64>("+inf").unwrap(), f64::INFINITY);
    assert_eq!(from_str::<f64>("-inf").unwrap(), f64::NEG_INFINITY);
}

#[test]
fn de_float_nan() {
    assert!(from_str::<f64>("nan").unwrap().is_nan());
}

#[test]
fn de_float_exponential() {
    assert_eq!(from_str::<f64>("1e10").unwrap(), 1e10);
    assert_eq!(from_str::<f64>("1.23015e+3").unwrap(), 1230.15);
    assert_eq!(from_str::<f64>("12.3015e+02").unwrap(), 1230.15);
}

// ── Integer as float ────────────────────────────────────────────

#[test]
fn de_integer_as_float() {
    assert_eq!(from_str::<f64>("42").unwrap(), 42.0);
    assert_eq!(from_str::<f64>("0").unwrap(), 0.0);
    assert_eq!(from_str::<f64>("-17").unwrap(), -17.0);
}

// ── Quoted strings preserve literal content ─────────────────────

#[test]
fn de_quoted_null() {
    assert_eq!(from_str::<String>(r#""null""#).unwrap(), "null");
}

#[test]
fn de_quoted_true() {
    assert_eq!(from_str::<String>(r#""true""#).unwrap(), "true");
}

#[test]
fn de_quoted_number() {
    assert_eq!(from_str::<String>(r#""42""#).unwrap(), "42");
}

// ── Escape sequences ────────────────────────────────────────────

#[test]
fn de_escape_basic() {
    assert_eq!(from_str::<String>(r#""\n""#).unwrap(), "\n");
    assert_eq!(from_str::<String>(r#""\t""#).unwrap(), "\t");
    assert_eq!(from_str::<String>(r#""\r""#).unwrap(), "\r");
    assert_eq!(from_str::<String>(r#""\\""#).unwrap(), "\\");
    assert_eq!(from_str::<String>(r#""\"""#).unwrap(), "\"");
    assert_eq!(from_str::<String>(r#""\/""#).unwrap(), "/");
}

#[test]
fn de_escape_special() {
    assert_eq!(from_str::<String>(r#""\0""#).unwrap(), "\0");
    assert_eq!(from_str::<String>(r#""\a""#).unwrap(), "\x07");
    assert_eq!(from_str::<String>(r#""\b""#).unwrap(), "\x08");
    assert_eq!(from_str::<String>(r#""\v""#).unwrap(), "\x0B");
    assert_eq!(from_str::<String>(r#""\f""#).unwrap(), "\x0C");
    assert_eq!(from_str::<String>(r#""\e""#).unwrap(), "\x1B");
}

#[test]
fn de_escape_hex() {
    assert_eq!(from_str::<String>(r#""\x0d\x0a""#).unwrap(), "\r\n");
    assert_eq!(from_str::<String>(r#""\x41""#).unwrap(), "A");
}

#[test]
fn de_escape_unicode_4() {
    assert_eq!(from_str::<String>(r#""\u263A""#).unwrap(), "☺");
    assert_eq!(from_str::<String>(r#""\u03A3""#).unwrap(), "Σ");
}

#[test]
fn de_escape_unicode_8() {
    assert_eq!(from_str::<String>(r#""\U0001F389""#).unwrap(), "🎉");
}

// ── from_reader / from_slice ────────────────────────────────────

#[test]
fn de_from_reader_struct() {
    #[derive(Deserialize, Debug, PartialEq)]
    struct Config {
        name: String,
        port: u16,
    }
    let data = b"name: myapp\nport: 3000" as &[u8];
    let c: Config = ayml::from_reader(data).unwrap();
    assert_eq!(
        c,
        Config {
            name: "myapp".into(),
            port: 3000
        }
    );
}

#[test]
fn de_from_slice_basic() {
    assert_eq!(ayml::from_slice::<i32>(b"42").unwrap(), 42);
    assert_eq!(ayml::from_slice::<String>(b"hello").unwrap(), "hello");
}

// ── Struct field rename ─────────────────────────────────────────

#[test]
fn de_serde_rename() {
    #[derive(Deserialize, Debug, PartialEq)]
    struct Config {
        #[serde(rename = "server-name")]
        server_name: String,
    }
    let c: Config = from_str("server-name: myhost").unwrap();
    assert_eq!(c.server_name, "myhost");
}

// ── Default fields ──────────────────────────────────────────────

#[test]
fn de_serde_default() {
    #[derive(Deserialize, Debug, PartialEq)]
    struct Config {
        name: String,
        #[serde(default)]
        debug: bool,
        #[serde(default)]
        tags: Vec<String>,
    }
    let c: Config = from_str("name: app").unwrap();
    assert_eq!(
        c,
        Config {
            name: "app".into(),
            debug: false,
            tags: vec![],
        }
    );
}

// ── Flattened structs ───────────────────────────────────────────

#[test]
fn de_serde_flatten() {
    #[derive(Deserialize, Debug, PartialEq)]
    struct Outer {
        name: String,
        #[serde(flatten)]
        extra: HashMap<String, String>,
    }
    let o: Outer = from_str("name: app\nfoo: bar\nbaz: qux").unwrap();
    assert_eq!(o.name, "app");
    assert_eq!(o.extra["foo"], "bar");
    assert_eq!(o.extra["baz"], "qux");
}

// ── Ignored fields ──────────────────────────────────────────────

#[test]
fn de_option_null_in_flow_seq() {
    let v: Vec<Option<i32>> = from_str("[null, 42]").unwrap();
    assert_eq!(v, vec![None, Some(42)]);
}

#[test]
fn de_option_null_in_flow_map() {
    let m: HashMap<String, Option<i32>> = from_str("{x: null, y: 1}").unwrap();
    assert_eq!(m["x"], None);
    assert_eq!(m["y"], Some(1));
}

#[test]
fn de_option_null_before_closing_bracket() {
    let v: Vec<Option<i32>> = from_str("[null]").unwrap();
    assert_eq!(v, vec![None]);
}

#[test]
fn de_option_null_before_closing_brace() {
    let m: HashMap<String, Option<i32>> = from_str("{x: null}").unwrap();
    assert_eq!(m["x"], None);
}

#[test]
fn de_triple_quoted_basic() {
    let input = "value: \"\"\"\n  hello world\n  \"\"\"";
    #[derive(Deserialize, Debug, PartialEq)]
    struct S {
        value: String,
    }
    let s: S = from_str(input).unwrap();
    assert_eq!(s.value, "hello world");
}

#[test]
fn de_triple_quoted_multiline() {
    let input = "msg: \"\"\"\n  line one\n  line two\n  \"\"\"";
    #[derive(Deserialize, Debug, PartialEq)]
    struct S {
        msg: String,
    }
    let s: S = from_str(input).unwrap();
    assert_eq!(s.msg, "line one\nline two");
}

#[test]
fn de_triple_quoted_with_escape() {
    let input = "msg: \"\"\"\n  hello\\nworld\n  \"\"\"";
    #[derive(Deserialize, Debug, PartialEq)]
    struct S {
        msg: String,
    }
    let s: S = from_str(input).unwrap();
    assert_eq!(s.msg, "hello\nworld");
}

#[test]
fn de_triple_quoted_line_continuation() {
    let input = "msg: \"\"\"\n  hello \\\n  world\n  \"\"\"";
    #[derive(Deserialize, Debug, PartialEq)]
    struct S {
        msg: String,
    }
    let s: S = from_str(input).unwrap();
    assert_eq!(s.msg, "hello world");
}

#[test]
fn de_triple_quoted_blank_line() {
    let input = "msg: \"\"\"\n  line one\n\n  line two\n  \"\"\"";
    #[derive(Deserialize, Debug, PartialEq)]
    struct S {
        msg: String,
    }
    let s: S = from_str(input).unwrap();
    assert_eq!(s.msg, "line one\n\nline two");
}

#[test]
fn de_triple_quoted_top_level() {
    let input = "\"\"\"\n  just text\n  \"\"\"";
    let s: String = from_str(input).unwrap();
    assert_eq!(s, "just text");
}

#[test]
fn de_triple_quoted_with_hash() {
    // # inside triple-quoted string is NOT a comment
    let input = "code: \"\"\"\n  x = foo(bar) # not a comment\n  \"\"\"";
    #[derive(Deserialize, Debug, PartialEq)]
    struct S {
        code: String,
    }
    let s: S = from_str(input).unwrap();
    assert_eq!(s.code, "x = foo(bar) # not a comment");
}

#[test]
fn de_depth_limit_nested_enums() {
    // Deeply nested enum mappings should hit the depth limit
    #[derive(Deserialize, Debug)]
    #[allow(dead_code)]
    enum E {
        A(Box<E>),
        B,
    }
    // 200 levels of nesting via block enum mappings
    let mut input = String::new();
    for _ in 0..200 {
        input.push_str("A: ");
    }
    input.push('B');
    let err = from_str::<E>(&input);
    assert!(err.is_err(), "should have hit depth limit");
    assert!(
        err.unwrap_err().to_string().contains("nesting depth"),
        "wrong error type"
    );
}

#[test]
fn de_io_read_no_oom_on_whitespace() {
    // IoRead no longer buffers the entire input, so an infinite stream of
    // whitespace just parses until EOF or error without OOM.  We verify
    // that parsing a finite but large whitespace-only input returns an
    // appropriate error (unexpected end of input) rather than hanging.
    let data = vec![b' '; 64 * 1024];
    let err = ayml::from_reader::<_, i32>(&data[..]);
    assert!(err.is_err());
}

#[test]
fn de_ignore_unknown_fields() {
    #[derive(Deserialize, Debug, PartialEq)]
    struct Config {
        name: String,
    }
    // Extra field 'port' should be ignored (serde default behavior)
    let c: Config = from_str("name: app\nport: 8080").unwrap();
    assert_eq!(c.name, "app");
}
