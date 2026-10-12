//! Helpers shared by the unit test files.

#![allow(dead_code)]

use ayml_core::{Error, ErrorKind, Node, Value, parse};

/// Parse or panic with the error and input.
#[track_caller]
pub fn ok(input: &str) -> Node {
    parse(input).unwrap_or_else(|e| panic!("parse failed: {e}\n--- input ---\n{input}\n---"))
}

/// Parse, and panic if the input is accepted.
#[track_caller]
pub fn err(input: &str) -> Error {
    match parse(input) {
        Ok(node) => panic!("expected an error, got {node:?}\n--- input ---\n{input}\n---"),
        Err(e) => e,
    }
}

/// The error message, without location.
#[track_caller]
pub fn err_msg(input: &str) -> String {
    err(input).kind.to_string()
}

/// True if the error is a syntax error whose message contains `needle`.
#[track_caller]
pub fn assert_syntax(input: &str, needle: &str) {
    let e = err(input);
    match &e.kind {
        ErrorKind::Syntax(msg) => assert!(
            msg.contains(needle),
            "message {msg:?} does not contain {needle:?}\n--- input ---\n{input}"
        ),
        other => panic!("expected a syntax error containing {needle:?}, got {other:?} ({e})"),
    }
}

/// The value of a root scalar document.
#[track_caller]
pub fn scalar(input: &str) -> Value {
    ok(input).value
}

/// Look up a mapping entry. Panic if absent.
#[track_caller]
pub fn get<'a>(node: &'a Node, key: &str) -> &'a Node {
    node.get(key)
        .unwrap_or_else(|| panic!("no key {key:?} in {:?}", node.value))
}

/// Return sequence node items.
#[track_caller]
pub fn items(node: &Node) -> &[Node] {
    node.value
        .as_sequence()
        .unwrap_or_else(|| panic!("not a sequence: {:?}", node.value))
}

/// Return mapping node keys, in order.
#[track_caller]
pub fn keys(node: &Node) -> Vec<&str> {
    node.value
        .as_mapping()
        .unwrap_or_else(|| panic!("not a mapping: {:?}", node.value))
        .keys()
        .map(ayml_core::Key::as_str)
        .collect()
}

pub fn s(text: &str) -> Value {
    Value::Str(text.to_owned())
}
