use std::fmt;

/// A byte-offset span in the source input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Span {
    /// Start byte offset (inclusive).
    pub start: usize,
    /// End byte offset (exclusive).
    pub end: usize,
}

impl Span {
    #[must_use]
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    #[must_use]
    pub const fn point(offset: usize) -> Self {
        Self {
            start: offset,
            end: offset,
        }
    }
}

/// A 1-based line and column in the source. Columns count Unicode scalar values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Location {
    pub line: usize,
    pub column: usize,
}

impl Location {
    /// Compute the location of a byte offset in `source`. CR, LF, and CRLF
    /// each count as one line break. Offsets past the end are clamped.
    #[must_use]
    pub fn of(source: &str, offset: usize) -> Self {
        let mut offset = offset.min(source.len());
        while !source.is_char_boundary(offset) {
            offset -= 1;
        }
        let before = &source[..offset];
        let bytes = before.as_bytes();
        let mut line = 1;
        let mut line_start = 0;
        for (i, &b) in bytes.iter().enumerate() {
            let is_break = b == b'\n' || (b == b'\r' && bytes.get(i + 1) != Some(&b'\n'));
            if is_break {
                line += 1;
                line_start = i + 1;
            }
        }
        let column = before[line_start..].chars().count() + 1;
        Self { line, column }
    }
}

impl fmt::Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line, self.column)
    }
}

/// The kind of error that occurred.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorKind {
    /// The input does not match the grammar.
    Syntax(String),
    /// The input ended in the middle of a construct.
    UnexpectedEof(String),
    /// The document contains no root node.
    EmptyDocument,
    /// Invalid escape sequence.
    InvalidEscape(String),
    /// Tab used for indentation.
    TabIndent,
    /// Duplicate mapping key.
    DuplicateKey(String),
    /// A byte order mark was found at the start of the input.
    ByteOrderMark,
    /// A character outside `c-printable` appeared literally.
    NonPrintable(char),
    /// An integer literal outside the 64-bit signed range.
    IntegerOverflow,
    /// A finite float literal too large to represent.
    FloatOverflow,
    /// Nesting depth exceeded the configured limit.
    RecursionLimit,
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Syntax(msg) => f.write_str(msg),
            Self::UnexpectedEof(what) => write!(f, "unexpected end of input {what}"),
            Self::EmptyDocument => f.write_str("document is empty (expected a root value)"),
            Self::InvalidEscape(s) => write!(f, "invalid escape sequence: {s}"),
            Self::TabIndent => f.write_str("tabs must not be used for indentation"),
            Self::DuplicateKey(k) => write!(f, "duplicate mapping key `{k}`"),
            Self::ByteOrderMark => f.write_str("byte order mark is not allowed"),
            Self::NonPrintable(c) => write!(
                f,
                "character U+{:04X} may not appear literally (use an escape sequence in a quoted string)",
                *c as u32
            ),
            Self::IntegerOverflow => {
                f.write_str("integer literal is out of range (AYML integers are 64-bit signed)")
            }
            Self::FloatOverflow => f.write_str("float literal is out of range"),
            Self::RecursionLimit => f.write_str("nesting depth exceeds the recursion limit"),
        }
    }
}

/// A parse error with location information.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    pub kind: ErrorKind,
    pub span: Span,
    /// Line and column of `span.start`.
    pub location: Location,
}

impl Error {
    /// Create an error, computing its line and column from the source input.
    #[must_use]
    pub fn new(kind: ErrorKind, span: Span, source: &str) -> Self {
        Self {
            kind,
            span,
            location: Location::of(source, span.start),
        }
    }

    /// 1-based line number.
    #[must_use]
    pub const fn line(&self) -> usize {
        self.location.line
    }

    /// 1-based column number, counted in Unicode scalar values.
    #[must_use]
    pub const fn column(&self) -> usize {
        self.location.column
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.location, self.kind)
    }
}

impl std::error::Error for Error {}
