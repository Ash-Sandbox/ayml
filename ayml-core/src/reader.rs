//! Pull parser for AYML.

use std::borrow::Cow;
use std::collections::HashSet;

use crate::error::{Error, ErrorKind, Span};
use crate::scalar::{self, Context, ResolveError, Scalar};

/// Default maximum nesting depth of collections.
pub const DEFAULT_MAX_DEPTH: usize = 128;

type Result<T> = std::result::Result<T, Error>;

/// A comment in a [`Node`](crate::Node) tree.
#[derive(Debug, Clone)]
pub struct Comment<'a>(Box<CommentRepr<'a>>);

#[derive(Debug, Clone)]
enum CommentRepr<'a> {
    Lines {
        src: &'a str,
        /// Text ranges of comment lines, from the first to last;
        /// `None` for a blank line in between.
        lines: Vec<Option<(usize, usize)>>,
    },
    Text(&'a str),
}

impl<'a> Comment<'a> {
    pub(crate) fn from_text(text: &'a str) -> Self {
        Self(Box::new(CommentRepr::Text(text)))
    }

    /// The comment text, with lines joined by `\n`.
    /// An empty line stands for a blank line or a bare `#`.
    #[must_use]
    pub fn text(&self) -> Cow<'a, str> {
        match &*self.0 {
            CommentRepr::Text(text) => Cow::Borrowed(text),
            CommentRepr::Lines { src, lines } => {
                let line = |l: &Option<(usize, usize)>| l.map_or("", |(s, e)| &src[s..e]);
                if let [only] = lines.as_slice() {
                    return Cow::Borrowed(line(only));
                }
                let mut out = String::new();
                for (i, l) in lines.iter().enumerate() {
                    if i > 0 {
                        out.push('\n');
                    }
                    out.push_str(line(l));
                }
                Cow::Owned(out)
            }
        }
    }
}

impl PartialEq for Comment<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.text() == other.text()
    }
}

impl Eq for Comment<'_> {}

/// An event produced by a [`Reader`].
#[derive(Debug, Clone, PartialEq)]
pub enum Event<'a> {
    SeqStart {
        /// Offset of the first `-` or of the `[`.
        start: usize,
        leading: Option<Comment<'a>>,
    },
    MapStart {
        /// Offset of the first key or of the `{`.
        start: usize,
        leading: Option<Comment<'a>>,
    },
    Key {
        name: Cow<'a, str>,
        span: Span,
    },
    Scalar {
        value: Scalar<'a>,
        span: Span,
        leading: Option<Comment<'a>>,
        inline: Option<&'a str>,
    },
    /// The end of the innermost open collection.
    End {
        /// End offset of the collection: after its last entry (block style)
        /// or after its closing bracket (flow style).
        end: usize,
        inline: Option<&'a str>,
        trailing: Option<Comment<'a>>,
    },
    /// The end of the document, with comment lines after the root value that
    /// no collection claimed.
    DocumentEnd {
        trailing: Option<Comment<'a>>,
    },
}

/// A blank or comment line inside a gap.
#[derive(Clone, Copy)]
struct GapLine {
    start: usize,
    indent: usize,
    text: Option<(usize, usize)>,
}

/// The first content line after a gap.
#[derive(Clone, Copy)]
struct NextLine {
    indent: usize,
    /// Offset of the first non-space character.
    content: usize,
}

/// How a block sequence may end when the next content line has the same
/// indentation as its entries but is not an entry.
#[derive(Clone, Copy, PartialEq, Eq)]
enum SeqEnd {
    /// The line is an error.
    Strict,
    SameIndentAsParent,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FlowState {
    Entry,
    Value,
    Separator,
}

impl FlowState {
    const fn after_entry(had_comma: bool) -> Self {
        if had_comma {
            Self::Entry
        } else {
            Self::Separator
        }
    }
}

enum FrameKind {
    BlockSeq {
        n: usize,
        end_mode: SeqEnd,
        first: bool,
    },
    BlockMap {
        n: usize,
        /// The scanned first key, until it is read.
        first: Option<(usize, usize)>,
    },
    Flow {
        is_map: bool,
        /// Offset of the opening bracket.
        start: usize,
        min_indent: usize,
        state: FlowState,
        /// Comment and blank lines since the last entry.
        pending: Vec<Option<(usize, usize)>>,
    },
}

struct Frame<'a> {
    kind: FrameKind,
    /// The comment after `key:` for a block collection under a key.
    inline: Option<&'a str>,
}

/// The keys of a mapping, for duplicate detection: a list for small
/// mappings, a hash set for large ones.
#[derive(Default)]
struct Keys<'a> {
    list: Vec<Cow<'a, str>>,
    set: Option<HashSet<Cow<'a, str>>>,
}

impl<'a> Keys<'a> {
    const LINEAR_LIMIT: usize = 16;

    /// Insert a key; returns false if it was already present.
    #[allow(clippy::ptr_arg)] // The `Cow` is cloned into the set.
    fn insert(&mut self, key: &Cow<'a, str>) -> bool {
        if let Some(set) = &mut self.set {
            return set.insert(key.clone());
        }
        if self.list.iter().any(|k| k == key) {
            return false;
        }
        self.list.push(key.clone());
        if self.list.len() > Self::LINEAR_LIMIT {
            self.set = Some(self.list.drain(..).collect());
        }
        true
    }

    fn clear(&mut self) {
        self.list.clear();
        self.set = None;
    }
}

/// What the reader does on the next call.
enum Next {
    Start,
    /// Read the value of the block mapping entry whose key ends at the
    /// current position (just past the `:`).
    MapValue {
        n: usize,
        key: Span,
    },
    /// Continue the innermost open collection, or end the document.
    Resume,
    Done,
    Failed(Box<Error>),
}

/// A pull parser over an AYML document.
pub struct Reader<'a> {
    src: &'a str,
    b: &'a [u8],
    pos: usize,
    max_depth: usize,
    stack: Vec<Frame<'a>>,
    next: Next,
    /// Leading comment of the next value.
    leading: Option<Comment<'a>>,
    /// Leading comment of the first entry of the current block collection.
    first_leading: Option<Comment<'a>>,
    /// End of the last scalar or flow collection read.
    last_end: usize,
    /// Scratch space for gaps.
    gap: Vec<GapLine>,
    /// Whether the scanned gap contains a comment line.
    gap_has_comment: bool,
    /// The keys seen by the mapping at each depth, reused across mappings.
    keys: Vec<Keys<'a>>,
}

impl<'a> Reader<'a> {
    /// A reader with the default maximum nesting depth.
    #[must_use]
    pub fn new(src: &'a str) -> Self {
        Self::with_max_depth(src, DEFAULT_MAX_DEPTH)
    }

    /// A reader with a custom maximum collection nesting depth.
    #[must_use]
    pub fn with_max_depth(src: &'a str, max_depth: usize) -> Self {
        Self {
            src,
            b: src.as_bytes(),
            pos: 0,
            max_depth,
            stack: Vec::new(),
            next: Next::Start,
            leading: None,
            first_leading: None,
            last_end: 0,
            gap: Vec::new(),
            gap_has_comment: false,
            keys: Vec::new(),
        }
    }

    /// The source text.
    #[must_use]
    pub const fn source(&self) -> &'a str {
        self.src
    }

    /// The number of open collections.
    #[must_use]
    pub fn depth(&self) -> usize {
        self.stack.len()
    }

    /// Read the next event.
    pub fn next_event(&mut self) -> Result<Event<'a>> {
        match std::mem::replace(&mut self.next, Next::Resume) {
            Next::Start => self.start(),
            Next::MapValue { n, key } => self.map_value(n, key),
            Next::Resume => self.resume(),
            Next::Done => {
                self.next = Next::Done;
                Ok(Event::DocumentEnd { trailing: None })
            }
            Next::Failed(e) => Err(*e),
        }
        .map_err(|e| self.fail(e))
    }

    #[cold]
    fn fail(&mut self, error: Error) -> Error {
        self.next = Next::Failed(Box::new(error.clone()));
        error
    }

    fn error(&self, kind: ErrorKind, start: usize, end: usize) -> Error {
        Error::new(kind, Span::new(start, end), self.src)
    }

    fn syntax(&self, message: impl Into<String>, at: usize) -> Error {
        self.error(ErrorKind::Syntax(message.into()), at, at)
    }

    fn eof(&self, context: &str, start: usize) -> Error {
        self.error(
            ErrorKind::UnexpectedEof(context.to_owned()),
            start,
            self.src.len(),
        )
    }

    fn push(&mut self, kind: FrameKind, inline: Option<&'a str>, is_map: bool) -> Result<()> {
        if self.stack.len() >= self.max_depth {
            return Err(self.error(ErrorKind::RecursionLimit, self.pos, self.pos));
        }
        if is_map {
            let depth = self.stack.len();
            if self.keys.len() <= depth {
                self.keys.resize_with(depth + 1, Keys::default);
            }
            self.keys[depth].clear();
        }
        self.stack.push(Frame { kind, inline });
        Ok(())
    }

    fn pop(&mut self) -> Frame<'a> {
        self.stack.pop().expect("open collection")
    }

    fn peek(&self) -> Option<u8> {
        self.b.get(self.pos).copied()
    }

    fn char_at(&self, at: usize) -> Option<char> {
        self.src.get(at..).and_then(|s| s.chars().next())
    }

    /// Offset of the line break (or end of input) at or after `from`.
    fn eol(&self, from: usize) -> usize {
        self.b[from..]
            .iter()
            .position(|&c| c == b'\n' || c == b'\r')
            .map_or(self.b.len(), |i| from + i)
    }

    /// Offset just past the line break at `at`.
    fn after_break(&self, at: usize) -> usize {
        match self.b.get(at) {
            Some(b'\r') if self.b.get(at + 1) == Some(&b'\n') => at + 2,
            Some(_) => at + 1,
            None => at,
        }
    }

    fn skip_white(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t')) {
            self.pos += 1;
        }
    }

    fn is_break_or_eof(&self, at: usize) -> bool {
        matches!(self.b.get(at), None | Some(b'\n' | b'\r'))
    }

    /// Comment text range for a `#` at offset `hash`.
    fn comment_range(&self, hash: usize) -> (usize, usize) {
        let eol = self.eol(hash);
        let mut start = hash + 1;
        if self.b.get(start) == Some(&b' ') && start < eol {
            start += 1;
        }
        let mut end = eol;
        while end > start && matches!(self.b[end - 1], b' ' | b'\t') {
            end -= 1;
        }
        (start, end)
    }

    /// Reject a byte order mark and characters outside `c-printable`.
    fn validate(&self) -> Result<()> {
        if self.src.starts_with('\u{FEFF}') {
            return Err(self.error(ErrorKind::ByteOrderMark, 0, 3));
        }
        let b = self.b;
        let mut i = 0;
        while i < b.len() {
            // Fast path
            if let Some(chunk) = b.get(i..i + 8) {
                const HIGH: u64 = 0x8080_8080_8080_8080;
                const LOW: u64 = 0x7F7F_7F7F_7F7F_7F7F;
                let word = u64::from_le_bytes(chunk.try_into().unwrap_or([0; 8]));
                if word & HIGH == 0 {
                    let below_space = word.wrapping_sub(0x2020_2020_2020_2020) & !word & HIGH;
                    let x = word ^ 0x7F7F_7F7F_7F7F_7F7F;
                    let is_del = x.wrapping_sub(0x0101_0101_0101_0101) & !x & HIGH;
                    // Bytes below 0x20 other than tab, LF, and CR.
                    let is_byte = |c: u64| {
                        let x = word ^ (c * 0x0101_0101_0101_0101);
                        !(((x & LOW) + LOW) | x) & HIGH
                    };
                    let allowed = is_byte(0x09) | is_byte(0x0A) | is_byte(0x0D);
                    if is_del == 0 && below_space & !allowed == 0 {
                        i += 8;
                        continue;
                    }
                }
            }
            let byte = b[i];
            let (ok, len) = if byte < 0x80 {
                (matches!(byte, b'\t' | b'\n' | b'\r' | 0x20..=0x7E), 1)
            } else {
                let ch = self.char_at(i).unwrap_or('\u{FFFD}');
                (scalar::is_printable(ch), ch.len_utf8())
            };
            if !ok {
                let ch = self.char_at(i).unwrap_or('\u{FFFD}');
                return Err(self.error(ErrorKind::NonPrintable(ch), i, i + len));
            }
            i += len;
        }
        Ok(())
    }

    /// Scan blank and comment lines starting at the line start `from` into
    /// `self.gap`, without consuming them. Returns the next content line and
    /// the offset of its start.
    fn scan_gap(&mut self, from: usize) -> Result<(Option<NextLine>, usize)> {
        self.gap.clear();
        self.gap_has_comment = false;
        let b = self.b;
        let mut p = from;
        loop {
            if p >= b.len() {
                return Ok((None, b.len()));
            }
            let mut i = p;
            while b.get(i) == Some(&b' ') {
                i += 1;
            }
            let indent = i - p;
            let mut j = i;
            while matches!(b.get(j), Some(b' ' | b'\t')) {
                j += 1;
            }
            if self.is_break_or_eof(j) {
                self.gap.push(GapLine {
                    start: p,
                    indent,
                    text: None,
                });
                p = self.after_break(j);
                continue;
            }
            if b[i] == b'\t' {
                return Err(self.error(ErrorKind::TabIndent, i, i + 1));
            }
            if b[i] == b'#' {
                self.gap_has_comment = true;
                let text = Some(self.comment_range(i));
                self.gap.push(GapLine {
                    start: p,
                    indent,
                    text,
                });
                p = self.after_break(self.eol(i));
                continue;
            }
            return Ok((Some(NextLine { indent, content: i }), p));
        }
    }

    /// The comment in the first `upto` lines of the scanned gap.
    fn gap_comment(&self, upto: usize) -> Option<Comment<'a>> {
        if !self.gap_has_comment {
            return None;
        }
        make_comment(self.src, self.gap[..upto].iter().map(|l| l.text))
    }

    /// Claim the trailing comment of a block collection with indentation `n`
    /// that ends before a content line of indentation `k`. Positions the reader
    /// at the first unclaimed gap line.
    fn claim_trailing(
        &mut self,
        n: usize,
        k: Option<usize>,
        gap_end: usize,
    ) -> Option<Comment<'a>> {
        let mut claimed = 0;
        for (i, line) in self.gap.iter().enumerate() {
            if line.text.is_some() {
                if line.indent >= n && k.is_none_or(|k| line.indent > k) {
                    claimed = i + 1;
                } else {
                    break;
                }
            }
        }
        self.pos = self.gap.get(claimed).map_or(gap_end, |l| l.start);
        self.gap_comment(claimed)
    }

    /// Parse the end of a content line and returns the comment.
    fn line_end(&mut self) -> Result<Option<&'a str>> {
        let start = self.pos;
        self.skip_white();
        match self.peek() {
            None => Ok(None),
            Some(b'\n' | b'\r') => {
                self.pos = self.after_break(self.pos);
                Ok(None)
            }
            Some(b'#') if self.pos > start => {
                let (s, e) = self.comment_range(self.pos);
                self.pos = self.after_break(self.eol(self.pos));
                Ok(Some(&self.src[s..e]))
            }
            Some(b'#') => Err(self.syntax(
                "`#` must be preceded by white space to start a comment; \
                 to include `#` in a string, quote the string",
                self.pos,
            )),
            Some(b':') => Err(self.syntax(
                "unexpected `:` after a value; a nested mapping must start on a new line, \
                 and a string containing `: ` must be quoted",
                self.pos,
            )),
            Some(_) => Err(self.syntax(
                format!(
                    "unexpected `{}` after a value",
                    self.char_at(self.pos).unwrap_or('?')
                ),
                self.pos,
            )),
        }
    }

    fn start(&mut self) -> Result<Event<'a>> {
        self.validate()?;
        let (next, end) = self.scan_gap(0)?;
        let Some(next) = next else {
            return Err(self.error(ErrorKind::EmptyDocument, end, end));
        };
        if next.indent != 0 {
            return Err(self.syntax("the root node must not be indented", next.content));
        }
        self.leading = self.gap_comment(self.gap.len());
        self.pos = next.content;
        self.block_node(0, 0)
    }

    fn document_end(&mut self) -> Result<Event<'a>> {
        let (next, _) = self.scan_gap(self.pos)?;
        if let Some(next) = next {
            return Err(self.syntax(
                "unexpected content after the root value (a document contains exactly one root node)",
                next.content,
            ));
        }
        self.next = Next::Done;
        Ok(Event::DocumentEnd {
            trailing: self.gap_comment(self.gap.len()),
        })
    }

    /// True if the content at `at` is a block sequence entry indicator.
    fn is_seq_entry(&self, at: usize) -> bool {
        self.b.get(at) == Some(&b'-')
            && !self
                .char_at(at + 1)
                .is_some_and(|c| scalar::is_plain_safe(c, Context::Block))
    }

    /// Read a block node whose first character is at column `col` of the current line.
    fn block_node(&mut self, col: usize, min_indent: usize) -> Result<Event<'a>> {
        if self.is_seq_entry(self.pos) {
            return self.begin_block_seq(col, SeqEnd::Strict, None);
        }
        if let Some(found) = self.block_key_end(self.pos) {
            return self.begin_block_map(col, found, None);
        }
        self.flow_node_in_block(min_indent)
    }

    fn begin_block_seq(
        &mut self,
        n: usize,
        end_mode: SeqEnd,
        inline: Option<&'a str>,
    ) -> Result<Event<'a>> {
        let kind = FrameKind::BlockSeq {
            n,
            end_mode,
            first: true,
        };
        self.push(kind, inline, false)?;
        Ok(Event::SeqStart {
            start: self.pos,
            leading: self.leading.take(),
        })
    }

    fn begin_block_map(
        &mut self,
        n: usize,
        found: (usize, usize),
        inline: Option<&'a str>,
    ) -> Result<Event<'a>> {
        let kind = FrameKind::BlockMap {
            n,
            first: Some(found),
        };
        self.push(kind, inline, true)?;
        Ok(Event::MapStart {
            start: self.pos,
            leading: self.leading.take(),
        })
    }

    /// Scan a block mapping key at `at`.
    fn block_key_end(&self, at: usize) -> Option<(usize, usize)> {
        let end = if self.b.get(at) == Some(&b'"') {
            if self.src[at..].starts_with("\"\"\"") {
                return None;
            }
            let mut i = at + 1;
            loop {
                match self.b.get(i)? {
                    b'"' => break i + 1,
                    b'\\' if !self.is_break_or_eof(i + 1) => i += 2,
                    b'\n' | b'\r' => return None,
                    _ => i += 1,
                }
            }
        } else {
            scalar::scan_bare(self.src, at, Context::Block)?
        };
        let mut q = end;
        while matches!(self.b.get(q), Some(b' ' | b'\t')) {
            q += 1;
        }
        let is_key = self.b.get(q) == Some(&b':')
            && matches!(self.b.get(q + 1), None | Some(b' ' | b'\t' | b'\n' | b'\r'));
        is_key.then_some((end, q + 1))
    }

    /// Read the key found by [`Self::block_key_end`].
    fn read_block_key(&self, at: usize, end: usize) -> Result<Cow<'a, str>> {
        if self.b[at] == b'"' {
            Ok(self.double_quoted(at)?.0)
        } else {
            Ok(Cow::Borrowed(&self.src[at..end]))
        }
    }

    fn unexpected_indentation(&self, next: NextLine) -> Error {
        self.syntax(
            "unexpected indentation (to continue text on the next line, use a triple-quoted string)",
            next.content,
        )
    }

    fn end_block(&mut self, n: usize, k: Option<usize>, gap_end: usize) -> Event<'a> {
        let trailing = self.claim_trailing(n, k, gap_end);
        let frame = self.pop();
        Event::End {
            end: self.last_end,
            inline: frame.inline,
            trailing,
        }
    }

    fn resume(&mut self) -> Result<Event<'a>> {
        let Some(frame) = self.stack.last_mut() else {
            return self.document_end();
        };
        match &mut frame.kind {
            FrameKind::BlockSeq { n, end_mode, first } => {
                let (n, end_mode) = (*n, *end_mode);
                let was_first = std::mem::replace(first, false);
                self.resume_block_seq(n, end_mode, was_first)
            }
            FrameKind::BlockMap { n, first } => {
                let (n, first) = (*n, first.take());
                self.resume_block_map(n, first)
            }
            FrameKind::Flow { .. } => self.resume_flow(),
        }
    }

    fn resume_block_seq(&mut self, n: usize, end_mode: SeqEnd, first: bool) -> Result<Event<'a>> {
        let leading = if first {
            self.first_leading.take()
        } else {
            let (next, gap_end) = self.scan_gap(self.pos)?;
            match next {
                Some(next) if next.indent == n && self.is_seq_entry(next.content) => {
                    self.pos = next.content;
                    self.gap_comment(self.gap.len())
                }
                Some(next) if next.indent > n => return Err(self.unexpected_indentation(next)),
                Some(next) if next.indent == n && end_mode == SeqEnd::Strict => {
                    return Err(self.syntax("expected a sequence entry (`- `)", next.content));
                }
                _ if end_mode == SeqEnd::SameIndentAsParent => {
                    return Ok(self.end_block(n, Some(n), gap_end));
                }
                Some(next) => return Ok(self.end_block(n, Some(next.indent), gap_end)),
                None => return Ok(self.end_block(n, None, gap_end)),
            }
        };

        let dash = self.pos;
        let mut q = dash + 1;
        while matches!(self.b.get(q), Some(b' ' | b'\t')) {
            q += 1;
        }
        if self.is_break_or_eof(q) || self.b[q] == b'#' {
            return Err(self.syntax(
                "sequence entry has no value (AYML has no empty values; write `null`)",
                dash,
            ));
        }
        if let Some(tab) = self.b[dash + 1..q].iter().position(|&c| c == b'\t') {
            let at = dash + 1 + tab;
            return Err(self.error(ErrorKind::TabIndent, at, at + 1));
        }
        self.pos = q;
        self.leading = leading;
        self.block_node(n + (q - dash), n)
    }

    fn resume_block_map(&mut self, n: usize, first: Option<(usize, usize)>) -> Result<Event<'a>> {
        let (found, leading) = if let Some(found) = first {
            (found, self.first_leading.take())
        } else {
            let (next, gap_end) = self.scan_gap(self.pos)?;
            match next {
                Some(next) if next.indent == n => {
                    let Some(found) = self.block_key_end(next.content) else {
                        let message = if self.is_seq_entry(next.content) {
                            "expected a mapping entry (`key: value`), found a sequence entry"
                        } else {
                            "expected a mapping entry (`key: value`)"
                        };
                        return Err(self.syntax(message, next.content));
                    };
                    self.pos = next.content;
                    (found, self.gap_comment(self.gap.len()))
                }
                Some(next) if next.indent > n => return Err(self.unexpected_indentation(next)),
                Some(next) => return Ok(self.end_block(n, Some(next.indent), gap_end)),
                None => return Ok(self.end_block(n, None, gap_end)),
            }
        };
        let (key_end, after_colon) = found;
        let key_start = self.pos;
        let name = self.read_block_key(key_start, key_end)?;
        let span = Span::new(key_start, key_end);
        self.check_duplicate(&name, span)?;
        self.pos = after_colon;
        self.leading = leading;
        self.next = Next::MapValue { n, key: span };
        Ok(Event::Key { name, span })
    }

    #[allow(clippy::ptr_arg)] // The `Cow` is cloned into the key set.
    fn check_duplicate(&mut self, name: &Cow<'a, str>, span: Span) -> Result<()> {
        let depth = self.stack.len() - 1;
        if self.keys[depth].insert(name) {
            Ok(())
        } else {
            Err(self.error(
                ErrorKind::DuplicateKey(name.clone().into_owned()),
                span.start,
                span.end,
            ))
        }
    }

    /// Read the value of a block mapping entry at indentation `n`.
    fn map_value(&mut self, n: usize, key: Span) -> Result<Event<'a>> {
        let colon_end = self.pos;
        self.skip_white();
        let value_on_next_line = self.is_break_or_eof(self.pos) || self.peek() == Some(b'#');
        if !value_on_next_line {
            if self.is_seq_entry(self.pos) && self.b.get(self.pos + 1) == Some(&b' ') {
                return Err(self.syntax(
                    "a block sequence must start on the line after its key",
                    self.pos,
                ));
            }
            return self.flow_node_in_block(n);
        }

        self.pos = colon_end;
        let inline = self.line_end()?;
        let (next, _) = self.scan_gap(self.pos)?;
        let missing = |this: &Self| {
            let name = this
                .read_block_key(key.start, key.end)
                .unwrap_or_else(|_| Cow::Borrowed(&this.src[key.start..key.end]));
            this.error(
                ErrorKind::Syntax(format!(
                    "missing value for key `{name}` (AYML has no empty values; write `null`)"
                )),
                key.start,
                key.end,
            )
        };
        let Some(next) = next else {
            return Err(missing(self));
        };
        let first_leading = self.gap_comment(self.gap.len());
        if next.indent >= n && self.is_seq_entry(next.content) {
            self.pos = next.content;
            let end_mode = if next.indent == n {
                SeqEnd::SameIndentAsParent
            } else {
                SeqEnd::Strict
            };
            self.first_leading = first_leading;
            self.begin_block_seq(next.indent, end_mode, inline)
        } else if next.indent > n {
            let Some(found) = self.block_key_end(next.content) else {
                return Err(self.syntax(
                    "a value on the line after its key must be a block mapping or sequence; \
                     write scalars and flow collections on the same line as the key",
                    next.content,
                ));
            };
            self.pos = next.content;
            self.first_leading = first_leading;
            self.begin_block_map(next.indent, found, inline)
        } else {
            Err(missing(self))
        }
    }

    /// Read a scalar or flow collection in block context.
    fn flow_node_in_block(&mut self, min_indent: usize) -> Result<Event<'a>> {
        match self.peek() {
            Some(b'[') => self.begin_flow(false, min_indent),
            Some(b'{') => self.begin_flow(true, min_indent),
            _ => {
                let (value, span) = self.scalar(Context::Block, min_indent)?;
                let inline = self.line_end()?;
                Ok(Event::Scalar {
                    value,
                    span,
                    leading: self.leading.take(),
                    inline,
                })
            }
        }
    }

    fn begin_flow(&mut self, is_map: bool, min_indent: usize) -> Result<Event<'a>> {
        let kind = FrameKind::Flow {
            is_map,
            start: self.pos,
            min_indent,
            state: FlowState::Entry,
            pending: Vec::new(),
        };
        self.push(kind, None, is_map)?;
        let start = self.pos;
        self.pos += 1;
        let leading = self.leading.take();
        Ok(if is_map {
            Event::MapStart { start, leading }
        } else {
            Event::SeqStart { start, leading }
        })
    }

    /// Read a scalar at the current position.
    fn scalar(&mut self, ctx: Context, min_indent: usize) -> Result<(Scalar<'a>, Span)> {
        let start = self.pos;
        let value = match self.peek() {
            None => return Err(self.eof("where a value was expected", start)),
            Some(b'"') if self.src[start..].starts_with("\"\"\"") => {
                let (s, end) = self.triple_quoted(start, min_indent)?;
                self.pos = end;
                Scalar::Str(Cow::Owned(s))
            }
            Some(b'"') => {
                let (s, end) = self.double_quoted(start)?;
                self.pos = end;
                Scalar::Str(s)
            }
            Some(_) => {
                let Some(end) = scalar::scan_bare(self.src, start, ctx) else {
                    return Err(self.unexpected_value_start(start, ctx));
                };
                self.pos = end;
                let text = &self.src[start..end];
                match scalar::resolve(text) {
                    Ok(v) => v,
                    Err(ResolveError::IntegerOverflow) => {
                        return Err(self.error(ErrorKind::IntegerOverflow, start, end));
                    }
                    Err(ResolveError::FloatOverflow) => {
                        return Err(self.error(ErrorKind::FloatOverflow, start, end));
                    }
                }
            }
        };
        self.last_end = self.pos;
        Ok((value, Span::new(start, self.pos)))
    }

    fn unexpected_value_start(&self, at: usize, ctx: Context) -> Error {
        let ch = self.char_at(at).unwrap_or('?');
        let message = match ch {
            '\'' => "single-quoted strings are not supported; use double quotes".to_owned(),
            '-' if ctx == Context::Block && self.b.get(at + 1) == Some(&b' ') => {
                "a block sequence cannot start here; block collections must begin on a new line"
                    .to_owned()
            }
            '-' | ':' => format!("a bare `{ch}` is not a valid value; quote it as \"{ch}\""),
            '\\' => "a bare string cannot start with `\\`; quote the string".to_owned(),
            '\n' | '\r' => "expected a value".to_owned(),
            _ => format!("unexpected `{ch}` where a value was expected"),
        };
        self.syntax(message, at)
    }

    /// Read a double-quoted string starting at `at`.
    fn double_quoted(&self, at: usize) -> Result<(Cow<'a, str>, usize)> {
        let b = self.b;
        let mut i = at + 1;
        // Fast path
        loop {
            match b.get(i) {
                None => return Err(self.eof("in double-quoted string", at)),
                Some(b'"') => return Ok((Cow::Borrowed(&self.src[at + 1..i]), i + 1)),
                Some(b'\\') => break,
                Some(b'\n' | b'\r') => return Err(self.unterminated(at)),
                Some(_) => i += 1,
            }
        }
        let mut out = String::from(&self.src[at + 1..i]);
        let mut run = i;
        loop {
            match b.get(i) {
                None => return Err(self.eof("in double-quoted string", at)),
                Some(b'"') => {
                    out.push_str(&self.src[run..i]);
                    return Ok((Cow::Owned(out), i + 1));
                }
                Some(b'\\') => {
                    out.push_str(&self.src[run..i]);
                    let (ch, next) = self.decode_escape(i)?;
                    out.push(ch);
                    i = next;
                    run = i;
                }
                Some(b'\n' | b'\r') => return Err(self.unterminated(at)),
                Some(_) => i += 1,
            }
        }
    }

    fn unterminated(&self, at: usize) -> Error {
        self.syntax(
            "unterminated double-quoted string (use a triple-quoted string for multiple lines)",
            at,
        )
    }

    fn decode_escape(&self, at: usize) -> Result<(char, usize)> {
        scalar::decode_escape(self.src, at)
            .map_err(|e| self.error(ErrorKind::InvalidEscape(e.message), at, e.end))
    }

    /// Read a triple-quoted string starting at `at`. Returns the string and
    /// the offset just past the closing `"""`.
    fn triple_quoted(&self, at: usize, min_indent: usize) -> Result<(String, usize)> {
        let b = self.b;
        let mut i = at + 3;
        while matches!(b.get(i), Some(b' ' | b'\t')) {
            i += 1;
        }
        if i == b.len() {
            return Err(self.eof("in triple-quoted string", at));
        }
        if !self.is_break_or_eof(i) {
            return Err(self.syntax(
                "the content of a triple-quoted string must start on the line after the opening `\"\"\"`",
                i,
            ));
        }
        i = self.after_break(i);

        let mut lines: Vec<(usize, usize)> = Vec::new();
        let (closing_line, indent) = loop {
            if i >= b.len() {
                return Err(self.eof("in triple-quoted string (missing closing `\"\"\"`)", at));
            }
            let mut s = i;
            while b.get(s) == Some(&b' ') {
                s += 1;
            }
            if self.src[s..].starts_with("\"\"\"") {
                break (i, s - i);
            }
            let eol = self.eol(i);
            lines.push((i, eol));
            i = self.after_break(eol);
        };
        if indent < min_indent {
            return Err(self.syntax(
                format!("the closing `\"\"\"` must be indented at least {min_indent} spaces"),
                closing_line + indent,
            ));
        }

        let mut out = String::new();
        for (idx, &(line_start, line_end)) in lines.iter().enumerate() {
            let line = &b[line_start..line_end];
            let content = if line.len() >= indent && line[..indent].iter().all(|&c| c == b' ') {
                line_start + indent
            } else if line.iter().all(|&c| c == b' ') {
                line_end
            } else {
                return Err(self.syntax(
                    format!(
                        "triple-quoted content must be indented at least {indent} spaces \
                         (the indentation of the closing `\"\"\"`)"
                    ),
                    line_start,
                ));
            };
            let mut j = content;
            let mut run = j;
            let mut continued = false;
            while j < line_end {
                if b[j] == b'\\' {
                    out.push_str(&self.src[run..j]);
                    if j + 1 == line_end {
                        continued = true;
                        j = line_end;
                    } else {
                        let (ch, next) = self.decode_escape(j)?;
                        out.push(ch);
                        j = next;
                    }
                    run = j;
                } else {
                    j += 1;
                }
            }
            out.push_str(&self.src[run..line_end.max(run)]);
            if !continued && idx + 1 < lines.len() {
                out.push('\n');
            }
        }
        Ok((out, closing_line + indent + 3))
    }

    /// The pending comment lines of the innermost (flow) frame.
    fn pending(&mut self) -> &mut Vec<Option<(usize, usize)>> {
        match self.stack.last_mut().map(|f| &mut f.kind) {
            Some(FrameKind::Flow { pending, .. }) => pending,
            _ => unreachable!("flow frame"),
        }
    }

    fn set_flow_state(&mut self, new: FlowState) {
        if let Some(Frame {
            kind: FrameKind::Flow { state, .. },
            ..
        }) = self.stack.last_mut()
        {
            *state = new;
        }
    }

    fn take_pending(&mut self) -> Option<Comment<'a>> {
        let src = self.src;
        let pending = self.pending();
        let comment = make_comment(src, pending.iter().copied());
        pending.clear();
        comment
    }

    /// Skip white space, line breaks, and comments inside a flow collection.
    /// Comments and blank lines are appended to the frame's pending lines.
    fn skip_flow_sep(&mut self, min_indent: usize) -> Result<()> {
        let mut line_start: Option<usize> = None;
        loop {
            match self.peek() {
                Some(b' ' | b'\t') => self.pos += 1,
                Some(b'\n' | b'\r') => {
                    if line_start.is_some() {
                        self.pending().push(None);
                    }
                    self.pos = self.after_break(self.pos);
                    line_start = Some(self.pos);
                }
                Some(b'#') => {
                    if let Some(ls) = line_start {
                        self.check_flow_indent(ls, min_indent)?;
                    } else if !matches!(self.b[self.pos - 1], b' ' | b'\t') {
                        return Err(self.syntax(
                            "`#` must be preceded by white space to start a comment",
                            self.pos,
                        ));
                    }
                    let range = self.comment_range(self.pos);
                    self.pending().push(Some(range));
                    self.pos = self.eol(self.pos);
                    line_start = None;
                }
                None => return Ok(()),
                Some(_) => {
                    if let Some(ls) = line_start {
                        self.check_flow_indent(ls, min_indent)?;
                    }
                    return Ok(());
                }
            }
        }
    }

    /// Check the indentation of a flow continuation line that starts at
    /// `line_start` and whose first non-white-space character is at the
    /// current position.
    fn check_flow_indent(&self, line_start: usize, min_indent: usize) -> Result<()> {
        let leading = &self.b[line_start..self.pos];
        if let Some(tab) = leading.iter().position(|&c| c == b'\t') {
            return Err(self.error(ErrorKind::TabIndent, line_start + tab, line_start + tab + 1));
        }
        if leading.len() < min_indent {
            return Err(self.syntax(
                format!(
                    "lines inside a flow collection must be indented at least {min_indent} spaces"
                ),
                self.pos,
            ));
        }
        Ok(())
    }

    /// After a flow entry: consume an optional `,` and an optional inline comment.
    fn flow_entry_end(&mut self) -> Result<(bool, Option<&'a str>)> {
        let mut ws_start = self.pos;
        self.skip_white();
        let had_comma = self.peek() == Some(b',');
        if had_comma {
            self.pos += 1;
            ws_start = self.pos;
            self.skip_white();
        }
        let mut inline = None;
        if self.peek() == Some(b'#') {
            if self.pos == ws_start {
                return Err(self.syntax(
                    "`#` must be preceded by white space to start a comment",
                    self.pos,
                ));
            }
            let (s, e) = self.comment_range(self.pos);
            inline = Some(&self.src[s..e]);
            self.pos = self.eol(self.pos);
        }
        Ok((had_comma, inline))
    }

    fn resume_flow(&mut self) -> Result<Event<'a>> {
        let (is_map, start, min_indent, state) = match self.stack.last().map(|f| &f.kind) {
            Some(FrameKind::Flow {
                is_map,
                start,
                min_indent,
                state,
                ..
            }) => (*is_map, *start, *min_indent, *state),
            _ => unreachable!("flow frame"),
        };
        let (close, what) = if is_map {
            (b'}', "in flow mapping (missing `}`)")
        } else {
            (b']', "in flow sequence (missing `]`)")
        };
        if state == FlowState::Value {
            return self.flow_map_value(min_indent);
        }
        if state == FlowState::Separator {
            self.skip_flow_sep(min_indent)?;
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(c) if c == close => {}
                None => return Err(self.eof(what, start)),
                Some(_) => {
                    let message = if is_map {
                        "expected `,` or `}`"
                    } else {
                        "expected `,` or `]`"
                    };
                    return Err(self.syntax(message, self.pos));
                }
            }
        }
        self.skip_flow_sep(min_indent)?;
        match self.peek() {
            None => return Err(self.eof(what, start)),
            Some(c) if c == close => return self.end_flow(),
            Some(b',') => return Err(self.syntax("unexpected `,` (empty entry)", self.pos)),
            _ => {}
        }
        if is_map {
            return self.flow_key(min_indent);
        }
        self.flow_entry_value(min_indent)
    }

    fn end_flow(&mut self) -> Result<Event<'a>> {
        self.pos += 1;
        let end = self.pos;
        self.last_end = end;
        let trailing = self.take_pending();
        self.pop();
        let inline = match self.stack.last().map(|f| &f.kind) {
            Some(FrameKind::Flow { .. }) => {
                let (had_comma, inline) = self.flow_entry_end()?;
                self.set_flow_state(FlowState::after_entry(had_comma));
                inline
            }
            _ => self.line_end()?,
        };
        Ok(Event::End {
            end,
            inline,
            trailing,
        })
    }

    /// Read a flow entry's value (a sequence entry or a mapping value).
    fn flow_entry_value(&mut self, min_indent: usize) -> Result<Event<'a>> {
        self.leading = self.take_pending();
        match self.peek() {
            Some(b'[') => self.begin_flow(false, min_indent),
            Some(b'{') => self.begin_flow(true, min_indent),
            _ => {
                let (value, span) = self.scalar(Context::Flow, min_indent)?;
                let (had_comma, inline) = self.flow_entry_end()?;
                self.set_flow_state(FlowState::after_entry(had_comma));
                Ok(Event::Scalar {
                    value,
                    span,
                    leading: self.leading.take(),
                    inline,
                })
            }
        }
    }

    fn flow_key(&mut self, min_indent: usize) -> Result<Event<'a>> {
        let start = self.pos;
        let (name, end) = if self.peek() == Some(b'"') {
            if self.src[start..].starts_with("\"\"\"") {
                return Err(self.syntax("triple-quoted strings cannot be mapping keys", start));
            }
            self.double_quoted(start)?
        } else {
            let Some(end) = scalar::scan_bare(self.src, start, Context::Flow) else {
                let ch = self.char_at(start).unwrap_or('?');
                let message = if ch == '\'' {
                    "single-quoted strings are not supported; use double quotes".to_owned()
                } else {
                    format!("unexpected `{ch}` where a mapping key was expected")
                };
                return Err(self.syntax(message, start));
            };
            (Cow::Borrowed(&self.src[start..end]), end)
        };
        self.pos = end;
        self.skip_flow_sep(min_indent)?;
        if self.peek() != Some(b':') {
            let message = if name.contains(':') && self.b[start] != b'"' {
                "expected `:` after mapping key (in a flow mapping, `:` must be followed by a space)"
            } else {
                "expected `:` after mapping key"
            };
            return Err(self.syntax(message, self.pos));
        }
        self.pos += 1;
        self.skip_flow_sep(min_indent)?;
        if matches!(self.peek(), Some(b',' | b'}')) {
            return Err(self.syntax(
                format!("missing value for key `{name}` (AYML has no empty values; write `null`)"),
                self.pos,
            ));
        }
        let span = Span::new(start, end);
        self.check_duplicate(&name, span)?;
        self.set_flow_state(FlowState::Value);
        Ok(Event::Key { name, span })
    }

    fn flow_map_value(&mut self, min_indent: usize) -> Result<Event<'a>> {
        if self.peek().is_none() {
            return Err(self.eof("where a value was expected", self.pos));
        }
        self.flow_entry_value(min_indent)
    }
}

/// Create a comment from gap lines.
fn make_comment(
    src: &str,
    lines: impl Iterator<Item = Option<(usize, usize)>> + Clone,
) -> Option<Comment<'_>> {
    if !lines.clone().any(|l| l.is_some()) {
        return None;
    }
    let mut lines: Vec<_> = lines.skip_while(Option::is_none).collect();
    let last = lines.iter().rposition(Option::is_some)?;
    lines.truncate(last + 1);
    Some(Comment(Box::new(CommentRepr::Lines { src, lines })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_stay_small() {
        assert!(
            std::mem::size_of::<Event<'_>>() <= 88,
            "{}",
            std::mem::size_of::<Event<'_>>()
        );
    }
}
