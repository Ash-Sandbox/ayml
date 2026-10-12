//! Render a canonical tree as AYML source with randomized formatting.
//!
//! The renderer exercises syntax that is never emitted canonically:
//! alternative number notations, optional escapes, triple-quoted strings with
//! line continuations, flow collections spanning lines with comments,
//! implicit sequence indentation, extra spaces after `-` and around `:`, blank
//! lines, bare `#` comment lines, trailing white space, and CR/CRLF line
//! breaks.

use ayml_core::scalar::{self, Context};
use ayml_core::{Key, Mapping, Node, Value};
use proptest::prelude::*;

use crate::tree::{arb_node, canonicalize, is_block};

/// A generated document and the tree it must be parsed as.
#[derive(Debug, Clone)]
pub struct Doc {
    pub text: String,
    pub expected: Node,
}

/// An arbitrary document in randomized style, with comments.
pub fn arb_doc() -> BoxedStrategy<Doc> {
    (arb_node(), any::<u64>())
        .prop_map(|(node, seed)| {
            let expected = canonicalize(&node);
            let text = render(&expected, seed);
            Doc { text, expected }
        })
        .boxed()
}

/// Render a canonical tree (see [`canonicalize`]) as AYML source.
pub fn render(root: &Node, seed: u64) -> String {
    let mut r = Renderer {
        rng: seed | 1,
        out: String::new(),
        nl: "\n",
    };
    r.nl = match r.below(6) {
        0 => "\r\n",
        1 => "\r",
        _ => "\n",
    };
    r.document(root);
    r.out
}

struct Renderer {
    rng: u64,
    out: String,
    nl: &'static str,
}

impl Renderer {
    fn next(&mut self) -> u64 {
        // xorshift64*
        self.rng ^= self.rng >> 12;
        self.rng ^= self.rng << 25;
        self.rng ^= self.rng >> 27;
        self.rng.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    /// True with probability `1 / n`.
    fn one_in(&mut self, n: usize) -> bool {
        self.below(n) == 0
    }

    fn pick<'a>(&mut self, items: &[&'a str]) -> &'a str {
        items[self.below(items.len())]
    }

    fn spaces(&mut self, n: usize) {
        self.out.extend(std::iter::repeat_n(' ', n));
    }

    /// End a line, optionally with trailing white space.
    fn newline(&mut self) {
        if self.one_in(6) {
            let ws = self.pick(&[" ", "  ", "\t", " \t"]);
            self.out.push_str(ws);
        }
        self.out.push_str(self.nl);
    }

    fn blank_lines(&mut self, max: usize) {
        for _ in 0..self.below(max + 1) {
            if self.one_in(3) {
                let ws = self.pick(&[" ", "    ", "\t"]);
                self.out.push_str(ws);
            }
            self.out.push_str(self.nl);
        }
    }

    /// Write comment lines, each indented by a fresh `indent()` value.
    fn comment_lines(&mut self, comment: Option<&str>, indent: &mut dyn FnMut(&mut Self) -> usize) {
        let Some(text) = comment else { return };
        let lines: Vec<&str> = text.split('\n').collect();
        let last = lines.len() - 1;
        for (i, line) in lines.iter().enumerate() {
            if line.is_empty() && i != 0 && i != last && self.one_in(2) {
                self.blank_lines(0);
                self.out.push_str(self.nl);
                continue;
            }
            let n = indent(self);
            self.spaces(n);
            self.out.push('#');
            if !line.is_empty() && (line.starts_with(' ') || !self.one_in(4)) {
                self.out.push(' ');
            }
            self.out.push_str(line);
            self.newline();
        }
    }

    fn inline_comment(&mut self, comment: Option<&str>) {
        if let Some(text) = comment {
            let ws = self.pick(&[" ", "  ", "\t", "   "]);
            self.out.push_str(ws);
            self.out.push('#');
            if !text.is_empty() && (text.starts_with(' ') || !self.one_in(4)) {
                self.out.push(' ');
            }
            self.out.push_str(text);
        }
    }

    fn document(&mut self, root: &Node) {
        self.blank_lines(1);
        self.comment_lines(root.comment.as_deref(), &mut |r| r.below(3));
        self.blank_lines(1);
        let flow_root = is_block(&root.value) && self.one_in(4);
        match &root.value {
            Value::Map(map) if is_block(&root.value) && !flow_root => self.block_map(map, 0, false),
            Value::Seq(items) if is_block(&root.value) && !flow_root => {
                self.block_seq(items, 0, false);
            }
            _ => {
                self.flow_node(root, Context::Block, 0);
                self.inline_comment(root.inline_comment.as_deref());
                self.newline();
            }
        }
        if root.value.is_scalar() {
            self.comment_lines(root.trailing_comment.as_deref(), &mut |r| r.below(4));
        } else if !flow_root && is_block(&root.value) {
            self.comment_lines(root.trailing_comment.as_deref(), &mut |_| 0);
        }
        self.blank_lines(1);
        if self.one_in(3) && self.out.ends_with(self.nl) {
            self.out.truncate(self.out.len() - self.nl.len());
        }
    }

    /// The leading comment of an entry at indentation `n`.
    fn leading(&mut self, comment: Option<&str>, n: usize) {
        self.blank_lines(1);
        self.comment_lines(comment, &mut |r| {
            if r.one_in(3) { r.below(n + 1) } else { n }
        });
        if comment.is_some() {
            self.blank_lines(1);
        }
    }

    /// A block mapping at indentation `n`.
    fn block_map(&mut self, map: &Mapping, n: usize, compact: bool) {
        for (i, (key, value)) in map.iter().enumerate() {
            if !(compact && i == 0) {
                self.leading(value.comment.as_deref(), n);
                self.spaces(n);
            }
            self.key(key, Context::Block);
            let ws = self.pick(&["", "", "", " ", "  "]);
            self.out.push_str(ws);
            self.out.push(':');
            if is_block(&value.value) && !self.one_in(4) {
                self.inline_comment(value.inline_comment.as_deref());
                self.newline();
                match &value.value {
                    Value::Map(inner) => {
                        let m = n + 1 + self.below(4);
                        self.block_map(inner, m, false);
                        self.trailing(value, m);
                    }
                    Value::Seq(items) => {
                        let m = if self.one_in(2) {
                            n
                        } else {
                            n + 1 + self.below(4)
                        };
                        self.block_seq(items, m, false);
                        self.trailing(value, if m == n { m + 1 } else { m });
                    }
                    _ => unreachable!(),
                }
            } else {
                let ws = self.pick(&[" ", " ", "  ", "\t"]);
                self.out.push_str(ws);
                self.flow_node(value, Context::Block, n);
                self.inline_comment(value.inline_comment.as_deref());
                self.newline();
            }
        }
    }

    /// A block sequence at indentation `n`.
    fn block_seq(&mut self, items: &[Node], n: usize, compact: bool) {
        for (i, item) in items.iter().enumerate() {
            if !(compact && i == 0) {
                self.leading(item.comment.as_deref(), n);
                self.spaces(n);
            }
            self.out.push('-');
            let k = 1 + if self.one_in(4) { self.below(3) } else { 0 };
            self.spaces(k);
            let c = n + 1 + k;
            match &item.value {
                Value::Map(map) if is_block(&item.value) && !self.one_in(5) => {
                    self.block_map(map, c, true);
                    self.trailing(item, c);
                }
                Value::Seq(inner) if is_block(&item.value) && !self.one_in(5) => {
                    self.block_seq(inner, c, true);
                    self.trailing(item, c);
                }
                _ => {
                    self.flow_node(item, Context::Block, n);
                    self.inline_comment(item.inline_comment.as_deref());
                    self.newline();
                }
            }
        }
    }

    fn trailing(&mut self, node: &Node, indent: usize) {
        self.comment_lines(node.trailing_comment.as_deref(), &mut |_| indent);
    }

    /// Write a key, bare or quoted. Returns true if it was quoted.
    fn key(&mut self, key: &Key, ctx: Context) -> bool {
        if scalar::is_bare_key(&key.name, ctx) && !self.one_in(4) {
            self.out.push_str(&key.name);
            false
        } else {
            self.double_quoted(&key.name);
            true
        }
    }

    /// A scalar or flow collection.
    fn flow_node(&mut self, node: &Node, ctx: Context, n: usize) {
        match &node.value {
            Value::Null => self.out.push_str("null"),
            Value::Bool(b) => self.out.push_str(if *b { "true" } else { "false" }),
            Value::Int(i) => self.int(*i),
            Value::Float(f) => self.float(*f),
            Value::Str(s) => self.string(s, ctx, n),
            Value::Seq(items) => self.flow_seq(node, items, n),
            Value::Map(map) => self.flow_map(node, map, n),
        }
    }

    fn int(&mut self, i: i64) {
        let magnitude = i.unsigned_abs();
        let sign = if i < 0 {
            "-"
        } else if self.one_in(5) {
            "+"
        } else {
            ""
        };
        let digits = match self.below(8) {
            0 => format!("0x{magnitude:x}"),
            1 => format!("0x{magnitude:X}"),
            2 => format!("0o{magnitude:o}"),
            3 => format!("0b{magnitude:b}"),
            _ => magnitude.to_string(),
        };
        self.out.push_str(sign);
        self.out.push_str(&digits);
    }

    fn float(&mut self, f: f64) {
        if f.is_nan() {
            self.out.push_str("nan");
            return;
        }
        if f.is_infinite() {
            let text = if f < 0.0 {
                "-inf"
            } else {
                self.pick(&["inf", "+inf"])
            };
            self.out.push_str(text);
            return;
        }
        let mut text = if self.one_in(2) {
            format!("{f:?}")
        } else {
            format!("{f:e}")
        };
        if let Some(e) = text.find('e') {
            if self.one_in(2) {
                text.replace_range(e..=e, "E");
            }
            if text.as_bytes().get(e + 1) != Some(&b'-') && self.one_in(2) {
                text.insert(e + 1, '+');
            }
        }
        if !text.starts_with('-') && self.one_in(5) {
            text.insert(0, '+');
        }
        self.out.push_str(&text);
    }

    fn string(&mut self, s: &str, ctx: Context, n: usize) {
        let bare_ok = scalar::is_bare_value(s, ctx);
        match self.below(4) {
            0 | 1 if bare_ok => self.out.push_str(s),
            0 if !s.contains('\n') => self.double_quoted(s),
            0 | 1 => self.triple_quoted(s, n),
            _ => self.double_quoted(s),
        }
    }

    /// An escape sequence for `ch`, chosen at random among valid forms.
    fn escape(&mut self, ch: char) -> String {
        let named = match ch {
            '\0' => Some("\\0"),
            '\x07' => Some("\\a"),
            '\x08' => Some("\\b"),
            '\t' => Some("\\t"),
            '\n' => Some("\\n"),
            '\x0B' => Some("\\v"),
            '\x0C' => Some("\\f"),
            '\r' => Some("\\r"),
            '\x1B' => Some("\\e"),
            ' ' => Some("\\ "),
            '"' => Some("\\\""),
            '/' => Some("\\/"),
            '\\' => Some("\\\\"),
            _ => None,
        };
        if let Some(named) = named
            && !self.one_in(3)
        {
            return named.to_owned();
        }
        let cp = ch as u32;
        match self.below(3) {
            0 if cp <= 0xFF => format!("\\x{cp:02x}"),
            1 if cp <= 0xFFFF => format!("\\u{cp:04X}"),
            _ => format!("\\U{cp:08x}"),
        }
    }

    fn must_escape(ch: char) -> bool {
        !scalar::is_printable(ch) || ch == '\n' || ch == '\r' || ch == '\\'
    }

    fn double_quoted(&mut self, s: &str) {
        self.out.push('"');
        for ch in s.chars() {
            if ch == '"' || Self::must_escape(ch) || self.one_in(12) {
                let e = self.escape(ch);
                self.out.push_str(&e);
            } else {
                self.out.push(ch);
            }
        }
        self.out.push('"');
    }

    /// A triple-quoted string whose closing delimiter is indented at least `n` spaces.
    fn triple_quoted(&mut self, s: &str, n: usize) {
        let m = n + self.below(4);
        self.out.push_str("\"\"\"");
        if self.one_in(4) {
            let ws = self.pick(&[" ", "\t"]);
            self.out.push_str(ws);
        }
        self.out.push_str(self.nl);
        for line in s.split('\n') {
            if line.is_empty() {
                let k = self.below(m + 1);
                self.spaces(k);
                self.out.push_str(self.nl);
                continue;
            }
            let tokens = self.triple_tokens(line);
            self.spaces(m);
            for (i, token) in tokens.iter().enumerate() {
                if i > 0 && self.one_in(10) {
                    // Line continuation.
                    self.out.push('\\');
                    self.out.push_str(self.nl);
                    self.spaces(m);
                }
                self.out.push_str(token);
            }
            self.out.push_str(self.nl);
        }
        self.spaces(m);
        self.out.push_str("\"\"\"");
    }

    /// Split one content line into tokens. Escape backslashes, line breaks
    /// and non-printable characters, every third consecutive quote, and a
    /// trailing space or tab.
    fn triple_tokens(&mut self, line: &str) -> Vec<String> {
        let chars: Vec<char> = line.chars().collect();
        let mut tokens = Vec::with_capacity(chars.len());
        let mut quotes = 0;
        for (i, &ch) in chars.iter().enumerate() {
            let is_last = i + 1 == chars.len();
            let escape = if ch == '"' {
                quotes += 1;
                quotes == 3 || self.one_in(12)
            } else {
                Self::must_escape(ch) || (is_last && (ch == ' ' || ch == '\t')) || self.one_in(12)
            };
            if escape {
                quotes = 0;
                let e = self.escape(ch);
                tokens.push(e);
            } else {
                if ch != '"' {
                    quotes = 0;
                }
                tokens.push(ch.to_string());
            }
        }
        tokens
    }

    /// White space between flow tokens.
    fn flow_sep(&mut self, n: usize) {
        match self.below(5) {
            0 => {
                self.newline();
                self.blank_lines(1);
                let k = n + self.below(3);
                self.spaces(k);
            }
            1 => self.out.push(' '),
            2 => self.out.push_str("  "),
            _ => {}
        }
    }

    /// Comment lines inside a flow collection.
    fn flow_comment_lines(&mut self, comment: Option<&str>, n: usize) {
        if comment.is_none() {
            return;
        }
        self.newline();
        self.comment_lines(comment, &mut |r| n + r.below(3));
        let k = n + self.below(3);
        self.spaces(k);
    }

    fn flow_seq(&mut self, node: &Node, items: &[Node], n: usize) {
        self.flow_collection(node, '[', ']', n, items.iter().map(|i| (None, i)).collect());
    }

    fn flow_map(&mut self, node: &Node, map: &Mapping, n: usize) {
        self.flow_collection(
            node,
            '{',
            '}',
            n,
            map.iter().map(|(k, v)| (Some(k), v)).collect(),
        );
    }

    fn flow_collection(
        &mut self,
        node: &Node,
        open: char,
        close: char,
        n: usize,
        entries: Vec<(Option<&Key>, &Node)>,
    ) {
        self.out.push(open);
        let count = entries.len();
        for (i, (key, value)) in entries.into_iter().enumerate() {
            self.flow_sep(n);
            self.flow_comment_lines(value.comment.as_deref(), n);
            if let Some(key) = key {
                let quoted = self.key(key, Context::Flow);
                if self.one_in(3) {
                    self.out.push(' ');
                }
                self.out.push(':');
                if !(quoted && self.one_in(3)) {
                    self.out.push(' ');
                }
            }
            self.flow_node(value, Context::Flow, n);
            if i + 1 < count || self.one_in(3) {
                if self.one_in(5) {
                    self.out.push(' ');
                }
                self.out.push(',');
            }
            if value.inline_comment.is_some() {
                self.inline_comment(value.inline_comment.as_deref());
                self.newline();
                let k = n + self.below(3);
                self.spaces(k);
            }
        }
        self.flow_sep(n);
        self.flow_comment_lines(node.trailing_comment.as_deref(), n);
        self.out.push(close);
    }
}
