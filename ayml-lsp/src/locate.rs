use ayml_core::{Node, Value};

/// Given a byte offset in the source, walk the Node tree to find the
/// deepest node containing that offset. Returns the JSON pointer path
/// segments leading to it (e.g. `["servers", "0", "port"]`).
///
/// If the cursor is on a mapping key, returns the path *to* that key's value
/// (so the schema lookup finds the property's schema, not the parent's).
pub fn path_at_offset(node: &Node, offset: usize) -> Vec<String> {
    let mut path = Vec::new();
    walk(node, offset, &mut path);
    path
}

fn walk(node: &Node, offset: usize, path: &mut Vec<String>) {
    match &node.value {
        Value::Map(map) => {
            for (key, value_node) in map {
                // Check if cursor is within the value node's span.
                if value_node.span.start <= offset && offset < value_node.span.end {
                    path.push(key.to_string());
                    walk(value_node, offset, path);
                    return;
                }
            }
            // Cursor might be on a key rather than a value.
            // We can't get the key's span directly, but if the cursor is
            // within the map's span and before any value, try to find which
            // key line it's on by checking if the offset falls between the
            // start of the map's span and the value span.
            for (key, value_node) in map {
                // If cursor is before this value but after the map start,
                // it's likely on the key text for this entry.
                if offset < value_node.span.start && offset >= node.span.start {
                    path.push(key.to_string());
                    return;
                }
            }
        }
        Value::Seq(items) => {
            for (i, item) in items.iter().enumerate() {
                if item.span.start <= offset && offset < item.span.end {
                    path.push(i.to_string());
                    walk(item, offset, path);
                    return;
                }
            }
        }
        _ => {}
    }
}
