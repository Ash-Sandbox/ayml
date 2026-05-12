use crate::value::{MapKey, Node, Value};
use std::fmt::Write as _;

/// Emit an AYML document to a string.
#[must_use]
pub fn emit(node: &Node) -> String {
    let mut out = String::new();
    emit_comment(&mut out, node.comment.as_deref(), 0);
    emit_value(&mut out, &node.value, 0, true);
    // Ensure trailing newline
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// Whether a value needs block-style emission (multiple lines).
/// Empty collections use flow style (`[]`, `{}`), so they don't.
fn needs_block(value: &Value) -> bool {
    match value {
        Value::Seq(entries) => !entries.is_empty(),
        Value::Map(map) => !map.is_empty(),
        _ => false,
    }
}

fn emit_comment(out: &mut String, comment: Option<&str>, indent: usize) {
    if let Some(comment) = comment {
        for line in comment.lines() {
            if line.is_empty() {
                out.push('\n');
            } else {
                emit_indent(out, indent);
                let _ = writeln!(out, "# {line}");
            }
        }
    }
}

fn emit_value(out: &mut String, value: &Value, indent: usize, top_level: bool) {
    match value {
        Value::Null => {
            if top_level {
                emit_indent(out, indent);
            }
            out.push_str("null");
        }
        Value::Bool(b) => {
            if top_level {
                emit_indent(out, indent);
            }
            let _ = write!(out, "{b}");
        }
        Value::Int(i) => {
            if top_level {
                emit_indent(out, indent);
            }
            let _ = write!(out, "{i}");
        }
        Value::Float(f) => {
            if top_level {
                emit_indent(out, indent);
            }
            emit_float(out, *f);
        }
        Value::Str(s) => {
            if top_level {
                emit_indent(out, indent);
            }
            emit_string(out, s, indent);
        }
        Value::Seq(entries) if entries.is_empty() => {
            if top_level {
                emit_indent(out, indent);
            }
            out.push_str("[]");
        }
        Value::Map(map) if map.is_empty() => {
            if top_level {
                emit_indent(out, indent);
            }
            out.push_str("{}");
        }
        Value::Seq(entries) => emit_sequence(out, entries, indent),
        Value::Map(map) => emit_mapping(out, map, indent),
    }
}

fn emit_float(out: &mut String, f: f64) {
    if f.is_nan() {
        out.push_str("nan");
    } else if f.is_infinite() {
        out.push_str(if f.is_sign_negative() { "-inf" } else { "inf" });
    } else {
        let s = format!("{f}");
        out.push_str(&s);
        if !s.contains('.') && !s.contains('e') && !s.contains('E') {
            out.push_str(".0");
        }
    }
}

fn emit_sequence(out: &mut String, entries: &[Node], indent: usize) {
    for entry in entries {
        emit_comment(out, entry.comment.as_deref(), indent);
        emit_indent(out, indent);
        out.push_str("- ");
        emit_seq_entry_value(out, entry, indent + 2);
        emit_inline_comment(out, entry.inline_comment.as_deref());
        out.push('\n');
    }
}

fn emit_mapping(out: &mut String, map: &indexmap::IndexMap<MapKey, Node>, indent: usize) {
    for (key, value_node) in map {
        emit_comment(out, value_node.comment.as_deref(), indent);

        emit_indent(out, indent);
        emit_map_key(out, key);
        out.push(':');

        if needs_block(&value_node.value) {
            out.push('\n');
            emit_value(out, &value_node.value, indent + 2, true);
        } else {
            out.push(' ');
            emit_value(out, &value_node.value, indent, false);
        }

        emit_inline_comment(out, value_node.inline_comment.as_deref());

        if !needs_block(&value_node.value) {
            out.push('\n');
        }
    }
}

fn emit_inline_comment(out: &mut String, comment: Option<&str>) {
    if let Some(ic) = comment {
        out.push_str(" # ");
        out.push_str(ic);
    }
}

fn emit_seq_entry_value(out: &mut String, node: &Node, indent: usize) {
    match &node.value {
        Value::Map(map) if !map.is_empty() => emit_compact_mapping(out, map, indent),
        Value::Seq(_) | Value::Map(_) => emit_flow_value(out, &node.value),
        _ => emit_value(out, &node.value, indent, false),
    }
}

fn emit_compact_mapping(out: &mut String, map: &indexmap::IndexMap<MapKey, Node>, indent: usize) {
    let mut first = true;
    for (key, value_node) in map {
        if !first {
            if !out.ends_with('\n') {
                out.push('\n');
            }
            emit_comment(out, value_node.comment.as_deref(), indent);
            emit_indent(out, indent);
        }
        first = false;

        emit_map_key(out, key);
        out.push(':');

        if needs_block(&value_node.value) {
            emit_inline_comment(out, value_node.inline_comment.as_deref());
            out.push('\n');
            emit_value(out, &value_node.value, indent + 2, true);
        } else {
            out.push(' ');
            emit_value(out, &value_node.value, indent, false);
            emit_inline_comment(out, value_node.inline_comment.as_deref());
        }
    }
}

fn emit_flow_value(out: &mut String, value: &Value) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => {
            let _ = write!(out, "{b}");
        }
        Value::Int(i) => {
            let _ = write!(out, "{i}");
        }
        Value::Float(f) => emit_float(out, *f),
        Value::Str(s) => {
            out.push('"');
            for ch in s.chars() {
                match ch {
                    '"' => out.push_str("\\\""),
                    '\\' => out.push_str("\\\\"),
                    '\n' => out.push_str("\\n"),
                    '\r' => out.push_str("\\r"),
                    '\t' => out.push_str("\\t"),
                    ch if ch.is_control() => {
                        let _ = write!(out, "\\u{:04X}", ch as u32);
                    }
                    _ => out.push(ch),
                }
            }
            out.push('"');
        }
        Value::Seq(entries) => {
            out.push('[');
            for (i, entry) in entries.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                emit_flow_value(out, &entry.value);
            }
            out.push(']');
        }
        Value::Map(map) => {
            out.push('{');
            let mut first = true;
            for (key, value_node) in map {
                if !first {
                    out.push_str(", ");
                }
                first = false;
                emit_map_key(out, key);
                out.push_str(": ");
                emit_flow_value(out, &value_node.value);
            }
            out.push('}');
        }
    }
}

fn emit_map_key(out: &mut String, key: &MapKey) {
    match key {
        MapKey::Bool(b) => {
            let _ = write!(out, "{b}");
        }
        MapKey::Int(i) => {
            let _ = write!(out, "{i}");
        }
        MapKey::String(s) => {
            if needs_quoting(s) {
                out.push('"');
                for ch in s.chars() {
                    match ch {
                        '"' => out.push_str("\\\""),
                        '\\' => out.push_str("\\\\"),
                        '\n' => out.push_str("\\n"),
                        '\r' => out.push_str("\\r"),
                        '\t' => out.push_str("\\t"),
                        ch if ch.is_control() => {
                            let _ = write!(out, "\\u{:04X}", ch as u32);
                        }
                        _ => out.push(ch),
                    }
                }
                out.push('"');
            } else {
                out.push_str(s);
            }
        }
    }
}

fn emit_string(out: &mut String, s: &str, indent: usize) {
    if s.contains('\n') {
        // Triple-quoted
        out.push_str("\"\"\"\n");
        for line in s.lines() {
            emit_indent(out, indent + 2);
            out.push_str(line);
            out.push('\n');
        }
        // Handle trailing newline in the original string
        if s.ends_with('\n') {
            emit_indent(out, indent + 2);
            out.push('\n');
        }
        emit_indent(out, indent + 2);
        out.push_str("\"\"\"");
    } else if needs_quoting(s) {
        out.push('"');
        for ch in s.chars() {
            match ch {
                '"' => out.push_str("\\\""),
                '\\' => out.push_str("\\\\"),
                '\t' => out.push_str("\\t"),
                '\r' => out.push_str("\\r"),
                ch if ch.is_control() => {
                    let _ = write!(out, "\\u{:04X}", ch as u32);
                }
                _ => out.push(ch),
            }
        }
        out.push('"');
    } else {
        out.push_str(s);
    }
}

fn needs_quoting(s: &str) -> bool {
    if s.is_empty() {
        return true;
    }
    // Reserved words
    if matches!(
        s,
        "null" | "true" | "false" | "inf" | "+inf" | "-inf" | "nan"
    ) {
        return true;
    }
    // If it looks like a number, quote it
    if s.parse::<i64>().is_ok() || s.parse::<f64>().is_ok() {
        return true;
    }
    // Leading/trailing whitespace
    if s.starts_with(' ') || s.ends_with(' ') {
        return true;
    }
    // Control characters
    if s.chars().any(char::is_control) {
        return true;
    }
    // Contains characters that would cause issues
    s.contains(": ")
        || s.ends_with(':')
        || s.contains('#')
        || s.starts_with('-')
        || s.starts_with(':')
        || s.starts_with('[')
        || s.starts_with(']')
        || s.starts_with('{')
        || s.starts_with('}')
        || s.starts_with(',')
        || s.starts_with('"')
        || s.contains('\\')
}

fn emit_indent(out: &mut String, n: usize) {
    for _ in 0..n {
        out.push(' ');
    }
}
