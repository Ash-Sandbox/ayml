//! Comment-preserving wrapper for AYML values.

use indexmap::IndexMap;
use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::cell::RefCell;
use std::fmt;
use std::marker::PhantomData;

use crate::Value;

pub(crate) const COMMENTED_STRUCT: &str = "$ayml::Commented";
pub(crate) const FIELD_COMMENT: &str = "$ayml::comment";
pub(crate) const FIELD_INLINE_COMMENT: &str = "$ayml::inline_comment";
pub(crate) const FIELD_TRAILING_COMMENT: &str = "$ayml::trailing_comment";
pub(crate) const FIELD_VALUE: &str = "$ayml::value";
pub(crate) const FIELDS: &[&str] = &[
    FIELD_COMMENT,
    FIELD_INLINE_COMMENT,
    FIELD_TRAILING_COMMENT,
    FIELD_VALUE,
];

/// A typed value with attached comment.
///
/// ```
/// use ayml::Commented;
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Serialize, Deserialize)]
/// struct Config {
///     port: Commented<u16>,
/// }
///
/// let input = "# listen port\nport: 8080 # default\n";
/// let config: Config = ayml::from_str(input).unwrap();
/// assert_eq!(config.port.value, 8080);
/// assert_eq!(config.port.comment.as_deref(), Some("listen port"));
/// assert_eq!(config.port.inline_comment.as_deref(), Some("default"));
/// assert_eq!(ayml::to_string(&config).unwrap(), input);
/// ```
///
/// Comments follow comment association rules; see [`ayml_core::Node`].
/// If the root or a sequence entry is not itself `Commented`, its leading
/// comment is given to its first entry if that entry is `Commented`.
///
/// Serde buffers the input of `#[serde(flatten)]` fields and of internally
/// tagged and untagged enums, so a `Commented<T>` inside them is
/// deserialized without comments.
///
/// `Commented<T>` is transparent to all non-AYML serde crates.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Commented<T> {
    pub comment: Option<String>,
    pub inline_comment: Option<String>,
    pub trailing_comment: Option<String>,
    /// The wrapped value.
    pub value: T,
}

impl<T> Commented<T> {
    /// Wrap a value with no comments.
    pub const fn new(value: T) -> Self {
        Self {
            comment: None,
            inline_comment: None,
            trailing_comment: None,
            value,
        }
    }

    /// Set the comment above the value.
    #[must_use]
    pub fn with_comment(mut self, comment: impl Into<String>) -> Self {
        self.comment = Some(comment.into());
        self
    }

    /// Set the comment at the end of the value's line.
    #[must_use]
    pub fn with_inline_comment(mut self, comment: impl Into<String>) -> Self {
        self.inline_comment = Some(comment.into());
        self
    }

    /// Set the comment after the last entry of a collection value.
    #[must_use]
    pub fn with_trailing_comment(mut self, comment: impl Into<String>) -> Self {
        self.trailing_comment = Some(comment.into());
        self
    }
}

impl<T> From<T> for Commented<T> {
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

/// The comments of a `Commented<T>` that is being serialized.
#[derive(Default)]
pub(crate) struct Comments {
    pub comment: Option<String>,
    pub inline_comment: Option<String>,
    pub trailing_comment: Option<String>,
}

impl Comments {
    /// Each comment of `self`, or else of `other`.
    pub fn or(self, other: Self) -> Self {
        Self {
            comment: self.comment.or(other.comment),
            inline_comment: self.inline_comment.or(other.inline_comment),
            trailing_comment: self.trailing_comment.or(other.trailing_comment),
        }
    }

    pub fn borrow(&self) -> ayml_core::Comments<'_> {
        ayml_core::Comments {
            comment: self.comment.as_deref(),
            inline: self.inline_comment.as_deref(),
            trailing: self.trailing_comment.as_deref(),
        }
    }
}

thread_local! {
    /// Comments passed from `Commented::serialize` to AYML's serializer.
    static PENDING_COMMENTS: RefCell<Option<Comments>> = const { RefCell::new(None) };
}

/// Set comments for a `Commented<T>` whose value is being serialized.
pub(crate) fn take_pending_comments() -> Option<Comments> {
    PENDING_COMMENTS.with(|pending| pending.borrow_mut().take())
}

impl<T: Serialize> Serialize for Commented<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let has_comments = self.comment.is_some()
            || self.inline_comment.is_some()
            || self.trailing_comment.is_some();
        if has_comments {
            PENDING_COMMENTS.with(|pending| {
                *pending.borrow_mut() = Some(Comments {
                    comment: self.comment.clone(),
                    inline_comment: self.inline_comment.clone(),
                    trailing_comment: self.trailing_comment.clone(),
                });
            });
        }
        let result = serializer.serialize_newtype_struct(COMMENTED_STRUCT, &self.value);
        if has_comments {
            take_pending_comments();
        }
        result
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Commented<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_newtype_struct(COMMENTED_STRUCT, CommentedVisitor(PhantomData))
    }
}

struct CommentedVisitor<T>(PhantomData<T>);

fn plain<'de, T: Deserialize<'de>, E: de::Error>(value: Value) -> Result<Commented<T>, E> {
    T::deserialize(value).map(Commented::new).map_err(E::custom)
}

impl<'de, T: Deserialize<'de>> Visitor<'de> for CommentedVisitor<T> {
    type Value = Commented<T>;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "a value")
    }

    fn visit_bool<E: de::Error>(self, v: bool) -> Result<Self::Value, E> {
        plain(Value::Bool(v))
    }

    fn visit_i64<E: de::Error>(self, v: i64) -> Result<Self::Value, E> {
        plain(Value::Int(v))
    }

    fn visit_u64<E: de::Error>(self, v: u64) -> Result<Self::Value, E> {
        let v = i64::try_from(v)
            .map_err(|_| E::custom(format_args!("integer {v} exceeds the 64-bit signed range")))?;
        plain(Value::Int(v))
    }

    fn visit_f64<E: de::Error>(self, v: f64) -> Result<Self::Value, E> {
        plain(Value::Float(v))
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
        plain(Value::Str(v.to_owned()))
    }

    fn visit_string<E: de::Error>(self, v: String) -> Result<Self::Value, E> {
        plain(Value::Str(v))
    }

    fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
        plain(Value::Null)
    }

    fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
        plain(Value::Null)
    }

    fn visit_some<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        T::deserialize(deserializer).map(Commented::new)
    }

    fn visit_newtype_struct<D: Deserializer<'de>>(
        self,
        deserializer: D,
    ) -> Result<Self::Value, D::Error> {
        deserializer.deserialize_any(self)
    }

    fn visit_seq<A: de::SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
        let mut items = Vec::new();
        while let Some(item) = seq.next_element::<Value>()? {
            items.push(item);
        }
        plain(Value::Seq(items))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        let Some(first) = map.next_key::<String>()? else {
            return plain(Value::Map(IndexMap::new()));
        };
        if !FIELDS.contains(&first.as_str()) {
            let mut entries = IndexMap::new();
            entries.insert(first, map.next_value::<Value>()?);
            while let Some((key, value)) = map.next_entry::<String, Value>()? {
                entries.insert(key, value);
            }
            return plain(Value::Map(entries));
        }

        let mut comment = None;
        let mut inline_comment = None;
        let mut trailing_comment = None;
        let mut value = None;
        let mut key = Some(first);
        while let Some(k) = key {
            match k.as_str() {
                FIELD_COMMENT => comment = map.next_value()?,
                FIELD_INLINE_COMMENT => inline_comment = map.next_value()?,
                FIELD_TRAILING_COMMENT => trailing_comment = map.next_value()?,
                FIELD_VALUE => value = Some(map.next_value()?),
                _ => {
                    map.next_value::<de::IgnoredAny>()?;
                }
            }
            key = map.next_key::<String>()?;
        }
        Ok(Commented {
            comment,
            inline_comment,
            trailing_comment,
            value: value.ok_or_else(|| de::Error::missing_field(FIELD_VALUE))?,
        })
    }
}

#[cfg(feature = "schemars")]
impl<T: schemars::JsonSchema> schemars::JsonSchema for Commented<T> {
    fn inline_schema() -> bool {
        T::inline_schema()
    }

    fn schema_name() -> std::borrow::Cow<'static, str> {
        T::schema_name()
    }

    fn schema_id() -> std::borrow::Cow<'static, str> {
        T::schema_id()
    }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        T::json_schema(generator)
    }
}
