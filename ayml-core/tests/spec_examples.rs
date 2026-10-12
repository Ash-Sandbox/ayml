//! Every example in `spec.md` parses, with values checked for the main ones.

mod common;

use ayml_core::{Value, emit, parse};
use common::{get, items, keys, ok, s};

const SPEC: &str = include_str!("../../spec.md");

/// The fenced code blocks of the spec that are AYML documents (blocks that
/// define grammar productions are skipped).
fn example_documents() -> Vec<&'static str> {
    let mut docs = Vec::new();
    let mut rest = SPEC;
    while let Some(start) = rest.find("```\n") {
        let body_start = start + 4;
        let Some(len) = rest[body_start..].find("```") else {
            break;
        };
        let body = &rest[body_start..body_start + len];
        if !body.contains("::=") {
            docs.push(body);
        }
        rest = &rest[body_start + len + 3..];
    }
    docs
}

#[test]
fn every_example_parses_and_round_trips() {
    let docs = example_documents();
    assert!(docs.len() >= 25, "found only {} examples", docs.len());
    for doc in docs {
        let node = parse(doc).unwrap_or_else(|e| panic!("{e}\n--- example ---\n{doc}"));
        let reparsed = parse(&emit(&node)).unwrap();
        assert_eq!(node, reparsed, "--- example ---\n{doc}");
    }
}

#[test]
fn player_statistics() {
    let root =
        ok("hr:  65    # Home runs\navg: 0.278 # Batting average\nrbi: 147   # Runs Batted In\n");
    assert_eq!(get(&root, "hr").value, Value::Int(65));
    assert_eq!(get(&root, "avg").value, Value::Float(0.278));
    assert_eq!(
        get(&root, "rbi").inline_comment.as_deref(),
        Some("Runs Batted In")
    );
}

#[test]
fn mapping_of_flow_mappings() {
    let root =
        ok("Mark McGwire: {hr: 65, avg: 0.278}\nSammy Sosa: {\n  hr: 63,\n  avg: 0.288,\n}\n");
    assert_eq!(get(get(&root, "Mark McGwire"), "hr").value, Value::Int(65));
    assert_eq!(
        get(get(&root, "Sammy Sosa"), "avg").value,
        Value::Float(0.288)
    );
}

#[test]
fn two_comments() {
    let root = ok(
        "hr: # 1998 hr ranking\n  - Mark McGwire\n  - Sammy Sosa\n# 1998 rbi ranking\nrbi:\n  - Sammy Sosa\n  - Ken Griffey\n",
    );
    assert_eq!(
        get(&root, "hr").inline_comment.as_deref(),
        Some("1998 hr ranking")
    );
    assert_eq!(
        get(&root, "rbi").comment.as_deref(),
        Some("1998 rbi ranking")
    );
}

#[test]
fn primitives() {
    let root = ok("decimal: 12345\nbinary: 0b10101010\noctal: 0o14\nhexadecimal: 0xC\n");
    assert_eq!(get(&root, "binary").value, Value::Int(170));
    assert_eq!(get(&root, "octal").value, Value::Int(12));
    assert_eq!(get(&root, "hexadecimal").value, Value::Int(12));
    let root = ok(
        "canonical: 1.23015e+3\nexponential: 12.3015e+02\nfixed: 1230.15\nnegative infinity: -inf\nnot a number: nan\n",
    );
    assert_eq!(get(&root, "canonical").value, Value::Float(1230.15));
    assert_eq!(get(&root, "exponential").value, Value::Float(1230.15));
    assert_eq!(
        get(&root, "negative infinity").value,
        Value::Float(f64::NEG_INFINITY)
    );
    assert!(get(&root, "not a number").value.as_f64().unwrap().is_nan());
}

#[test]
fn string_escapes() {
    let root = ok(
        "unicode: \"Sosa did fine.\\u263A\"\ncontrol: \"\\b1998\\t1999\\t2000\\n\"\nhex esc: \"\\x0d\\x0a is \\r\\n\"\nhowdy: \"\\\"Howdy!\\\" he cried.\"\n",
    );
    assert_eq!(get(&root, "unicode").value, s("Sosa did fine.☺"));
    assert_eq!(get(&root, "control").value, s("\x081998\t1999\t2000\n"));
    assert_eq!(get(&root, "hex esc").value, s("\r\n is \r\n"));
    assert_eq!(get(&root, "howdy").value, s("\"Howdy!\" he cried."));
}

#[test]
fn triple_quoted_examples() {
    let root = ok(
        "plain: This unquoted scalar is single-line only.\n\nmulti-line: \"\"\"\n  This string\n  spans many lines.\n  \"\"\"\n\nfolded: \"\"\"\n  This is where \\\n  you might think there is a line break \\\n  but there is no line break here.\n  \"\"\"\n",
    );
    assert_eq!(
        get(&root, "multi-line").value,
        s("This string\nspans many lines.")
    );
    assert_eq!(
        get(&root, "folded").value,
        s("This is where you might think there is a line break but there is no line break here.")
    );
}

#[test]
fn invoice() {
    let doc = example_documents()
        .into_iter()
        .find(|d| d.starts_with("invoice: 34843"))
        .unwrap();
    let root = ok(doc);
    assert_eq!(get(&root, "invoice").value, Value::Int(34843));
    assert_eq!(get(&root, "date").value, s("2001-01-23"));
    assert_eq!(get(&root, "ship-to").value, Value::Null);
    let address = get(get(&root, "bill-to"), "address");
    assert_eq!(
        get(address, "lines").value,
        s("458 Walkman Dr.\nSuite #292")
    );
    assert_eq!(get(address, "city").value, s("Royal Oak"));
    assert_eq!(get(address, "postal").value, Value::Int(48046));
    let products = items(get(&root, "product"));
    assert_eq!(
        keys(&products[0]),
        vec!["sku", "quantity", "description", "price"]
    );
    assert_eq!(get(&products[1], "price").value, Value::Float(2392.0));
    assert_eq!(get(&root, "tax").value, Value::Float(251.42));
    assert_eq!(
        get(&root, "comments").value,
        s("Late afternoon is best.\nBackup contact is Nancy\nBillsmer @ 338-4338.")
    );
}

#[test]
fn log_file() {
    let doc = example_documents()
        .into_iter()
        .find(|d| d.starts_with("Date: 2001-11-23"))
        .unwrap();
    let root = ok(doc);
    assert_eq!(get(&root, "Date").value, s("2001-11-23T15:03:17-5:00"));
    assert_eq!(get(&root, "Fatal").value, s("Unknown variable \"bar\""));
    let stack = items(get(&root, "Stack"));
    assert_eq!(get(&stack[0], "line").value, Value::Int(23));
    assert_eq!(get(&stack[0], "code").value, s("x = MoreObject(\"345\n\")"));
    assert_eq!(get(&stack[1], "code").value, s("foo = bar"));
}

#[test]
fn network_rules_with_comments() {
    let doc = example_documents()
        .into_iter()
        .find(|d| d.starts_with("# Network connection rules"))
        .unwrap();
    let root = ok(doc);
    assert_eq!(
        root.comment.as_deref(),
        Some("Network connection rules\nUse these rules to allow socket connections")
    );
    let network = get(&root, "network");
    let rule = &items(get(network, "rules"))[0];
    let ports = get(rule, "ports");
    assert_eq!(items(ports)[0].inline_comment.as_deref(), Some("Git (SSH)"));
    assert_eq!(ports.trailing_comment.as_deref(), Some("- 8080 (disabled)"));
}

#[test]
fn block_sequence_example() {
    let doc = example_documents()
        .into_iter()
        .find(|d| d.contains("- - nested"))
        .unwrap();
    let root = ok(doc);
    let entries = items(&root);
    assert_eq!(items(&entries[2]).len(), 2);
    assert_eq!(keys(&entries[3]), vec!["name", "value"]);
}
