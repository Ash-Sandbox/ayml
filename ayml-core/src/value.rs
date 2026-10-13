use indexmap::IndexMap;
use std::borrow::{Borrow, Cow};
use std::fmt;
use std::hash::{Hash, Hasher};

use crate::error::Span;
use crate::scalar::Scalar;

/// A node in the AYML document tree.
///
/// Equality compares values and comments, not spans.
#[derive(Debug, Clone, Default)]
pub struct Node {
    /// The value of this node.
    pub value: Value,
    /// Comment lines immediately above this node.
    pub comment: Option<String>,
    /// The comment at the end of this node's line.
    pub inline_comment: Option<String>,
    /// Comment lines after the last entry of a collection.
    pub trailing_comment: Option<String>,
    /// Byte range of the node in the source.
    pub span: Span,
}

impl Node {
    /// Create a node with no comments.
    #[must_use]
    pub const fn new(value: Value) -> Self {
        Self {
            value,
            comment: None,
            inline_comment: None,
            trailing_comment: None,
            span: Span::new(0, 0),
        }
    }

    /// Set the comment above this node.
    #[must_use]
    pub fn with_comment(mut self, comment: impl Into<String>) -> Self {
        self.comment = Some(comment.into());
        self
    }

    /// Set the comment at the end of this node's line.
    #[must_use]
    pub fn with_inline_comment(mut self, comment: impl Into<String>) -> Self {
        self.inline_comment = Some(comment.into());
        self
    }

    /// Set the comment after the last entry of this collection.
    #[must_use]
    pub fn with_trailing_comment(mut self, comment: impl Into<String>) -> Self {
        self.trailing_comment = Some(comment.into());
        self
    }

    /// Look up a mapping entry by key.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&Self> {
        self.value.as_mapping()?.get(key)
    }
}

impl PartialEq for Node {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
            && self.comment == other.comment
            && self.inline_comment == other.inline_comment
            && self.trailing_comment == other.trailing_comment
    }
}

impl From<Value> for Node {
    fn from(value: Value) -> Self {
        Self::new(value)
    }
}

/// A mapping key: a string plus its source location.
#[derive(Debug, Clone, Default)]
pub struct Key {
    /// The key's unescaped string value.
    pub name: String,
    /// Byte range of the key, including quotation marks, in the source.
    pub span: Span,
}

impl Key {
    /// Create a key with an empty span.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            span: Span::new(0, 0),
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.name
    }
}

impl PartialEq for Key {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl Eq for Key {}

impl Hash for Key {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}

impl Borrow<str> for Key {
    fn borrow(&self) -> &str {
        &self.name
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name)
    }
}

impl From<&str> for Key {
    fn from(name: &str) -> Self {
        Self::new(name)
    }
}

impl From<String> for Key {
    fn from(name: String) -> Self {
        Self::new(name)
    }
}

/// An ordered AYML mapping.
pub type Mapping = IndexMap<Key, Node>;

/// An AYML value.
///
/// Equality treats two NaN floats as equal, so that structural comparison of
/// documents is reflexive, and compares mapping entries in order.
#[derive(Debug, Clone, Default)]
pub enum Value {
    #[default]
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Seq(Vec<Node>),
    Map(Mapping),
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Null, Self::Null) => true,
            (Self::Bool(a), Self::Bool(b)) => a == b,
            (Self::Int(a), Self::Int(b)) => a == b,
            (Self::Float(a), Self::Float(b)) => a == b || (a.is_nan() && b.is_nan()),
            (Self::Str(a), Self::Str(b)) => a == b,
            (Self::Seq(a), Self::Seq(b)) => a == b,
            (Self::Map(a), Self::Map(b)) => a.len() == b.len() && a.iter().eq(b.iter()),
            _ => false,
        }
    }
}

impl Value {
    #[must_use]
    pub const fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

    #[must_use]
    pub const fn is_scalar(&self) -> bool {
        !self.is_collection()
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
    pub const fn as_mapping(&self) -> Option<&Mapping> {
        match self {
            Self::Map(m) => Some(m),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_scalar(&self) -> Option<Scalar<'_>> {
        Some(match self {
            Self::Null => Scalar::Null,
            Self::Bool(b) => Scalar::Bool(*b),
            Self::Int(i) => Scalar::Int(*i),
            Self::Float(f) => Scalar::Float(*f),
            Self::Str(s) => Scalar::Str(Cow::Borrowed(s)),
            Self::Seq(_) | Self::Map(_) => return None,
        })
    }

    /// Value type name, for error messages.
    #[must_use]
    pub const fn type_name(&self) -> &'static str {
        match self {
            Self::Null => "null",
            Self::Bool(_) => "bool",
            Self::Int(_) => "int",
            Self::Float(_) => "float",
            Self::Str(_) => "string",
            Self::Seq(_) => "sequence",
            Self::Map(_) => "mapping",
        }
    }
}

impl From<Scalar<'_>> for Value {
    fn from(scalar: Scalar<'_>) -> Self {
        match scalar {
            Scalar::Null => Self::Null,
            Scalar::Bool(b) => Self::Bool(b),
            Scalar::Int(i) => Self::Int(i),
            Scalar::Float(f) => Self::Float(f),
            Scalar::Str(s) => Self::Str(s.into_owned()),
        }
    }
}

impl From<bool> for Value {
    fn from(b: bool) -> Self {
        Self::Bool(b)
    }
}

impl From<i64> for Value {
    fn from(i: i64) -> Self {
        Self::Int(i)
    }
}

impl From<f64> for Value {
    fn from(f: f64) -> Self {
        Self::Float(f)
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Self::Str(s.to_owned())
    }
}

impl From<String> for Value {
    fn from(s: String) -> Self {
        Self::Str(s)
    }
}

impl From<Vec<Node>> for Value {
    fn from(items: Vec<Node>) -> Self {
        Self::Seq(items)
    }
}

impl From<Mapping> for Value {
    fn from(map: Mapping) -> Self {
        Self::Map(map)
    }
}

impl FromIterator<Node> for Value {
    fn from_iter<I: IntoIterator<Item = Node>>(iter: I) -> Self {
        Self::Seq(iter.into_iter().collect())
    }
}

impl<K: Into<Key>> FromIterator<(K, Node)> for Value {
    fn from_iter<I: IntoIterator<Item = (K, Node)>>(iter: I) -> Self {
        Self::Map(iter.into_iter().map(|(k, v)| (k.into(), v)).collect())
    }
}
