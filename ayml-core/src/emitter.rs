//! Canonical AYML emitter.
//!
//! [`Writer`] writes a document from a stream of values without needing a
//! tree. [`emit`] writes a [`Node`] tree. Output is block style for every
//! non-empty collection, with two-space indentation.
//!
//! Comments are written where the parser reads them back, except where AYML
//! has no place for them. `parse(emit(node))` normalizes comments as follows:
//!
//! * Comment lines are trimmed of trailing white space, line breaks inside
//!   inline comments become spaces, and characters that may not appear in a
//!   comment become U+FFFD.
//! * A non-empty collection at the root or in a sequence begins on the line of
//!   its first entry, so its inline comment and its first entry's leading
//!   comment join its leading comment.
//! * A scalar's trailing comment joins the next entry's leading comment, or
//!   the enclosing collection's trailing comment after the last entry.

use std::fmt::Write as _;

use crate::scalar::{self, Context, Scalar};
use crate::value::{Node, Value};

/// Emit a node as an AYML document string.
#[must_use]
pub fn emit(node: &Node) -> String {
    let mut w = Writer::new();
    write_node(&mut w, None, node);
    w.finish()
}

fn write_node(w: &mut Writer, key: Option<&str>, node: &Node) {
    let comments = Comments {
        comment: node.comment.as_deref(),
        inline: node.inline_comment.as_deref(),
        trailing: node.trailing_comment.as_deref(),
    };
    match &node.value {
        Value::Seq(items) => {
            w.begin_seq(key, comments);
            for item in items {
                write_node(w, None, item);
            }
            w.end();
        }
        Value::Map(map) => {
            w.begin_map(key, comments);
            for (key, value) in map {
                write_node(w, Some(&key.name), value);
            }
            w.end();
        }
        scalar => w.scalar(key, scalar.as_scalar().expect("scalar value"), comments),
    }
}

/// Emit a value on a single line in flow style, without comments.
#[must_use]
pub fn emit_flow(value: &Value) -> String {
    let mut out = String::new();
    write_flow(&mut out, value);
    out
}

/// Value comments to write.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Comments<'a> {
    pub comment: Option<&'a str>,
    pub inline: Option<&'a str>,
    pub trailing: Option<&'a str>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Position {
    Root,
    MapValue,
    SeqItem,
}

struct Frame {
    is_map: bool,
    indent: usize,
    entry_indent: usize,
    position: Position,
    opened: bool,
    inline: Option<String>,
    trailing: Option<String>,
}

/// Streaming AYML writer.
///
/// Each value is written with its key, which must be `Some` for a mapping
/// value and `None` otherwise.
///
/// # Panics
/// Invalid calls (a key that does not match the enclosing collection, an
/// `end` without a collection, `finish` with open collections) panic.
pub struct Writer {
    out: String,
    stack: Vec<Frame>,
    pending: bool,
    pending_comments: Vec<String>,
    pending_indent: usize,
    pending_dashes: usize,
    after_colon: bool,
}

impl Default for Writer {
    fn default() -> Self {
        Self::new()
    }
}

impl Writer {
    #[must_use]
    pub fn new() -> Self {
        Self {
            out: String::with_capacity(256),
            stack: Vec::new(),
            pending: false,
            pending_comments: Vec::new(),
            pending_indent: 0,
            pending_dashes: 0,
            after_colon: false,
        }
    }

    /// Return the written document.
    ///
    /// # Panics
    /// Panics if a collection is still open.
    #[must_use]
    pub fn finish(self) -> String {
        assert!(self.stack.is_empty(), "unclosed collection");
        self.out
    }

    /// Write a scalar.
    pub fn scalar(&mut self, key: Option<&str>, value: Scalar<'_>, comments: Comments<'_>) {
        let n = self.begin_entry(key, comments.comment);
        self.flush_pending();
        if std::mem::take(&mut self.after_colon) {
            self.out.push(' ');
        }
        match value {
            Scalar::Str(s) => self.string(&s, n),
            other => write_plain(&mut self.out, &other),
        }
        self.inline_comment(comments.inline);
        self.out.push('\n');
        let trailing_indent = if self.stack.is_empty() { 0 } else { n + 2 };
        self.comment(comments.trailing, trailing_indent);
    }

    /// Start a sequence.
    pub fn begin_seq(&mut self, key: Option<&str>, comments: Comments<'_>) {
        self.begin_collection(key, false, comments);
    }

    /// Start a mapping.
    pub fn begin_map(&mut self, key: Option<&str>, comments: Comments<'_>) {
        self.begin_collection(key, true, comments);
    }

    /// End the innermost collection.
    ///
    /// # Panics
    /// Panics if no collection is open.
    pub fn end(&mut self) {
        let frame = self.stack.pop().expect("end without a collection");
        if frame.opened {
            self.comment(frame.trailing.as_deref(), frame.indent);
            return;
        }
        // An empty collection, in flow style.
        self.flush_pending();
        if std::mem::take(&mut self.after_colon) {
            self.out.push(' ');
        }
        let (open, close) = if frame.is_map { ('{', '}') } else { ('[', ']') };
        self.out.push(open);
        let lines = comment_lines(frame.trailing.as_deref());
        if frame.position == Position::Root {
            self.out.push(close);
            self.inline_comment(frame.inline.as_deref());
            self.out.push('\n');
            self.comment_block(&lines, 0);
            return;
        }
        if !lines.is_empty() {
            self.out.push('\n');
            self.comment_block(&lines, frame.entry_indent + 2);
            self.indent(frame.entry_indent);
        }
        self.out.push(close);
        self.inline_comment(frame.inline.as_deref());
        self.out.push('\n');
    }

    fn begin_collection(&mut self, key: Option<&str>, is_map: bool, comments: Comments<'_>) {
        let n = self.begin_entry(key, comments.comment);
        let position = match self.stack.last() {
            None => Position::Root,
            Some(f) if f.is_map => Position::MapValue,
            Some(_) => Position::SeqItem,
        };
        let indent = if position == Position::Root { 0 } else { n + 2 };
        self.stack.push(Frame {
            is_map,
            indent,
            entry_indent: n,
            position,
            opened: false,
            inline: comments.inline.map(str::to_owned),
            trailing: comments.trailing.map(str::to_owned),
        });
    }

    /// Start a new entry in the innermost collection (or the root value) and
    /// make the pending line its first line. Returns entry indentation.
    fn begin_entry(&mut self, key: Option<&str>, comment: Option<&str>) -> usize {
        let leading = if comment.is_some() {
            comment_lines(comment)
        } else {
            Vec::new()
        };
        let is_map = self.stack.last().is_some_and(|top| top.is_map);
        assert_eq!(
            key.is_some(),
            is_map,
            "a value needs a key exactly when it is in a mapping"
        );
        let Some(top) = self.stack.last_mut() else {
            self.pending = true;
            self.pending_indent = 0;
            self.pending_comments = leading;
            self.pending_dashes = 0;
            return 0;
        };
        let n = top.indent;
        if !top.opened {
            top.opened = true;
            let inline = top.inline.take();
            if top.position == Position::MapValue {
                self.after_colon = false;
                self.inline_comment(inline.as_deref());
                self.out.push('\n');
            } else {
                if inline.is_some() {
                    self.pending_comments
                        .extend(comment_lines(inline.as_deref()));
                }
                self.pending_comments.extend(leading);
                if let Some(key) = key {
                    self.flush_pending();
                    self.write_key(key);
                } else {
                    self.pending_dashes += 1;
                }
                return n;
            }
        }
        if let Some(key) = key {
            self.comment_block(&leading, n);
            self.indent(n);
            self.write_key(key);
        } else {
            self.pending = true;
            self.pending_indent = n;
            self.pending_comments = leading;
            self.pending_dashes = 1;
        }
        n
    }

    fn write_key(&mut self, key: &str) {
        if scalar::is_bare_key(key, Context::Block) {
            self.out.push_str(key);
        } else {
            write_double_quoted(&mut self.out, key);
        }
        self.out.push(':');
        self.after_colon = true;
    }

    /// Write the pending line start.
    fn flush_pending(&mut self) {
        if !self.pending {
            return;
        }
        self.pending = false;
        if !self.pending_comments.is_empty() {
            let comments = std::mem::take(&mut self.pending_comments);
            self.comment_block(&comments, self.pending_indent);
        }
        self.indent(self.pending_indent);
        for _ in 0..self.pending_dashes {
            self.out.push_str("- ");
        }
    }

    fn indent(&mut self, n: usize) {
        push_spaces(&mut self.out, n);
    }

    /// Write comment lines at indentation `n`.
    fn comment_block(&mut self, lines: &[String], n: usize) {
        let last = lines.len().saturating_sub(1);
        for (i, line) in lines.iter().enumerate() {
            if line.is_empty() && i != 0 && i != last {
                self.out.push('\n');
            } else {
                self.indent(n);
                self.out.push('#');
                if !line.is_empty() {
                    self.out.push(' ');
                    self.out.push_str(line);
                }
                self.out.push('\n');
            }
        }
    }

    fn comment(&mut self, comment: Option<&str>, n: usize) {
        if comment.is_some() {
            self.comment_block(&comment_lines(comment), n);
        }
    }

    fn inline_comment(&mut self, comment: Option<&str>) {
        if let Some(text) = comment {
            let text = sanitize_inline(text);
            self.out.push_str(" #");
            if !text.is_empty() {
                self.out.push(' ');
                self.out.push_str(&text);
            }
        }
    }

    /// Write a string value of an entry at indentation `n`.
    fn string(&mut self, s: &str, n: usize) {
        if scalar::is_bare_value(s, Context::Block) {
            self.out.push_str(s);
        } else if s.contains('\n') {
            self.triple_quoted(s, n + 2);
        } else {
            write_double_quoted(&mut self.out, s);
        }
    }

    /// Write a triple-quoted string with content and closing delimiter at indentation `m`.
    fn triple_quoted(&mut self, s: &str, m: usize) {
        self.out.push_str("\"\"\"\n");
        for line in s.split('\n') {
            if !line.is_empty() {
                self.indent(m);
                write_triple_line(&mut self.out, line);
            }
            self.out.push('\n');
        }
        self.indent(m);
        self.out.push_str("\"\"\"");
    }
}

fn push_spaces(out: &mut String, mut n: usize) {
    const SPACES: &str = "                                                                ";
    while n > 0 {
        let k = n.min(SPACES.len());
        out.push_str(&SPACES[..k]);
        n -= k;
    }
}

/// Write a non-string scalar.
fn write_plain(out: &mut String, value: &Scalar<'_>) {
    match value {
        Scalar::Null => out.push_str("null"),
        Scalar::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Scalar::Int(i) => push_int(out, *i),
        Scalar::Float(f) => scalar::write_float(out, *f),
        Scalar::Str(_) => unreachable!("strings are quoted by context"),
    }
}

/// Write an integer in decimal.
fn push_int(out: &mut String, i: i64) {
    let mut buf = [0u8; 20];
    let mut pos = buf.len();
    let mut n = i.unsigned_abs();
    loop {
        pos -= 1;
        buf[pos] = b'0' + (n % 10) as u8;
        n /= 10;
        if n == 0 {
            break;
        }
    }
    if i < 0 {
        out.push('-');
    }
    out.push_str(std::str::from_utf8(&buf[pos..]).unwrap_or_default());
}

/// Split comment text into sanitized lines.
fn comment_lines(comment: Option<&str>) -> Vec<String> {
    let Some(text) = comment else {
        return Vec::new();
    };
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    normalized
        .split('\n')
        .map(|line| sanitize_line(line).trim_end_matches([' ', '\t']).to_owned())
        .collect()
}

fn sanitize_line(line: &str) -> String {
    line.chars()
        .map(|c| {
            if c == '\t' || (scalar::is_printable(c) && c != '\n' && c != '\r') {
                c
            } else {
                '\u{FFFD}'
            }
        })
        .collect()
}

fn sanitize_inline(text: &str) -> String {
    let joined = text.replace("\r\n", " ").replace(['\n', '\r'], " ");
    sanitize_line(&joined)
        .trim_end_matches([' ', '\t'])
        .to_owned()
}

/// Write an escape sequence for a non-printable character.
/// Returns false if the character is printable.
fn write_escape(out: &mut String, ch: char) -> bool {
    let escaped = match ch {
        '\\' => "\\\\",
        '\0' => "\\0",
        '\x07' => "\\a",
        '\x08' => "\\b",
        '\t' => "\\t",
        '\n' => "\\n",
        '\x0B' => "\\v",
        '\x0C' => "\\f",
        '\r' => "\\r",
        '\x1B' => "\\e",
        c if !scalar::is_printable(c) || scalar::is_confusable(c) => {
            let cp = c as u32;
            if cp <= 0xFF {
                let _ = write!(out, "\\x{cp:02X}");
            } else {
                let _ = write!(out, "\\u{cp:04X}");
            }
            return true;
        }
        _ => return false,
    };
    out.push_str(escaped);
    true
}

fn write_double_quoted(out: &mut String, s: &str) {
    out.push('"');
    for ch in s.chars() {
        if ch == '"' {
            out.push_str("\\\"");
        } else if !write_escape(out, ch) {
            out.push(ch);
        }
    }
    out.push('"');
}

/// Write one line of triple-quoted content, without indentation.
fn write_triple_line(out: &mut String, line: &str) {
    let trailing_ws = line.len() - line.trim_end_matches([' ', '\t']).len();
    let last = line.len() - 1;
    let mut quotes = 0;
    for (i, ch) in line.char_indices() {
        if ch == '"' {
            quotes += 1;
            if quotes == 3 {
                out.push_str("\\\"");
                quotes = 0;
            } else {
                out.push('"');
            }
            continue;
        }
        quotes = 0;
        if trailing_ws > 0 && i == last && ch == ' ' {
            out.push_str("\\ ");
        } else if ch == '\t' && !(trailing_ws > 0 && i == last) {
            out.push('\t');
        } else if !write_escape(out, ch) {
            out.push(ch);
        }
    }
}

fn write_flow(out: &mut String, value: &Value) {
    match value {
        Value::Str(s) => {
            if scalar::is_bare_value(s, Context::Flow) {
                out.push_str(s);
            } else {
                write_double_quoted(out, s);
            }
        }
        Value::Seq(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                write_flow(out, &item.value);
            }
            out.push(']');
        }
        Value::Map(map) => {
            out.push('{');
            for (i, (key, item)) in map.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                if scalar::is_bare_key(&key.name, Context::Flow) {
                    out.push_str(&key.name);
                } else {
                    write_double_quoted(out, &key.name);
                }
                out.push_str(": ");
                write_flow(out, &item.value);
            }
            out.push('}');
        }
        scalar => write_plain(out, &scalar.as_scalar().expect("scalar value")),
    }
}
