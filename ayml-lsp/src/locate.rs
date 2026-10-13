use ayml_core::{Node, Span, Value};

/// Given a byte offset in the source, walk the Node tree to find the
/// deepest node containing that offset. Returns the JSON pointer path
/// segments leading to it (e.g. `["servers", "0", "port"]`).
///
/// A cursor on a mapping key (including the position just after it, where
/// the `:` usually is) resolves to the path of that key's value, so the
/// schema lookup finds the property's schema rather than the parent's.
pub fn path_at_offset(node: &Node, offset: usize) -> Vec<String> {
    let mut path = Vec::new();
    walk(node, offset, &mut path);
    path
}

fn contains(span: Span, offset: usize) -> bool {
    span.start <= offset && offset < span.end
}

fn walk(node: &Node, offset: usize, path: &mut Vec<String>) {
    match &node.value {
        Value::Map(map) => {
            for (key, value) in map {
                if key.span.start <= offset && offset <= key.span.end {
                    path.push(key.name.clone());
                    return;
                }
                if contains(value.span, offset) {
                    path.push(key.name.clone());
                    walk(value, offset, path);
                    return;
                }
            }
        }
        Value::Seq(items) => {
            for (i, item) in items.iter().enumerate() {
                if contains(item.span, offset) {
                    path.push(i.to_string());
                    walk(item, offset, path);
                    return;
                }
            }
        }
        _ => {}
    }
}

/// Collect the spans of every key and scalar in the tree. Text outside these
/// spans is structure, white space, or comments.
///
/// Spans are pushed in document order and never overlap.
pub fn token_spans(node: &Node, out: &mut Vec<Span>) {
    match &node.value {
        Value::Map(map) => {
            for (key, value) in map {
                out.push(key.span);
                token_spans(value, out);
            }
        }
        Value::Seq(items) => {
            for item in items {
                token_spans(item, out);
            }
        }
        _ => out.push(node.span),
    }
}

/// The node at a JSON pointer (RFC 6901), such as `/servers/0/port`.
pub fn node_at_pointer<'a>(root: &'a Node, pointer: &str) -> Option<&'a Node> {
    if pointer.is_empty() {
        return Some(root);
    }
    pointer
        .strip_prefix('/')?
        .split('/')
        .try_fold(root, |node, segment| {
            child(node, &segment.replace("~1", "/").replace("~0", "~"))
        })
}

/// The child of a mapping (by key) or sequence (by index).
pub fn child<'a>(node: &'a Node, segment: &str) -> Option<&'a Node> {
    match &node.value {
        Value::Map(map) => map.get(segment),
        Value::Seq(items) => items.get(segment.parse::<usize>().ok()?),
        _ => None,
    }
}
