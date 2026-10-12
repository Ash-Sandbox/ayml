#![allow(clippy::items_after_statements)]

use std::collections::{BTreeMap, HashMap};

use ayml::{Value, from_node, from_reader, from_slice, from_str};
use indexmap::IndexMap;
use serde::Deserialize;

#[test]
fn null_and_unit() {
    from_str::<()>("null").unwrap();
    #[derive(Deserialize, Debug, PartialEq)]
    struct Marker;
    assert_eq!(from_str::<Marker>("null").unwrap(), Marker);
}

#[test]
fn bools() {
    assert!(from_str::<bool>("true").unwrap());
    assert!(!from_str::<bool>("false").unwrap());
}

#[test]
fn integers_in_every_notation() {
    assert_eq!(from_str::<i32>("42").unwrap(), 42);
    assert_eq!(from_str::<i32>("-17").unwrap(), -17);
    assert_eq!(from_str::<i32>("+17").unwrap(), 17);
    assert_eq!(from_str::<u8>("255").unwrap(), 255);
    assert_eq!(from_str::<u32>("0xFF").unwrap(), 255);
    assert_eq!(from_str::<i32>("-0x10").unwrap(), -16);
    assert_eq!(from_str::<i32>("0b1010").unwrap(), 10);
    assert_eq!(from_str::<i32>("0o77").unwrap(), 63);
    assert_eq!(from_str::<i64>("-0x8000000000000000").unwrap(), i64::MIN);
    assert_eq!(from_str::<i64>("9223372036854775807").unwrap(), i64::MAX);
}

#[test]
fn wide_integers() {
    assert_eq!(from_str::<i128>("-5").unwrap(), -5);
    assert_eq!(from_str::<u128>("5").unwrap(), 5);
    assert_eq!(
        from_str::<u64>("9223372036854775807").unwrap(),
        i64::MAX as u64
    );
}

#[test]
#[allow(clippy::float_cmp)]
fn floats() {
    assert_eq!(from_str::<f64>("3.25").unwrap(), 3.25);
    assert_eq!(from_str::<f64>("-0.5").unwrap(), -0.5);
    assert_eq!(from_str::<f64>("1e10").unwrap(), 1e10);
    assert_eq!(from_str::<f64>("1.5E-3").unwrap(), 1.5e-3);
    assert_eq!(from_str::<f64>("12.3015e+02").unwrap(), 1230.15);
    assert_eq!(from_str::<f64>("inf").unwrap(), f64::INFINITY);
    assert_eq!(from_str::<f64>("+inf").unwrap(), f64::INFINITY);
    assert_eq!(from_str::<f64>("-inf").unwrap(), f64::NEG_INFINITY);
    assert!(from_str::<f64>("nan").unwrap().is_nan());
    assert_eq!(from_str::<f32>("0.1").unwrap(), 0.1f32);
}

#[test]
#[allow(clippy::float_cmp)]
fn integers_widen_to_floats() {
    assert_eq!(from_str::<f64>("42").unwrap(), 42.0);
    assert_eq!(from_str::<f32>("-0x10").unwrap(), -16.0);
}

#[test]
fn strings() {
    assert_eq!(from_str::<String>("hello world").unwrap(), "hello world");
    assert_eq!(
        from_str::<String>(r#""hello\nworld""#).unwrap(),
        "hello\nworld"
    );
    assert_eq!(from_str::<String>(r#""null""#).unwrap(), "null");
    assert_eq!(from_str::<String>(r#""42""#).unwrap(), "42");
    assert_eq!(from_str::<String>("yes").unwrap(), "yes");
    assert_eq!(from_str::<String>("2001-01-23").unwrap(), "2001-01-23");
    assert_eq!(from_str::<String>("0755").unwrap(), "0755");
    assert_eq!(
        from_str::<String>("https://example.com:8080/a").unwrap(),
        "https://example.com:8080/a"
    );
}

#[test]
fn escapes() {
    assert_eq!(
        from_str::<String>(r#""\t\r\0\a\b\v\f\e\ \"\/\\""#).unwrap(),
        "\t\r\0\x07\x08\x0B\x0C\x1B \"/\\"
    );
    assert_eq!(from_str::<String>(r#""\x41☺\U0001F600""#).unwrap(), "A☺😀");
}

#[test]
fn triple_quoted() {
    let s: String = from_str("\"\"\"\n  line one\n  line two\n  \"\"\"").unwrap();
    assert_eq!(s, "line one\nline two");
    let s: String = from_str("\"\"\"\n  a\n\n  \"\"\"").unwrap();
    assert_eq!(s, "a\n");
    let s: String = from_str("\"\"\"\n  folded \\\n  line\n  \"\"\"").unwrap();
    assert_eq!(s, "folded line");
    let s: String = from_str("\"\"\"\n  # not a comment\n    indented\n  \"\"\"").unwrap();
    assert_eq!(s, "# not a comment\n  indented");
}

#[test]
fn chars() {
    assert_eq!(from_str::<char>("a").unwrap(), 'a');
    assert_eq!(from_str::<char>(r#""x""#).unwrap(), 'x');
    assert_eq!(from_str::<char>(r#""\n""#).unwrap(), '\n');
}

#[test]
fn options() {
    assert_eq!(from_str::<Option<i32>>("null").unwrap(), None);
    assert_eq!(from_str::<Option<i32>>("42").unwrap(), Some(42));
    assert_eq!(
        from_str::<Option<String>>(r#""null""#).unwrap(),
        Some("null".into())
    );
    let v: Vec<Option<i64>> = from_str("[1, null, 3, null]").unwrap();
    assert_eq!(v, vec![Some(1), None, Some(3), None]);
}

#[test]
#[allow(clippy::float_cmp)]
fn newtype_and_tuple_structs() {
    #[derive(Deserialize, Debug, PartialEq)]
    struct Meters(f64);
    #[derive(Deserialize, Debug, PartialEq)]
    struct Point(i32, i32);
    assert_eq!(from_str::<Meters>("3.5").unwrap(), Meters(3.5));
    assert_eq!(from_str::<Point>("[1, 2]").unwrap(), Point(1, 2));
    assert_eq!(from_str::<Point>("- 1\n- 2").unwrap(), Point(1, 2));
}

#[test]
fn bytes_from_sequence_or_string() {
    let b: serde_bytes::ByteBuf = from_str("[1, 2, 255]").unwrap();
    assert_eq!(b.as_slice(), &[1, 2, 255]);
    let b: serde_bytes::ByteBuf = from_str("hi").unwrap();
    assert_eq!(b.as_slice(), b"hi");
}

#[test]
fn sequences() {
    assert_eq!(from_str::<Vec<i32>>("[1, 2, 3]").unwrap(), vec![1, 2, 3]);
    assert_eq!(from_str::<Vec<i32>>("[1, 2,]").unwrap(), vec![1, 2]);
    assert_eq!(from_str::<Vec<i32>>("[]").unwrap(), Vec::<i32>::new());
    assert_eq!(
        from_str::<Vec<String>>("- apple\n- banana").unwrap(),
        vec!["apple", "banana"]
    );
    assert_eq!(
        from_str::<Vec<i32>>("[\n  1, # one\n  # two\n  2,\n]").unwrap(),
        vec![1, 2]
    );
}

#[test]
fn nested_sequences() {
    let v: Vec<Vec<i32>> = from_str("[[1, 2], [3, 4]]").unwrap();
    assert_eq!(v, vec![vec![1, 2], vec![3, 4]]);
    let v: Vec<Vec<i32>> = from_str("- [1, 2]\n- [3, 4]").unwrap();
    assert_eq!(v, vec![vec![1, 2], vec![3, 4]]);
    let v: Vec<Vec<i32>> = from_str("- - 1\n  - 2\n- - 3").unwrap();
    assert_eq!(v, vec![vec![1, 2], vec![3]]);
}

#[test]
fn tuples() {
    let t: (i32, String, bool) = from_str("[42, hello, true]").unwrap();
    assert_eq!(t, (42, "hello".to_string(), true));
    let a: [u8; 3] = from_str("- 1\n- 2\n- 3").unwrap();
    assert_eq!(a, [1, 2, 3]);
}

#[test]
fn mappings() {
    let m: HashMap<String, i32> = from_str("{a: 1, b: 2}").unwrap();
    assert_eq!(m["a"], 1);
    assert_eq!(m["b"], 2);
    let m: HashMap<String, i32> = from_str("{\"a\":1,}").unwrap();
    assert_eq!(m["a"], 1);
    let m: HashMap<String, i32> = from_str("{}").unwrap();
    assert!(m.is_empty());
    let m: IndexMap<String, i32> = from_str("z: 1\na: 2\nm: 3").unwrap();
    assert_eq!(m.keys().collect::<Vec<_>>(), ["z", "a", "m"]);
}

#[test]
fn keys_are_strings_parsed_on_demand() {
    let m: BTreeMap<u32, String> = from_str("1: one\n0x2: two\n\"3\": three").unwrap();
    assert_eq!(
        m,
        BTreeMap::from([(1, "one".into()), (2, "two".into()), (3, "three".into())])
    );
    let m: BTreeMap<bool, i32> = from_str("true: 1\nfalse: 0").unwrap();
    assert_eq!(m, BTreeMap::from([(true, 1), (false, 0)]));
    let m: BTreeMap<String, i32> = from_str("true: 1\nnull: 2\n1.5: 3").unwrap();
    assert_eq!(m.keys().collect::<Vec<_>>(), ["1.5", "null", "true"]);
    #[derive(Deserialize, Debug, PartialEq, Eq, PartialOrd, Ord)]
    #[serde(rename_all = "lowercase")]
    enum Env {
        Dev,
        Prod,
    }
    let m: BTreeMap<Env, u8> = from_str("dev: 1\nprod: 2").unwrap();
    assert_eq!(m, BTreeMap::from([(Env::Dev, 1), (Env::Prod, 2)]));
}

#[test]
fn structs() {
    #[derive(Deserialize, Debug, PartialEq)]
    struct Config {
        host: String,
        port: u16,
        tags: Vec<String>,
        inner: Inner,
    }
    #[derive(Deserialize, Debug, PartialEq)]
    struct Inner {
        value: i32,
    }
    let input = "\
# leading comment
host: localhost # the host
port: 8080

tags:
- alpha
- beta
inner:
  value: 42
# trailing comment
";
    let c: Config = from_str(input).unwrap();
    assert_eq!(
        c,
        Config {
            host: "localhost".into(),
            port: 8080,
            tags: vec!["alpha".into(), "beta".into()],
            inner: Inner { value: 42 },
        }
    );
}

#[test]
fn sequence_of_structs() {
    #[derive(Deserialize, Debug, PartialEq)]
    struct Item {
        name: String,
        qty: u32,
    }
    let items: Vec<Item> =
        from_str("- name: Widget\n  qty: 5\n-   name: Gadget\n    qty: 3").unwrap();
    assert_eq!(
        items,
        vec![
            Item {
                name: "Widget".into(),
                qty: 5
            },
            Item {
                name: "Gadget".into(),
                qty: 3
            },
        ]
    );
}

#[test]
fn serde_attributes() {
    #[derive(Deserialize, Debug, PartialEq)]
    struct Config {
        #[serde(rename = "server-name")]
        name: String,
        #[serde(default)]
        retries: u32,
        #[serde(flatten)]
        extra: BTreeMap<String, Value>,
    }
    let c: Config = from_str("server-name: x\nother: [1, 2]").unwrap();
    assert_eq!(c.name, "x");
    assert_eq!(c.retries, 0);
    assert_eq!(
        c.extra["other"],
        Value::Seq(vec![Value::Int(1), Value::Int(2)])
    );
}

#[test]
fn unknown_fields_are_ignored_by_default() {
    #[derive(Deserialize, Debug, PartialEq)]
    struct Config {
        a: i32,
    }
    let c: Config = from_str("a: 1\nb: {nested: [1, 2]}\nc: \"x\"").unwrap();
    assert_eq!(c, Config { a: 1 });
}

#[derive(Deserialize, Debug, PartialEq)]
enum Shape {
    Empty,
    Circle(f64),
    Move(i32, i32),
    Rect { w: u32, h: u32 },
}

#[test]
fn enum_unit_variants() {
    assert_eq!(from_str::<Shape>("Empty").unwrap(), Shape::Empty);
    assert_eq!(from_str::<Shape>("\"Empty\"").unwrap(), Shape::Empty);
    let v: Vec<Shape> = from_str("- Empty\n- Empty").unwrap();
    assert_eq!(v, vec![Shape::Empty, Shape::Empty]);
}

#[test]
fn enum_variants_block_and_flow() {
    assert_eq!(
        from_str::<Shape>("Circle: 3.25").unwrap(),
        Shape::Circle(3.25)
    );
    assert_eq!(
        from_str::<Shape>("{Circle: 3.25}").unwrap(),
        Shape::Circle(3.25)
    );
    assert_eq!(
        from_str::<Shape>("Move: [10, 20]").unwrap(),
        Shape::Move(10, 20)
    );
    assert_eq!(
        from_str::<Shape>("Move:\n  - 10\n  - 20").unwrap(),
        Shape::Move(10, 20)
    );
    assert_eq!(
        from_str::<Shape>("{Rect: {w: 1, h: 2}}").unwrap(),
        Shape::Rect { w: 1, h: 2 }
    );
    assert_eq!(
        from_str::<Shape>("Rect:\n  w: 1\n  h: 2").unwrap(),
        Shape::Rect { w: 1, h: 2 }
    );
}

#[test]
fn enum_in_struct() {
    #[derive(Deserialize, Debug, PartialEq)]
    struct S {
        shape: Shape,
        shapes: Vec<Shape>,
    }
    let s: S = from_str("shape:\n  Circle: 1.0\nshapes:\n  - Empty\n  - Move: [1, 2]").unwrap();
    assert_eq!(
        s,
        S {
            shape: Shape::Circle(1.0),
            shapes: vec![Shape::Empty, Shape::Move(1, 2)]
        }
    );
}

#[test]
fn internally_tagged_and_untagged_enums() {
    #[derive(Deserialize, Debug, PartialEq)]
    #[serde(tag = "type")]
    enum Tagged {
        A { x: i32 },
        B,
    }
    #[derive(Deserialize, Debug, PartialEq)]
    #[serde(untagged)]
    enum Untagged {
        Int(i64),
        Str(String),
        List(Vec<i64>),
    }
    assert_eq!(
        from_str::<Tagged>("type: A\nx: 1").unwrap(),
        Tagged::A { x: 1 }
    );
    assert_eq!(from_str::<Tagged>("{type: B}").unwrap(), Tagged::B);
    assert_eq!(from_str::<Untagged>("5").unwrap(), Untagged::Int(5));
    assert_eq!(
        from_str::<Untagged>("\"5\"").unwrap(),
        Untagged::Str("5".into())
    );
    assert_eq!(
        from_str::<Untagged>("[5]").unwrap(),
        Untagged::List(vec![5])
    );
}

#[test]
fn value_resolves_every_type() {
    let v: Value = from_str("[null, true, 1, 1.5, inf, x, \"1\", [], {}]").unwrap();
    assert_eq!(
        v,
        Value::Seq(vec![
            Value::Null,
            Value::Bool(true),
            Value::Int(1),
            Value::Float(1.5),
            Value::Float(f64::INFINITY),
            Value::Str("x".into()),
            Value::Str("1".into()),
            Value::Seq(vec![]),
            Value::Map(IndexMap::new()),
        ])
    );
}

#[test]
fn value_keys_stay_strings() {
    let v: Value = from_str("1: a\ntrue: b").unwrap();
    let Value::Map(m) = v else { panic!() };
    assert_eq!(m.keys().collect::<Vec<_>>(), ["1", "true"]);
}

#[test]
fn from_slice_and_reader() {
    assert_eq!(from_slice::<i32>(b"42").unwrap(), 42);
    assert_eq!(from_slice::<String>(b"\"quoted\"").unwrap(), "quoted");
    assert_eq!(from_reader::<_, i32>(&b"42\n"[..]).unwrap(), 42);
    #[derive(Deserialize, Debug, PartialEq)]
    struct Config {
        name: String,
        port: u16,
    }
    let c: Config = from_reader(&b"name: myapp\nport: 3000"[..]).unwrap();
    assert_eq!(
        c,
        Config {
            name: "myapp".into(),
            port: 3000
        }
    );
}

#[test]
fn from_node_borrows_from_the_tree() {
    #[derive(Deserialize, Debug, PartialEq)]
    struct Borrowed<'a> {
        name: &'a str,
        tags: Vec<&'a str>,
    }
    let node = ayml_core::parse("name: x\ntags: [a, b]").unwrap();
    let b: Borrowed = from_node(&node).unwrap();
    assert_eq!(
        b,
        Borrowed {
            name: "x",
            tags: vec!["a", "b"]
        }
    );
    let sub: Vec<String> = from_node(node.get("tags").unwrap()).unwrap();
    assert_eq!(sub, ["a", "b"]);
}

#[test]
fn line_endings() {
    #[derive(Deserialize, Debug, PartialEq)]
    struct S {
        a: i32,
        b: Vec<i32>,
    }
    for nl in ["\n", "\r\n", "\r"] {
        let input = format!("a: 1{nl}b:{nl}  - 2{nl}  - 3{nl}");
        assert_eq!(
            from_str::<S>(&input).unwrap(),
            S {
                a: 1,
                b: vec![2, 3]
            },
            "{nl:?}"
        );
    }
}

#[test]
fn borrows_unescaped_strings() {
    use std::borrow::Cow;
    #[derive(Deserialize)]
    struct Borrowed<'a> {
        name: &'a str,
        #[serde(borrow)]
        tags: Vec<&'a str>,
        #[serde(borrow)]
        scores: std::collections::BTreeMap<&'a str, i32>,
        #[serde(borrow)]
        note: Cow<'a, str>,
    }
    let input = String::from(
        "name: alice\ntags: [x, \"y z\"]\nscores: {math: 1, \"art\": 2}\nnote: \"a\\tb\"\n",
    );
    let b: Borrowed = ayml::from_str(&input).unwrap();
    let range = input.as_bytes().as_ptr_range();
    let borrowed = |s: &str| range.contains(&s.as_ptr());
    assert_eq!(b.name, "alice");
    assert!(borrowed(b.name));
    assert_eq!(b.tags, ["x", "y z"]);
    assert!(b.tags.iter().all(|t| borrowed(t)));
    assert!(b.scores.keys().all(|k| borrowed(k)));
    assert!(matches!(b.note, Cow::Owned(ref s) if s == "a\tb"));
}

#[test]
fn escaped_strings_need_owned_types() {
    assert!(ayml::from_str::<&str>("\"a\\nb\"").is_err());
    assert_eq!(ayml::from_str::<String>("\"a\\nb\"").unwrap(), "a\nb");
}
