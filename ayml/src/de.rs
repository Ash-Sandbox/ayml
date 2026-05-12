use std::collections::HashSet;

use serde::de::{self, DeserializeOwned, Visitor};

use crate::error::{Error, Result};
use crate::read::{IoRead, Read, SliceRead};

/// Deserialize a `T` from a string of AYML text.
///
/// The bound `T: Deserialize<'a>` (rather than `DeserializeOwned`) allows
/// zero-copy deserialization of borrowed types like `&'a str`.
///
/// # Errors
///
/// Returns an error if the input is not valid AYML or cannot be
/// deserialized into `T`.
pub fn from_str<'a, T: serde::Deserialize<'a>>(s: &'a str) -> Result<T> {
    let mut de = Deserializer::from_str(s);
    let value = T::deserialize(&mut de)?;
    de.end()?;
    Ok(value)
}

/// Deserialize a `T` from a slice of AYML bytes.
///
/// AYML is UTF-8; this validates the input and then deserializes.
/// Supports borrowing from the input slice (`T: Deserialize<'a>`).
///
/// # Errors
///
/// Returns an error if the input is not valid UTF-8, not valid AYML,
/// or cannot be deserialized into `T`.
pub fn from_slice<'a, T: serde::Deserialize<'a>>(bytes: &'a [u8]) -> Result<T> {
    // Validate UTF-8 upfront
    let s = std::str::from_utf8(bytes)?;
    from_str(s)
}

/// Deserialize a `T` from an AYML reader.
///
/// Data is read lazily from the reader as the deserializer advances.
/// The deserialized value cannot borrow from the input; use [`from_str`]
/// or [`from_slice`] for zero-copy deserialization.
///
/// # Errors
///
/// Returns an error on I/O failure, invalid AYML, or if the data
/// cannot be deserialized into `T`.
pub fn from_reader<R: std::io::Read, T: DeserializeOwned>(rdr: R) -> Result<T> {
    let mut de = Deserializer::from_reader(rdr);
    let value = T::deserialize(&mut de)?;
    de.end()?;
    Ok(value)
}

// ── Parsing context ──────────────────────────────────────────────

/// Block vs flow parsing context, mirroring ayml-core's grammar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Context {
    Block,
    Flow,
}

/// How a mapping key was written in the source. Tracked so that downstream
/// code (key validation, the `PrescannedKey` path) can distinguish a quoted
/// key from a bare key without inspecting raw source bytes.
///
/// Triple-quoted scalars are intentionally not represented here: per the
/// AYML spec they span multiple lines and cannot appear as mapping keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MappingKeyKind {
    /// Unquoted (e.g. `foo`, `42`, `true`). Subject to scalar resolution.
    Bare,
    /// Double-quoted (e.g. `"foo"`). Always a string regardless of content.
    DoubleQuoted,
}

// ── Deserializer ─────────────────────────────────────────────────

/// Maximum nesting depth for collections to prevent stack overflow / OOM.
const MAX_DEPTH: usize = 64;

pub(crate) struct Deserializer<R> {
    read: R,
    ctx: Context,
    pending_top_comment: Option<String>,
    depth: usize,
    scratch: String,
    line: usize,
    line_start: usize,
}

impl<'a> Deserializer<SliceRead<'a>> {
    fn from_str(s: &'a str) -> Self {
        Self {
            read: SliceRead::new(s.as_bytes()),
            ctx: Context::Block,
            pending_top_comment: None,
            depth: 0,
            scratch: String::new(),
            line: 1,
            line_start: 0,
        }
    }
}

impl<R: std::io::Read> Deserializer<IoRead<R>> {
    fn from_reader(rdr: R) -> Self {
        Self {
            read: IoRead::new(rdr),
            ctx: Context::Block,
            pending_top_comment: None,
            depth: 0,
            scratch: String::new(),
            line: 1,
            line_start: 0,
        }
    }
}

// ── Character-level helpers ──────────────────────────────────────

impl<R: Read> Deserializer<R> {
    /// Consume the next byte.
    fn next_byte(&mut self) -> Result<Option<u8>> {
        self.read.next()
    }

    /// Peek at the next byte.
    fn peek(&mut self) -> Result<Option<u8>> {
        self.read.peek()
    }

    /// Peek at byte n positions ahead.
    fn peek_at(&mut self, n: usize) -> Result<Option<u8>> {
        self.read.peek_at(n)
    }

    fn is_eof(&mut self) -> Result<bool> {
        Ok(self.peek()?.is_none())
    }

    fn is_break_or_eof(&mut self) -> Result<bool> {
        match self.peek()? {
            None | Some(b'\n' | b'\r') => Ok(true),
            _ => Ok(false),
        }
    }

    fn skip_inline_whitespace(&mut self) -> Result<()> {
        while let Some(b' ' | b'\t') = self.peek()? {
            self.next_byte()?;
        }
        Ok(())
    }

    /// Read the rest of the line into a buffer.
    fn rest_of_line_into(&mut self, buf: &mut Vec<u8>) -> Result<()> {
        loop {
            match self.peek()? {
                Some(b'\n' | b'\r') | None => break,
                Some(b) => {
                    self.next_byte()?;
                    buf.push(b);
                }
            }
        }
        Ok(())
    }

    /// Consume a line break (\n, \r, or \r\n). Returns true if consumed.
    fn eat_break(&mut self) -> Result<bool> {
        match self.peek()? {
            Some(b'\r') => {
                self.next_byte()?;
                self.line += 1;
                if self.peek()? == Some(b'\n') {
                    self.next_byte()?;
                }
                self.line_start = self.read.byte_offset();
                Ok(true)
            }
            Some(b'\n') => {
                self.next_byte()?;
                self.line += 1;
                self.line_start = self.read.byte_offset();
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    fn byte_offset(&self) -> usize {
        self.read.byte_offset()
    }

    /// Current column (0-based indent) = distance from `line_start`.
    fn current_indent(&self) -> usize {
        self.read.byte_offset().saturating_sub(self.line_start)
    }

    // ── Depth tracking ────────────────────────────────────────────

    fn enter_collection(&mut self) -> Result<()> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            Err(self.error("nesting depth limit exceeded"))
        } else {
            Ok(())
        }
    }

    fn leave_collection(&mut self) {
        debug_assert!(
            self.depth > 0,
            "leave_collection called without matching enter_collection"
        );
        self.depth -= 1;
    }

    // ── Error helpers ────────────────────────────────────────────

    fn error(&self, msg: &str) -> Error {
        let col = self.read.byte_offset().saturating_sub(self.line_start) + 1;
        Error::Message(format!("{}:{}: {}", self.line, col, msg))
    }

    fn error_at(&self, msg: &str, offset: usize) -> Error {
        // For SliceRead we could compute exactly, but we keep it simple:
        // use current line tracking which is close enough (same as serde_json).
        // In practice error_at is called right after scanning, so line/col
        // are very close.
        let col = offset.saturating_sub(self.line_start) + 1;
        Error::Message(format!("{}:{}: {}", self.line, col, msg))
    }

    fn end(&mut self) -> Result<()> {
        self.skip_whitespace_and_comments()?;
        if self.is_eof()? {
            Ok(())
        } else {
            Err(self.error("trailing characters after value"))
        }
    }

    fn skip_whitespace_and_comments(&mut self) -> Result<()> {
        let mut seen_comment = false;
        let mut blank_lines = 0u32;
        loop {
            match self.peek()? {
                Some(b' ' | b'\t') => {
                    self.next_byte()?;
                }
                Some(b'\n' | b'\r') => {
                    self.eat_break()?;
                    if seen_comment {
                        blank_lines += 1;
                    }
                }
                Some(b'#') => {
                    if !seen_comment {
                        self.pending_top_comment = None;
                        seen_comment = true;
                    }
                    // Preserve blank lines between comment lines.
                    if blank_lines > 0 {
                        if let Some(existing) = &mut self.pending_top_comment {
                            for _ in 0..blank_lines {
                                existing.push('\n');
                            }
                        }
                        blank_lines = 0;
                    }
                    self.capture_comment_line()?;
                }
                _ => break,
            }
        }
        Ok(())
    }

    /// Consume a `# ...` comment line including the trailing break,
    /// appending the comment text to `pending_top_comment`.
    fn capture_comment_line(&mut self) -> Result<()> {
        self.next_byte()?; // skip '#'
        // Skip optional space after '#'
        if self.peek()? == Some(b' ') {
            self.next_byte()?;
        }
        let mut buf = Vec::new();
        self.rest_of_line_into(&mut buf)?;
        let text = String::from_utf8(buf)?;
        match &mut self.pending_top_comment {
            Some(existing) => {
                existing.push('\n');
                existing.push_str(&text);
            }
            None => {
                self.pending_top_comment = Some(text);
            }
        }
        self.eat_break()?;
        Ok(())
    }

    // ── Block-level helpers ────────────────────────────────────────

    fn eat(&mut self, expected: u8) -> Result<bool> {
        if self.peek()? == Some(expected) {
            self.next_byte()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Count leading spaces without consuming them (uses `peek_at`).
    ///
    /// Bounded to [`MAX_INDENT`] to prevent unbounded buffer growth on
    /// pathological input when using `IoRead`.
    fn count_spaces(&mut self) -> Result<usize> {
        const MAX_INDENT: usize = 1024;
        let mut count = 0;
        loop {
            if count >= MAX_INDENT {
                return Err(self.error("indentation exceeds maximum depth"));
            }
            match self.peek_at(count)? {
                Some(b' ') => count += 1,
                Some(b'\t') => {
                    // Consume the spaces we counted, then error at the tab position
                    for _ in 0..count {
                        self.next_byte()?;
                    }
                    return Err(self.error("tabs not allowed for indentation"));
                }
                _ => break,
            }
        }
        Ok(count)
    }

    /// Consume exactly `n` spaces. Returns false if not enough spaces.
    fn eat_spaces(&mut self, n: usize) -> Result<bool> {
        if n == 0 {
            return Ok(true);
        }
        for i in 0..n {
            match self.peek_at(i)? {
                Some(b' ') => {}
                _ => return Ok(false),
            }
        }
        for _ in 0..n {
            self.next_byte()?;
        }
        Ok(true)
    }

    fn skip_blank_lines(&mut self) -> Result<usize> {
        let mut count = 0;
        loop {
            // Peek ahead: is this line blank (only spaces/tabs then newline)?
            let mut i = 0;
            loop {
                match self.peek_at(i)? {
                    Some(b' ' | b'\t') => i += 1,
                    Some(b'\n' | b'\r') => {
                        // Blank line — consume everything including the break
                        for _ in 0..i {
                            self.next_byte()?;
                        }
                        self.eat_break()?;
                        count += 1;
                        break;
                    }
                    _ => return Ok(count),
                }
            }
        }
    }

    fn skip_block_gaps(&mut self) -> Result<()> {
        let mut has_comment = false;
        loop {
            let saved_offset = self.byte_offset();
            let blanks = self.skip_blank_lines()?;
            // Check for comment lines
            let spaces = self.count_spaces()?;
            if let Some(b'#') = self.peek_at(spaces)? {
                // Preserve blank lines between comment lines.
                if has_comment
                    && blanks > 0
                    && let Some(existing) = &mut self.pending_top_comment
                {
                    for _ in 0..blanks {
                        existing.push('\n');
                    }
                }
                has_comment = true;
                // Consume the spaces and the comment
                for _ in 0..spaces {
                    self.next_byte()?;
                }
                self.capture_comment_line()?;
                continue;
            }
            if self.byte_offset() == saved_offset {
                break;
            }
        }
        Ok(())
    }

    /// Skip from current position to the start of the next line.
    /// Consumes inline whitespace, optional inline comment, and the line break.
    /// If the newline was already consumed (by a nested value), this is a no-op.
    /// Does NOT consume any indentation on the next line.
    fn skip_to_next_line(&mut self) -> Result<()> {
        // If we're already at a newline or BOL, just eat the break
        if self.is_break_or_eof()? {
            self.eat_break()?;
            return Ok(());
        }
        // Peek to find out what's ahead: inline ws, then possible #comment, then break
        // We need to consume everything up to and including the break.
        // But if there's no break (nested value already consumed it), we stop.
        let mut i = 0;
        loop {
            match self.peek_at(i)? {
                Some(b' ' | b'\t') => i += 1,
                Some(b'#') => {
                    // Comment — find the end of line
                    loop {
                        i += 1;
                        match self.peek_at(i)? {
                            Some(b'\n' | b'\r') | None => break,
                            _ => {}
                        }
                    }
                    break;
                }
                Some(b'\n' | b'\r') => break,
                None => return Ok(()), // EOF
                _ => {
                    // Non-whitespace, non-comment, non-break: the newline
                    // was already consumed by a nested value. Don't consume.
                    return Ok(());
                }
            }
        }
        // Consume everything we peeked past (inline ws + comment text)
        for _ in 0..i {
            self.next_byte()?;
        }
        // Consume the break
        self.eat_break()?;
        Ok(())
    }

    fn is_mapping_value_indicator(&mut self) -> Result<bool> {
        if self.peek()? != Some(b':') {
            return Ok(false);
        }
        let next = self.peek_at(1)?;
        Ok(next.is_none()
            || next == Some(b' ')
            || next == Some(b'\t')
            || next == Some(b'\n')
            || next == Some(b'\r'))
    }

    // ── Scalar scanning ──────────────────────────────────────────

    fn scan_quoted_string(&mut self) -> Result<String> {
        // Check for triple-quote
        if self.peek_at(1)? == Some(b'"') && self.peek_at(2)? == Some(b'"') {
            let after = self.peek_at(3)?;
            if after == Some(b'\n') || after == Some(b'\r') || after.is_none() {
                return self.scan_triple_quoted();
            }
        }
        self.scan_double_quoted()
    }

    fn scan_double_quoted(&mut self) -> Result<String> {
        let start_offset = self.byte_offset();
        self.next_byte()?; // opening `"`
        self.scratch.clear();

        loop {
            match self.peek()? {
                Some(b'"') => {
                    self.next_byte()?;
                    return Ok(std::mem::take(&mut self.scratch));
                }
                Some(b'\\') => {
                    self.next_byte()?;
                    let ch = self.parse_escape()?;
                    self.scratch.push(ch);
                }
                Some(b'\n' | b'\r') => {
                    return Err(
                        self.error_at("expected closing `\"` before line break", start_offset)
                    );
                }
                Some(b) => {
                    // Check for non-printable ASCII control chars
                    if b < 0x20 && b != b'\t' {
                        return Err(
                            self.error(&format!("non-printable character U+{:04X}", u32::from(b)))
                        );
                    }
                    self.next_byte()?;
                    if b >= 0x80 {
                        self.read_utf8_char(b)?;
                    } else {
                        self.scratch.push(b as char);
                    }
                }
                None => {
                    return Err(self.error_at("unexpected end of input in string", start_offset));
                }
            }
        }
    }

    /// Read a multi-byte UTF-8 character given its leading byte, validate it,
    /// and push it to scratch. The leading byte must NOT have been pushed yet.
    fn read_utf8_char(&mut self, leading: u8) -> Result<()> {
        let n = if leading & 0xE0 == 0xC0 {
            1
        } else if leading & 0xF0 == 0xE0 {
            2
        } else if leading & 0xF8 == 0xF0 {
            3
        } else {
            return Err(self.error("invalid UTF-8 byte"));
        };
        let mut buf = [0u8; 4];
        buf[0] = leading;
        for i in 0..n {
            match self.next_byte()? {
                Some(b) if b & 0xC0 == 0x80 => {
                    buf[i + 1] = b;
                }
                _ => return Err(self.error("invalid UTF-8 continuation")),
            }
        }
        let s = std::str::from_utf8(&buf[..=n])?;
        let ch = s.chars().next().unwrap();
        if !is_printable(ch) {
            return Err(self.error(&format!("non-printable character U+{:04X}", ch as u32)));
        }
        self.scratch.push(ch);
        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    fn scan_triple_quoted(&mut self) -> Result<String> {
        let start_offset = self.byte_offset();
        // Consume opening `"""`
        self.next_byte()?;
        self.next_byte()?;
        self.next_byte()?;

        // Must be followed by a line break
        if !self.eat_break()? {
            return Err(self.error_at("expected line break after opening `\"\"\"`", start_offset));
        }

        // Collect raw content lines until closing `"""`
        let mut raw_lines: Vec<String> = Vec::new();
        let closing_indent;

        loop {
            if self.is_eof()? {
                return Err(self.error_at(
                    "unexpected end of input in triple-quoted string",
                    start_offset,
                ));
            }

            // Count leading spaces (without consuming)
            let spaces = self.count_spaces()?;

            // Check if this line is the closing `"""`
            if self.peek_at(spaces)? == Some(b'"')
                && self.peek_at(spaces + 1)? == Some(b'"')
                && self.peek_at(spaces + 2)? == Some(b'"')
            {
                let after = self.peek_at(spaces + 3)?;
                if after.is_none()
                    || after == Some(b'\n')
                    || after == Some(b'\r')
                    || after == Some(b' ')
                    || after == Some(b'\t')
                    || after == Some(b'#')
                    || after == Some(b',')
                    || after == Some(b']')
                    || after == Some(b'}')
                {
                    closing_indent = spaces;
                    // Consume spaces + `"""`
                    for _ in 0..spaces + 3 {
                        self.next_byte()?;
                    }
                    break;
                }
            }

            // Content line: read entire raw line (including leading spaces)
            let mut line_buf = Vec::new();
            self.rest_of_line_into(&mut line_buf)?;
            let line = String::from_utf8(line_buf)?;
            raw_lines.push(line);

            if !self.eat_break()? && !self.is_eof()? {
                return Err(self.error("expected line break in triple-quoted string"));
            }
        }

        // Process: strip indentation, handle escapes, line continuations.
        let mut result = String::new();
        let mut continuation = false;
        for (i, line) in raw_lines.iter().enumerate() {
            if i > 0 && !continuation {
                result.push('\n');
            }
            continuation = false;

            let stripped = if line.len() >= closing_indent
                && line.as_bytes()[..closing_indent].iter().all(|&b| b == b' ')
            {
                &line[closing_indent..]
            } else if line.trim().is_empty() {
                ""
            } else {
                return Err(self.error(&format!(
                    "insufficient indentation in triple-quoted string (expected at least {closing_indent} spaces)"
                )));
            };

            let mut chars = stripped.chars().peekable();
            while let Some(ch) = chars.next() {
                if ch == '\\' {
                    if chars.peek().is_none() {
                        continuation = true;
                        continue;
                    }
                    let decoded = decode_escape_char(&mut chars)
                        .map_err(|msg| self.error_at(&msg, start_offset))?;
                    result.push(decoded);
                } else {
                    result.push(ch);
                }
            }
        }

        Ok(result)
    }

    /// Parse a double-quoted escape sequence (after consuming the `\`).
    fn parse_escape(&mut self) -> Result<char> {
        let esc_start = self.byte_offset().saturating_sub(1);
        match self.next_byte()? {
            Some(b'0') => Ok('\0'),
            Some(b'a') => Ok('\x07'),
            Some(b'b') => Ok('\x08'),
            Some(b't') => Ok('\t'),
            Some(b'n') => Ok('\n'),
            Some(b'v') => Ok('\x0B'),
            Some(b'f') => Ok('\x0C'),
            Some(b'r') => Ok('\r'),
            Some(b'e') => Ok('\x1B'),
            Some(b' ') => Ok(' '),
            Some(b'"') => Ok('"'),
            Some(b'/') => Ok('/'),
            Some(b'\\') => Ok('\\'),
            Some(b'x') => self.parse_hex_escape(2),
            Some(b'u') => self.parse_hex_escape(4),
            Some(b'U') => self.parse_hex_escape(8),
            Some(b) => {
                let ch = b as char;
                Err(self.error_at(&format!("invalid escape: \\{ch}"), esc_start))
            }
            None => Err(self.error_at("unexpected end of input in escape", esc_start)),
        }
    }

    fn parse_hex_escape(&mut self, digits: usize) -> Result<char> {
        let start = self.byte_offset();
        let mut value: u32 = 0;
        for _ in 0..digits {
            match self.next_byte()? {
                Some(b) if (b as char).is_ascii_hexdigit() => {
                    value = value * 16 + (b as char).to_digit(16).unwrap();
                }
                _ => {
                    return Err(self.error_at(&format!("expected {digits} hex digits"), start));
                }
            }
        }
        char::from_u32(value).ok_or_else(|| {
            self.error_at(&format!("invalid unicode code point U+{value:04X}"), start)
        })
    }

    /// Scan a bare (unquoted) scalar string.
    fn scan_bare_string(&mut self, ctx: Context) -> Result<String> {
        self.scratch.clear();

        // ns-plain-first-char: `-` and `:` require a following ns-char
        match self.peek()? {
            Some(b'-' | b':') => {
                let next = self.peek_at(1)?;
                match next {
                    Some(b) if !is_ascii_whitespace(b) && is_printable_byte_start(b) => {
                        let b0 = self.next_byte()?.unwrap();
                        self.scratch.push(b0 as char);
                    }
                    _ => {
                        return Err(self.error("unexpected character"));
                    }
                }
            }
            Some(b) if is_plain_first_byte(b) => {
                self.next_byte()?;
                if b >= 0x80 {
                    self.read_utf8_char(b)?;
                } else {
                    self.scratch.push(b as char);
                }
            }
            Some(b) => {
                // Try to decode as UTF-8 char for error message
                let ch = if b < 0x80 { b as char } else { '?' };
                return Err(self.error(&format!("unexpected character `{ch}`")));
            }
            None => {
                return Err(self.error("unexpected end of input"));
            }
        }

        loop {
            // Track where trailing whitespace starts in scratch
            let ws_mark = self.scratch.len();

            // Accumulate inline whitespace into scratch
            while let Some(b' ' | b'\t') = self.peek()? {
                let b = self.next_byte()?.unwrap();
                self.scratch.push(b as char);
            }

            if self.is_break_or_eof()? {
                self.scratch.truncate(ws_mark);
                return Ok(std::mem::take(&mut self.scratch));
            }

            match self.peek()? {
                Some(b'#') => {
                    // '#' terminates bare strings; drop trailing ws
                    self.scratch.truncate(ws_mark);
                    return Ok(std::mem::take(&mut self.scratch));
                }
                Some(b':') => {
                    let next = self.peek_at(1)?;
                    if next.is_none()
                        || next == Some(b' ')
                        || next == Some(b'\t')
                        || next == Some(b'\n')
                        || next == Some(b'\r')
                    {
                        // Mapping value indicator — drop trailing ws
                        self.scratch.truncate(ws_mark);
                        return Ok(std::mem::take(&mut self.scratch));
                    }
                    self.next_byte()?;
                    self.scratch.push(':');
                }
                Some(b',' | b']' | b'}') if ctx == Context::Flow => {
                    self.scratch.truncate(ws_mark);
                    return Ok(std::mem::take(&mut self.scratch));
                }
                Some(b) if b < 0x20 && b != b'\t' => {
                    return Err(
                        self.error(&format!("non-printable character U+{:04X}", u32::from(b)))
                    );
                }
                Some(b) => {
                    self.next_byte()?;
                    if b >= 0x80 {
                        self.read_utf8_char(b)?;
                    } else {
                        self.scratch.push(b as char);
                    }
                }
                None => {
                    self.scratch.truncate(ws_mark);
                    return Ok(std::mem::take(&mut self.scratch));
                }
            }
        }
    }

    fn scan_scalar_string(&mut self, ctx: Context) -> Result<String> {
        match self.peek()? {
            Some(b'"') => self.scan_quoted_string(),
            _ => self.scan_bare_string(ctx),
        }
    }

    fn parse_bool(&mut self) -> Result<bool> {
        let text = self.scan_bare_string(self.ctx)?;
        match text.as_str() {
            "true" => Ok(true),
            "false" => Ok(false),
            _ => Err(self.error(&format!("expected boolean, got `{text}`"))),
        }
    }

    fn parse_int<T: TryFrom<i64>>(&mut self) -> Result<T>
    where
        T::Error: std::fmt::Display,
    {
        let start = self.byte_offset();
        let text = self.scan_bare_string(self.ctx)?;
        let i = try_parse_int(&text).map_err(|()| self.error_at("integer overflow", start))?;
        let i = i.ok_or_else(|| self.error(&format!("expected integer, got `{text}`")))?;
        T::try_from(i).map_err(|e| self.error(&format!("integer out of range: {e}")))
    }

    fn parse_float<'de, V: Visitor<'de>>(&mut self, visitor: V) -> Result<V::Value> {
        let text = self.scan_bare_string(self.ctx)?;
        if let Some(f) = try_parse_float(&text) {
            visitor.visit_f64(f)
        } else if let Ok(Some(i)) = try_parse_int(&text) {
            #[allow(clippy::cast_precision_loss)]
            visitor.visit_f64(i as f64)
        } else {
            Err(self.error(&format!("expected float, got `{text}`")))
        }
    }

    fn deserialize_commented<'de, V: de::Visitor<'de>>(&mut self, visitor: V) -> Result<V::Value> {
        self.skip_whitespace_and_comments()?;
        let top_comment = self.pending_top_comment.take();
        let saved_line = self.line;
        visitor.visit_map(CommentedAccess {
            de: self,
            top_comment,
            state: CommentedState::TopComment,
            value_start_line: saved_line,
        })
    }

    fn capture_inline_comment(&mut self) -> Result<Option<String>> {
        self.skip_inline_whitespace()?;
        if self.peek()? == Some(b'#') {
            self.next_byte()?; // skip '#'
            if self.peek()? == Some(b' ') {
                self.next_byte()?; // skip space
            }
            let mut buf = Vec::new();
            self.rest_of_line_into(&mut buf)?;
            let text = String::from_utf8(buf)?;
            Ok(Some(text))
        } else {
            Ok(None)
        }
    }

    /// Check if the upcoming bytes spell "null" followed by a terminator.
    fn is_null_ahead(&mut self) -> Result<bool> {
        Ok(self.peek_at(0)? == Some(b'n')
            && self.peek_at(1)? == Some(b'u')
            && self.peek_at(2)? == Some(b'l')
            && self.peek_at(3)? == Some(b'l')
            && matches!(
                self.peek_at(4)?,
                None | Some(b' ' | b'\t' | b'\n' | b'\r' | b'#' | b',' | b']' | b'}')
            ))
    }
}

impl<'de, R: Read> de::Deserializer<'de> for &mut Deserializer<R> {
    type Error = Error;

    #[allow(clippy::too_many_lines)]
    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.skip_whitespace_and_comments()?;
        match self.peek()? {
            Some(b'"') => {
                let start_indent = self.current_indent();
                let s = self.scan_quoted_string()?;
                if self.ctx == Context::Block {
                    self.skip_inline_whitespace()?;
                    if self.is_mapping_value_indicator()? {
                        self.next_byte()?; // consume ':'
                        self.skip_inline_whitespace()?;
                        self.enter_collection()?;
                        let key = PrescannedKey {
                            text: s,
                            kind: MappingKeyKind::DoubleQuoted,
                        };
                        let value = visitor.visit_map(MapAccess::with_prescanned_key(
                            self,
                            MapStyle::Block(start_indent),
                            key,
                        ));
                        self.leave_collection();
                        return value;
                    }
                }
                visitor.visit_string(s)
            }
            Some(b'[') => {
                self.next_byte()?;
                let prev_ctx = self.ctx;
                self.enter_collection()?;
                let result = visitor.visit_seq(SeqAccess::new(self, SeqStyle::Flow));
                self.leave_collection();
                self.ctx = prev_ctx;
                let value = result?;
                self.skip_whitespace_and_comments()?;
                if !self.eat(b']')? {
                    return Err(self.error("expected `]` to close sequence"));
                }
                Ok(value)
            }
            Some(b'{') => {
                self.next_byte()?;
                let prev_ctx = self.ctx;
                self.enter_collection()?;
                let result = visitor.visit_map(MapAccess::new(self, MapStyle::Flow));
                self.leave_collection();
                self.ctx = prev_ctx;
                let value = result?;
                self.skip_whitespace_and_comments()?;
                if !self.eat(b'}')? {
                    return Err(self.error("expected `}` to close mapping"));
                }
                Ok(value)
            }
            Some(b'-') if self.ctx == Context::Block && self.peek_at(1)? == Some(b' ') => {
                let indent = self.current_indent();
                self.enter_collection()?;
                let value = visitor.visit_seq(SeqAccess::new(self, SeqStyle::Block(indent)));
                self.leave_collection();
                value
            }
            Some(_) => {
                let start_indent = self.current_indent();
                let start_offset = self.byte_offset();
                let text = self.scan_bare_string(self.ctx)?;
                if self.ctx == Context::Block {
                    self.skip_inline_whitespace()?;
                    if self.is_mapping_value_indicator()? {
                        self.next_byte()?; // consume ':'
                        self.skip_inline_whitespace()?;
                        self.enter_collection()?;
                        let key = PrescannedKey {
                            text,
                            kind: MappingKeyKind::Bare,
                        };
                        let value = visitor.visit_map(MapAccess::with_prescanned_key(
                            self,
                            MapStyle::Block(start_indent),
                            key,
                        ));
                        self.leave_collection();
                        return value;
                    }
                }
                match text.as_str() {
                    "null" => visitor.visit_unit(),
                    "true" => visitor.visit_bool(true),
                    "false" => visitor.visit_bool(false),
                    _ => match try_parse_int(&text) {
                        Ok(Some(i)) => visitor.visit_i64(i),
                        Err(()) => Err(self.error_at("integer overflow", start_offset)),
                        Ok(None) => {
                            if let Some(f) = try_parse_float(&text) {
                                visitor.visit_f64(f)
                            } else {
                                visitor.visit_string(text)
                            }
                        }
                    },
                }
            }
            None => Err(self.error("unexpected end of input")),
        }
    }

    fn deserialize_bool<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.skip_whitespace_and_comments()?;
        visitor.visit_bool(self.parse_bool()?)
    }

    fn deserialize_i8<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.skip_whitespace_and_comments()?;
        visitor.visit_i8(self.parse_int()?)
    }

    fn deserialize_i16<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.skip_whitespace_and_comments()?;
        visitor.visit_i16(self.parse_int()?)
    }

    fn deserialize_i32<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.skip_whitespace_and_comments()?;
        visitor.visit_i32(self.parse_int()?)
    }

    fn deserialize_i64<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.skip_whitespace_and_comments()?;
        visitor.visit_i64(self.parse_int()?)
    }

    fn deserialize_u8<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.skip_whitespace_and_comments()?;
        visitor.visit_u8(self.parse_int()?)
    }

    fn deserialize_u16<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.skip_whitespace_and_comments()?;
        visitor.visit_u16(self.parse_int()?)
    }

    fn deserialize_u32<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.skip_whitespace_and_comments()?;
        visitor.visit_u32(self.parse_int()?)
    }

    fn deserialize_u64<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.skip_whitespace_and_comments()?;
        let start = self.byte_offset();
        let text = self.scan_bare_string(self.ctx)?;
        let val: u64 = parse_unsigned(&text).ok_or_else(|| {
            self.error_at(&format!("expected unsigned integer, got `{text}`"), start)
        })?;
        visitor.visit_u64(val)
    }

    fn deserialize_f32<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.skip_whitespace_and_comments()?;
        self.parse_float(visitor)
    }

    fn deserialize_f64<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.skip_whitespace_and_comments()?;
        self.parse_float(visitor)
    }

    fn deserialize_char<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.skip_whitespace_and_comments()?;
        let s = self.scan_scalar_string(self.ctx)?;
        let mut chars = s.chars();
        let ch = chars
            .next()
            .ok_or_else(|| self.error("expected a character, got empty string"))?;
        if chars.next().is_some() {
            return Err(self.error("expected a single character"));
        }
        visitor.visit_char(ch)
    }

    fn deserialize_str<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.skip_whitespace_and_comments()?;
        let s = self.scan_scalar_string(self.ctx)?;
        visitor.visit_string(s)
    }

    fn deserialize_string<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.deserialize_str(visitor)
    }

    fn deserialize_bytes<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.skip_whitespace_and_comments()?;
        let s = self.scan_scalar_string(self.ctx)?;
        visitor.visit_bytes(s.as_bytes())
    }

    fn deserialize_byte_buf<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.deserialize_bytes(visitor)
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.skip_whitespace_and_comments()?;
        if self.is_null_ahead()? {
            // Consume "null"
            self.next_byte()?;
            self.next_byte()?;
            self.next_byte()?;
            self.next_byte()?;
            return visitor.visit_none();
        }
        visitor.visit_some(self)
    }

    fn deserialize_unit<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.skip_whitespace_and_comments()?;
        let text = self.scan_bare_string(self.ctx)?;
        if text == "null" {
            visitor.visit_unit()
        } else {
            Err(self.error(&format!("expected null, got `{text}`")))
        }
    }

    fn deserialize_unit_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value> {
        self.deserialize_unit(visitor)
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value> {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_seq<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.skip_whitespace_and_comments()?;
        self.enter_collection()?;
        match self.peek()? {
            Some(b'[') => {
                self.next_byte()?;
                let prev_ctx = self.ctx;
                let result = visitor.visit_seq(SeqAccess::new(self, SeqStyle::Flow));
                self.leave_collection();
                self.ctx = prev_ctx;
                let value = result?;
                self.skip_whitespace_and_comments()?;
                if !self.eat(b']')? {
                    return Err(self.error("expected `]` to close sequence"));
                }
                Ok(value)
            }
            Some(b'-') if self.peek_at(1)? == Some(b' ') => {
                let indent = self.current_indent();
                let value = visitor.visit_seq(SeqAccess::new(self, SeqStyle::Block(indent)));
                self.leave_collection();
                value
            }
            _ => {
                self.leave_collection();
                Err(self.error("expected sequence (`[` or `- `)"))
            }
        }
    }

    fn deserialize_tuple<V: Visitor<'de>>(self, _len: usize, visitor: V) -> Result<V::Value> {
        self.deserialize_seq(visitor)
    }

    fn deserialize_tuple_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value> {
        self.deserialize_seq(visitor)
    }

    fn deserialize_map<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.skip_whitespace_and_comments()?;
        self.enter_collection()?;
        if let Some(b'{') = self.peek()? {
            self.next_byte()?;
            let prev_ctx = self.ctx;
            let result = visitor.visit_map(MapAccess::new(self, MapStyle::Flow));
            self.leave_collection();
            self.ctx = prev_ctx;
            let value = result?;
            self.skip_whitespace_and_comments()?;
            if !self.eat(b'}')? {
                return Err(self.error("expected `}` to close mapping"));
            }
            Ok(value)
        } else {
            let indent = self.current_indent();
            let value = visitor.visit_map(MapAccess::new(self, MapStyle::Block(indent)));
            self.leave_collection();
            value
        }
    }

    fn deserialize_struct<V: Visitor<'de>>(
        self,
        name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value> {
        if name == crate::commented::COMMENTED_STRUCT {
            return self.deserialize_commented(visitor);
        }
        self.deserialize_map(visitor)
    }

    fn deserialize_enum<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value> {
        self.skip_whitespace_and_comments()?;
        if let Some(b'{') = self.peek()? {
            self.next_byte()?;
            let prev_ctx = self.ctx;
            self.ctx = Context::Flow;
            self.enter_collection()?;
            self.skip_whitespace_and_comments()?;
            let result = visitor.visit_enum(EnumAccess {
                de: self,
                style: EnumStyle::Mapping,
                prescanned_variant: None,
            });
            self.leave_collection();
            self.ctx = prev_ctx;
            let value = result?;
            self.skip_whitespace_and_comments()?;
            if !self.eat(b'}')? {
                return Err(self.error("expected `}` to close enum mapping"));
            }
            Ok(value)
        } else {
            let text = self.scan_scalar_string(self.ctx)?;
            self.skip_inline_whitespace()?;
            if self.is_mapping_value_indicator()? {
                self.next_byte()?; // consume ':'
                self.skip_inline_whitespace()?;
                self.enter_collection()?;
                let value = visitor.visit_enum(EnumAccess {
                    de: self,
                    style: EnumStyle::Mapping,
                    prescanned_variant: Some(text),
                });
                self.leave_collection();
                value
            } else {
                visitor.visit_enum(EnumAccess {
                    de: self,
                    style: EnumStyle::Unit,
                    prescanned_variant: Some(text),
                })
            }
        }
    }

    fn deserialize_identifier<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.deserialize_str(visitor)
    }

    fn deserialize_ignored_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.deserialize_any(visitor)
    }
}

// ── SeqAccess ────────────────────────────────────────────────────

enum SeqStyle {
    Flow,
    Block(usize),
}

struct SeqAccess<'a, R> {
    de: &'a mut Deserializer<R>,
    style: SeqStyle,
    first: bool,
}

impl<'a, R> SeqAccess<'a, R> {
    fn new(de: &'a mut Deserializer<R>, style: SeqStyle) -> Self {
        Self {
            de,
            style,
            first: true,
        }
    }
}

impl<'de, R: Read> de::SeqAccess<'de> for SeqAccess<'_, R> {
    type Error = Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>>
    where
        T: de::DeserializeSeed<'de>,
    {
        match &self.style {
            SeqStyle::Flow => {
                self.de.ctx = Context::Flow;
                self.de.skip_whitespace_and_comments()?;
                if self.de.peek()? == Some(b']') {
                    return Ok(None);
                }
                if !self.first {
                    if !self.de.eat(b',')? {
                        return Err(self.de.error("expected `,` or `]`"));
                    }
                    self.de.skip_whitespace_and_comments()?;
                    if self.de.peek()? == Some(b']') {
                        return Ok(None); // trailing comma
                    }
                }
                self.first = false;
                seed.deserialize(&mut *self.de).map(Some)
            }
            SeqStyle::Block(indent) => {
                let indent = *indent;
                if self.first {
                    self.first = false;
                    if !self.de.eat(b'-')? || self.de.peek()? != Some(b' ') {
                        return Err(self.de.error("expected `- `"));
                    }
                    self.de.next_byte()?; // eat space
                    return seed.deserialize(&mut *self.de).map(Some);
                }

                // Between entries: finish current line, move to next.
                // Use peek-based approach to avoid consuming indentation.
                self.de.skip_to_next_line()?;
                self.de.skip_block_gaps()?;

                if self.de.is_eof()? {
                    return Ok(None);
                }

                let spaces = self.de.count_spaces()?;
                if spaces != indent {
                    return Ok(None);
                }
                // Check for `- ` at position indent BEFORE consuming spaces
                if self.de.peek_at(indent)? != Some(b'-')
                    || self.de.peek_at(indent + 1)? != Some(b' ')
                {
                    // Not a sequence entry — don't consume the indent
                    return Ok(None);
                }
                // Now consume indent + `- `
                for _ in 0..indent + 2 {
                    self.de.next_byte()?;
                }
                seed.deserialize(&mut *self.de).map(Some)
            }
        }
    }
}

// ── MapAccess ────────────────────────────────────────────────────

enum MapStyle {
    Flow,
    Block(usize),
}

struct MapAccess<'a, R> {
    de: &'a mut Deserializer<R>,
    style: MapStyle,
    first: bool,
    seen_keys: HashSet<String>,
    prescanned_first_key: Option<PrescannedKey>,
}

struct PrescannedKey {
    text: String,
    kind: MappingKeyKind,
}

impl<'a, R> MapAccess<'a, R> {
    fn new(de: &'a mut Deserializer<R>, style: MapStyle) -> Self {
        Self {
            de,
            style,
            first: true,
            seen_keys: HashSet::new(),
            prescanned_first_key: None,
        }
    }

    fn with_prescanned_key(
        de: &'a mut Deserializer<R>,
        style: MapStyle,
        key: PrescannedKey,
    ) -> Self {
        Self {
            de,
            style,
            first: true,
            seen_keys: HashSet::new(),
            prescanned_first_key: Some(key),
        }
    }
}

impl<R: Read> MapAccess<'_, R> {
    /// Scan the next key from `self.de`, returning the parsed text and the
    /// scalar kind. AYML keys are either double-quoted strings or bare
    /// scalars (which may resolve to int/bool/string).
    fn scan_key(&mut self) -> Result<(String, MappingKeyKind)> {
        if self.de.peek()? == Some(b'"') {
            let s = self.de.scan_quoted_string()?;
            Ok((s, MappingKeyKind::DoubleQuoted))
        } else {
            let s = self.de.scan_bare_string(self.de.ctx)?;
            Ok((s, MappingKeyKind::Bare))
        }
    }

    /// Reject keys whose bare form resolves to `null` or a float, per AYML spec.
    /// Double-quoted keys are always strings and need no validation.
    fn validate_key(&self, text: &str, kind: MappingKeyKind) -> Result<()> {
        if kind == MappingKeyKind::DoubleQuoted {
            return Ok(());
        }
        if text == "null" {
            return Err(self
                .de
                .error("unquoted `null` is not allowed as a mapping key; use \"null\""));
        }
        if try_parse_float(text).is_some() {
            return Err(self.de.error(&format!(
                "unquoted `{text}` resolves to float and is not allowed as a mapping key; quote it"
            )));
        }
        Ok(())
    }

}

impl<'de, R: Read> de::MapAccess<'de> for MapAccess<'_, R> {
    type Error = Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>>
    where
        K: de::DeserializeSeed<'de>,
    {
        match &self.style {
            MapStyle::Flow => {
                self.de.ctx = Context::Flow;
                self.de.skip_whitespace_and_comments()?;
                if self.de.peek()? == Some(b'}') {
                    return Ok(None);
                }
                if !self.first {
                    if !self.de.eat(b',')? {
                        return Err(self.de.error("expected `,` or `}`"));
                    }
                    self.de.skip_whitespace_and_comments()?;
                    if self.de.peek()? == Some(b'}') {
                        return Ok(None);
                    }
                }
                self.first = false;

                let (text, kind) = self.scan_key()?;
                self.validate_key(&text, kind)?;
                if !self.seen_keys.insert(text.clone()) {
                    return Err(self.de.error(&format!("duplicate key `{text}`")));
                }
                self.de.skip_whitespace_and_comments()?;
                if !self.de.eat(b':')? {
                    return Err(self.de.error("expected `:` after mapping key"));
                }
                self.de.skip_whitespace_and_comments()?;
                Ok(Some(seed.deserialize(KeyDeserializer { text, kind })?))
            }
            MapStyle::Block(indent) => {
                let indent = *indent;
                if self.first {
                    self.first = false;
                    if let Some(key) = self.prescanned_first_key.take() {
                        // The colon and post-colon whitespace were already
                        // consumed by deserialize_any before MapAccess was
                        // created, so we go straight to seed dispatch.
                        self.validate_key(&key.text, key.kind)?;
                        if !self.seen_keys.insert(key.text.clone()) {
                            return Err(self.de.error(&format!("duplicate key `{}`", key.text)));
                        }
                        let val = seed.deserialize(KeyDeserializer {
                            text: key.text,
                            kind: key.kind,
                        })?;
                        return Ok(Some(val));
                    }
                } else {
                    // Between entries
                    self.de.skip_to_next_line()?;
                    self.de.skip_block_gaps()?;

                    if self.de.is_eof()? {
                        return Ok(None);
                    }

                    let spaces = self.de.count_spaces()?;
                    if spaces != indent {
                        return Ok(None);
                    }
                    // Check for sequence indicator BEFORE consuming indent
                    if self.de.peek_at(indent)? == Some(b'-')
                        && self.de.peek_at(indent + 1)? == Some(b' ')
                    {
                        // Not a mapping entry — it's a sequence entry at this indent
                        return Ok(None);
                    }
                    self.de.eat_spaces(indent)?;
                }

                let (text, kind) = self.scan_key()?;
                self.validate_key(&text, kind)?;
                if !self.seen_keys.insert(text.clone()) {
                    return Err(self.de.error(&format!("duplicate key `{text}`")));
                }
                self.de.skip_inline_whitespace()?;
                if !self.de.eat(b':')? {
                    return Err(self.de.error("expected `:` after mapping key"));
                }
                self.de.skip_inline_whitespace()?;
                Ok(Some(seed.deserialize(KeyDeserializer { text, kind })?))
            }
        }
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value>
    where
        V: de::DeserializeSeed<'de>,
    {
        seed.deserialize(&mut *self.de)
    }
}

// ── CommentedAccess (virtual MapAccess for Commented<T>) ──────

enum CommentedState {
    TopComment,
    Value,
    InlineComment,
    Done,
}

struct CommentedAccess<'a, R> {
    de: &'a mut Deserializer<R>,
    top_comment: Option<String>,
    state: CommentedState,
    value_start_line: usize,
}

impl<'de, R: Read> de::MapAccess<'de> for CommentedAccess<'_, R> {
    type Error = Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>>
    where
        K: de::DeserializeSeed<'de>,
    {
        use crate::commented::{FIELD_INLINE_COMMENT, FIELD_TOP_COMMENT, FIELD_VALUE};
        let key = match self.state {
            CommentedState::TopComment => FIELD_TOP_COMMENT,
            CommentedState::Value => FIELD_VALUE,
            CommentedState::InlineComment => FIELD_INLINE_COMMENT,
            CommentedState::Done => return Ok(None),
        };
        seed.deserialize(de::value::BorrowedStrDeserializer::new(key))
            .map(Some)
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value>
    where
        V: de::DeserializeSeed<'de>,
    {
        match self.state {
            CommentedState::TopComment => {
                self.state = CommentedState::Value;
                let comment = self.top_comment.take();
                seed.deserialize(OptionStringDeserializer(comment))
            }
            CommentedState::Value => {
                self.state = CommentedState::InlineComment;
                self.value_start_line = self.de.line;
                seed.deserialize(&mut *self.de)
            }
            CommentedState::InlineComment => {
                self.state = CommentedState::Done;
                let crossed_line = self.de.line != self.value_start_line;
                let comment = if crossed_line {
                    None
                } else {
                    self.de.capture_inline_comment()?
                };
                seed.deserialize(OptionStringDeserializer(comment))
            }
            CommentedState::Done => Err(self.de.error("CommentedAccess: unexpected state")),
        }
    }
}

/// Mini deserializer that presents an `Option<String>` to serde.
struct OptionStringDeserializer(Option<String>);

impl<'de> de::Deserializer<'de> for OptionStringDeserializer {
    type Error = Error;

    fn deserialize_any<V: de::Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        match self.0 {
            Some(s) => visitor.visit_some(de::value::StringDeserializer::new(s)),
            None => visitor.visit_none(),
        }
    }

    fn deserialize_option<V: de::Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        match self.0 {
            Some(s) => visitor.visit_some(de::value::StringDeserializer::new(s)),
            None => visitor.visit_none(),
        }
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 u8 u16 u32 u64 f32 f64 char str string
        bytes byte_buf unit unit_struct newtype_struct seq tuple
        tuple_struct map struct enum identifier ignored_any
    }
}

// ── KeyDeserializer ──────────────────────────────────────────────

/// Deserializer that delivers a pre-scanned map key to a seed. Scalar
/// resolution for bare keys mirrors `deserialize_any` on the main parser
/// (bool / int / string), so `HashMap<i64, _>`, `HashMap<bool, _>`, etc.
/// continue to work without re-parsing source bytes.
struct KeyDeserializer {
    text: String,
    kind: MappingKeyKind,
}

impl<'de> de::Deserializer<'de> for KeyDeserializer {
    type Error = Error;

    fn deserialize_any<V: de::Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        match self.kind {
            MappingKeyKind::DoubleQuoted => visitor.visit_string(self.text),
            MappingKeyKind::Bare => match self.text.as_str() {
                "true" => visitor.visit_bool(true),
                "false" => visitor.visit_bool(false),
                _ => match try_parse_int(&self.text) {
                    Ok(Some(i)) => visitor.visit_i64(i),
                    Err(()) => Err(Error::Message(format!(
                        "integer key overflow: `{}`",
                        self.text
                    ))),
                    // null / float keys were rejected by validate_key earlier;
                    // anything else is a string.
                    Ok(None) => visitor.visit_string(self.text),
                },
            },
        }
    }

    fn deserialize_str<V: de::Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        visitor.visit_string(self.text)
    }

    fn deserialize_string<V: de::Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        visitor.visit_string(self.text)
    }

    fn deserialize_identifier<V: de::Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        visitor.visit_string(self.text)
    }

    fn deserialize_bool<V: de::Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        match self.text.as_str() {
            "true" => visitor.visit_bool(true),
            "false" => visitor.visit_bool(false),
            _ => Err(Error::Message(format!(
                "expected boolean key, got `{}`",
                self.text
            ))),
        }
    }

    fn deserialize_i8<V: de::Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        visitor.visit_i8(parse_int_key(&self.text)?)
    }
    fn deserialize_i16<V: de::Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        visitor.visit_i16(parse_int_key(&self.text)?)
    }
    fn deserialize_i32<V: de::Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        visitor.visit_i32(parse_int_key(&self.text)?)
    }
    fn deserialize_i64<V: de::Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        visitor.visit_i64(parse_int_key(&self.text)?)
    }
    fn deserialize_u8<V: de::Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        visitor.visit_u8(parse_int_key(&self.text)?)
    }
    fn deserialize_u16<V: de::Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        visitor.visit_u16(parse_int_key(&self.text)?)
    }
    fn deserialize_u32<V: de::Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        visitor.visit_u32(parse_int_key(&self.text)?)
    }
    fn deserialize_u64<V: de::Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        visitor.visit_u64(parse_int_key(&self.text)?)
    }

    fn deserialize_option<V: de::Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        // Null keys are forbidden by validate_key, so any key we see here is
        // a real value — never None.
        visitor.visit_some(self)
    }

    fn deserialize_newtype_struct<V: de::Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value> {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_enum<V: de::Visitor<'de>>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value> {
        visitor.visit_enum(de::value::StringDeserializer::<Error>::new(self.text))
    }

    serde::forward_to_deserialize_any! {
        f32 f64 char bytes byte_buf unit unit_struct
        seq tuple tuple_struct map struct ignored_any
    }
}

/// Parse `text` as an integer key, mapping it through `T` (e.g. `i8`, `u32`).
fn parse_int_key<T: TryFrom<i64>>(text: &str) -> Result<T>
where
    T::Error: std::fmt::Display,
{
    match try_parse_int(text) {
        Ok(Some(i)) => {
            T::try_from(i).map_err(|e| Error::Message(format!("integer key out of range: {e}")))
        }
        Err(()) => Err(Error::Message(format!("integer key overflow: `{text}`"))),
        Ok(None) => Err(Error::Message(format!("expected integer key, got `{text}`"))),
    }
}

// ── EnumAccess / VariantAccess ────────────────────────────────────

enum EnumStyle {
    Unit,
    Mapping,
}

struct EnumAccess<'a, R> {
    de: &'a mut Deserializer<R>,
    style: EnumStyle,
    prescanned_variant: Option<String>,
}

impl<'a, 'de, R: Read> de::EnumAccess<'de> for EnumAccess<'a, R> {
    type Error = Error;
    type Variant = VariantAccess<'a, R>;

    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant)>
    where
        V: de::DeserializeSeed<'de>,
    {
        match self.style {
            EnumStyle::Unit => {
                let variant = if let Some(text) = self.prescanned_variant {
                    seed.deserialize(de::value::StringDeserializer::<Error>::new(text))?
                } else {
                    seed.deserialize(&mut *self.de)?
                };
                Ok((variant, VariantAccess { de: self.de }))
            }
            EnumStyle::Mapping => {
                let variant = if let Some(text) = self.prescanned_variant {
                    seed.deserialize(de::value::StringDeserializer::<Error>::new(text))?
                } else {
                    let variant = seed.deserialize(&mut *self.de)?;
                    self.de.skip_inline_whitespace()?;
                    if !self.de.eat(b':')? {
                        return Err(self.de.error("expected `:` after enum variant name"));
                    }
                    self.de.skip_inline_whitespace()?;
                    variant
                };
                Ok((variant, VariantAccess { de: self.de }))
            }
        }
    }
}

struct VariantAccess<'a, R> {
    de: &'a mut Deserializer<R>,
}

impl<'de, R: Read> de::VariantAccess<'de> for VariantAccess<'_, R> {
    type Error = Error;

    fn unit_variant(self) -> Result<()> {
        Ok(())
    }

    fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value>
    where
        T: de::DeserializeSeed<'de>,
    {
        seed.deserialize(self.de)
    }

    fn tuple_variant<V>(self, _len: usize, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        de::Deserializer::deserialize_seq(self.de, visitor)
    }

    fn struct_variant<V>(self, _fields: &'static [&'static str], visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        de::Deserializer::deserialize_map(self.de, visitor)
    }
}

// ── Scalar parsing helpers ───────────────────────────────────────

fn decode_escape_char(
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
) -> std::result::Result<char, String> {
    match chars.next() {
        Some('0') => Ok('\0'),
        Some('a') => Ok('\x07'),
        Some('b') => Ok('\x08'),
        Some('t') => Ok('\t'),
        Some('n') => Ok('\n'),
        Some('v') => Ok('\x0B'),
        Some('f') => Ok('\x0C'),
        Some('r') => Ok('\r'),
        Some('e') => Ok('\x1B'),
        Some(' ') => Ok(' '),
        Some('"') => Ok('"'),
        Some('/') => Ok('/'),
        Some('\\') => Ok('\\'),
        Some('x') => decode_hex_escape(chars, 2),
        Some('u') => decode_hex_escape(chars, 4),
        Some('U') => decode_hex_escape(chars, 8),
        Some(c) => Err(format!("invalid escape: \\{c}")),
        None => Err("unexpected end of escape".into()),
    }
}

fn decode_hex_escape(
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
    n: usize,
) -> std::result::Result<char, String> {
    let mut value: u32 = 0;
    for _ in 0..n {
        match chars.next() {
            Some(ch) if ch.is_ascii_hexdigit() => {
                value = value * 16 + ch.to_digit(16).unwrap();
            }
            _ => return Err(format!("expected {n} hex digits")),
        }
    }
    char::from_u32(value).ok_or_else(|| format!("invalid unicode code point U+{value:04X}"))
}

fn is_ascii_whitespace(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | b'\r')
}

fn is_indicator_byte(b: u8) -> bool {
    matches!(
        b,
        b'-' | b':' | b',' | b'[' | b']' | b'{' | b'}' | b'#' | b'"' | b'\\'
    )
}

/// Check if a byte can start a plain (bare) scalar.
///
/// Note: `-` and `:` require a lookahead check (must be followed by `ns-char`).
/// They are handled by the caller before this function is reached, so this
/// function correctly returns `false` for all indicator bytes.
fn is_plain_first_byte(b: u8) -> bool {
    if b >= 0x80 {
        // Start of multi-byte UTF-8 — allowed as plain first
        return true;
    }
    if is_indicator_byte(b) {
        return false;
    }
    !is_ascii_whitespace(b) && is_printable_byte_start(b)
}

/// Check if byte is a valid start for printable content.
fn is_printable_byte_start(b: u8) -> bool {
    if b >= 0x80 {
        return true; // Multi-byte UTF-8, validated later
    }
    // Printable ASCII (tab is allowed in inline whitespace context)
    b == 0x09 || (0x20..=0x7E).contains(&b)
}

fn is_printable(ch: char) -> bool {
    let cp = ch as u32;
    matches!(
        cp,
        0x09 | 0x0A | 0x0D |
        0x20..=0x7E |
        0x85 |
        0xA0..=0xD7FF |
        0xE000..=0xFFFD |
        0x10000..=0x10_FFFF
    )
}

fn try_parse_int(s: &str) -> std::result::Result<Option<i64>, ()> {
    let (unsigned, negative) = match s.strip_prefix('-') {
        Some(rest) => (rest, true),
        None => (s.strip_prefix('+').unwrap_or(s), false),
    };

    let abs = if let Some(bin) = unsigned.strip_prefix("0b") {
        match u64::from_str_radix(bin, 2) {
            Ok(v) => v,
            Err(_) if !bin.is_empty() && bin.chars().all(|c| c == '0' || c == '1') => {
                return Err(());
            }
            Err(_) => return Ok(None),
        }
    } else if let Some(oct) = unsigned.strip_prefix("0o") {
        match u64::from_str_radix(oct, 8) {
            Ok(v) => v,
            Err(_) if !oct.is_empty() && oct.chars().all(|c| c.is_ascii_digit() && c < '8') => {
                return Err(());
            }
            Err(_) => return Ok(None),
        }
    } else if let Some(hex) = unsigned.strip_prefix("0x") {
        match u64::from_str_radix(hex, 16) {
            Ok(v) => v,
            Err(_) if !hex.is_empty() && hex.chars().all(|c| c.is_ascii_hexdigit()) => {
                return Err(());
            }
            Err(_) => return Ok(None),
        }
    } else {
        if unsigned.is_empty() || !unsigned.chars().all(|c| c.is_ascii_digit()) {
            return Ok(None);
        }
        match unsigned.parse::<u64>() {
            Ok(v) => v,
            Err(_) => return Err(()),
        }
    };

    let signed = if negative {
        -i128::from(abs)
    } else {
        i128::from(abs)
    };
    i64::try_from(signed).map(Some).map_err(|_| ())
}

fn parse_unsigned(s: &str) -> Option<u64> {
    let s = s.strip_prefix('+').unwrap_or(s);

    if let Some(bin) = s.strip_prefix("0b") {
        u64::from_str_radix(bin, 2).ok()
    } else if let Some(oct) = s.strip_prefix("0o") {
        u64::from_str_radix(oct, 8).ok()
    } else if let Some(hex) = s.strip_prefix("0x") {
        u64::from_str_radix(hex, 16).ok()
    } else {
        s.parse().ok()
    }
}

fn try_parse_float(s: &str) -> Option<f64> {
    match s {
        "inf" | "+inf" => return Some(f64::INFINITY),
        "-inf" => return Some(f64::NEG_INFINITY),
        "nan" => return Some(f64::NAN),
        _ => {}
    }

    let (s, negative) = match s.strip_prefix('-') {
        Some(rest) => (rest, true),
        None => (s.strip_prefix('+').unwrap_or(s), false),
    };

    let has_dot = s.contains('.');
    let has_exp = s.contains('e') || s.contains('E');

    if !has_dot && !has_exp {
        return None;
    }

    if has_dot {
        let (int_part, frac_and_exp) = s.split_once('.')?;
        if int_part.is_empty() || !int_part.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        if let Some(e_pos) = frac_and_exp.find(['e', 'E']) {
            let frac = &frac_and_exp[..e_pos];
            let exp = &frac_and_exp[e_pos + 1..];
            if frac.is_empty() || !frac.chars().all(|c| c.is_ascii_digit()) {
                return None;
            }
            if !valid_exponent(exp) {
                return None;
            }
        } else if frac_and_exp.is_empty() || !frac_and_exp.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
    } else {
        let e_pos = s.find(['e', 'E'])?;
        let int_part = &s[..e_pos];
        let exp = &s[e_pos + 1..];
        if int_part.is_empty() || !int_part.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        if !valid_exponent(exp) {
            return None;
        }
    }

    let val = s.parse::<f64>().ok()?;
    Some(if negative { -val } else { val })
}

fn valid_exponent(exp: &str) -> bool {
    let exp = exp
        .strip_prefix('+')
        .or_else(|| exp.strip_prefix('-'))
        .unwrap_or(exp);
    !exp.is_empty() && exp.chars().all(|c| c.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use crate::value::Value;

    use super::*;

    #[test]
    fn test_null() {
        let result: () = from_str("null").unwrap();
        assert_eq!(result, ());
    }

    #[test]
    fn test_bool() {
        assert!(from_str::<bool>("true").unwrap());
        assert!(!from_str::<bool>("false").unwrap());
    }

    #[test]
    fn test_integers() {
        assert_eq!(from_str::<i32>("42").unwrap(), 42);
        assert_eq!(from_str::<i32>("-17").unwrap(), -17);
        assert_eq!(from_str::<i64>("0").unwrap(), 0);
        assert_eq!(from_str::<u8>("255").unwrap(), 255);
        assert_eq!(from_str::<u32>("0xFF").unwrap(), 255);
        assert_eq!(from_str::<i32>("0b1010").unwrap(), 10);
        assert_eq!(from_str::<i32>("0o77").unwrap(), 63);
    }

    #[test]
    fn test_u64_large() {
        let val: u64 = from_str("18446744073709551615").unwrap();
        assert_eq!(val, u64::MAX);
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn test_floats() {
        assert_eq!(from_str::<f64>("3.25").unwrap(), 3.25);
        assert_eq!(from_str::<f64>("-0.5").unwrap(), -0.5);
        assert_eq!(from_str::<f64>("1e10").unwrap(), 1e10);
        assert!(from_str::<f64>("inf").unwrap().is_infinite());
        assert!(from_str::<f64>("nan").unwrap().is_nan());
    }

    #[test]
    fn test_string_bare() {
        assert_eq!(from_str::<String>("hello").unwrap(), "hello");
        assert_eq!(from_str::<String>("hello world").unwrap(), "hello world");
    }

    #[test]
    fn test_string_quoted() {
        assert_eq!(from_str::<String>(r#""hello""#).unwrap(), "hello");
        assert_eq!(
            from_str::<String>(r#""hello\nworld""#).unwrap(),
            "hello\nworld"
        );
    }

    #[test]
    fn test_char() {
        assert_eq!(from_str::<char>("a").unwrap(), 'a');
        assert_eq!(from_str::<char>(r#""x""#).unwrap(), 'x');
    }

    #[test]
    fn test_option_none() {
        assert_eq!(from_str::<Option<i32>>("null").unwrap(), None);
    }

    #[test]
    fn test_option_some() {
        assert_eq!(from_str::<Option<i32>>("42").unwrap(), Some(42));
        assert_eq!(
            from_str::<Option<String>>(r#""hello""#).unwrap(),
            Some("hello".to_string())
        );
    }

    #[test]
    fn test_newtype_struct() {
        #[derive(serde::Deserialize, Debug, PartialEq)]
        struct Meters(f64);

        assert_eq!(from_str::<Meters>("3.5").unwrap(), Meters(3.5));
    }

    #[test]
    fn test_with_comments() {
        assert_eq!(from_str::<i32>("# a comment\n42").unwrap(), 42);
        assert_eq!(from_str::<i32>("42 # trailing").unwrap(), 42);
    }

    #[test]
    fn test_trailing_whitespace() {
        assert_eq!(from_str::<i32>("42  \n").unwrap(), 42);
    }

    #[test]
    fn test_any_resolution() {
        #[derive(serde::Deserialize, Debug, PartialEq)]
        #[serde(untagged)]
        enum AnyScalar {
            Bool(bool),
            Int(i64),
            Float(f64),
            Str(String),
        }

        assert_eq!(
            from_str::<AnyScalar>("true").unwrap(),
            AnyScalar::Bool(true)
        );
        assert_eq!(from_str::<AnyScalar>("42").unwrap(), AnyScalar::Int(42));
        assert_eq!(
            from_str::<AnyScalar>("3.25").unwrap(),
            AnyScalar::Float(3.25)
        );
        assert_eq!(
            from_str::<AnyScalar>("hello").unwrap(),
            AnyScalar::Str("hello".to_string())
        );
    }

    // ── Collection tests ──────────────────────────────────────────

    #[test]
    fn test_flow_sequence() {
        let v: Vec<i32> = from_str("[1, 2, 3]").unwrap();
        assert_eq!(v, vec![1, 2, 3]);
    }

    #[test]
    fn test_flow_sequence_trailing_comma() {
        let v: Vec<i32> = from_str("[1, 2,]").unwrap();
        assert_eq!(v, vec![1, 2]);
    }

    #[test]
    fn test_flow_sequence_empty() {
        let v: Vec<i32> = from_str("[]").unwrap();
        assert_eq!(v, Vec::<i32>::new());
    }

    #[test]
    fn test_flow_sequence_strings() {
        let v: Vec<String> = from_str(r#"["hello", "world"]"#).unwrap();
        assert_eq!(v, vec!["hello", "world"]);
    }

    #[test]
    fn test_flow_mapping() {
        use std::collections::HashMap;
        let m: HashMap<String, i32> = from_str("{a: 1, b: 2}").unwrap();
        assert_eq!(m["a"], 1);
        assert_eq!(m["b"], 2);
    }

    #[test]
    fn test_flow_mapping_trailing_comma() {
        use std::collections::HashMap;
        let m: HashMap<String, i32> = from_str("{a: 1,}").unwrap();
        assert_eq!(m["a"], 1);
    }

    #[test]
    fn test_flow_mapping_empty() {
        use std::collections::HashMap;
        let m: HashMap<String, i32> = from_str("{}").unwrap();
        assert!(m.is_empty());
    }

    #[test]
    fn test_block_sequence() {
        let v: Vec<String> = from_str("- apple\n- banana\n- cherry").unwrap();
        assert_eq!(v, vec!["apple", "banana", "cherry"]);
    }

    #[test]
    fn test_block_sequence_integers() {
        let v: Vec<i32> = from_str("- 1\n- 2\n- 3").unwrap();
        assert_eq!(v, vec![1, 2, 3]);
    }

    #[test]
    fn test_block_mapping_struct() {
        #[derive(serde::Deserialize, Debug, PartialEq)]
        struct Person {
            name: String,
            age: u32,
        }
        let p: Person = from_str("name: John\nage: 30").unwrap();
        assert_eq!(
            p,
            Person {
                name: "John".to_string(),
                age: 30
            }
        );
    }

    #[test]
    fn test_block_mapping_with_comments() {
        #[derive(serde::Deserialize, Debug, PartialEq)]
        struct Config {
            host: String,
            port: u16,
        }
        let c: Config = from_str("host: localhost # the host\nport: 8080").unwrap();
        assert_eq!(
            c,
            Config {
                host: "localhost".to_string(),
                port: 8080
            }
        );
    }

    #[test]
    fn test_nested_struct() {
        #[derive(serde::Deserialize, Debug, PartialEq)]
        struct Outer {
            inner: Inner,
        }
        #[derive(serde::Deserialize, Debug, PartialEq)]
        struct Inner {
            value: i32,
        }
        let o: Outer = from_str("inner:\n  value: 42").unwrap();
        assert_eq!(
            o,
            Outer {
                inner: Inner { value: 42 }
            }
        );
    }

    #[test]
    fn test_struct_with_sequence() {
        #[derive(serde::Deserialize, Debug, PartialEq)]
        struct Config {
            items: Vec<String>,
        }
        let c: Config = from_str("items:\n- alpha\n- beta").unwrap();
        assert_eq!(
            c,
            Config {
                items: vec!["alpha".to_string(), "beta".to_string()]
            }
        );
    }

    #[test]
    fn test_sequence_of_structs() {
        #[derive(serde::Deserialize, Debug, PartialEq)]
        struct Item {
            name: String,
            qty: u32,
        }
        let items: Vec<Item> =
            from_str("- name: Widget\n  qty: 5\n- name: Gadget\n  qty: 3").unwrap();
        assert_eq!(
            items,
            vec![
                Item {
                    name: "Widget".to_string(),
                    qty: 5
                },
                Item {
                    name: "Gadget".to_string(),
                    qty: 3
                },
            ]
        );
    }

    #[test]
    fn test_tuple() {
        let t: (i32, String, bool) = from_str("[42, hello, true]").unwrap();
        assert_eq!(t, (42, "hello".to_string(), true));
    }

    #[test]
    fn test_flow_nested() {
        let v: Vec<Vec<i32>> = from_str("[[1, 2], [3, 4]]").unwrap();
        assert_eq!(v, vec![vec![1, 2], vec![3, 4]]);
    }

    #[test]
    fn test_block_sequence_with_flow() {
        let v: Vec<Vec<i32>> = from_str("- [1, 2]\n- [3, 4]").unwrap();
        assert_eq!(v, vec![vec![1, 2], vec![3, 4]]);
    }

    #[test]
    fn test_struct_with_flow_mapping() {
        #[derive(serde::Deserialize, Debug, PartialEq)]
        struct Config {
            tags: std::collections::HashMap<String, String>,
        }
        let c: Config = from_str("tags: {env: prod, region: us}").unwrap();
        assert_eq!(c.tags["env"], "prod");
        assert_eq!(c.tags["region"], "us");
    }

    #[test]
    fn test_from_reader_struct() {
        #[derive(serde::Deserialize, Debug, PartialEq)]
        struct Config {
            name: String,
            port: u16,
        }
        let data = b"name: myapp\nport: 3000" as &[u8];
        let c: Config = from_reader(data).unwrap();
        assert_eq!(
            c,
            Config {
                name: "myapp".to_string(),
                port: 3000
            }
        );
    }

    #[test]
    fn test_error_trailing() {
        assert!(from_str::<i32>("42 extra").is_err());
    }

    #[test]
    fn test_error_type_mismatch() {
        assert!(from_str::<bool>("42").is_err());
        assert!(from_str::<i32>("hello").is_err());
    }

    #[test]
    fn test_from_slice() {
        assert_eq!(from_slice::<i32>(b"42").unwrap(), 42);
        assert_eq!(from_slice::<String>(b"hello").unwrap(), "hello");
        assert_eq!(from_slice::<String>(b"\"quoted\"").unwrap(), "quoted");
    }

    #[test]
    fn test_from_slice_invalid_utf8() {
        assert!(from_slice::<String>(&[0xFF, 0xFE]).is_err());
    }

    #[test]
    fn test_from_reader() {
        let data = b"42" as &[u8];
        assert_eq!(from_reader::<_, i32>(data).unwrap(), 42);

        let data = b"\"hello\"" as &[u8];
        assert_eq!(from_reader::<_, String>(data).unwrap(), "hello");
    }

    // ── Enum tests ───────────────────────────────────────────────

    #[test]
    fn test_enum_unit_variant() {
        #[derive(serde::Deserialize, Debug, PartialEq)]
        enum Color {
            Red,
            Green,
            Blue,
        }
        assert_eq!(from_str::<Color>("Red").unwrap(), Color::Red);
        assert_eq!(from_str::<Color>("Green").unwrap(), Color::Green);
        assert_eq!(from_str::<Color>("Blue").unwrap(), Color::Blue);
    }

    #[test]
    fn test_enum_newtype_variant_flow() {
        #[derive(serde::Deserialize, Debug, PartialEq)]
        enum Shape {
            Circle(f64),
            Label(String),
        }
        assert_eq!(
            from_str::<Shape>("{Circle: 3.25}").unwrap(),
            Shape::Circle(3.25)
        );
        assert_eq!(
            from_str::<Shape>("{Label: hello}").unwrap(),
            Shape::Label("hello".to_string())
        );
    }

    #[test]
    fn test_enum_newtype_variant_block() {
        #[derive(serde::Deserialize, Debug, PartialEq)]
        enum Shape {
            Circle(f64),
        }
        assert_eq!(
            from_str::<Shape>("Circle: 3.25").unwrap(),
            Shape::Circle(3.25)
        );
    }

    #[test]
    fn test_enum_tuple_variant_flow() {
        #[derive(serde::Deserialize, Debug, PartialEq)]
        enum Cmd {
            Move(i32, i32),
        }
        assert_eq!(
            from_str::<Cmd>("{Move: [10, 20]}").unwrap(),
            Cmd::Move(10, 20)
        );
    }

    #[test]
    fn test_enum_tuple_variant_block() {
        #[derive(serde::Deserialize, Debug, PartialEq)]
        enum Cmd {
            Move(i32, i32),
        }
        assert_eq!(
            from_str::<Cmd>("Move: [10, 20]").unwrap(),
            Cmd::Move(10, 20)
        );
    }

    #[test]
    fn test_enum_struct_variant_flow() {
        #[derive(serde::Deserialize, Debug, PartialEq)]
        enum Shape {
            Rect { w: u32, h: u32 },
        }
        assert_eq!(
            from_str::<Shape>("{Rect: {w: 10, h: 20}}").unwrap(),
            Shape::Rect { w: 10, h: 20 }
        );
    }

    #[test]
    fn test_enum_struct_variant_block() {
        #[derive(serde::Deserialize, Debug, PartialEq)]
        enum Shape {
            Rect { w: u32, h: u32 },
        }
        let input = "Rect:\n  w: 10\n  h: 20";
        assert_eq!(
            from_str::<Shape>(input).unwrap(),
            Shape::Rect { w: 10, h: 20 }
        );
    }

    #[test]
    fn test_enum_unit_in_sequence() {
        #[derive(serde::Deserialize, Debug, PartialEq)]
        enum Color {
            Red,
            Blue,
        }
        let v: Vec<Color> = from_str("- Red\n- Blue").unwrap();
        assert_eq!(v, vec![Color::Red, Color::Blue]);
    }

    #[test]
    fn test_enum_in_struct_field() {
        #[derive(serde::Deserialize, Debug, PartialEq)]
        enum Status {
            Active,
            Inactive,
        }
        #[derive(serde::Deserialize, Debug, PartialEq)]
        struct User {
            name: String,
            status: Status,
        }
        let u: User = from_str("name: Alice\nstatus: Active").unwrap();
        assert_eq!(
            u,
            User {
                name: "Alice".to_string(),
                status: Status::Active,
            }
        );
    }

    #[test]
    fn test_enum_option_variant() {
        #[derive(serde::Deserialize, Debug, PartialEq)]
        enum Value {
            Num(i32),
            None,
        }
        assert_eq!(from_str::<Value>("None").unwrap(), Value::None);
        assert_eq!(from_str::<Value>("{Num: 42}").unwrap(), Value::Num(42));
    }

    #[test]
    fn test_enum_quoted_variant() {
        #[derive(serde::Deserialize, Debug, PartialEq)]
        enum Color {
            Red,
        }
        assert_eq!(from_str::<Color>(r#""Red""#).unwrap(), Color::Red);
    }

    #[test]
    fn test_depth_limit_flow_seq() {
        let input = "[".repeat(MAX_DEPTH + 1);
        let err = from_str::<Value>(&input).unwrap_err();
        assert!(
            err.to_string().contains("nesting depth limit exceeded"),
            "{err}"
        );
    }

    #[test]
    fn test_depth_limit_flow_map() {
        let input = "{a: ".repeat(MAX_DEPTH + 1) + &"}".repeat(MAX_DEPTH + 1);
        let err = from_str::<Value>(&input).unwrap_err();
        assert!(
            err.to_string().contains("nesting depth limit exceeded"),
            "{err}"
        );
    }

    #[test]
    fn test_depth_within_limit() {
        let val: Value = from_str("[[1, 2], [3, 4]]").unwrap();
        assert!(matches!(val, Value::Seq(_)));
    }

    #[test]
    fn test_duplicate_key_block() {
        use std::collections::HashMap;
        let err = from_str::<HashMap<String, i32>>("a: 1\na: 2").unwrap_err();
        assert!(err.to_string().contains("duplicate key"), "{err}");
    }

    #[test]
    fn test_duplicate_key_flow() {
        use std::collections::HashMap;
        let err = from_str::<HashMap<String, i32>>("{a: 1, a: 2}").unwrap_err();
        assert!(err.to_string().contains("duplicate key"), "{err}");
    }
}
