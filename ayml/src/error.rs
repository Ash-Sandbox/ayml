use std::fmt;

use ayml_core::{Location, Span};

/// Error type for AYML serialization and deserialization.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The input is not valid AYML.
    #[error(transparent)]
    Syntax(#[from] ayml_core::Error),
    /// The input is valid AYML but does not match the target type,
    /// or a value cannot be represented in AYML.
    #[error("{}", DataDisplay(self))]
    Data {
        message: String,
        /// Byte range of the offending node or key.
        span: Option<Span>,
        /// Line and column of the span.
        location: Option<Location>,
    },
    /// I/O error while reading or writing.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// The input is not valid UTF-8.
    #[error(transparent)]
    Utf8(#[from] std::str::Utf8Error),
}

struct DataDisplay<'a>(&'a Error);

impl fmt::Display for DataDisplay<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Error::Data {
            message, location, ..
        } = self.0
        {
            if let Some(location) = location {
                write!(f, "{location}: ")?;
            }
            f.write_str(message)?;
        }
        Ok(())
    }
}

impl Error {
    pub(crate) fn data(message: impl fmt::Display) -> Self {
        Self::Data {
            message: message.to_string(),
            span: None,
            location: None,
        }
    }

    /// Attach a span if the error does not have one yet.
    pub(crate) fn at(mut self, at: Span) -> Self {
        if let Self::Data { span, .. } = &mut self
            && span.is_none()
        {
            *span = Some(at);
        }
        self
    }

    /// Resolve the span to a line and column in `source`.
    pub(crate) fn locate(mut self, source: &str) -> Self {
        if let Self::Data { span, location, .. } = &mut self
            && let Some(span) = span
        {
            *location = Some(Location::of(source, span.start));
        }
        self
    }

    /// The line and column the error refers to, if known.
    #[must_use]
    pub fn location(&self) -> Option<Location> {
        match self {
            Self::Syntax(e) => Some(e.location),
            Self::Data { location, .. } => *location,
            Self::Io(_) | Self::Utf8(_) => None,
        }
    }
}

impl serde::de::Error for Error {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Self::data(msg)
    }
}

impl serde::ser::Error for Error {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Self::data(msg)
    }
}

/// Alias for `Result<T, Error>`.
pub type Result<T> = std::result::Result<T, Error>;
