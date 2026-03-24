use ayml::{
    commented::Commented,
    commented_value::{CommentedValue, CommentedValueKind},
};
use indexmap::IndexMap;

#[test]
fn de_commented_value_scalar_with_inline() {
    let input = "42 # the answer\n";
    let cv: CommentedValue = ayml::from_str(input).unwrap();
    assert_eq!(cv.inline_comment.as_deref(), Some("the answer"));
    assert!(matches!(cv.value, CommentedValueKind::Int(42)));
}

#[test]
fn de_commented_value_scalar_no_comment() {
    let input = "hello\n";
    let cv: CommentedValue = ayml::from_str(input).unwrap();
    assert_eq!(cv.top_comment, None);
    assert_eq!(cv.inline_comment, None);
    assert!(matches!(cv.value, CommentedValueKind::Str(ref s) if s == "hello"));
}

#[test]
fn de_commented_value_map_with_comments() {
    let input = "\
# server config
host: localhost
# port number
port: 8080 # default
";
    let cv: CommentedValue = ayml::from_str(input).unwrap();
    assert_eq!(cv.top_comment.as_deref(), Some("server config"));

    let map = match &cv.value {
        CommentedValueKind::Map(m) => m,
        other => panic!("expected map, got {other:?}"),
    };

    let host = &map["host"];
    assert!(matches!(host.value, CommentedValueKind::Str(ref s) if s == "localhost"));

    let port = &map["port"];
    assert!(matches!(port.value, CommentedValueKind::Int(8080)));
    assert_eq!(port.top_comment.as_deref(), Some("port number"));
    assert_eq!(port.inline_comment.as_deref(), Some("default"));
}

#[test]
fn roundtrip_commented_value_scalar() {
    let cv = CommentedValue {
        top_comment: None,
        inline_comment: Some("note".into()),
        value: CommentedValueKind::Str("test".into()),
    };
    let s = ayml::to_string(&cv).unwrap();
    assert_eq!(s, "test # note\n");
    let cv2: CommentedValue = ayml::from_str(&s).unwrap();
    assert_eq!(cv, cv2);
}

#[test]
fn roundtrip_commented_value_map() {
    let input = "\
host: localhost
port: 8080 # default
";
    let cv: CommentedValue = ayml::from_str(input).unwrap();
    let output = ayml::to_string(&cv).unwrap();
    // Re-parse and compare values (map order may differ)
    let cv2: CommentedValue = ayml::from_str(&output).unwrap();
    assert_eq!(cv, cv2);
}

#[test]
fn de_commented_value_seq() {
    let input = "\
- 1
- 2 # second
- 3
";
    let cv: CommentedValue = ayml::from_str(input).unwrap();
    let seq = match &cv.value {
        CommentedValueKind::Seq(s) => s,
        other => panic!("expected seq, got {other:?}"),
    };
    assert_eq!(seq.len(), 3);
    assert!(matches!(seq[0].value, CommentedValueKind::Int(1)));
    assert!(matches!(seq[1].value, CommentedValueKind::Int(2)));
    assert_eq!(seq[1].inline_comment.as_deref(), Some("second"));
    assert!(matches!(seq[2].value, CommentedValueKind::Int(3)));
}

#[test]
fn de_commented_value_null() {
    let input = "null\n";
    let cv: CommentedValue = ayml::from_str(input).unwrap();
    assert!(matches!(cv.value, CommentedValueKind::Null));
}

#[test]
fn de_commented_value_bool() {
    let input = "true # yes\n";
    let cv: CommentedValue = ayml::from_str(input).unwrap();
    assert!(matches!(cv.value, CommentedValueKind::Bool(true)));
    assert_eq!(cv.inline_comment.as_deref(), Some("yes"));
}

#[test]
fn de_commented_value_float() {
    let input = "3.25\n";
    let cv: CommentedValue = ayml::from_str(input).unwrap();
    assert!(matches!(cv.value, CommentedValueKind::Float(v) if v == 3.25));
}

#[test]
fn display_commented_value() {
    let cv = CommentedValue {
        top_comment: Some("a top comment".into()),
        inline_comment: None,
        value: CommentedValueKind::Int(42),
    };
    assert_eq!(format!("{cv}"), "# a top comment\n42");

    let cv = CommentedValue {
        top_comment: None,
        inline_comment: Some("an inline comment".into()),
        value: CommentedValueKind::Int(42),
    };
    assert_eq!(format!("{cv}"), "42 # an inline comment");

    let cv = CommentedValue {
        top_comment: Some("top".into()),
        inline_comment: Some("inline".into()),
        value: CommentedValueKind::Int(42),
    };
    assert_eq!(format!("{cv}"), "# top\n42 # inline");

    let cv = CommentedValue {
        top_comment: Some("line 1\nline 2".into()),
        inline_comment: None,
        value: CommentedValueKind::Int(42),
    };
    assert_eq!(format!("{cv}"), "# line 1\n# line 2\n42");

    let cv = CommentedValue {
        top_comment: None,
        inline_comment: None,
        value: CommentedValueKind::Int(42),
    };
    assert_eq!(format!("{cv}"), "42");
}

#[test]
fn ser_nested_map_in_map() {
    // Test that a map value that is itself a Commented<Map> serializes with correct indentation
    let mut inner_map = IndexMap::new();
    inner_map.insert(
        "L".to_string(),
        Commented::new(CommentedValueKind::Str("42".into())),
    );
    let mut outer_map = IndexMap::new();
    outer_map.insert(
        "a".to_string(),
        Commented::new(CommentedValueKind::Map(inner_map)),
    );
    let v = Commented::new(CommentedValueKind::Map(outer_map));
    let s = ayml::to_string(&v).unwrap();

    // Verify it roundtrips
    let d: CommentedValue = ayml::from_str(&s).unwrap();
    assert_eq!(v, d, "roundtrip failed\nserialized:\n{s}");
}

#[test]
fn ser_multi_key_nested_map() {
    let mut inner_map = IndexMap::new();
    inner_map.insert(
        "L".to_string(),
        Commented::new(CommentedValueKind::Str("42".into())),
    );
    let mut outer_map = IndexMap::new();
    outer_map.insert(
        "false".to_string(),
        Commented::new(CommentedValueKind::Int(-1)),
    );
    outer_map.insert("null".to_string(), Commented::new(CommentedValueKind::Null));
    outer_map.insert(
        "42".to_string(),
        Commented::new(CommentedValueKind::Map(inner_map)),
    );
    let v = Commented::new(CommentedValueKind::Map(outer_map));
    let s = ayml::to_string(&v).unwrap();

    let d: CommentedValue = ayml::from_str(&s).unwrap();
    assert_eq!(v, d, "roundtrip failed\nserialized:\n{s}");
}

#[test]
fn ser_proptest_repro() {
    // Exact reproduction of proptest failure
    let mut inner_map = IndexMap::new();
    inner_map.insert(
        "L".to_string(),
        CommentedValue {
            top_comment: Some("top comment line1\ntop comment line2".into()),
            inline_comment: None,
            value: CommentedValueKind::Str("42".into()),
        },
    );

    let mut outer_map = IndexMap::new();
    outer_map.insert(
        "false".to_string(),
        Commented::new(CommentedValueKind::Int(-1)),
    );
    outer_map.insert(
        "null".to_string(),
        CommentedValue {
            top_comment: Some("null comment".into()),
            inline_comment: None,
            value: CommentedValueKind::Null,
        },
    );
    outer_map.insert(
        "42".to_string(),
        Commented::new(CommentedValueKind::Map(inner_map)),
    );

    let v = Commented::new(CommentedValueKind::Map(outer_map));
    let s = ayml::to_string(&v).unwrap();

    // Check that "42"'s nested map is indented
    assert!(
        s.contains("  L:"),
        "L: should be at indent 2\nserialized:\n{s}"
    );
    let d: CommentedValue = ayml::from_str(&s).unwrap();
    assert_eq!(v, d, "roundtrip failed\nserialized:\n{s}");
}

#[test]
fn ser_commented_then_nested_map() {
    // Force order: commented value first, then nested map
    // Using a struct to guarantee field order
    #[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq)]
    struct Outer {
        first: Commented<CommentedValueKind>,
        second: Commented<CommentedValueKind>,
        third: Commented<CommentedValueKind>,
    }

    let mut inner_map = IndexMap::new();
    inner_map.insert(
        "L".to_string(),
        CommentedValue {
            top_comment: Some("inner comment".into()),
            inline_comment: None,
            value: CommentedValueKind::Str("val".into()),
        },
    );

    let o = Outer {
        first: CommentedValue {
            top_comment: Some("first comment".into()),
            inline_comment: None,
            value: CommentedValueKind::Null,
        },
        second: Commented::new(CommentedValueKind::Map(inner_map)),
        third: Commented::new(CommentedValueKind::Int(-1)),
    };
    let s = ayml::to_string(&o).unwrap();

    assert!(
        s.contains("  L:"),
        "L: should be at indent 2\nserialized:\n{s}"
    );
    let d: Outer = ayml::from_str(&s).unwrap();
    assert_eq!(o, d, "roundtrip failed\nserialized:\n{s}");
}

#[test]
fn ser_nested_map_with_top_comment() {
    let mut inner_map = IndexMap::new();
    inner_map.insert(
        "L".to_string(),
        CommentedValue {
            top_comment: Some("comment".into()),
            inline_comment: None,
            value: CommentedValueKind::Str("42".into()),
        },
    );
    let mut outer_map = IndexMap::new();
    outer_map.insert(
        "false".to_string(),
        Commented::new(CommentedValueKind::Int(-1)),
    );
    outer_map.insert(
        "42".to_string(),
        Commented::new(CommentedValueKind::Map(inner_map)),
    );
    let v = Commented::new(CommentedValueKind::Map(outer_map));
    let s = ayml::to_string(&v).unwrap();

    let d: CommentedValue = ayml::from_str(&s).unwrap();
    assert_eq!(v, d, "roundtrip failed\nserialized:\n{s}");
}
