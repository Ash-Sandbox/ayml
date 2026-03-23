use std::fmt;

/// A byte-offset span in the source input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

/// The kind of error that occurred.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorKind {
    /// Unexpected character encountered.
    UnexpectedChar(char),
    /// Unexpected end of input.
    UnexpectedEof,
    /// Invalid escape sequence.
    InvalidEscape(String),
    /// Tab used for indentation.
    TabIndent,
    /// Duplicate mapping key.
    DuplicateKey(String),
    /// Null used as a mapping key.
    NullKey,
    /// Float used as a mapping key.
    FloatKey,
    /// Expected a specific token or production.
    Expected(String),
    /// A byte order mark was found.
    ByteOrderMark,
    /// Non-printable character found.
    NonPrintable(char),
    /// An integer literal that overflows i64.
    IntegerOverflow,
    /// Nesting depth exceeded the configured limit.
    RecursionLimit,
}

/// A parse or emit error with location information.
#[derive(Debug, Clone)]
pub struct Error {
    pub kind: ErrorKind,
    pub span: Span,
    /// Human-readable line number (1-based).
    pub line: usize,
    /// Human-readable column number (1-based).
    pub column: usize,
}

impl Error {
    /// Create an error and compute line/column from the source input.
    #[must_use]
    pub fn new(kind: ErrorKind, span: Span, source: &str) -> Self {
        let (line, column) = offset_to_line_col(source, span.start);
        Self {
            kind,
            span,
            line,
            column,
        }
    }
}

fn offset_to_line_col(source: &str, offset: usize) -> (usize, usize) {
    let offset = offset.min(source.len());
    let mut line = 1;
    let mut col = 1;
    let bytes = source.as_bytes();
    let mut i = 0;
    while i < offset {
        if bytes[i] == b'\r' {
            line += 1;
            col = 1;
            // If \r\n, skip the \n as part of the same line break
            if i + 1 < bytes.len() && bytes[i + 1] == b'\n' {
                i += 1;
            }
        } else if bytes[i] == b'\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
        i += 1;
    }
    (line, col)
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}: ", self.line, self.column)?;
        match &self.kind {
            ErrorKind::UnexpectedChar(c) => write!(f, "unexpected character '{c}'"),
            ErrorKind::UnexpectedEof => write!(f, "unexpected end of input"),
            ErrorKind::InvalidEscape(s) => write!(f, "invalid escape sequence: '{s}'"),
            ErrorKind::TabIndent => write!(f, "tabs must not be used for indentation"),
            ErrorKind::DuplicateKey(k) => write!(f, "duplicate mapping key: '{k}'"),
            ErrorKind::NullKey => write!(f, "null cannot be used as a mapping key"),
            ErrorKind::FloatKey => write!(f, "float cannot be used as a mapping key"),
            ErrorKind::Expected(what) => write!(f, "expected {what}"),
            ErrorKind::ByteOrderMark => write!(f, "byte order mark is not allowed"),
            ErrorKind::NonPrintable(c) => {
                write!(f, "non-printable character U+{:04X}", *c as u32)
            }
            ErrorKind::IntegerOverflow => write!(f, "integer literal overflows i64"),
            ErrorKind::RecursionLimit => write!(f, "nesting depth exceeds recursion limit"),
        }
    }
}

impl std::error::Error for Error {}
