//! Arbitrary node trees, canonicalization, and diffing.

use std::fmt::Write as _;

use ayml_core::{Key, Mapping, Node, Value};
use proptest::prelude::*;

use crate::strings::{
    arb_comment, arb_f64, arb_i64, arb_inline_comment, arb_key, arb_lookalike, arb_string,
};

/// An arbitrary scalar value.
pub fn arb_scalar() -> BoxedStrategy<Value> {
    prop_oneof![
        1 => Just(Value::Null),
        1 => any::<bool>().prop_map(Value::Bool),
        2 => arb_i64().prop_map(Value::Int),
        2 => arb_f64().prop_map(Value::Float),
        6 => arb_string().prop_map(Value::Str),
        1 => arb_lookalike().prop_map(Value::Str),
    ]
    .boxed()
}

fn with_comments(
    value: Value,
    (comment, inline, trailing): (Option<String>, Option<String>, Option<String>),
) -> Node {
    Node {
        value,
        comment,
        inline_comment: inline,
        trailing_comment: trailing,
        ..Node::default()
    }
}

fn arb_comments() -> BoxedStrategy<(Option<String>, Option<String>, Option<String>)> {
    (arb_comment(), arb_inline_comment(), arb_comment()).boxed()
}

fn mapping(entries: Vec<(String, Node)>) -> Mapping {
    let mut map = Mapping::new();
    for (k, v) in entries {
        map.entry(Key::new(k)).or_insert(v);
    }
    map
}

/// An arbitrary node tree with comments in arbitrary positions.
pub fn arb_node() -> BoxedStrategy<Node> {
    arb_node_with(arb_comments())
}

/// An arbitrary node tree without comments.
pub fn arb_plain_node() -> BoxedStrategy<Node> {
    arb_node_with(Just((None, None, None)).boxed())
}

fn arb_node_with(
    comments: BoxedStrategy<(Option<String>, Option<String>, Option<String>)>,
) -> BoxedStrategy<Node> {
    let leaf = (arb_scalar(), comments.clone()).prop_map(|(v, c)| with_comments(v, c));
    leaf.prop_recursive(5, 40, 5, move |inner| {
        let collection = prop_oneof![
            prop::collection::vec(inner.clone(), 0..5).prop_map(Value::Seq),
            prop::collection::vec((arb_key(), inner), 0..5)
                .prop_map(|entries| Value::Map(mapping(entries))),
        ];
        (collection, comments.clone()).prop_map(|(v, c)| with_comments(v, c))
    })
    .boxed()
}

/// True for a non-empty collection.
pub fn is_block(value: &Value) -> bool {
    match value {
        Value::Seq(items) => !items.is_empty(),
        Value::Map(map) => !map.is_empty(),
        _ => false,
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Position {
    Root,
    MapValue,
    SeqItem,
}

fn lines(comment: Option<String>) -> Vec<String> {
    comment.map_or_else(Vec::new, |c| c.split('\n').map(str::to_owned).collect())
}

fn join(lines: Vec<String>) -> Option<String> {
    (!lines.is_empty()).then(|| lines.join("\n"))
}

fn concat(a: Option<String>, b: Option<String>) -> Option<String> {
    let mut all = lines(a);
    all.extend(lines(b));
    join(all)
}

/// Returns the canonical tree that `parse(emit(node))` must produce,
/// for a tree whose comments are already normalized.
pub fn canonicalize(node: &Node) -> Node {
    let mut node = node.clone();
    canon(&mut node, Position::Root);
    node
}

fn canon(node: &mut Node, position: Position) {
    if matches!(position, Position::Root | Position::SeqItem) && is_block(&node.value) {
        let mut hoisted = lines(node.comment.take());
        hoisted.extend(lines(node.inline_comment.take()));
        hoist_first(&mut node.value, &mut hoisted);
        node.comment = join(hoisted);
    }
    let (children, child_position): (Vec<&mut Node>, Position) = match &mut node.value {
        Value::Seq(items) => (items.iter_mut().collect(), Position::SeqItem),
        Value::Map(map) => (map.values_mut().collect(), Position::MapValue),
        _ => return,
    };
    let mut carry: Option<String> = None;
    for child in children {
        canon(child, child_position);
        child.comment = concat(carry.take(), child.comment.take());
        if child.value.is_scalar() {
            carry = child.trailing_comment.take();
        }
    }
    node.trailing_comment = concat(carry, node.trailing_comment.take());
}

fn hoist_first(value: &mut Value, hoisted: &mut Vec<String>) {
    match value {
        Value::Seq(items) if !items.is_empty() => {
            let first = &mut items[0];
            hoisted.extend(lines(first.comment.take()));
            if is_block(&first.value) {
                hoisted.extend(lines(first.inline_comment.take()));
                hoist_first(&mut first.value, hoisted);
            }
        }
        Value::Map(map) if !map.is_empty() => {
            if let Some((_, first)) = map.first_mut() {
                hoisted.extend(lines(first.comment.take()));
            }
        }
        _ => {}
    }
}

/// Remove all comments from a tree.
pub fn strip_comments(node: &Node) -> Node {
    let value = match &node.value {
        Value::Seq(items) => Value::Seq(items.iter().map(strip_comments).collect()),
        Value::Map(map) => Value::Map(
            map.iter()
                .map(|(k, v)| (k.clone(), strip_comments(v)))
                .collect(),
        ),
        other => other.clone(),
    };
    Node::new(value)
}

/// Describe the first difference between two trees, comparing values
/// strictly (floats by bit pattern, any NaN equal to any NaN; keys and
/// entries in order) and comparing all comments. Spans are ignored.
pub fn diff(expected: &Node, actual: &Node) -> Option<String> {
    let mut path = String::from("$");
    diff_at(expected, actual, &mut path)
}

fn diff_at(expected: &Node, actual: &Node, path: &mut String) -> Option<String> {
    for (what, e, a) in [
        ("comment", &expected.comment, &actual.comment),
        (
            "inline_comment",
            &expected.inline_comment,
            &actual.inline_comment,
        ),
        (
            "trailing_comment",
            &expected.trailing_comment,
            &actual.trailing_comment,
        ),
    ] {
        if e != a {
            return Some(format!("{path}: {what}: expected {e:?}, got {a:?}"));
        }
    }
    diff_values(&expected.value, &actual.value, path, &mut |e, a, p| {
        diff_at(e, a, p)
    })
}

/// Like [`diff`], but ignoring comments.
pub fn diff_values_only(expected: &Node, actual: &Node) -> Option<String> {
    let mut path = String::from("$");
    diff_values_at(expected, actual, &mut path)
}

fn diff_values_at(expected: &Node, actual: &Node, path: &mut String) -> Option<String> {
    diff_values(&expected.value, &actual.value, path, &mut |e, a, p| {
        diff_values_at(e, a, p)
    })
}

type ChildDiff<'a> = dyn FnMut(&Node, &Node, &mut String) -> Option<String> + 'a;

fn diff_values(
    expected: &Value,
    actual: &Value,
    path: &mut String,
    child: &mut ChildDiff<'_>,
) -> Option<String> {
    let mismatch = || Some(format!("{path}: expected {expected:?}, got {actual:?}"));
    match (expected, actual) {
        (Value::Null, Value::Null) => None,
        (Value::Bool(a), Value::Bool(b)) if a == b => None,
        (Value::Int(a), Value::Int(b)) if a == b => None,
        (Value::Float(a), Value::Float(b))
            if a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan()) =>
        {
            None
        }
        (Value::Str(a), Value::Str(b)) if a == b => None,
        (Value::Seq(a), Value::Seq(b)) => {
            if a.len() != b.len() {
                return Some(format!(
                    "{path}: expected {} items, got {}",
                    a.len(),
                    b.len()
                ));
            }
            for (i, (x, y)) in a.iter().zip(b).enumerate() {
                let len = path.len();
                let _ = write!(path, "[{i}]");
                if let Some(d) = child(x, y, path) {
                    return Some(d);
                }
                path.truncate(len);
            }
            None
        }
        (Value::Map(a), Value::Map(b)) => {
            let ka: Vec<&str> = a.keys().map(Key::as_str).collect();
            let kb: Vec<&str> = b.keys().map(Key::as_str).collect();
            if ka != kb {
                return Some(format!("{path}: expected keys {ka:?}, got {kb:?}"));
            }
            for ((k, x), y) in a.iter().zip(b.values()) {
                let len = path.len();
                let _ = write!(path, ".{:?}", k.name);
                if let Some(d) = child(x, y, path) {
                    return Some(d);
                }
                path.truncate(len);
            }
            None
        }
        _ => mismatch(),
    }
}
