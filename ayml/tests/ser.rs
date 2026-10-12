#![allow(clippy::items_after_statements)]

use std::collections::BTreeMap;

use ayml::{Value, to_node, to_string, to_vec, to_writer};
use indexmap::IndexMap;
use serde::Serialize;

#[track_caller]
fn check<T: Serialize + ?Sized>(value: &T, expected: &str) {
    assert_eq!(to_string(value).unwrap(), expected);
}

#[test]
fn scalars() {
    check(&true, "true\n");
    check(&(), "null\n");
    check(&None::<i32>, "null\n");
    check(&Some(42), "42\n");
    check(&-17i64, "-17\n");
    check(&255u8, "255\n");
    check(&i64::MIN, "-9223372036854775808\n");
    check(&(i64::MAX as u64), "9223372036854775807\n");
    check(&5u128, "5\n");
}

#[test]
fn floats() {
    check(&3.25f64, "3.25\n");
    check(&1.0f64, "1.0\n");
    check(&-0.0f64, "-0.0\n");
    check(&1e300f64, "1e300\n");
    check(&1e-7f64, "1e-7\n");
    check(&f64::INFINITY, "inf\n");
    check(&f64::NEG_INFINITY, "-inf\n");
    check(&f64::NAN, "nan\n");
    check(&0.1f32, "0.1\n");
    check(&16_777_216f32, "16777216.0\n");
}

#[test]
fn strings_are_quoted_only_when_needed() {
    check(&"hello world", "hello world\n");
    check(
        &"https://example.com:8080/a",
        "https://example.com:8080/a\n",
    );
    check(&"**/*.rs", "**/*.rs\n");
    check(&"007", "007\n");
    check(&"", "\"\"\n");
    for reserved in ["null", "true", "false", "inf", "+inf", "-inf", "nan"] {
        check(&reserved, &format!("\"{reserved}\"\n"));
    }
    for number in ["42", "-7", "3.25", "1e5", "0xFF", "0o7", "0b1"] {
        check(&number, &format!("\"{number}\"\n"));
    }
    check(&"key: value", "\"key: value\"\n");
    check(&"a #b", "\"a #b\"\n");
    check(&"a#b", "\"a#b\"\n");
    check(&"- item", "\"- item\"\n");
    check(&"-", "\"-\"\n");
    check(&"[array", "\"[array\"\n");
    check(&"'quoted'", "\"'quoted'\"\n");
    check(&" leading", "\" leading\"\n");
    check(&"trailing ", "\"trailing \"\n");
    check(&"tab\there", "\"tab\\there\"\n");
    check(&"say \"hi\"", "say \"hi\"\n");
    check(&"\"hi\"", "\"\\\"hi\\\"\"\n");
    check(&"back\\slash", "back\\slash\n");
    check(&"\\start", "\"\\\\start\"\n");
    check(
        &"\0\x07\x1b\u{7f}\u{85}\u{feff}",
        "\"\\0\\a\\e\\x7F\\x85\\uFEFF\"\n",
    );
}

#[test]
fn multiline_strings_are_triple_quoted() {
    check(&"line\nbreak", "\"\"\"\n  line\n  break\n  \"\"\"\n");
    check(&"ends\n", "\"\"\"\n  ends\n\n  \"\"\"\n");
    check(
        &"a\n\"\"\"\n\\n",
        "\"\"\"\n  a\n  \"\"\\\"\n  \\\\n\n  \"\"\"\n",
    );
    check(&"a \nb\r", "\"\"\"\n  a\\ \n  b\\r\n  \"\"\"\n");
    #[derive(Serialize)]
    struct S {
        text: String,
    }
    check(
        &S {
            text: "x\ny".into(),
        },
        "text: \"\"\"\n  x\n  y\n  \"\"\"\n",
    );
}

#[test]
fn chars_and_bytes() {
    check(&'a', "a\n");
    check(&'"', "\"\\\"\"\n");
    check(&serde_bytes::Bytes::new(&[1, 2, 3]), "- 1\n- 2\n- 3\n");
}

#[test]
fn sequences() {
    check(&vec![1, 2, 3], "- 1\n- 2\n- 3\n");
    check(&Vec::<i32>::new(), "[]\n");
    check(&(42, "hello", true), "- 42\n- hello\n- true\n");
    check(
        &vec![vec![1, 2], vec![3], vec![]],
        "- - 1\n  - 2\n- - 3\n- []\n",
    );
}

#[test]
fn mappings() {
    let mut m = IndexMap::new();
    m.insert("b", 1);
    m.insert("a", 2);
    check(&m, "b: 1\na: 2\n");
    check(&BTreeMap::<String, i32>::new(), "{}\n");
    let keys = BTreeMap::from([
        ("true", 1),
        ("1", 2),
        ("null", 3),
        ("with: colon", 4),
        ("", 5),
    ]);
    check(
        &keys,
        "\"\": 5\n1: 2\nnull: 3\ntrue: 1\n\"with: colon\": 4\n",
    );
}

#[test]
fn non_string_keys() {
    check(&BTreeMap::from([(1, "a"), (-2, "b")]), "-2: b\n1: a\n");
    check(&BTreeMap::from([(true, 1)]), "true: 1\n");
    check(&BTreeMap::from([('c', 1)]), "c: 1\n");
    #[derive(Serialize, PartialEq, Eq, PartialOrd, Ord)]
    enum K {
        Alpha,
    }
    check(&BTreeMap::from([(K::Alpha, 1)]), "Alpha: 1\n");
}

#[test]
fn structs() {
    #[derive(Serialize)]
    struct Config {
        name: String,
        debug: bool,
        label: Option<String>,
        inner: Inner,
        ports: Vec<u16>,
        empty: Vec<u16>,
        servers: Vec<Inner>,
    }
    #[derive(Serialize)]
    struct Inner {
        host: String,
        port: u16,
    }
    let c = Config {
        name: "app".into(),
        debug: true,
        label: None,
        inner: Inner {
            host: "localhost".into(),
            port: 1,
        },
        ports: vec![8080, 9090],
        empty: vec![],
        servers: vec![
            Inner {
                host: "a".into(),
                port: 2,
            },
            Inner {
                host: "b".into(),
                port: 3,
            },
        ],
    };
    check(
        &c,
        "\
name: app
debug: true
label: null
inner:
  host: localhost
  port: 1
ports:
  - 8080
  - 9090
empty: []
servers:
  - host: a
    port: 2
  - host: b
    port: 3
",
    );
}

#[test]
fn newtype_and_unit_structs() {
    #[derive(Serialize)]
    struct Meters(f64);
    #[derive(Serialize)]
    struct Marker;
    #[derive(Serialize)]
    struct Point(i32, i32);
    check(&Meters(3.5), "3.5\n");
    check(&Marker, "null\n");
    check(&Point(1, 2), "- 1\n- 2\n");
}

#[test]
fn enums() {
    #[derive(Serialize)]
    enum Shape {
        Empty,
        Circle(f64),
        Move(i32, i32),
        Rect { w: u32, h: u32 },
    }
    check(&Shape::Empty, "Empty\n");
    check(&Shape::Circle(3.25), "Circle: 3.25\n");
    check(&Shape::Move(10, 20), "Move:\n  - 10\n  - 20\n");
    check(&Shape::Rect { w: 10, h: 20 }, "Rect:\n  w: 10\n  h: 20\n");
    check(
        &vec![Shape::Empty, Shape::Circle(1.0)],
        "- Empty\n- Circle: 1.0\n",
    );
}

#[test]
fn value_display_is_flow_style() {
    let v: Value = ayml::from_str("a: [1, \"x y\", null]\nb: {c: \"1\"}\n\"d e\": \"\"").unwrap();
    assert_eq!(
        v.to_string(),
        "{a: [1, x y, null], b: {c: \"1\"}, d e: \"\"}"
    );
    assert_eq!(Value::Str("a,b".into()).to_string(), "\"a,b\"");
    assert_eq!(
        Value::Str("line\nbreak".into()).to_string(),
        "\"line\\nbreak\""
    );
    assert_eq!(Value::Float(2.0).to_string(), "2.0");
}

#[test]
fn to_node_allows_editing_comments() {
    #[derive(Serialize)]
    struct Config {
        port: u16,
    }
    let mut node = to_node(&Config { port: 8080 }).unwrap();
    if let ayml_core::Value::Map(map) = &mut node.value {
        map["port"].inline_comment = Some("default".into());
    }
    assert_eq!(ayml_core::emit(&node), "port: 8080 # default\n");
}

#[test]
fn to_vec_and_writer() {
    assert_eq!(to_vec(&42i32).unwrap(), b"42\n");
    let mut buf = Vec::new();
    to_writer(&mut buf, &"hello").unwrap();
    assert_eq!(buf, b"hello\n");
}
