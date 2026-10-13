//! Block and flow collections, keys, and document structure.

mod common;

use ayml_core::{ErrorKind, Value, parse};
use common::{assert_syntax, err, get, items, keys, ok, s};

#[test]
fn sequence_of_scalars() {
    let node = ok("- Mark McGwire\n- Sammy Sosa\n- Ken Griffey\n");
    let v: Vec<_> = items(&node).iter().map(|n| n.value.clone()).collect();
    assert_eq!(
        v,
        vec![s("Mark McGwire"), s("Sammy Sosa"), s("Ken Griffey")]
    );
}

#[test]
fn sequence_of_mixed_scalars() {
    let node = ok("- 1\n- 2.5\n- true\n- null\n- text\n");
    let v: Vec<_> = items(&node).iter().map(|n| n.value.clone()).collect();
    assert_eq!(
        v,
        vec![
            Value::Int(1),
            Value::Float(2.5),
            Value::Bool(true),
            Value::Null,
            s("text")
        ]
    );
}

#[test]
fn extra_spaces_after_dash() {
    let node = ok("-   a\n-  b\n");
    let v: Vec<_> = items(&node).iter().map(|n| n.value.clone()).collect();
    assert_eq!(v, vec![s("a"), s("b")]);
}

#[test]
fn dash_must_be_followed_by_a_space() {
    assert_syntax("-\n- a", "sequence entry has no value");
    assert_syntax("- a\n-", "sequence entry has no value");
    assert_syntax("-  # comment\n", "sequence entry has no value");
    assert_eq!(err("-\ta").kind, ErrorKind::TabIndent);
    assert_eq!(err("- \ta").kind, ErrorKind::TabIndent);
}

#[test]
fn compact_sequence() {
    let node = ok("- - a\n  - b\n- - c\n");
    let outer = items(&node);
    assert_eq!(outer.len(), 2);
    let first: Vec<_> = items(&outer[0]).iter().map(|n| n.value.clone()).collect();
    assert_eq!(first, vec![s("a"), s("b")]);
    assert_eq!(items(&outer[1])[0].value, s("c"));
}

#[test]
fn deeply_compact_sequence() {
    let node = ok("- - - x\n    - y\n");
    let level2 = &items(&items(&node)[0])[0];
    let v: Vec<_> = items(level2).iter().map(|n| n.value.clone()).collect();
    assert_eq!(v, vec![s("x"), s("y")]);
}

#[test]
fn compact_mapping() {
    let node = ok("- name: Mark\n  hr: 65\n- name: Sammy\n  hr: 63\n");
    let entries = items(&node);
    assert_eq!(keys(&entries[0]), vec!["name", "hr"]);
    assert_eq!(get(&entries[1], "hr").value, Value::Int(63));
}

#[test]
fn compact_mapping_aligned_by_extra_dash_spaces() {
    let node = ok("-   name: aligned\n    value: 1\n-   other: 2\n");
    let entries = items(&node);
    assert_eq!(keys(&entries[0]), vec!["name", "value"]);
    assert_eq!(keys(&entries[1]), vec!["other"]);
}

#[test]
fn compact_mapping_with_nested_block_values() {
    let node = ok("- a:\n    - x\n  b:\n    c: 1\n");
    let entry = &items(&node)[0];
    assert_eq!(items(get(entry, "a"))[0].value, s("x"));
    assert_eq!(get(get(entry, "b"), "c").value, Value::Int(1));
}

#[test]
fn compact_mapping_with_implicit_sequence() {
    let node = ok("- a:\n  - x\n  b: 1\n");
    let entry = &items(&node)[0];
    assert_eq!(items(get(entry, "a"))[0].value, s("x"));
    assert_eq!(get(entry, "b").value, Value::Int(1));
}

#[test]
fn compact_mapping_misaligned_continuation_is_an_error() {
    assert_syntax("- a: 1\n    b: 2", "unexpected indentation");
    assert!(parse("- a: 1\n b: 2").is_err());
}

#[test]
fn compact_sequence_followed_by_mapping_line_is_an_error() {
    assert_syntax("- - a\n  b: 1", "expected a sequence entry");
}

#[test]
fn block_sequence_cannot_start_after_key() {
    assert_syntax("k: - a", "must start on the line after its key");
}

#[test]
fn root_sequence_followed_by_mapping_is_an_error() {
    assert_syntax("- a\nb: 1", "expected a sequence entry");
}

// ── Block mappings ──────────────────────────────────────────────────

#[test]
fn mapping_scalars() {
    let node = ok("hr:  65\navg: 0.278\nrbi: 147\n");
    assert_eq!(keys(&node), vec!["hr", "avg", "rbi"]);
    assert_eq!(get(&node, "avg").value, Value::Float(0.278));
}

#[test]
fn mapping_preserves_order() {
    let node = ok("z: 1\na: 2\nm: 3\n");
    assert_eq!(keys(&node), vec!["z", "a", "m"]);
}

#[test]
fn spaces_before_colon() {
    let node = ok("city    : Royal Oak\nstate\t: MI\n");
    assert_eq!(get(&node, "city").value, s("Royal Oak"));
    assert_eq!(get(&node, "state").value, s("MI"));
}

#[test]
fn tab_after_colon() {
    assert_eq!(get(&ok("a:\t1"), "a").value, Value::Int(1));
}

#[test]
fn keys_with_spaces_and_punctuation() {
    let node = ok("key two: 1\nbill-to: 2\na.b/c: 3\nx[0]: 4\n-a: 5\n");
    assert_eq!(
        keys(&node),
        vec!["key two", "bill-to", "a.b/c", "x[0]", "-a"]
    );
}

#[test]
fn nested_mappings() {
    let node = ok("a:\n  b:\n    c: 1\n  d: 2\ne: 3\n");
    assert_eq!(get(get(get(&node, "a"), "b"), "c").value, Value::Int(1));
    assert_eq!(get(get(&node, "a"), "d").value, Value::Int(2));
    assert_eq!(get(&node, "e").value, Value::Int(3));
}

#[test]
fn nested_mapping_indentation_is_detected() {
    let node = ok("a:\n     b: 1\n     c: 2\n");
    assert_eq!(keys(get(&node, "a")), vec!["b", "c"]);
}

#[test]
fn mapping_of_sequences() {
    let node = ok("american:\n  - Boston\n  - Detroit\nnational:\n  - Mets\n");
    assert_eq!(items(get(&node, "american")).len(), 2);
    assert_eq!(items(get(&node, "national"))[0].value, s("Mets"));
}

#[test]
fn implicit_sequence_indentation() {
    let node = ok("items:\n- alpha\n- beta\nnext: 1\n");
    assert_eq!(items(get(&node, "items")).len(), 2);
    assert_eq!(get(&node, "next").value, Value::Int(1));
}

#[test]
fn blank_lines_between_entries_and_before_nested_value() {
    let node = ok("\n\na: 1\n\n\nb:\n\n  c: 2\n\n");
    assert_eq!(get(get(&node, "b"), "c").value, Value::Int(2));
}

#[test]
fn inconsistent_indentation_is_an_error() {
    assert_syntax("a:\n    b: 1\n  c: 2", "unexpected indentation");
    assert!(parse("a: 1\n b: 2").is_err());
}

#[test]
fn next_line_scalar_values_are_rejected() {
    assert_syntax("a:\n  b", "must be a block mapping or sequence");
    assert_syntax("a:\n  [1, 2]", "must be a block mapping or sequence");
    assert_syntax("a:\n  \"x\"", "must be a block mapping or sequence");
}

#[test]
fn missing_values_are_rejected() {
    assert_syntax("a:\nb: 1", "missing value for key `a`");
    assert_syntax("a:", "missing value for key `a`");
    assert_syntax("a:  # comment\n", "missing value for key `a`");
    assert_syntax("a:\n  # only a comment\n", "missing value for key `a`");
}

#[test]
fn mapping_in_mapping_value_on_same_line_is_an_error() {
    assert_syntax("a: b: c", "unexpected `:`");
}

#[test]
fn mapping_line_that_is_not_an_entry_is_an_error() {
    assert_syntax("a: 1\nb\n", "expected a mapping entry");
    assert_syntax("a: 1\n- b\n", "found a sequence entry");
}

#[test]
fn value_continuation_line_is_an_error() {
    assert_syntax("a: b\n  c", "unexpected indentation");
}

#[test]
fn keys_are_never_resolved() {
    let node = ok("true: a\n1: b\nnull: c\n1.5: d\n0x10: e\ninf: f\n");
    assert_eq!(keys(&node), vec!["true", "1", "null", "1.5", "0x10", "inf"]);
}

#[test]
fn quoted_keys() {
    let node = ok("\"key three\": 1\n\"with: colon\": 2\n\"\": 3\n\"a\\nb\": 4\n");
    assert_eq!(keys(&node), vec!["key three", "with: colon", "", "a\nb"]);
}

#[test]
fn duplicate_keys_are_rejected() {
    assert_eq!(err("a: 1\na: 2").kind, ErrorKind::DuplicateKey("a".into()));
    assert_eq!(
        err("a: 1\n\"a\": 2").kind,
        ErrorKind::DuplicateKey("a".into())
    );
    assert_eq!(
        err("\"\\x61\": 1\na: 2").kind,
        ErrorKind::DuplicateKey("a".into())
    );
    assert_eq!(
        err("{a: 1, \"a\": 2}").kind,
        ErrorKind::DuplicateKey("a".into())
    );
    assert_eq!(
        err("- x: 1\n  x: 2").kind,
        ErrorKind::DuplicateKey("x".into())
    );
}

#[test]
fn keys_that_look_alike_are_distinct() {
    let node = ok("1: a\n0x1: b\n01: c\n");
    assert_eq!(keys(&node), vec!["1", "0x1", "01"]);
}

#[test]
fn triple_quoted_strings_cannot_be_keys() {
    assert!(parse("\"\"\"\n  x\n  \"\"\": 1").is_err());
    assert_syntax("{\"\"\"\n  x\n  \"\"\": 1}", "cannot be mapping keys");
}

#[test]
fn key_followed_by_colon_without_space_is_not_a_key() {
    assert_eq!(keys(&ok("a:b: 1")), vec!["a:b"]);
    assert_eq!(ok("a:b").value, s("a:b"));
}

#[test]
fn flow_sequences() {
    let node = ok("[name, hr, avg]");
    let v: Vec<_> = items(&node).iter().map(|n| n.value.clone()).collect();
    assert_eq!(v, vec![s("name"), s("hr"), s("avg")]);
    assert_eq!(items(&ok("[]")), []);
    assert_eq!(items(&ok("[ ]")), []);
    assert_eq!(items(&ok("[1, 2,]")).len(), 2);
    assert_eq!(
        items(&ok("[Mark McGwire, 65, 0.278]"))[0].value,
        s("Mark McGwire")
    );
}

#[test]
fn nested_flow_collections() {
    let node = ok("[[1, 2], {a: [3]}, []]");
    let outer = items(&node);
    assert_eq!(items(&outer[0]).len(), 2);
    assert_eq!(items(get(&outer[1], "a"))[0].value, Value::Int(3));
}

#[test]
fn flow_mappings() {
    let node = ok("{hr: 65, avg: 0.278}");
    assert_eq!(keys(&node), vec!["hr", "avg"]);
    assert_eq!(keys(&ok("{}")), Vec::<&str>::new());
    assert_eq!(keys(&ok("{a: 1,}")), vec!["a"]);
}

#[test]
fn json_style_flow_mapping() {
    let node = ok("{\"key\": [\"value\", 3, true, null], \"b\":1}");
    assert_eq!(items(get(&node, "key")).len(), 4);
    assert_eq!(get(&node, "b").value, Value::Int(1));
}

#[test]
fn multi_line_flow_collections() {
    let node = ok("Sammy Sosa: {\n  hr: 63,\n  avg: 0.288,\n}\n");
    assert_eq!(keys(get(&node, "Sammy Sosa")), vec!["hr", "avg"]);
    let node = ok("k: [\n  1,\n\n  2\n]\n");
    assert_eq!(items(get(&node, "k")).len(), 2);
}

#[test]
fn flow_continuation_lines_must_respect_indentation() {
    assert_syntax(
        "a:\n  b: [1,\n2]\n",
        "lines inside a flow collection must be indented at least 2 spaces",
    );
    // At the entry's own indentation is fine.
    assert_eq!(
        items(get(get(&ok("a:\n  b: [1,\n  2]\n"), "a"), "b")).len(),
        2
    );
}

#[test]
fn tab_indented_flow_continuation_is_an_error() {
    assert_eq!(err("[1,\n\t2]").kind, ErrorKind::TabIndent);
}

#[test]
fn flow_bare_strings_stop_at_flow_indicators() {
    let node = ok("[a b, c:d, e]");
    let v: Vec<_> = items(&node).iter().map(|n| n.value.clone()).collect();
    assert_eq!(v, vec![s("a b"), s("c:d"), s("e")]);
    assert!(parse("[a[b]]").is_err());
}

#[test]
fn flow_mapping_key_without_space_after_colon() {
    assert_syntax("{a:1}", "`:` must be followed by a space");
    assert_syntax("{a}", "expected `:` after mapping key");
}

#[test]
fn flow_errors() {
    assert_syntax("[1, , 2]", "empty entry");
    assert_syntax("[,]", "empty entry");
    assert_syntax("[\"a\" b]", "expected `,` or `]`");
    assert!(matches!(err("[1, 2").kind, ErrorKind::UnexpectedEof(_)));
    assert!(matches!(err("{a: 1").kind, ErrorKind::UnexpectedEof(_)));
    assert_syntax("{a: }", "missing value for key `a`");
    assert_syntax("[a: 1]", "expected `,` or `]`");
    assert_syntax("[1] x", "unexpected `x` after a value");
}

#[test]
fn flow_collection_as_mapping_key_is_an_error() {
    assert!(parse("[a]: 1").is_err());
}

#[test]
fn crlf_and_cr_line_breaks() {
    for input in [
        "a: 1\r\nb:\r\n  - x\r\n",
        "a: 1\rb:\r  - x\r",
        "a: 1\nb:\r\n  - x\r",
    ] {
        let node = ok(input);
        assert_eq!(get(&node, "a").value, Value::Int(1));
        assert_eq!(items(get(&node, "b"))[0].value, s("x"));
    }
}

#[test]
fn missing_final_newline() {
    assert_eq!(get(&ok("a: 1"), "a").value, Value::Int(1));
}

#[test]
fn trailing_whitespace_on_lines() {
    let node = ok("a: 1   \nb:  \n  - x \t\n");
    assert_eq!(get(&node, "a").value, Value::Int(1));
}

#[test]
fn root_must_not_be_indented() {
    assert_syntax("  a: 1", "root node must not be indented");
    assert_syntax("  42", "root node must not be indented");
}

#[test]
fn empty_documents_are_errors() {
    for input in ["", "\n\n", "   \n", "# just a comment\n", "# a\n\n# b"] {
        assert_eq!(err(input).kind, ErrorKind::EmptyDocument, "{input:?}");
    }
}

#[test]
fn one_root_node_per_document() {
    assert_syntax("a\nb", "unexpected content after the root value");
    assert_syntax("[1]\n[2]", "unexpected content after the root value");
}

#[test]
fn root_scalars_and_flow_collections() {
    assert_eq!(ok("42 # answer\n").value, Value::Int(42));
    assert_eq!(items(&ok("[1,2,3,4,] # side comment")).len(), 4);
}
