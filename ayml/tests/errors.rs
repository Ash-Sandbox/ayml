use std::collections::HashMap;

use ayml::{Error, from_slice, from_str, to_string};
use serde::{Deserialize, Serialize};

fn location<T: for<'de> Deserialize<'de> + std::fmt::Debug>(input: &str) -> (usize, usize, String) {
    let err = from_str::<T>(input).unwrap_err();
    let loc = err
        .location()
        .unwrap_or_else(|| panic!("no location: {err}"));
    (loc.line, loc.column, err.to_string())
}

// ── Syntax errors ───────────────────────────────────────────────────

#[test]
fn syntax_errors() {
    for input in [
        "",
        "# only a comment\n",
        "42 extra\nmore",
        "\"hello",
        "\"hello\nworld\"",
        r#""\q""#,
        r#""\xGG""#,
        r#""\uD800""#,
        "[1, 2",
        "{a: 1",
        "a: 1\na: 2",
        "password: abc#123",
        "key: 'single'",
        "9999999999999999999",
        "1e400",
        "a:\n\tb: 1",
        "\u{FEFF}a: 1",
    ] {
        let err = from_str::<ayml::Value>(input).unwrap_err();
        assert!(matches!(err, Error::Syntax(_)), "{input:?}: {err}");
        assert!(err.location().is_some(), "{input:?}");
    }
}

#[test]
fn syntax_error_inside_mistyped_value_wins() {
    #[derive(Deserialize, Debug)]
    struct S {
        #[allow(dead_code)]
        a: u8,
    }
    let err = from_str::<S>("a: [1, {b: 2, b: 3}]\n").unwrap_err();
    assert_eq!(err.to_string(), "1:15: duplicate mapping key `b`");
}

#[test]
fn syntax_error_location() {
    let err = from_str::<HashMap<String, String>>("a: 1\nb: x#y\n").unwrap_err();
    let loc = err.location().unwrap();
    assert_eq!((loc.line, loc.column), (2, 5), "{err}");
    assert!(err.to_string().starts_with("2:5: "), "{err}");
}

#[test]
fn invalid_utf8() {
    assert!(matches!(
        from_slice::<String>(&[0xFF, 0xFE]),
        Err(Error::Utf8(_))
    ));
    assert!(matches!(
        ayml::from_reader::<_, String>(&[b'a', 0xFF][..]),
        Err(Error::Utf8(_))
    ));
}

#[test]
fn scalars_must_match_their_type() {
    assert!(from_str::<bool>("42").is_err());
    assert!(from_str::<bool>("yes").is_err());
    assert!(from_str::<i32>("hello").is_err());
    assert!(from_str::<i32>("3.14").is_err());
    assert!(from_str::<i32>("true").is_err());
    assert!(from_str::<u16>("\"8080\"").is_err());
    assert!(from_str::<f64>("\"1.5\"").is_err());
    assert!(from_str::<String>("1.0").is_err());
    assert!(from_str::<String>("42").is_err());
    assert!(from_str::<String>("null").is_err());
    assert!(from_str::<String>("true").is_err());
    assert!(from_str::<()>("0").is_err());
    assert!(from_str::<Vec<i32>>("42").is_err());
    assert!(from_str::<HashMap<String, i32>>("[1]").is_err());
}

#[test]
fn integer_ranges() {
    assert!(from_str::<i8>("128").is_err());
    assert!(from_str::<i8>("-129").is_err());
    assert!(from_str::<u8>("256").is_err());
    assert!(from_str::<u8>("-1").is_err());
    assert!(from_str::<u64>("-1").is_err());
}

#[test]
fn chars_need_exactly_one_character() {
    assert!(from_str::<char>("ab").is_err());
    assert!(from_str::<char>(r#""""#).is_err());
}

#[test]
fn tuple_length_must_match() {
    assert!(from_str::<(i32, i32)>("[1, 2, 3]").is_err());
    assert!(from_str::<(i32, i32)>("[1]").is_err());
}

#[test]
fn invalid_typed_keys() {
    assert!(from_str::<HashMap<u32, i32>>("abc: 1").is_err());
    assert!(from_str::<HashMap<u8, i32>>("300: 1").is_err());
    assert!(from_str::<HashMap<bool, i32>>("yes: 1").is_err());
}

#[test]
fn enum_errors() {
    #[derive(Deserialize, Debug)]
    #[allow(dead_code)]
    enum E {
        A,
        B(i32),
    }
    assert!(from_str::<E>("C").is_err());
    assert!(from_str::<E>("{A: 1, B: 2}").is_err());
    assert!(from_str::<E>("A: 1").is_err());
    assert!(from_str::<E>("B").is_err());
    assert!(from_str::<E>("42").is_err());
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
struct Server {
    host: String,
    port: u16,
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
struct Config {
    servers: Vec<Server>,
}

#[test]
fn type_error_points_at_value() {
    let (line, col, msg) =
        location::<Config>("servers:\n  - host: a\n    port: 80\n  - host: b\n    port: high\n");
    assert_eq!((line, col), (5, 11), "{msg}");
    assert!(msg.contains("invalid type: string \"high\""), "{msg}");
}

#[test]
fn range_error_points_at_value() {
    let (line, col, msg) = location::<Server>("host: a\nport: 70000\n");
    assert_eq!((line, col), (2, 7), "{msg}");
}

#[test]
fn unknown_field_points_at_key() {
    let (line, col, msg) = location::<Server>("host: a\nport: 1\nprot: 2\n");
    assert_eq!((line, col), (3, 1), "{msg}");
    assert!(msg.contains("unknown field `prot`"), "{msg}");
}

#[test]
fn missing_field_points_at_mapping() {
    let (line, col, msg) = location::<Config>("# c\nservers:\n  - host: a\n");
    assert_eq!((line, col), (3, 5), "{msg}");
    assert!(msg.contains("missing field `port`"), "{msg}");
}

#[test]
fn from_node_errors_have_spans_but_no_location() {
    let node = ayml_core::parse("port: x").unwrap();
    let err = ayml::from_node::<Server>(&node).unwrap_err();
    let Error::Data { span, location, .. } = &err else {
        panic!("{err}")
    };
    assert_eq!(*span, Some(ayml_core::Span::new(6, 7)));
    assert!(location.is_none());
}

#[test]
fn unrepresentable_values() {
    assert!(to_string(&u64::MAX).is_err());
    assert!(to_string(&(i128::from(i64::MAX) + 1)).is_err());
    let mut m: HashMap<Option<i32>, i32> = HashMap::new();
    m.insert(None, 1);
    assert!(to_string(&m).is_err());
    let mut m = HashMap::new();
    m.insert(vec![1], 1);
    assert!(to_string(&m).is_err());
}

#[test]
fn duplicate_keys_after_key_conversion() {
    struct Dup;
    impl Serialize for Dup {
        fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
            use serde::ser::SerializeMap;
            let mut m = s.serialize_map(Some(2))?;
            m.serialize_entry(&1, &"a")?;
            m.serialize_entry("1", &"b")?;
            m.end()
        }
    }
    let err = to_string(&Dup).unwrap_err();
    assert!(err.to_string().contains("duplicate"), "{err}");
}
