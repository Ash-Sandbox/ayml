//! Canonical AYML emitter.
//!
//! [`Writer`] writes a document from a stream of values without needing a
//!  tree. [`emit`] writes a [`Node`] tree. Output is block style for every
//! non-empty collection, with two-space indentation. For every node tree,
//! `parse(emit(node))` normalizes the following:
//!
//! * Comment lines are trimmed of trailing white space.
//! * Line breaks inside inline comments become spaces.
//! * Characters that are invalid in a comment are replaced with U+FFFD.
//! * An inline comment in the root collection is turned into a leading comment.
//! * An inline comment on the first element of a sequence becomes a leading comment.
//! * The trailing comment of an empty root collection is written after its brackets.
//! * The trailing comment of a scalar is written after its entry.

use std::fmt::Write as _;

use crate::scalar::{self, Context, Scalar};
use crate::value::{Node, Value};

/// Emit a node as an AYML document string.
#[must_use]
pub fn emit(node: &Node) -> String {
    let mut w = Writer::new();
    write_node(&mut w, node);
    w.finish()
}

fn write_node(w: &mut Writer, node: &Node) {
    let comments = Comments {
        comment: node.comment.as_deref(),
        inline: node.inline_comment.as_deref(),
        trailing: node.trailing_comment.as_deref(),
    };
    match &node.value {
        Value::Seq(items) => {
            w.begin_seq(comments);
            for item in items {
                write_node(w, item);
            }
            w.end();
        }
        Value::Map(map) => {
            w.begin_map(comments);
            for (key, value) in map {
                w.key(&key.name);
                write_node(w, value);
            }
            w.end();
        }
        scalar => w.scalar(scalar.as_scalar().expect("scalar value"), comments),
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
/// # Panics
/// Calls in an invalid order (a value in a mapping without a key, an `end`
/// without a collection, `finish` with open collections) panic.
pub struct Writer {
    out: String,
    stack: Vec<Frame>,
    pending: bool,
    pending_comments: Vec<String>,
    pending_indent: usize,
    pending_dashes: usize,
    after_colon: bool,
    key: String,
    static_key: Option<&'static str>,
    has_key: bool,
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
            key: String::new(),
            static_key: None,
            has_key: false,
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

    /// Set the key of the next mapping value.
    pub fn key(&mut self, key: &str) {
        self.static_key = None;
        self.key.clear();
        if scalar::is_bare_key(key, Context::Block) {
            self.key.push_str(key);
        } else {
            write_double_quoted(&mut self.key, key);
        }
        self.has_key = true;
    }

    /// Set the key of the next mapping value.
    pub fn static_key(&mut self, key: &'static str) {
        if scalar::is_bare_key(key, Context::Block) {
            self.static_key = Some(key);
            self.has_key = true;
        } else {
            self.key(key);
        }
    }

    /// Write a scalar.
    pub fn scalar(&mut self, value: Scalar<'_>, comments: Comments<'_>) {
        let n = self.begin_entry(comments.comment);
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
    pub fn begin_seq(&mut self, comments: Comments<'_>) {
        self.begin_collection(false, comments);
    }

    /// Start a mapping.
    pub fn begin_map(&mut self, comments: Comments<'_>) {
        self.begin_collection(true, comments);
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

    fn begin_collection(&mut self, is_map: bool, comments: Comments<'_>) {
        let n = self.begin_entry(comments.comment);
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
    fn begin_entry(&mut self, comment: Option<&str>) -> usize {
        let leading = if comment.is_some() {
            comment_lines(comment)
        } else {
            Vec::new()
        };
        let Some(top) = self.stack.last_mut() else {
            self.pending = true;
            self.pending_indent = 0;
            self.pending_comments = leading;
            self.pending_dashes = 0;
            return 0;
        };
        let n = top.indent;
        let is_map = top.is_map;
        assert!(!is_map || self.has_key, "mapping value without a key");
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
                if is_map {
                    self.flush_pending();
                    self.write_key();
                } else {
                    self.pending_dashes += 1;
                }
                return n;
            }
        }
        if is_map {
            self.comment_block(&leading, n);
            self.indent(n);
            self.write_key();
        } else {
            self.pending = true;
            self.pending_indent = n;
            self.pending_comments = leading;
            self.pending_dashes = 1;
        }
        n
    }

    fn write_key(&mut self) {
        match self.static_key.take() {
            Some(key) => self.out.push_str(key),
            None => self.out.push_str(&self.key),
        }
        self.out.push(':');
        self.has_key = false;
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
