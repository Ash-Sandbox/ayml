//! Fixture files in `examples/`.

mod common;

use std::path::Path;

use ayml_core::{Value, directive, emit, parse};
use common::{get, items, ok};

fn fixtures() -> Vec<(String, String)> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples");
    let mut out: Vec<(String, String)> = std::fs::read_dir(&dir)
        .expect("examples/ directory")
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|e| e == "ayml"))
        .map(|path| {
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            (name, std::fs::read_to_string(&path).unwrap())
        })
        .collect();
    out.sort();
    assert!(!out.is_empty(), "no fixtures found in {}", dir.display());
    out
}

#[test]
fn all_fixtures_parse_and_round_trip() {
    for (name, text) in fixtures() {
        let node = parse(&text).unwrap_or_else(|e| panic!("{name}: {e}"));
        let emitted = emit(&node);
        let reparsed = parse(&emitted).unwrap_or_else(|e| panic!("{name}: {e}\n{emitted}"));
        // Emitting may move comments as documented (for example, the inline
        // comment of a root flow sequence written in block style).
        let expected = ayml_testgen::tree::canonicalize(&node);
        assert_eq!(expected, reparsed, "{name}\n--- emitted ---\n{emitted}");
        assert_eq!(
            emit(&reparsed),
            emitted,
            "{name}: emitting is not idempotent"
        );
    }
}

fn fixture(name: &str) -> String {
    fixtures().into_iter().find(|(n, _)| n == name).unwrap().1
}

#[test]
fn policy_fixture() {
    let root = ok(&fixture("policy.ayml"));
    assert_eq!(
        directive::document_schema_uri(&root),
        Some("https://hub.ashell.dev/schemas/policy/v1.json")
    );
    assert_eq!(get(&root, "schema_version").value, Value::Int(1));
    let rules = items(get(get(&root, "network"), "rules"));
    assert_eq!(rules.len(), 3);
    assert_eq!(rules[0].comment.as_deref(), Some("Allow GitHub"));
    let host = get(&rules[0], "host");
    assert_eq!(host.value, Value::Str("hello\\\"#world\"".into()));
    assert_eq!(host.inline_comment.as_deref(), Some("primary domain"));
    assert_eq!(get(&rules[1], "precedence").value, Value::Int(-1));
    assert_eq!(get(&rules[2], "action").value, Value::Str("allow".into()));
}

#[test]
fn array_of_maps_fixture() {
    let root = ok(&fixture("array_of_maps.ayml"));
    let libs = items(&root);
    assert_eq!(libs[1].comment.as_deref(), Some("b library"));
    let rules = get(&libs[2], "rules");
    assert_eq!(
        rules.inline_comment.as_deref(),
        Some("what are these rules doing?")
    );
    assert_eq!(
        get(&items(rules)[0], "1").inline_comment.as_deref(),
        Some("this one is doing 4")
    );
    assert_eq!(
        get(&libs[2], "date").value,
        Value::Str("2026-0-3-10".into())
    );
}

#[test]
fn numbers_fixture() {
    let root = ok(&fixture("numbers.ayml"));
    let v: Vec<Value> = items(&root).iter().map(|n| n.value.clone()).collect();
    assert_eq!(v[0], Value::Int(100));
    assert_eq!(v[1], Value::Float(12.5));
    assert_eq!(v[3], Value::Float(1.3e9));
    assert_eq!(v[5], Value::Float(f64::NEG_INFINITY));
}

#[test]
fn trailing_comment_fixture() {
    let root = ok(&fixture("trailing-comment.ayml"));
    assert_eq!(root.comment.as_deref(), Some("Starting Comment"));
    assert_eq!(get(&root, "this").comment, None);
    assert_eq!(
        root.trailing_comment.as_deref(),
        Some("Trailing Comment\nWith multiple lines")
    );
}

#[test]
fn side_comment_fixture() {
    let root = ok(&fixture("side-comment.ayml"));
    assert_eq!(items(&root).len(), 4);
    assert_eq!(
        root.inline_comment.as_deref(),
        Some("this is a side comment")
    );
}
