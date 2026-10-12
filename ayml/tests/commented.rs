use ayml::Commented;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct Config {
    host: Commented<String>,
    port: Commented<u16>,
}

#[test]
fn reads_all_comment_kinds() {
    let input = "\
# the host
# (multi-line)
host: localhost # local only
port: 8080
";
    let c: Config = ayml::from_str(input).unwrap();
    assert_eq!(c.host.value, "localhost");
    assert_eq!(c.host.comment.as_deref(), Some("the host\n(multi-line)"));
    assert_eq!(c.host.inline_comment.as_deref(), Some("local only"));
    assert_eq!(c.port, Commented::new(8080));
}

#[test]
fn writes_comments_above_keys() {
    let c = Config {
        host: Commented::new("localhost".into()),
        port: Commented::new(8080)
            .with_comment("listen port")
            .with_inline_comment("default"),
    };
    assert_eq!(
        ayml::to_string(&c).unwrap(),
        "host: localhost\n# listen port\nport: 8080 # default\n"
    );
}

#[test]
fn first_field_comment_round_trips() {
    let input = "# the host\nhost: localhost\nport: 8080\n";
    let c: Config = ayml::from_str(input).unwrap();
    assert_eq!(c.host.comment.as_deref(), Some("the host"));
    assert_eq!(ayml::to_string(&c).unwrap(), input);
}

#[test]
fn blank_lines_between_comment_lines_are_kept() {
    let input = "host: x\n# first\n\n# second\nport: 1\n";
    let c: Config = ayml::from_str(input).unwrap();
    assert_eq!(c.port.comment.as_deref(), Some("first\n\nsecond"));
    assert_eq!(ayml::to_string(&c).unwrap(), input);
}

#[test]
fn collections_carry_inline_and_trailing_comments() {
    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct List {
        items: Commented<Vec<Commented<String>>>,
        empty: Commented<Vec<i32>>,
    }
    let input = "\
# the items
items: # inline after the key
  # first
  - a # a!
  - b
  # trailing, inside the list
empty: [
  # nothing yet
] # inline after the brackets
";
    let l: List = ayml::from_str(input).unwrap();
    assert_eq!(l.items.comment.as_deref(), Some("the items"));
    assert_eq!(
        l.items.inline_comment.as_deref(),
        Some("inline after the key")
    );
    assert_eq!(
        l.items.trailing_comment.as_deref(),
        Some("trailing, inside the list")
    );
    assert_eq!(l.items.value[0].comment.as_deref(), Some("first"));
    assert_eq!(l.items.value[0].inline_comment.as_deref(), Some("a!"));
    assert_eq!(l.items.value[1], Commented::new("b".to_string()));
    assert_eq!(l.empty.trailing_comment.as_deref(), Some("nothing yet"));
    assert_eq!(
        l.empty.inline_comment.as_deref(),
        Some("inline after the brackets")
    );
    assert_eq!(ayml::to_string(&l).unwrap(), input);
}

#[test]
fn sequence_entry_comments() {
    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct Rule {
        host: String,
        port: u16,
    }
    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct Rules {
        rules: Vec<Commented<Rule>>,
    }
    let input = "\
rules:
  # allow github
  - host: github.com
    port: 22
  # allow crates
  - host: crates.io
    port: 443
";
    let r: Rules = ayml::from_str(input).unwrap();
    assert_eq!(r.rules[0].comment.as_deref(), Some("allow github"));
    assert_eq!(r.rules[1].comment.as_deref(), Some("allow crates"));
    assert_eq!(ayml::to_string(&r).unwrap(), input);
}

#[test]
fn sequence_entry_comment_falls_back_to_first_field() {
    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct Rule {
        host: Commented<String>,
        port: u16,
    }
    let input = "# allow github\n- host: github.com\n  port: 22\n";
    let rules: Vec<Rule> = ayml::from_str(input).unwrap();
    assert_eq!(rules[0].host.comment.as_deref(), Some("allow github"));
    assert_eq!(ayml::to_string(&rules).unwrap(), input);

    let nested: Vec<Vec<Commented<i32>>> = ayml::from_str("# c\n- - 1\n  - 2\n").unwrap();
    assert_eq!(nested[0][0].comment.as_deref(), Some("c"));
    assert_eq!(nested[0][1].comment, None);
}

#[test]
fn claimed_comments_are_not_inherited() {
    #[derive(Deserialize, Debug, PartialEq)]
    struct Rule {
        host: Commented<String>,
    }
    let rules: Vec<Commented<Rule>> = ayml::from_str("# c\n- host: x\n").unwrap();
    assert_eq!(rules[0].comment.as_deref(), Some("c"));
    assert_eq!(rules[0].value.host.comment, None);
}

#[test]
fn optional_commented_fields() {
    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct S {
        a: Option<Commented<i32>>,
        b: Option<Commented<i32>>,
    }
    let input = "a: null\n# about b\nb: 2 # two\n";
    let s: S = ayml::from_str(input).unwrap();
    assert_eq!(s.a, None);
    assert_eq!(
        s.b,
        Some(
            Commented::new(2)
                .with_comment("about b")
                .with_inline_comment("two")
        )
    );
    assert_eq!(ayml::to_string(&s).unwrap(), input);
}

#[test]
fn root_commented_scalar() {
    let c: Commented<i32> = ayml::from_str("# top\n42 # side\n# after\n").unwrap();
    assert_eq!(c.comment.as_deref(), Some("top"));
    assert_eq!(c.inline_comment.as_deref(), Some("side"));
    assert_eq!(c.trailing_comment.as_deref(), Some("after"));
    assert_eq!(ayml::to_string(&c).unwrap(), "# top\n42 # side\n# after\n");
}

#[test]
fn comments_are_ignored_without_commented() {
    #[derive(Deserialize, Debug, PartialEq)]
    struct Plain {
        host: String,
    }
    let p: Plain = ayml::from_str("# c\nhost: x # i\n# t\n").unwrap();
    assert_eq!(p, Plain { host: "x".into() });
}

#[test]
fn other_formats_see_only_the_value() {
    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct Config {
        port: Commented<u16>,
        tags: Commented<Vec<String>>,
    }
    let config = Config {
        port: Commented::new(8080).with_comment("listen port"),
        tags: Commented::new(vec!["a".into()]).with_trailing_comment("more later"),
    };
    let json = serde_json::to_string(&config).unwrap();
    assert_eq!(json, r#"{"port":8080,"tags":["a"]}"#);
    let back: Config = serde_json::from_str(&json).unwrap();
    assert_eq!(back.port, Commented::new(8080));
    assert_eq!(back.tags, Commented::new(vec!["a".to_owned()]));
}

#[test]
fn commented_inside_buffered_types_keeps_the_value() {
    #[derive(Deserialize, Debug, PartialEq)]
    #[serde(untagged)]
    enum Untagged {
        Port { port: Commented<u16> },
        Name(String),
    }
    #[derive(Deserialize, Debug, PartialEq)]
    #[serde(tag = "kind")]
    enum Tagged {
        Server {
            port: Commented<u16>,
            tags: Commented<Vec<String>>,
        },
    }
    #[derive(Deserialize, Debug, PartialEq)]
    struct Inner {
        port: Commented<u16>,
    }
    #[derive(Deserialize, Debug, PartialEq)]
    struct Outer {
        name: String,
        #[serde(flatten)]
        inner: Inner,
    }

    let u: Untagged = ayml::from_str("# c\nport: 8080 # i\n").unwrap();
    assert_eq!(
        u,
        Untagged::Port {
            port: Commented::new(8080)
        }
    );
    let u: Untagged = ayml::from_str("plain\n").unwrap();
    assert_eq!(u, Untagged::Name("plain".into()));

    let t: Tagged = ayml::from_str("kind: Server\n# c\nport: 8080 # i\ntags: [a]\n").unwrap();
    assert_eq!(
        t,
        Tagged::Server {
            port: Commented::new(8080),
            tags: Commented::new(vec!["a".into()])
        }
    );

    let o: Outer = ayml::from_str("name: x\n# c\nport: 8080 # i\n").unwrap();
    assert_eq!(o.inner.port, Commented::new(8080));
}

#[test]
fn commented_inside_buffered_types_serializes_the_value() {
    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct Flat {
        z: i32,
        #[serde(flatten)]
        extra: Commented<std::collections::BTreeMap<String, i32>>,
    }
    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct S {
        a: i32,
    }
    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    #[serde(tag = "t")]
    enum Tagged {
        V(Commented<S>),
    }

    let flat = Flat {
        z: 1,
        extra: Commented::new([("k".to_owned(), 2)].into()).with_comment("dropped"),
    };
    let text = ayml::to_string(&flat).unwrap();
    assert_eq!(text, "z: 1\nk: 2\n");
    let back: Flat = ayml::from_str(&text).unwrap();
    assert_eq!(back.extra.value, flat.extra.value);

    let tagged = Tagged::V(Commented::new(S { a: 1 }).with_comment("dropped"));
    let text = ayml::to_string(&tagged).unwrap();
    assert_eq!(text, "t: V\na: 1\n");
    let back: Tagged = ayml::from_str(&text).unwrap();
    assert_eq!(back, Tagged::V(Commented::new(S { a: 1 })));
}

#[test]
fn root_comment_goes_to_root_commented_or_first_field() {
    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct Root {
        a: Commented<i32>,
        b: i32,
    }
    let commented_root = Commented::new(Root {
        a: Commented::new(1),
        b: 2,
    })
    .with_comment("doc");
    let text = ayml::to_string(&commented_root).unwrap();
    assert_eq!(text, "# doc\na: 1\nb: 2\n");
    assert_eq!(
        ayml::from_str::<Commented<Root>>(&text).unwrap(),
        commented_root
    );

    let root: Root = ayml::from_str(&text).unwrap();
    assert_eq!(root.a, Commented::new(1).with_comment("doc"));
    assert_eq!(ayml::to_string(&root).unwrap(), text);

    let list = Commented::new(vec![1, 2]).with_comment("numbers");
    let text = ayml::to_string(&list).unwrap();
    assert_eq!(ayml::from_str::<Commented<Vec<i32>>>(&text).unwrap(), list);
}

#[test]
fn value_deserializes_typed_keys() {
    use serde::Deserialize as _;
    let value: ayml::Value = ayml::from_str("1: a\n2: b\n").unwrap();
    let map = std::collections::BTreeMap::<u32, String>::deserialize(value).unwrap();
    assert_eq!(map, [(1, "a".to_owned()), (2, "b".to_owned())].into());
    let value: ayml::Value = ayml::from_str("true: 1\n").unwrap();
    let map = std::collections::BTreeMap::<bool, i32>::deserialize(value).unwrap();
    assert_eq!(map, [(true, 1)].into());
}
