use ayml::commented::Commented;
use serde::{Deserialize, Serialize};

// ── Deserialization tests ───────────────────────────────────────────

#[test]
fn de_commented_inline_comment() {
    #[derive(Deserialize, Debug)]
    struct Config {
        port: Commented<u16>,
    }
    let input = "port: 8080 # the port\n";
    let c: Config = ayml::from_str(input).unwrap();
    assert_eq!(c.port.value, 8080);
    assert_eq!(c.port.top_comment, None);
    assert_eq!(c.port.inline_comment.as_deref(), Some("the port"));
}

#[test]
fn de_commented_top_comment() {
    #[derive(Deserialize, Debug)]
    struct Config {
        port: Commented<u16>,
    }
    let input = "# listen port\nport: 8080\n";
    let c: Config = ayml::from_str(input).unwrap();
    assert_eq!(c.port.value, 8080);
    assert_eq!(c.port.top_comment.as_deref(), Some("listen port"));
    assert_eq!(c.port.inline_comment, None);
}

#[test]
fn de_commented_both_comments() {
    #[derive(Deserialize, Debug)]
    struct Config {
        port: Commented<u16>,
    }
    let input = "# listen port\nport: 8080 # default\n";
    let c: Config = ayml::from_str(input).unwrap();
    assert_eq!(c.port.value, 8080);
    assert_eq!(c.port.top_comment.as_deref(), Some("listen port"));
    assert_eq!(c.port.inline_comment.as_deref(), Some("default"));
}

#[test]
fn de_commented_no_comments() {
    #[derive(Deserialize, Debug)]
    struct Config {
        port: Commented<u16>,
    }
    let input = "port: 8080\n";
    let c: Config = ayml::from_str(input).unwrap();
    assert_eq!(c.port.value, 8080);
    assert_eq!(c.port.top_comment, None);
    assert_eq!(c.port.inline_comment, None);
}

#[test]
fn de_commented_multiline_top_comment() {
    #[derive(Deserialize, Debug)]
    struct Config {
        port: Commented<u16>,
    }
    let input = "# line one\n# line two\nport: 8080\n";
    let c: Config = ayml::from_str(input).unwrap();
    assert_eq!(c.port.value, 8080);
    assert_eq!(c.port.top_comment.as_deref(), Some("line one\nline two"));
}

#[test]
fn de_commented_string_value() {
    #[derive(Deserialize, Debug)]
    struct Config {
        name: Commented<String>,
    }
    let input = "# the name\nname: hello # greeting\n";
    let c: Config = ayml::from_str(input).unwrap();
    assert_eq!(c.name.value, "hello");
    assert_eq!(c.name.top_comment.as_deref(), Some("the name"));
    assert_eq!(c.name.inline_comment.as_deref(), Some("greeting"));
}

#[test]
fn de_commented_bool_value() {
    #[derive(Deserialize, Debug)]
    struct Config {
        debug: Commented<bool>,
    }
    let input = "debug: true # enable debug\n";
    let c: Config = ayml::from_str(input).unwrap();
    assert!(c.debug.value);
    assert_eq!(c.debug.inline_comment.as_deref(), Some("enable debug"));
}

#[test]
fn de_commented_multiple_fields() {
    #[derive(Deserialize, Debug)]
    struct Config {
        host: Commented<String>,
        port: Commented<u16>,
    }
    let input = "# hostname\nhost: localhost # server\n# port number\nport: 3000\n";
    let c: Config = ayml::from_str(input).unwrap();
    assert_eq!(c.host.value, "localhost");
    assert_eq!(c.host.top_comment.as_deref(), Some("hostname"));
    assert_eq!(c.host.inline_comment.as_deref(), Some("server"));
    assert_eq!(c.port.value, 3000);
    assert_eq!(c.port.top_comment.as_deref(), Some("port number"));
    assert_eq!(c.port.inline_comment, None);
}

#[test]
fn de_commented_mixed_with_plain() {
    #[derive(Deserialize, Debug)]
    struct Config {
        host: String,
        port: Commented<u16>,
        debug: bool,
    }
    let input = "host: localhost\n# the port\nport: 8080 # default\ndebug: false\n";
    let c: Config = ayml::from_str(input).unwrap();
    assert_eq!(c.host, "localhost");
    assert_eq!(c.port.value, 8080);
    assert_eq!(c.port.top_comment.as_deref(), Some("the port"));
    assert_eq!(c.port.inline_comment.as_deref(), Some("default"));
    assert!(!c.debug);
}

// ── Serialization tests ─────────────────────────────────────────────

#[test]
fn ser_commented_inline_comment() {
    #[derive(Serialize)]
    struct Config {
        port: Commented<u16>,
    }
    let c = Config {
        port: Commented {
            top_comment: None,
            inline_comment: Some("the port".into()),
            value: 8080,
        },
    };
    let s = ayml::to_string(&c).unwrap();
    assert_eq!(s, "port: 8080 # the port\n");
}

#[test]
fn ser_commented_top_comment() {
    #[derive(Serialize)]
    struct Config {
        port: Commented<u16>,
    }
    let c = Config {
        port: Commented {
            top_comment: Some("listen port".into()),
            inline_comment: None,
            value: 8080,
        },
    };
    let s = ayml::to_string(&c).unwrap();
    assert_eq!(s, "port:\n  # listen port\n  8080\n");
}

#[test]
fn ser_commented_both_comments() {
    #[derive(Serialize)]
    struct Config {
        port: Commented<u16>,
    }
    let c = Config {
        port: Commented {
            top_comment: Some("listen port".into()),
            inline_comment: Some("default".into()),
            value: 8080,
        },
    };
    let s = ayml::to_string(&c).unwrap();
    assert_eq!(s, "port:\n  # listen port\n  8080 # default\n");
}

#[test]
fn ser_commented_no_comments() {
    #[derive(Serialize)]
    struct Config {
        port: Commented<u16>,
    }
    let c = Config {
        port: Commented {
            top_comment: None,
            inline_comment: None,
            value: 8080,
        },
    };
    let s = ayml::to_string(&c).unwrap();
    assert_eq!(s, "port: 8080\n");
}

#[test]
fn ser_commented_multiline_top_comment() {
    #[derive(Serialize)]
    struct Config {
        port: Commented<u16>,
    }
    let c = Config {
        port: Commented {
            top_comment: Some("line one\nline two".into()),
            inline_comment: None,
            value: 8080,
        },
    };
    let s = ayml::to_string(&c).unwrap();
    assert_eq!(s, "port:\n  # line one\n  # line two\n  8080\n");
}

// ── Roundtrip tests ─────────────────────────────────────────────────

#[test]
fn roundtrip_commented_inline() {
    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct Config {
        port: Commented<u16>,
    }
    let input = "port: 8080 # the port\n";
    let c: Config = ayml::from_str(input).unwrap();
    let output = ayml::to_string(&c).unwrap();
    assert_eq!(output, input);
}

#[test]
fn roundtrip_commented_no_comments() {
    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct Config {
        port: Commented<u16>,
    }
    let input = "port: 8080\n";
    let c: Config = ayml::from_str(input).unwrap();
    let output = ayml::to_string(&c).unwrap();
    assert_eq!(output, input);
}

#[test]
fn roundtrip_commented_both_comments() {
    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct Config {
        port: Commented<u16>,
    }
    let input = "port:\n  # listen port\n  8080 # default\n";
    let c: Config = ayml::from_str(input).unwrap();
    assert_eq!(c.port.value, 8080);
    assert_eq!(c.port.top_comment.as_deref(), Some("listen port"));
    assert_eq!(c.port.inline_comment.as_deref(), Some("default"));
    let output = ayml::to_string(&c).unwrap();
    assert_eq!(output, input);
}

#[test]
fn roundtrip_commented_multiple_fields() {
    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct Config {
        host: Commented<String>,
        port: Commented<u16>,
    }
    let c = Config {
        host: Commented {
            top_comment: Some("hostname".into()),
            inline_comment: None,
            value: "localhost".into(),
        },
        port: Commented {
            top_comment: None,
            inline_comment: Some("default".into()),
            value: 3000,
        },
    };
    let s = ayml::to_string(&c).unwrap();
    let c2: Config = ayml::from_str(&s).unwrap();
    assert_eq!(c, c2);
}

#[test]
fn de_commented_seq_elements() {
    #[derive(Deserialize, Debug)]
    struct List {
        items: Vec<Commented<String>>,
    }
    let mut input = String::new();
    input.push_str("items:\n");
    input.push_str("# first\n");
    input.push_str("- a\n");
    input.push_str("# second\n");
    input.push_str("- b\n");
    let l: List = ayml::from_str(&input).unwrap();
    assert_eq!(l.items.len(), 2);
    assert_eq!(l.items[0].value, "a");
    assert_eq!(l.items[0].top_comment.as_deref(), Some("first"));
    assert_eq!(l.items[1].value, "b");
    assert_eq!(l.items[1].top_comment.as_deref(), Some("second"));
}

#[test]
fn ser_commented_seq_as_map_value() {
    #[derive(Serialize, Debug)]
    struct Inner {
        a: Commented<Vec<String>>,
    }
    #[derive(Serialize, Debug)]
    struct Outer {
        x: Inner,
    }
    let o = Outer {
        x: Inner {
            a: Commented {
                top_comment: Some("comment".into()),
                inline_comment: None,
                value: vec!["hello".into()],
            },
        },
    };
    let s = ayml::to_string(&o).unwrap();
    // The comment and "- hello" should both be at indent 4 (under x: / a:)
    assert!(s.contains("    # comment"), "comment at wrong indent:\n{s}");
    assert!(s.contains("    - hello"), "dash at wrong indent:\n{s}");
}

#[test]
fn ser_commented_seq_as_toplevel_map_value() {
    use ayml::{CommentedValue, CommentedValueKind};
    use indexmap::IndexMap;
    let mut m = IndexMap::new();
    m.insert(
        "A".to_string(),
        CommentedValue {
            top_comment: Some("a".into()),
            inline_comment: None,
            value: CommentedValueKind::Seq(vec![Commented::new(CommentedValueKind::Null)]),
        },
    );
    let v = Commented::new(CommentedValueKind::Map(m));
    let s = ayml::to_string(&v).unwrap();
    assert!(s.contains("  # a"), "comment at wrong indent:\n{s}");
}

#[test]
fn ser_commented_value_seq_in_map() {
    use ayml::{CommentedValue, CommentedValueKind};
    use indexmap::IndexMap;
    let mut inner = IndexMap::new();
    inner.insert(
        "a".to_string(),
        CommentedValue {
            top_comment: Some("?".into()),
            inline_comment: None,
            value: CommentedValueKind::Seq(vec![Commented::new(CommentedValueKind::Null)]),
        },
    );
    let mut outer = IndexMap::new();
    outer.insert(
        "_".to_string(),
        Commented::new(CommentedValueKind::Map(inner)),
    );
    let v = Commented::new(CommentedValueKind::Map(outer));
    let s = ayml::to_string(&v).unwrap();
    assert!(s.contains("    # ?"), "comment at wrong indent:\n{s}");
    assert!(
        s.contains("    - null") || s.contains("  - null"),
        "unexpected output:\n{s}"
    );
}
