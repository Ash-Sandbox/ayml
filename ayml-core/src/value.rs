use indexmap::IndexMap;
use std::fmt;

use crate::error::Span;

/// A node in the AYML document tree.
///
/// Wraps a [`Value`] with optional comment metadata. The reference
/// implementation preserves comments; other implementations may discard them.
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    /// Comments on the line(s) immediately above this node.
    /// Multi-line comments are joined with newlines. The `#` prefix and
    /// leading whitespace are stripped.
    pub comment: Option<String>,
    /// Inline comment trailing this node on the same line.
    pub inline_comment: Option<String>,
    /// The value of this node.
    pub value: Value,
    /// Byte-offset span of this node in the source input.
    /// Defaults to `Span::new(0, 0)` when not set by the parser.
    pub span: Span,
}

impl Node {
    /// Create a node with no comments and a default (empty) span.
    #[must_use]
    pub const fn new(value: Value) -> Self {
        Self {
            comment: None,
            inline_comment: None,
            value,
            span: Span::new(0, 0),
        }
    }

    /// Create a node with a top comment and a default (empty) span.
    pub fn with_comment(value: Value, comment: impl Into<String>) -> Self {
        Self {
            comment: Some(comment.into()),
            inline_comment: None,
            value,
            span: Span::new(0, 0),
        }
    }
}

/// A key in an AYML mapping.
///
/// Restricted to hashable, equality-comparable types: bool, integer, and
/// string. Float and null keys are not allowed per the spec.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MapKey {
    Bool(bool),
    Int(i64),
    String(String),
}

impl fmt::Display for MapKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bool(b) => write!(f, "{b}"),
            Self::Int(i) => write!(f, "{i}"),
            Self::String(s) => write!(f, "{s}"),
        }
    }
}

/// An AYML value.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Seq(Vec<Node>),
    Map(IndexMap<MapKey, Node>),
}

impl Value {
    #[must_use]
    pub const fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

    #[must_use]
    pub const fn is_scalar(&self) -> bool {
        !matches!(self, Self::Seq(_) | Self::Map(_))
    }

    #[must_use]
    pub const fn is_collection(&self) -> bool {
        matches!(self, Self::Seq(_) | Self::Map(_))
    }

    #[must_use]
    pub const fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(b) => Some(*b),
            _ => None,
        }
    }

    #[must_use]
    pub const fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Int(i) => Some(*i),
            _ => None,
        }
    }

    #[must_use]
    pub const fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Float(f) => Some(*f),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Str(s) => Some(s),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_sequence(&self) -> Option<&[Node]> {
        match self {
            Self::Seq(s) => Some(s),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_mapping(&self) -> Option<&IndexMap<MapKey, Node>> {
        match self {
            Self::Map(m) => Some(m),
            _ => None,
        }
    }
}
