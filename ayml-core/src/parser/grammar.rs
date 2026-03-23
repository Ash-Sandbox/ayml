use indexmap::IndexMap;

use super::scanner::Scanner;
use crate::error::{Error, ErrorKind, Span};
use crate::value::{MapKey, Node, Value};

/// The parsing context: block or flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Context {
    Block,
    Flow,
}

/// Returns true if the error is a "hard" semantic error that should always
/// propagate (not be swallowed by backtracking).
const fn is_hard_error(e: &Error) -> bool {
    matches!(
        e.kind,
        ErrorKind::DuplicateKey(_)
            | ErrorKind::NullKey
            | ErrorKind::FloatKey
            | ErrorKind::InvalidEscape(_)
            | ErrorKind::NonPrintable(_)
            | ErrorKind::ByteOrderMark
            | ErrorKind::IntegerOverflow
            | ErrorKind::UnexpectedEof
            | ErrorKind::RecursionLimit
            | ErrorKind::TabIndent
    )
}

/// A raw mapping key that may need validation. Null and float keys are
/// deferred errors — they only fire when we've confirmed a colon follows.
enum RawMapKey {
    Valid(MapKey),
    Null(usize),  // offset of the `null` token
    Float(usize), // offset of the float token
}

impl RawMapKey {
    fn validate(self, source: &str, end_offset: usize) -> Result<MapKey, Error> {
        match self {
            Self::Valid(k) => Ok(k),
            Self::Null(offset) => Err(Error::new(
                ErrorKind::NullKey,
                Span::new(offset, end_offset),
                source,
            )),
            Self::Float(offset) => Err(Error::new(
                ErrorKind::FloatKey,
                Span::new(offset, end_offset),
                source,
            )),
        }
    }
}

/// Default maximum nesting depth for the parser.
pub const DEFAULT_MAX_DEPTH: usize = 128;

/// Recursive descent parser for AYML.
///
/// Method names mirror the BNF production names from the spec where possible.
pub struct Parser<'a> {
    scanner: Scanner<'a>,
    depth: usize,
    max_depth: usize,
}

impl<'a> Parser<'a> {
    pub const fn new(input: &'a str) -> Self {
        Self {
            scanner: Scanner::new(input),
            depth: 0,
            max_depth: DEFAULT_MAX_DEPTH,
        }
    }

    pub const fn with_max_depth(mut self, max_depth: usize) -> Self {
        self.max_depth = max_depth;
        self
    }

    fn enter_nested(&mut self) -> Result<(), Error> {
        self.depth += 1;
        if self.depth > self.max_depth {
            return Err(self.scanner.error(ErrorKind::RecursionLimit));
        }
        Ok(())
    }

    fn leave_nested(&mut self) {
        self.depth -= 1;
    }

    /// l-ayml-document
    pub fn parse_document(&mut self) -> Result<Node, Error> {
        // Reject BOM
        if self.scanner.peek() == Some('\u{FEFF}') {
            return Err(self.scanner.error(ErrorKind::ByteOrderMark));
        }

        let start = self.scanner.offset;

        // Leading comments
        let comment = self.parse_comment_block(0);

        // Skip blank lines between leading comment and content
        self.skip_blank_lines();

        // Root node
        let mut node = self.parse_block_node(0)?;

        // Attach leading document comment to root node
        if comment.is_some() {
            node.comment = comment;
        }

        // Inline comment on same line as root node
        node.inline_comment = node.inline_comment.or_else(|| self.parse_inline_comment());

        // Trailing content: blank lines and comments
        self.skip_trailing(0);

        if !self.scanner.is_eof() {
            return Err(self
                .scanner
                .error(ErrorKind::Expected("end of input".into())));
        }

        node.span = Span::new(start, self.scanner.offset);
        Ok(node)
    }

    /// Parse a block of consecutive comment lines at indentation <= n.
    /// Returns the joined comment text (without `#` prefixes), or None.
    /// Blank lines between comment lines are preserved as empty strings.
    fn parse_comment_block(&mut self, n: usize) -> Option<String> {
        let mut lines: Vec<String> = Vec::new();

        loop {
            let saved = self.scanner.offset;
            // Allow blank lines between comment lines, preserving them.
            let blank_count = self.count_and_skip_blank_lines();

            // Tab in indentation here just means "not a comment line" —
            // the tab will be rejected by the main parse path.
            let spaces = self.scanner.count_spaces().unwrap_or(0);
            // l-comment(n): comment indent must be <= n, and line must start with `#`
            if spaces > n || self.scanner.peek_byte_at(spaces) != Some(b'#') {
                self.scanner.offset = saved;
                break;
            }

            // Preserve blank lines between comment lines (not before the first).
            if !lines.is_empty() {
                for _ in 0..blank_count {
                    lines.push(String::new());
                }
            }

            // Consume indent + `#`
            self.scanner.eat_spaces(spaces);
            self.scanner.advance(); // `#`

            // Optional space after `#`
            if self.scanner.peek() == Some(' ') {
                self.scanner.advance();
            }

            let text = self.scanner.rest_of_line().to_string();
            lines.push(text);

            if !self.scanner.eat_break() && !self.scanner.is_eof() {
                break;
            }
        }

        if lines.is_empty() {
            None
        } else {
            Some(lines.join("\n"))
        }
    }

    /// Parse an optional inline comment (` # ...`). Returns the text or None.
    fn parse_inline_comment(&mut self) -> Option<String> {
        let saved = self.scanner.offset;
        if !self.scanner.is_white() {
            return None;
        }
        self.scanner.skip_inline_whitespace();
        if self.scanner.eat('#') {
            // Optional space after `#`
            if self.scanner.peek() == Some(' ') {
                self.scanner.advance();
            }
            let text = self.scanner.rest_of_line().to_string();
            Some(text)
        } else {
            self.scanner.offset = saved;
            None
        }
    }

    // ── Block gaps ───────────────────────────────────────────────────

    /// Skip blank lines (whitespace-only lines).
    fn skip_blank_lines(&mut self) {
        self.count_and_skip_blank_lines();
    }

    /// Skip blank lines and return how many were consumed.
    fn count_and_skip_blank_lines(&mut self) -> usize {
        let mut count = 0;
        loop {
            let saved = self.scanner.offset;
            self.scanner.skip_inline_whitespace();
            if self.scanner.is_break() {
                self.scanner.eat_break();
                count += 1;
            } else {
                self.scanner.offset = saved;
                break;
            }
        }
        count
    }

    /// Skip block gaps (blank lines and comment lines) at indent n.
    /// Returns any accumulated comment text (to attach to the next node).
    /// Multiple comment blocks separated by blank lines are joined,
    /// preserving the blank lines between them.
    fn skip_block_gaps(&mut self, n: usize) -> Option<String> {
        let mut accumulated: Option<String> = None;

        loop {
            let saved = self.scanner.offset;

            // Try blank lines
            let blanks = self.count_and_skip_blank_lines();

            // Try comment block
            if let Some(comment) = self.parse_comment_block(n) {
                accumulated = Some(match accumulated {
                    Some(mut prev) => {
                        // Preserve blank lines between comment blocks.
                        for _ in 0..blanks {
                            prev.push('\n');
                        }
                        prev.push('\n');
                        prev.push_str(&comment);
                        prev
                    }
                    None => comment,
                });
                continue;
            }

            // If we consumed any blank lines but no comment, check if we're
            // at the right position
            if self.scanner.offset == saved {
                break;
            }
        }

        accumulated
    }

    /// Skip trailing blank lines and comments after the root node.
    fn skip_trailing(&mut self, n: usize) {
        loop {
            let saved = self.scanner.offset;
            self.scanner.skip_inline_whitespace();
            if self.scanner.eat_break() {
                continue;
            }
            self.scanner.offset = saved;

            let Ok(spaces) = self.scanner.count_spaces() else {
                break;
            };

            if spaces <= n && self.scanner.peek_byte_at(spaces) == Some(b'#') {
                self.scanner.eat_spaces(spaces);
                self.scanner.advance(); // `#`
                let _ = self.scanner.rest_of_line();
                if !self.scanner.eat_break() {
                    break;
                }
                continue;
            }

            break;
        }
    }

    // ── Nodes ────────────────────────────────────────────────────────

    /// ns-block-node(n) — a block sequence, block mapping, or flow node.
    fn parse_block_node(&mut self, n: usize) -> Result<Node, Error> {
        self.enter_nested()?;
        let result = self.parse_block_node_inner(n);
        self.leave_nested();
        result
    }

    fn parse_block_node_inner(&mut self, n: usize) -> Result<Node, Error> {
        // Try block sequence
        if let Some(node) = self.try_block_sequence(n)? {
            return Ok(node);
        }

        // Try block mapping
        if let Some(node) = self.try_block_mapping(n)? {
            return Ok(node);
        }

        // Flow node (scalar or flow collection)
        self.parse_flow_node(Context::Block)
    }

    /// ns-flow-node(c) — a scalar or flow collection.
    fn parse_flow_node(&mut self, ctx: Context) -> Result<Node, Error> {
        self.enter_nested()?;
        let result = self.parse_flow_node_inner(ctx);
        self.leave_nested();
        result
    }

    fn parse_flow_node_inner(&mut self, ctx: Context) -> Result<Node, Error> {
        match self.scanner.peek() {
            Some('[') => self.parse_flow_sequence(),
            Some('{') => self.parse_flow_mapping(),
            Some(_) => self.parse_scalar(ctx),
            None => Err(self.scanner.error(ErrorKind::UnexpectedEof)),
        }
    }

    // ── Scalars ──────────────────────────────────────────────────────

    /// ns-scalar(c) — parse a scalar value with resolution.
    fn parse_scalar(&mut self, ctx: Context) -> Result<Node, Error> {
        match self.scanner.peek() {
            Some('"') => {
                // Could be double-quoted or triple-quoted
                let rest = &self.scanner.input[self.scanner.offset..];
                if let Some(after_quotes) = rest.strip_prefix("\"\"\"") {
                    // Verify the opening `"""` is followed by a line break
                    // before committing to triple-quoted parsing. This gives
                    // a clear error message instead of a confusing backtrack.
                    if let Some(ch) = after_quotes.chars().next()
                        && ch != '\n'
                        && ch != '\r'
                    {
                        self.scanner.offset += 3;
                        return Err(self.scanner.error(ErrorKind::Expected(
                            "line break after `\"\"\"`: content must start on the next line".into(),
                        )));
                    }
                    self.parse_triple_quoted()
                } else {
                    self.parse_double_quoted()
                }
            }
            _ => self.parse_bare_scalar(ctx),
        }
    }

    /// c-double-quoted — parse a double-quoted string.
    fn parse_double_quoted(&mut self) -> Result<Node, Error> {
        let start = self.scanner.offset;
        self.scanner.advance(); // opening `"`
        let mut value = String::new();

        loop {
            match self.scanner.peek() {
                Some('"') => {
                    self.scanner.advance();
                    let mut node = Node::new(Value::Str(value));
                    node.span = Span::new(start, self.scanner.offset);
                    return Ok(node);
                }
                Some('\\') => {
                    self.scanner.advance();
                    let ch = self.scanner.parse_escape()?;
                    value.push(ch);
                }
                Some(ch) if ch == '\n' || ch == '\r' => {
                    return Err(Error::new(
                        ErrorKind::Expected("closing `\"` before line break".into()),
                        Span::new(start, self.scanner.offset),
                        self.scanner.source(),
                    ));
                }
                Some(ch) => {
                    if !Scanner::is_printable(ch) {
                        return Err(self.scanner.error(ErrorKind::NonPrintable(ch)));
                    }
                    self.scanner.advance();
                    value.push(ch);
                }
                None => {
                    return Err(Error::new(
                        ErrorKind::UnexpectedEof,
                        Span::new(start, self.scanner.offset),
                        self.scanner.source(),
                    ));
                }
            }
        }
    }

    /// c-triple-quoted — parse a triple-quoted string.
    fn parse_triple_quoted(&mut self) -> Result<Node, Error> {
        let start = self.scanner.offset;
        // Consume opening `"""`
        self.scanner.eat_str("\"\"\"");

        // Must be followed by a line break
        if !self.scanner.eat_break() {
            return Err(Error::new(
                ErrorKind::Expected("line break after opening `\"\"\"`".into()),
                Span::point(self.scanner.offset),
                self.scanner.source(),
            ));
        }

        // Collect content lines until we find the closing `"""`
        let mut raw_lines: Vec<String> = Vec::new();
        let closing_indent;

        loop {
            if self.scanner.is_eof() {
                return Err(Error::new(
                    ErrorKind::UnexpectedEof,
                    Span::new(start, self.scanner.offset),
                    self.scanner.source(),
                ));
            }

            // Check if this line is the closing `"""`
            let spaces = self.scanner.count_spaces()?;

            // Peek ahead: is this `<indent>"""`?
            let after_spaces = &self.scanner.input[self.scanner.offset + spaces..];
            if let Some(after_close) = after_spaces.strip_prefix("\"\"\"") {
                // Check it's followed by end-of-line or EOF
                if after_close.is_empty()
                    || after_close.starts_with('\n')
                    || after_close.starts_with('\r')
                    || after_close.starts_with(' ')
                    || after_close.starts_with('\t')
                    || after_close.starts_with('#')
                    || after_close.starts_with(',')
                    || after_close.starts_with(']')
                    || after_close.starts_with('}')
                {
                    closing_indent = spaces;
                    self.scanner.offset += spaces + 3;
                    break;
                }
            }

            // Content line or blank line
            let line_content = self.scanner.rest_of_line();
            raw_lines.push(line_content.to_string());

            if !self.scanner.eat_break() && !self.scanner.is_eof() {
                return Err(Error::new(
                    ErrorKind::Expected("line break in triple-quoted string".into()),
                    Span::point(self.scanner.offset),
                    self.scanner.source(),
                ));
            }
        }

        let result = Self::process_triple_quoted_lines(
            &raw_lines,
            closing_indent,
            start,
            self.scanner.source(),
        )?;
        let mut node = Node::new(Value::Str(result));
        node.span = Span::new(start, self.scanner.offset);
        Ok(node)
    }

    /// Helper: take `n` hex digits from a char iterator and decode.
    fn take_hex(
        chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
        n: usize,
        start: usize,
        source: &str,
    ) -> Result<char, Error> {
        let mut value: u32 = 0;
        for _ in 0..n {
            match chars.next() {
                Some(ch) if ch.is_ascii_hexdigit() => {
                    value = value * 16 + ch.to_digit(16).unwrap();
                }
                _ => {
                    return Err(Error::new(
                        ErrorKind::InvalidEscape(format!("expected {n} hex digits")),
                        Span::point(start),
                        source,
                    ));
                }
            }
        }
        char::from_u32(value).ok_or_else(|| {
            Error::new(
                ErrorKind::InvalidEscape(format!("invalid unicode code point U+{value:04X}")),
                Span::point(start),
                source,
            )
        })
    }

    /// Process raw lines from a triple-quoted string: strip indentation and
    /// handle escape sequences.
    fn process_triple_quoted_lines(
        raw_lines: &[String],
        closing_indent: usize,
        start: usize,
        source: &str,
    ) -> Result<String, Error> {
        let mut result = String::new();
        let mut continuation = false;
        for (i, line) in raw_lines.iter().enumerate() {
            if i > 0 {
                if continuation {
                    continuation = false;
                } else {
                    result.push('\n');
                }
            }

            let stripped = if line.len() >= closing_indent
                && line[..closing_indent].chars().all(|c| c == ' ')
            {
                &line[closing_indent..]
            } else if line.trim().is_empty() {
                ""
            } else {
                return Err(Error::new(
                    ErrorKind::Expected(format!(
                        "at least {closing_indent} spaces of indentation in triple-quoted string"
                    )),
                    Span::point(start),
                    source,
                ));
            };

            let mut chars = stripped.chars().peekable();
            while let Some(ch) = chars.next() {
                if ch == '\\' {
                    if chars.peek().is_none() {
                        continuation = true;
                        continue;
                    }
                    Self::process_escape_char(&mut chars, &mut result, start, source)?;
                } else {
                    result.push(ch);
                }
            }
        }
        Ok(result)
    }

    /// Process a single escape character from an iterator (after consuming the backslash).
    fn process_escape_char(
        chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
        result: &mut String,
        start: usize,
        source: &str,
    ) -> Result<(), Error> {
        match chars.next() {
            Some('0') => result.push('\0'),
            Some('a') => result.push('\x07'),
            Some('b') => result.push('\x08'),
            Some('t') => result.push('\t'),
            Some('n') => result.push('\n'),
            Some('v') => result.push('\x0B'),
            Some('f') => result.push('\x0C'),
            Some('r') => result.push('\r'),
            Some('e') => result.push('\x1B'),
            Some(' ') => result.push(' '),
            Some('"') => result.push('"'),
            Some('/') => result.push('/'),
            Some('\\') => result.push('\\'),
            None => {
                return Err(Error::new(
                    ErrorKind::InvalidEscape("\\<eof>".into()),
                    Span::point(start),
                    source,
                ));
            }
            Some('x') => result.push(Self::take_hex(chars, 2, start, source)?),
            Some('u') => result.push(Self::take_hex(chars, 4, start, source)?),
            Some('U') => result.push(Self::take_hex(chars, 8, start, source)?),
            Some(c) => {
                return Err(Error::new(
                    ErrorKind::InvalidEscape(format!("\\{c}")),
                    Span::point(start),
                    source,
                ));
            }
        }
        Ok(())
    }

    /// Parse a bare (unquoted) scalar with type resolution.
    fn parse_bare_scalar(&mut self, ctx: Context) -> Result<Node, Error> {
        let start = self.scanner.offset;
        let text = self.scan_bare_string(ctx)?;

        // Scalar resolution: null → bool → int → float → string
        let value = if text == "null" {
            Value::Null
        } else if text == "true" {
            Value::Bool(true)
        } else if text == "false" {
            Value::Bool(false)
        } else {
            match Self::try_parse_int(&text) {
                Ok(Some(i)) => Value::Int(i),
                Err(()) => {
                    return Err(self.scanner.error_at(ErrorKind::IntegerOverflow, start));
                }
                Ok(None) => {
                    if let Some(f) = Self::try_parse_float(&text) {
                        Value::Float(f)
                    } else {
                        Value::Str(text)
                    }
                }
            }
        };

        let mut node = Node::new(value);
        node.span = Span::new(start, self.scanner.offset);
        Ok(node)
    }

    /// Scan a bare string according to ns-bare-string(c).
    fn scan_bare_string(&mut self, ctx: Context) -> Result<String, Error> {
        let start = self.scanner.offset;

        // ns-plain-first-char
        match self.scanner.peek() {
            Some(ch) if Self::is_plain_first(ch) => {
                self.scanner.advance();
            }
            Some(ch) => {
                return Err(self.scanner.error(ErrorKind::UnexpectedChar(ch)));
            }
            None => {
                return Err(self.scanner.error(ErrorKind::UnexpectedEof));
            }
        }

        // ( s-white* ns-plain-char(c) )*
        loop {
            // Accumulate inline whitespace tentatively
            let ws_start = self.scanner.offset;
            self.scanner.skip_inline_whitespace();
            let ws_end = self.scanner.offset;

            if self.scanner.is_break_or_eof() {
                // Trim trailing whitespace
                let text = self.scanner.input[start..ws_start].to_string();
                return Ok(text);
            }

            match self.scanner.peek() {
                Some('#') => {
                    // '#' is not allowed in bare strings; terminate here
                    if ws_end > ws_start {
                        self.scanner.offset = ws_start;
                    }
                    let text = self.scanner.input[start..self.scanner.offset].to_string();
                    return Ok(text);
                }
                Some(':') => {
                    // `:` followed by space/break/eof ends the scalar (it's a mapping value)
                    let next = self.scanner.peek_nth(1);
                    if next.is_none()
                        || next == Some(' ')
                        || next == Some('\t')
                        || next == Some('\n')
                        || next == Some('\r')
                    {
                        let text = self.scanner.input[start..ws_start].to_string();
                        if ws_end > ws_start {
                            self.scanner.offset = ws_start;
                        }
                        return Ok(text);
                    }
                    self.scanner.advance();
                }
                Some(',' | ']' | '}') if ctx == Context::Flow => {
                    // Flow indicators terminate bare strings in flow context
                    let text = self.scanner.input[start..ws_start].to_string();
                    return Ok(text);
                }
                Some(ch) if !Scanner::is_printable(ch) => {
                    return Err(self.scanner.error(ErrorKind::NonPrintable(ch)));
                }
                Some(_) => {
                    self.scanner.advance();
                }
                None => {
                    let text = self.scanner.input[start..ws_start].to_string();
                    return Ok(text);
                }
            }
        }
    }

    /// Check if a character can start a bare string (ns-plain-first-char).
    const fn is_plain_first(ch: char) -> bool {
        if Self::is_indicator(ch) {
            // `-` and `:` are allowed if followed by ns-char (checked by caller context)
            // For simplicity, we allow `-` and `:` here and validate context later
            ch == '-' || ch == ':'
        } else {
            !ch.is_ascii_whitespace() && Scanner::is_printable(ch)
        }
    }

    /// Check if a character is a c-indicator.
    const fn is_indicator(ch: char) -> bool {
        matches!(
            ch,
            '-' | ':' | ',' | '[' | ']' | '{' | '}' | '#' | '"' | '\\'
        )
    }

    /// Try to parse a string as an AYML integer.
    ///
    /// Returns `Ok(Some(i64))` if it's a valid integer, `Ok(None)` if the
    /// string doesn't match the integer grammar, or `Err(())` if it matches
    /// but overflows i64.
    ///
    /// Parses the magnitude as `u64` then applies the sign via `i128`,
    /// so that `i64::MIN` is representable in all radixes.
    fn try_parse_int(s: &str) -> Result<Option<i64>, ()> {
        let (unsigned, negative) = match s.strip_prefix('-') {
            Some(rest) => (rest, true),
            None => (s.strip_prefix('+').unwrap_or(s), false),
        };

        let abs = if let Some(bin) = unsigned.strip_prefix("0b") {
            match u64::from_str_radix(bin, 2) {
                Ok(v) => v,
                Err(_) if bin.chars().all(|c| c == '0' || c == '1') => return Err(()),
                Err(_) => return Ok(None),
            }
        } else if let Some(oct) = unsigned.strip_prefix("0o") {
            match u64::from_str_radix(oct, 8) {
                Ok(v) => v,
                Err(_) if oct.chars().all(|c| c.is_ascii_digit() && c < '8') => return Err(()),
                Err(_) => return Ok(None),
            }
        } else if let Some(hex) = unsigned.strip_prefix("0x") {
            match u64::from_str_radix(hex, 16) {
                Ok(v) => v,
                Err(_) if hex.chars().all(|c| c.is_ascii_hexdigit()) => return Err(()),
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

    /// Try to parse a string as an AYML float.
    fn try_parse_float(s: &str) -> Option<f64> {
        // Special values
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

        // Must match: digits.digits([eE][+-]?digits)? or digits[eE][+-]?digits
        let has_dot = s.contains('.');
        let has_exp = s.contains('e') || s.contains('E');

        if !has_dot && !has_exp {
            return None; // Would be an integer, not a float
        }

        // Validate structure
        if has_dot {
            let (int_part, frac_and_exp) = s.split_once('.')?;

            if int_part.is_empty() || !int_part.chars().all(|c| c.is_ascii_digit()) {
                return None;
            }

            // frac_and_exp might have an exponent
            if let Some(e_pos) = frac_and_exp.find(['e', 'E']) {
                let frac = &frac_and_exp[..e_pos];
                let exp = &frac_and_exp[e_pos + 1..];
                if frac.is_empty() || !frac.chars().all(|c| c.is_ascii_digit()) {
                    return None;
                }
                if !Self::valid_exponent(exp) {
                    return None;
                }
            } else if frac_and_exp.is_empty() || !frac_and_exp.chars().all(|c| c.is_ascii_digit()) {
                return None;
            }
        } else {
            // Pure exponential: digits e [+-]? digits
            let e_pos = s.find(['e', 'E'])?;
            let int_part = &s[..e_pos];
            let exp = &s[e_pos + 1..];
            if int_part.is_empty() || !int_part.chars().all(|c| c.is_ascii_digit()) {
                return None;
            }
            if !Self::valid_exponent(exp) {
                return None;
            }
        }

        // Parse the validated string
        let full = if negative {
            format!("-{s}")
        } else {
            s.to_string()
        };
        full.parse::<f64>().ok()
    }

    fn valid_exponent(exp: &str) -> bool {
        let exp = exp
            .strip_prefix('+')
            .or_else(|| exp.strip_prefix('-'))
            .unwrap_or(exp);
        !exp.is_empty() && exp.chars().all(|c| c.is_ascii_digit())
    }

    // ── Block Sequence ───────────────────────────────────────────────

    /// Try to parse a block sequence at indentation n.
    fn try_block_sequence(&mut self, n: usize) -> Result<Option<Node>, Error> {
        // Check if the current line starts with `- ` at indent n
        let spaces = self.scanner.count_spaces()?;
        if spaces < n {
            return Ok(None);
        }

        let indent = spaces;
        let after_indent = self.scanner.offset + indent;
        let rest = &self.scanner.input[after_indent..];
        if !rest.starts_with("- ") {
            return Ok(None);
        }

        // Parse the sequence at the detected indent level
        let start = self.scanner.offset;
        let entries = self.parse_block_sequence(indent)?;
        let mut node = Node::new(Value::Seq(entries));
        node.span = Span::new(start, self.scanner.offset);
        Ok(Some(node))
    }

    /// l-block-sequence(n)
    fn parse_block_sequence(&mut self, n: usize) -> Result<Vec<Node>, Error> {
        let mut entries = Vec::new();
        let mut pending_comment: Option<String> = None;

        loop {
            // l-block-seq-entry(n)
            if !self.scanner.eat_spaces(n) {
                break;
            }

            if !self.scanner.eat('-') {
                self.scanner.offset -= n;
                break;
            }

            if !self.scanner.eat(' ') {
                return Err(self
                    .scanner
                    .error(ErrorKind::Expected("space after `-`".into())));
            }

            // The content is at implicit indentation n+2
            let mut entry = self.parse_seq_entry_value(n + 2)?;

            // Inline comment (only if we're still on the same line)
            if !self.scanner.is_break_or_eof() {
                let inline_comment = self.parse_inline_comment();
                entry.inline_comment = inline_comment;
            }

            // Attach pending comment from a previous gap
            if let Some(c) = pending_comment.take() {
                entry.comment = Some(c);
            }

            entries.push(entry);

            // After the entry, we may already be at the start of a new line
            // (compact mappings consume their own line breaks). Or we may
            // still be on the same line and need to consume the break.
            if !self.scanner.is_eof() && !self.scanner.eat_break() {
                // Not at a line break and not EOF — if we're at column 0-ish
                // (start of line), the compact mapping already ate the break.
                // Check if we're at the start of potential next content.
                let spaces = self.scanner.count_spaces()?;
                if spaces == n {
                    let rest = &self.scanner.input[self.scanner.offset + n..];
                    if rest.starts_with("- ") {
                        continue;
                    }
                }
                break;
            }

            // Skip gaps, collect comments for next entry
            pending_comment = self.skip_block_gaps(n);

            // Peek ahead to see if the next line is another seq entry
            let spaces = self.scanner.count_spaces()?;
            if spaces == n {
                let rest = &self.scanner.input[self.scanner.offset + n..];
                if rest.starts_with("- ") {
                    continue;
                }
            }

            break;
        }

        Ok(entries)
    }

    /// Parse the value part of a sequence entry at implicit indentation m.
    fn parse_seq_entry_value(&mut self, m: usize) -> Result<Node, Error> {
        // Try compact mapping: `- key: value`
        if let Some(node) = self.try_compact_mapping(m)? {
            return Ok(node);
        }

        // Otherwise: flow node (scalar or flow collection)
        self.parse_flow_node(Context::Block)
    }

    /// Try to parse a compact mapping (a mapping that starts on the same
    /// line as `- `).
    fn try_compact_mapping(&mut self, n: usize) -> Result<Option<Node>, Error> {
        let saved = self.scanner.offset;
        let start = self.scanner.offset;

        // Try to parse a mapping key followed by `: `
        let raw_key = match self.try_parse_mapping_key(Context::Block) {
            Ok(Some(k)) => k,
            Ok(None) => return Ok(None),
            Err(e) if is_hard_error(&e) => return Err(e),
            Err(_) => {
                self.scanner.offset = saved;
                return Ok(None);
            }
        };

        // Check for `: `
        self.scanner.skip_inline_whitespace();
        if !self.scanner.eat(':') || (!self.scanner.is_white() && !self.scanner.is_break_or_eof()) {
            // Not a mapping — rewind
            self.scanner.offset = saved;
            return Ok(None);
        }

        // Confirmed mapping — now validate the key
        let key = raw_key.validate(self.scanner.source(), self.scanner.offset)?;

        let mut map = IndexMap::new();
        let mut value_node = self.parse_mapping_value(n)?;
        value_node.inline_comment = value_node
            .inline_comment
            .or_else(|| self.parse_inline_comment());
        map.insert(key, value_node);

        // Check for additional mapping entries on subsequent lines
        loop {
            if !self.scanner.is_eof() && !self.scanner.eat_break() {
                // Collection value may have already consumed the line break.
                // Check if we're at the right indent to continue.
                let spaces = self.scanner.count_spaces()?;
                if spaces != n {
                    break;
                }
                // Fall through — we're already at a valid line start
            }
            if self.scanner.is_eof() {
                break;
            }
            let comment = self.skip_block_gaps(n);
            let spaces = self.scanner.count_spaces()?;
            if spaces != n {
                break;
            }

            let entry_saved = self.scanner.offset;
            self.scanner.eat_spaces(n);

            let Ok(Some(next_raw)) = self.try_parse_mapping_key(Context::Block) else {
                self.scanner.offset = entry_saved;
                break;
            };

            self.scanner.skip_inline_whitespace();
            if !self.scanner.eat(':')
                || (!self.scanner.is_white() && !self.scanner.is_break_or_eof())
            {
                self.scanner.offset = entry_saved;
                break;
            }

            let next_key = next_raw.validate(self.scanner.source(), self.scanner.offset)?;

            let mut value_node = self.parse_mapping_value(n)?;
            value_node.inline_comment = value_node
                .inline_comment
                .or_else(|| self.parse_inline_comment());
            if let Some(c) = comment {
                value_node.comment = Some(c);
            }

            let key_str = format!("{next_key}");
            if map.contains_key(&next_key) {
                return Err(Error::new(
                    ErrorKind::DuplicateKey(key_str),
                    Span::point(entry_saved),
                    self.scanner.source(),
                ));
            }
            map.insert(next_key, value_node);
        }

        let mut node = Node::new(Value::Map(map));
        node.span = Span::new(start, self.scanner.offset);
        Ok(Some(node))
    }

    // ── Block Mapping ────────────────────────────────────────────────

    /// Try to parse a block mapping at indentation n.
    fn try_block_mapping(&mut self, n: usize) -> Result<Option<Node>, Error> {
        let saved = self.scanner.offset;

        // Detect indentation
        let spaces = self.scanner.count_spaces()?;
        if spaces < n {
            return Ok(None);
        }

        let indent = spaces;

        // Try to parse a mapping key at this indent
        let key_saved = self.scanner.offset + indent;
        // Peek: is there a valid mapping key followed by `:`?
        let rest = &self.scanner.input[key_saved..];

        // Quick check: if it starts with `- `, it's a sequence, not a mapping
        if rest.starts_with("- ") {
            return Ok(None);
        }

        // Try to parse a full mapping
        let start = self.scanner.offset;
        match self.parse_block_mapping(indent) {
            Ok(map) => {
                let mut node = Node::new(Value::Map(map));
                node.span = Span::new(start, self.scanner.offset);
                Ok(Some(node))
            }
            Err(e) if is_hard_error(&e) => Err(e),
            Err(e) => {
                // If the error occurred well past the start, we committed
                // to this parse path (e.g. parsed several entries before
                // hitting an error in a value). Propagate instead of
                // silently backtracking.
                if e.span.start > start + indent {
                    return Err(e);
                }
                self.scanner.offset = saved;
                Ok(None)
            }
        }
    }

    /// l-block-mapping(n)
    fn parse_block_mapping(&mut self, n: usize) -> Result<IndexMap<MapKey, Node>, Error> {
        let mut map = IndexMap::new();
        let mut pending_comment: Option<String> = None;

        loop {
            // l-block-mapping-entry(n)
            if !self.scanner.eat_spaces(n) {
                break;
            }

            let entry_start = self.scanner.offset;
            let Some(raw_key) = self.try_parse_mapping_key(Context::Block)? else {
                self.scanner.offset -= n;
                break;
            };

            self.scanner.skip_inline_whitespace();

            if !self.scanner.eat(':') {
                self.scanner.offset = entry_start - n;
                break;
            }

            // `:` must be followed by space, break, or EOF
            if !self.scanner.is_white() && !self.scanner.is_break_or_eof() {
                self.scanner.offset = entry_start - n;
                break;
            }

            // Confirmed mapping entry — validate the key
            let key = raw_key.validate(self.scanner.source(), self.scanner.offset)?;

            let value_node = self.parse_mapping_value(n)?;
            let inline = self.parse_inline_comment();
            let mut final_node = value_node;
            final_node.inline_comment = final_node.inline_comment.or(inline);

            // Attach pending comment from a previous gap
            if let Some(c) = pending_comment.take() {
                final_node.comment = final_node.comment.or(Some(c));
            }

            let key_str = format!("{key}");
            if map.contains_key(&key) {
                return Err(Error::new(
                    ErrorKind::DuplicateKey(key_str),
                    Span::point(entry_start),
                    self.scanner.source(),
                ));
            }
            map.insert(key, final_node);

            // After the entry, we may already be at the start of a new line
            // (indented values like sequences consume their own line breaks).
            if !self.scanner.is_eof() && !self.scanner.eat_break() {
                // Check if we're already at the start of the next entry
                let spaces = self.scanner.count_spaces()?;
                if spaces == n {
                    let rest = &self.scanner.input[self.scanner.offset + n..];
                    if !rest.starts_with("- ") {
                        continue;
                    }
                }
                break;
            }

            // Skip gaps, collect comments for next entry
            pending_comment = self.skip_block_gaps(n);

            // Check for next entry at same indent
            let spaces = self.scanner.count_spaces()?;
            if spaces == n {
                // Peek: is this another mapping entry?
                let rest = &self.scanner.input[self.scanner.offset + n..];
                if rest.starts_with("- ") {
                    // Not a mapping entry
                    break;
                }
                continue;
            }

            break;
        }

        if map.is_empty() {
            return Err(self
                .scanner
                .error(ErrorKind::Expected("mapping entry".into())));
        }

        Ok(map)
    }

    /// Parse the value side of a mapping entry. Handles both same-line
    /// and next-line values.
    fn parse_mapping_value(&mut self, n: usize) -> Result<Node, Error> {
        if self.scanner.is_white() {
            // Value on same line
            self.scanner.skip_inline_whitespace();
            if self.scanner.is_break_or_eof() {
                // No value on this line — check next lines
                return self.parse_indented_value(n);
            }
            // Inline comment — value is on next lines
            if self.scanner.peek() == Some('#') {
                // Skip the comment text (leave the scanner at the line break)
                let _ = self.scanner.rest_of_line();
                return self.parse_indented_value(n);
            }
            self.parse_flow_node(Context::Block)
        } else if self.scanner.is_break_or_eof() {
            // Value on next line(s)
            self.parse_indented_value(n)
        } else {
            Err(self
                .scanner
                .error(ErrorKind::Expected("value after `:`".into())))
        }
    }

    /// Parse an indented value appearing on lines after a mapping key.
    fn parse_indented_value(&mut self, n: usize) -> Result<Node, Error> {
        self.enter_nested()?;
        let result = self.parse_indented_value_inner(n);
        self.leave_nested();
        result
    }

    fn parse_indented_value_inner(&mut self, n: usize) -> Result<Node, Error> {
        // Consume the line break
        if !self.scanner.eat_break() && !self.scanner.is_eof() {
            return Err(self.scanner.error(ErrorKind::Expected(
                "line break before indented value".into(),
            )));
        }

        // Skip blank lines and comments. We use usize::MAX here because the
        // value's indentation hasn't been determined yet — comments belonging
        // to the indented block may appear at any depth > n.
        let comment = self.skip_block_gaps(usize::MAX);

        // Auto-detect indentation
        let m = self.scanner.count_spaces()?;

        // Try block sequence first — sequences are allowed at m >= n
        // because `- ` provides implicit nesting (+2).
        if m >= n
            && let Some(mut node) = self.try_block_sequence(m)?
        {
            node.comment = comment;
            return Ok(node);
        }

        // For mappings and scalars, m must be > n.
        if m <= n {
            return Err(self.scanner.error(ErrorKind::Expected(format!(
                "indented value (indent > {n})"
            ))));
        }

        // Try block mapping (m > n)
        if let Some(mut node) = self.try_block_mapping(m)? {
            node.comment = comment;
            return Ok(node);
        }

        // Scalar or flow value at indent m
        self.scanner.eat_spaces(m);
        let mut node = self.parse_flow_node(Context::Block)?;
        node.comment = comment;
        Ok(node)
    }

    /// Try to parse a mapping key. Returns None if the current position
    /// doesn't look like a valid key (without consuming input).
    fn try_parse_mapping_key(&mut self, ctx: Context) -> Result<Option<RawMapKey>, Error> {
        let saved = self.scanner.offset;

        match self.scanner.peek() {
            Some('"') => {
                // Quoted key
                if self.scanner.input[self.scanner.offset..].starts_with("\"\"\"") {
                    let node = self.parse_triple_quoted()?;
                    match node.value {
                        Value::Str(s) => Ok(Some(RawMapKey::Valid(MapKey::String(s)))),
                        _ => unreachable!(),
                    }
                } else {
                    let node = self.parse_double_quoted()?;
                    match node.value {
                        Value::Str(s) => Ok(Some(RawMapKey::Valid(MapKey::String(s)))),
                        _ => unreachable!(),
                    }
                }
            }
            Some(ch) if Self::is_plain_first(ch) => {
                // Bare key — scan until `:` followed by space/break/eof
                let text = self.scan_mapping_key_bare(ctx)?;
                if text.is_empty() {
                    self.scanner.offset = saved;
                    return Ok(None);
                }

                // Resolve the key. Null and float produce deferred errors
                // — the caller must call `validate_key` after confirming
                // this is actually a mapping entry (colon follows).
                if text == "null" {
                    Ok(Some(RawMapKey::Null(saved)))
                } else if text == "true" {
                    Ok(Some(RawMapKey::Valid(MapKey::Bool(true))))
                } else if text == "false" {
                    Ok(Some(RawMapKey::Valid(MapKey::Bool(false))))
                } else {
                    match Self::try_parse_int(&text) {
                        Ok(Some(i)) => Ok(Some(RawMapKey::Valid(MapKey::Int(i)))),
                        Err(()) => Err(self.scanner.error_at(ErrorKind::IntegerOverflow, saved)),
                        Ok(None) => {
                            if Self::try_parse_float(&text).is_some() {
                                Ok(Some(RawMapKey::Float(saved)))
                            } else {
                                Ok(Some(RawMapKey::Valid(MapKey::String(text))))
                            }
                        }
                    }
                }
            }
            _ => Ok(None),
        }
    }

    /// Scan a bare mapping key (stops before `:` followed by space/break/eof).
    fn scan_mapping_key_bare(&mut self, ctx: Context) -> Result<String, Error> {
        let start = self.scanner.offset;

        loop {
            match self.scanner.peek() {
                Some(':') => {
                    // Check if followed by space, break, or eof
                    let next = self.scanner.peek_nth(1);
                    if next.is_none()
                        || next == Some(' ')
                        || next == Some('\t')
                        || next == Some('\n')
                        || next == Some('\r')
                    {
                        // End of key
                        let end = self.scanner.offset;
                        let text = self.scanner.input[start..end].trim_end().to_string();
                        return Ok(text);
                    }
                    self.scanner.advance();
                }
                Some(ch) if ch == '\n' || ch == '\r' => {
                    let text = self.scanner.input[start..self.scanner.offset]
                        .trim_end()
                        .to_string();
                    return Ok(text);
                }
                Some(',' | ']' | '}') if ctx == Context::Flow => {
                    let text = self.scanner.input[start..self.scanner.offset]
                        .trim_end()
                        .to_string();
                    return Ok(text);
                }
                Some('#') => {
                    // '#' is not allowed in bare strings; terminate here
                    let text = self.scanner.input[start..self.scanner.offset]
                        .trim_end()
                        .to_string();
                    return Ok(text);
                }
                Some(ch) if !Scanner::is_printable(ch) => {
                    return Err(self.scanner.error(ErrorKind::NonPrintable(ch)));
                }
                Some(_) => {
                    self.scanner.advance();
                }
                None => {
                    let text = self.scanner.input[start..self.scanner.offset]
                        .trim_end()
                        .to_string();
                    return Ok(text);
                }
            }
        }
    }

    // ── Flow Collections ─────────────────────────────────────────────

    /// c-flow-sequence
    fn parse_flow_sequence(&mut self) -> Result<Node, Error> {
        let start = self.scanner.offset;
        self.scanner.advance(); // `[`
        self.skip_flow_whitespace();

        let mut entries = Vec::new();

        if self.scanner.peek() != Some(']') {
            let node = self.parse_flow_node(Context::Flow)?;
            entries.push(node);

            loop {
                self.skip_flow_whitespace();
                if self.scanner.eat(',') {
                    self.skip_flow_whitespace();
                    if self.scanner.peek() == Some(']') {
                        break; // trailing comma
                    }
                    let node = self.parse_flow_node(Context::Flow)?;
                    entries.push(node);
                } else {
                    break;
                }
            }
        }

        self.skip_flow_whitespace();
        if !self.scanner.eat(']') {
            return Err(self
                .scanner
                .error(ErrorKind::Expected("`]` to close flow sequence".into())));
        }

        let mut node = Node::new(Value::Seq(entries));
        node.span = Span::new(start, self.scanner.offset);
        Ok(node)
    }

    /// c-flow-mapping
    fn parse_flow_mapping(&mut self) -> Result<Node, Error> {
        let start = self.scanner.offset;
        self.scanner.advance(); // `{`
        self.skip_flow_whitespace();

        let mut map = IndexMap::new();

        if self.scanner.peek() != Some('}') {
            let (key, value) = self.parse_flow_mapping_entry()?;
            map.insert(key, value);

            loop {
                self.skip_flow_whitespace();
                if self.scanner.eat(',') {
                    self.skip_flow_whitespace();
                    if self.scanner.peek() == Some('}') {
                        break; // trailing comma
                    }
                    let (key, value) = self.parse_flow_mapping_entry()?;
                    let key_str = format!("{key}");
                    if map.contains_key(&key) {
                        return Err(Error::new(
                            ErrorKind::DuplicateKey(key_str),
                            Span::point(self.scanner.offset),
                            self.scanner.source(),
                        ));
                    }
                    map.insert(key, value);
                } else {
                    break;
                }
            }
        }

        self.skip_flow_whitespace();
        if !self.scanner.eat('}') {
            return Err(self
                .scanner
                .error(ErrorKind::Expected("`}` to close flow mapping".into())));
        }

        let mut node = Node::new(Value::Map(map));
        node.span = Span::new(start, self.scanner.offset);
        Ok(node)
    }

    /// ns-flow-mapping-entry
    fn parse_flow_mapping_entry(&mut self) -> Result<(MapKey, Node), Error> {
        let raw_key = self.try_parse_mapping_key(Context::Flow)?.ok_or_else(|| {
            self.scanner
                .error(ErrorKind::Expected("mapping key".into()))
        })?;

        self.skip_flow_whitespace();

        if !self.scanner.eat(':') {
            return Err(self
                .scanner
                .error(ErrorKind::Expected("`:` after mapping key".into())));
        }

        // Confirmed mapping — validate the key
        let key = raw_key.validate(self.scanner.source(), self.scanner.offset)?;

        self.skip_flow_whitespace();

        let value = self.parse_flow_node(Context::Flow)?;

        Ok((key, value))
    }

    /// Skip whitespace in flow context (spaces, tabs, line breaks, comments).
    fn skip_flow_whitespace(&mut self) {
        loop {
            match self.scanner.peek() {
                Some(' ' | '\t') => {
                    self.scanner.advance();
                }
                Some('\n' | '\r') => {
                    self.scanner.eat_break();
                }
                Some('#') => {
                    // Comment — skip to end of line
                    let _ = self.scanner.rest_of_line();
                }
                _ => break,
            }
        }
    }
}
