//! Deserialization from AYML text or a parsed [`Node`] tree.

use std::borrow::Cow;

use serde::de::{self, IntoDeserializer, Unexpected, Visitor};

use ayml_core::reader::{Comment, Event};
use ayml_core::{Node, NodeReader, Reader, Scalar, Span, scalar};

use crate::commented::{COMMENTED_STRUCT, FIELDS as COMMENTED_FIELDS};
use crate::error::{Error, Result};

/// Deserialize a `T` from a string of AYML text.
///
/// # Errors
///
/// Returns an error if the input is not valid AYML or cannot be
/// deserialized into `T`.
pub fn from_str<'a, T: de::Deserialize<'a>>(s: &'a str) -> Result<T> {
    let mut de = Deserializer::new(Reader::new(s));
    match T::deserialize(&mut de) {
        Ok(value) => {
            de.end()?;
            Ok(value)
        }
        Err(e @ Error::Syntax(_)) => Err(e),
        Err(e) => Err(de.syntax_error().unwrap_or_else(|| e.locate(s))),
    }
}

/// Deserialize a `T` from a slice of AYML bytes.
///
/// # Errors
///
/// Returns an error if the input is not valid UTF-8, not valid AYML,
/// or cannot be deserialized into `T`.
pub fn from_slice<'a, T: de::Deserialize<'a>>(bytes: &'a [u8]) -> Result<T> {
    from_str(std::str::from_utf8(bytes)?)
}

/// Deserialize a `T` from an AYML reader. The whole input is read before
/// parsing.
///
/// # Errors
///
/// Returns an error on I/O failure, invalid UTF-8, invalid AYML, or if the
/// data cannot be deserialized into `T`.
pub fn from_reader<R: std::io::Read, T: de::DeserializeOwned>(mut rdr: R) -> Result<T> {
    let mut bytes = Vec::new();
    rdr.read_to_end(&mut bytes)?;
    from_slice(&bytes)
}

/// Deserialize a `T` from a parsed node tree.
///
/// # Errors
///
/// Returns an error if the tree cannot be deserialized into `T`.
pub fn from_node<'de, T: de::Deserialize<'de>>(node: &'de Node) -> Result<T> {
    let mut de = Deserializer::new(NodeReader::new(node));
    let value = T::deserialize(&mut de)?;
    de.end()?;
    Ok(value)
}

/// A stream of events: a [`Reader`] or a [`NodeReader`].
trait Events<'de> {
    fn next_event(&mut self) -> Result<Event<'de>>;
}

impl<'de> Events<'de> for Reader<'de> {
    fn next_event(&mut self) -> Result<Event<'de>> {
        Ok(Reader::next_event(self)?)
    }
}

impl<'de> Events<'de> for NodeReader<'de> {
    fn next_event(&mut self) -> Result<Event<'de>> {
        Ok(NodeReader::next_event(self))
    }
}

fn unexpected<'a>(value: &'a Scalar<'_>) -> Unexpected<'a> {
    match value {
        Scalar::Null => Unexpected::Unit,
        Scalar::Bool(b) => Unexpected::Bool(*b),
        Scalar::Int(i) => Unexpected::Signed(*i),
        Scalar::Float(f) => Unexpected::Float(*f),
        Scalar::Str(s) => Unexpected::Str(s),
    }
}

/// The first event of a value, classified.
enum Head<'de> {
    Scalar(Scalar<'de>, Span),
    Seq(usize, Option<Comment<'de>>),
    Map(usize, Option<Comment<'de>>),
}

struct Deserializer<'de, E> {
    events: E,
    peeked: Option<Event<'de>>,
    /// The number of open collections.
    depth: usize,
    /// The inline comment of the most recently-read value.
    last_inline: Option<&'de str>,
    /// The trailing comment of the most recently-read value.
    last_trailing: Option<Comment<'de>>,
    /// A leading, unclaimed comment of an enclosing node that begins on
    /// the same line as the next value but is unclaimed.
    inherited: Option<Comment<'de>>,
    /// Whether the next value is the root or a sequence entry,
    /// whose first child begins on the same line.
    shares_first_line: bool,
}

impl<'de, E: Events<'de>> Deserializer<'de, E> {
    const fn new(events: E) -> Self {
        Self {
            events,
            peeked: None,
            depth: 0,
            last_inline: None,
            last_trailing: None,
            inherited: None,
            shares_first_line: true,
        }
    }

    fn next(&mut self) -> Result<Event<'de>> {
        match self.peeked.take() {
            Some(event) => Ok(event),
            None => self.events.next_event(),
        }
    }

    fn peek(&mut self) -> Result<&mut Event<'de>> {
        if self.peeked.is_none() {
            self.peeked = Some(self.events.next_event()?);
        }
        Ok(self.peeked.as_mut().expect("peeked event"))
    }

    /// Read the first event of a value.
    fn head(&mut self) -> Result<Head<'de>> {
        match self.next()? {
            Event::Scalar {
                value,
                span,
                inline,
                ..
            } => {
                self.last_inline = inline;
                self.last_trailing = None;
                Ok(Head::Scalar(value, span))
            }
            Event::SeqStart { start, leading } => {
                self.depth += 1;
                Ok(Head::Seq(start, leading))
            }
            Event::MapStart { start, leading } => {
                self.depth += 1;
                Ok(Head::Map(start, leading))
            }
            other => Err(Error::data(format!("unexpected event {other:?}"))),
        }
    }

    /// Read the end of a collection and record its comments.
    fn expect_end(&mut self) -> Result<()> {
        match self.next()? {
            Event::End {
                inline, trailing, ..
            } => {
                self.depth -= 1;
                self.last_inline = inline;
                self.last_trailing = trailing;
                Ok(())
            }
            other => Err(Error::data(format!("unexpected event {other:?}"))),
        }
    }

    /// Read the rest of the document and return its first syntax error.
    fn syntax_error(&mut self) -> Option<Error> {
        if self
            .peeked
            .take()
            .is_some_and(|e| matches!(e, Event::DocumentEnd { .. }))
        {
            return None;
        }
        loop {
            match self.events.next_event() {
                Ok(Event::DocumentEnd { .. }) => return None,
                Ok(_) => {}
                Err(e) => return Some(e),
            }
        }
    }

    /// Check that the document has ended.
    fn end(&mut self) -> Result<()> {
        match self.next()? {
            Event::DocumentEnd { .. } => Ok(()),
            other => Err(Error::data(format!("unexpected event {other:?}"))),
        }
    }

    /// Skip the rest of a collection whose start has been read.
    /// Returns the number of entries that have been skipped.
    fn skip_entries(&mut self) -> Result<usize> {
        let mut entries = 0;
        loop {
            match self.peek()? {
                Event::End { .. } => {
                    self.expect_end()?;
                    return Ok(entries);
                }
                Event::Key { .. } => {
                    self.next()?;
                }
                _ => {
                    self.skip_value()?;
                    entries += 1;
                }
            }
        }
    }

    fn skip_value(&mut self) -> Result<()> {
        match self.head()? {
            Head::Scalar(..) => Ok(()),
            Head::Seq(..) | Head::Map(..) => self.skip_entries().map(drop),
        }
    }

    /// The comment that a collection passes to its first entry.
    fn comment_for_first_entry(&mut self, leading: Option<Comment<'de>>) -> Option<Comment<'de>> {
        let inherited = self.inherited.take();
        if std::mem::replace(&mut self.shares_first_line, false) {
            leading.or(inherited)
        } else {
            None
        }
    }

    fn visit_seq<V: Visitor<'de>>(
        &mut self,
        start: usize,
        leading: Option<Comment<'de>>,
        visitor: V,
    ) -> Result<V::Value> {
        let inherited = self.comment_for_first_entry(leading);
        let mut access = SeqAccess {
            de: self,
            inherited,
            count: 0,
        };
        let value = visitor
            .visit_seq(&mut access)
            .map_err(|e: Error| e.at(Span::point(start)))?;
        let count = access.count;
        let extra = self.skip_entries()?;
        if extra > 0 {
            let expected = if count == 1 {
                "1 element".to_owned()
            } else {
                format!("{count} elements")
            };
            return Err(
                <Error as de::Error>::invalid_length(count + extra, &expected.as_str())
                    .at(Span::point(start)),
            );
        }
        Ok(value)
    }

    fn visit_map<V: Visitor<'de>>(
        &mut self,
        start: usize,
        leading: Option<Comment<'de>>,
        visitor: V,
    ) -> Result<V::Value> {
        let inherited = self.comment_for_first_entry(leading);
        let value = visitor
            .visit_map(MapAccess {
                de: self,
                inherited,
            })
            .map_err(|e: Error| e.at(Span::point(start)))?;
        self.skip_entries()?;
        Ok(value)
    }

    fn invalid_head(&mut self, head: Head<'de>, visitor: &dyn de::Expected) -> Error {
        match head {
            Head::Scalar(value, span) => {
                <Error as de::Error>::invalid_type(unexpected(&value), visitor).at(span)
            }
            Head::Seq(start, _) => {
                let _ = self.skip_entries();
                Error::invalid_type_at(Unexpected::Seq, visitor, start)
            }
            Head::Map(start, _) => {
                let _ = self.skip_entries();
                Error::invalid_type_at(Unexpected::Map, visitor, start)
            }
        }
    }

    /// Read a float or integer; `fits` rejects finite floats that the target
    /// type would round to infinity.
    fn float<V: Visitor<'de>>(&mut self, visitor: V, fits: fn(f64) -> bool) -> Result<V::Value> {
        match self.head()? {
            Head::Scalar(Scalar::Float(f), span) if fits(f) => {
                visitor.visit_f64(f).map_err(|e: Error| e.at(span))
            }
            Head::Scalar(Scalar::Float(f), span) => {
                Err(<Error as de::Error>::invalid_value(Unexpected::Float(f), &visitor).at(span))
            }
            Head::Scalar(Scalar::Int(i), span) => {
                visitor.visit_i64(i).map_err(|e: Error| e.at(span))
            }
            head => Err(self.invalid_head(head, &visitor)),
        }
    }

    /// Take comments from the value that was just read, for `Commented<T>`.
    fn take_completed_comments(&mut self) -> Result<Completed<'de>> {
        let inline = self.last_inline.take();
        let mut trailing = self.last_trailing.take().map(|c| c.text());
        if self.depth == 0
            && let Event::DocumentEnd { trailing: rest } = self.peek()?
            && let Some(rest) = rest.take()
        {
            trailing = Some(match trailing {
                Some(t) => Cow::Owned(format!("{t}\n{}", rest.text())),
                None => rest.text(),
            });
        }
        Ok((inline, trailing))
    }
}

impl Error {
    fn invalid_type_at(unexp: Unexpected<'_>, exp: &dyn de::Expected, at: usize) -> Self {
        <Self as de::Error>::invalid_type(unexp, exp).at(Span::point(at))
    }
}

macro_rules! deserialize_int {
    ($($method:ident)*) => {$(
        fn $method<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
            match self.head()? {
                Head::Scalar(Scalar::Int(i), span) => visitor.visit_i64(i).map_err(|e: Error| e.at(span)),
                head => Err(self.invalid_head(head, &visitor)),
            }
        }
    )*};
}

/// Inline and trailing comments of a value that has been read.
type Completed<'de> = (Option<&'de str>, Option<Cow<'de, str>>);

impl<'de, E: Events<'de>> de::Deserializer<'de> for &mut Deserializer<'de, E> {
    type Error = Error;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        match self.head()? {
            Head::Scalar(value, span) => match value {
                Scalar::Null => visitor.visit_unit(),
                Scalar::Bool(b) => visitor.visit_bool(b),
                Scalar::Int(i) => visitor.visit_i64(i),
                Scalar::Float(f) => visitor.visit_f64(f),
                Scalar::Str(Cow::Borrowed(s)) => visitor.visit_borrowed_str(s),
                Scalar::Str(Cow::Owned(s)) => visitor.visit_string(s),
            }
            .map_err(|e: Error| e.at(span)),
            Head::Seq(start, leading) => self.visit_seq(start, leading, visitor),
            Head::Map(start, leading) => self.visit_map(start, leading, visitor),
        }
    }

    fn deserialize_bool<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        match self.head()? {
            Head::Scalar(Scalar::Bool(b), span) => {
                visitor.visit_bool(b).map_err(|e: Error| e.at(span))
            }
            head => Err(self.invalid_head(head, &visitor)),
        }
    }

    deserialize_int! {
        deserialize_i8 deserialize_i16 deserialize_i32 deserialize_i64 deserialize_i128
        deserialize_u8 deserialize_u16 deserialize_u32 deserialize_u64 deserialize_u128
    }

    fn deserialize_f32<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.float(visitor, |f| !f.is_finite() || (f as f32).is_finite())
    }

    fn deserialize_f64<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.float(visitor, |_| true)
    }

    fn deserialize_char<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.deserialize_str(visitor)
    }

    fn deserialize_str<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        match self.head()? {
            Head::Scalar(Scalar::Str(s), span) => match s {
                Cow::Borrowed(s) => visitor.visit_borrowed_str(s),
                Cow::Owned(s) => visitor.visit_string(s),
            }
            .map_err(|e: Error| e.at(span)),
            head => Err(self.invalid_head(head, &visitor)),
        }
    }

    fn deserialize_string<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.deserialize_str(visitor)
    }

    fn deserialize_bytes<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        match self.head()? {
            Head::Scalar(Scalar::Str(s), span) => match s {
                Cow::Borrowed(s) => visitor.visit_borrowed_bytes(s.as_bytes()),
                Cow::Owned(s) => visitor.visit_byte_buf(s.into_bytes()),
            }
            .map_err(|e: Error| e.at(span)),
            Head::Seq(start, leading) => self.visit_seq(start, leading, visitor),
            head => Err(self.invalid_head(head, &visitor)),
        }
    }

    fn deserialize_byte_buf<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.deserialize_bytes(visitor)
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        if let Event::Scalar {
            value: Scalar::Null,
            ..
        } = self.peek()?
        {
            self.head()?;
            return visitor.visit_none();
        }
        visitor.visit_some(self)
    }

    fn deserialize_unit<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        match self.head()? {
            Head::Scalar(Scalar::Null, span) => visitor.visit_unit().map_err(|e: Error| e.at(span)),
            head => Err(self.invalid_head(head, &visitor)),
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
        name: &'static str,
        visitor: V,
    ) -> Result<V::Value> {
        if name == COMMENTED_STRUCT {
            let leading = match self.peek()? {
                Event::Scalar { leading, .. }
                | Event::SeqStart { leading, .. }
                | Event::MapStart { leading, .. } => leading.take(),
                _ => None,
            };
            let comment = leading.or_else(|| self.inherited.take());
            return visitor.visit_map(CommentedAccess {
                de: self,
                comment: comment.map(|c| c.text()),
                completed: None,
                field: 0,
            });
        }
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_seq<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        match self.head()? {
            Head::Seq(start, leading) => self.visit_seq(start, leading, visitor),
            head => Err(self.invalid_head(head, &visitor)),
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
        match self.head()? {
            Head::Map(start, leading) => self.visit_map(start, leading, visitor),
            head => Err(self.invalid_head(head, &visitor)),
        }
    }

    fn deserialize_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value> {
        self.deserialize_map(visitor)
    }

    fn deserialize_enum<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value> {
        const EXPECTED: &str = "a string or a mapping with a single key";
        match self.head()? {
            Head::Scalar(Scalar::Str(s), span) => match s {
                Cow::Borrowed(s) => visitor.visit_enum(s.into_deserializer()),
                Cow::Owned(s) => visitor.visit_enum(s.into_deserializer()),
            }
            .map_err(|e: Error| e.at(span)),
            Head::Map(start, _) => {
                let Event::Key { name, span } = self.next()? else {
                    return Err(Error::invalid_type_at(Unexpected::Map, &EXPECTED, start));
                };
                let value = visitor.visit_enum(EnumAccess {
                    de: &mut *self,
                    name,
                    span,
                })?;
                if self.skip_entries()? > 0 {
                    return Err(Error::invalid_type_at(Unexpected::Map, &EXPECTED, start));
                }
                Ok(value)
            }
            head => Err(self.invalid_head(head, &EXPECTED)),
        }
    }

    fn deserialize_identifier<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.deserialize_str(visitor)
    }

    fn deserialize_ignored_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.inherited = None;
        self.shares_first_line = false;
        self.skip_value()?;
        visitor.visit_unit()
    }
}

// ── Sequences and mappings ─────────────────────────────────────────

struct SeqAccess<'a, 'de, E> {
    de: &'a mut Deserializer<'de, E>,
    inherited: Option<Comment<'de>>,
    count: usize,
}

impl<'de, E: Events<'de>> de::SeqAccess<'de> for &mut SeqAccess<'_, 'de, E> {
    type Error = Error;

    fn next_element_seed<T: de::DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>> {
        if let Event::End { .. } = self.de.peek()? {
            return Ok(None);
        }
        self.count += 1;
        self.de.inherited = self.inherited.take();
        self.de.shares_first_line = true;
        let value = seed.deserialize(&mut *self.de);
        self.de.inherited = None;
        value.map(Some)
    }
}

struct MapAccess<'a, 'de, E> {
    de: &'a mut Deserializer<'de, E>,
    /// The comment for the first value.
    inherited: Option<Comment<'de>>,
}

impl<'de, E: Events<'de>> de::MapAccess<'de> for MapAccess<'_, 'de, E> {
    type Error = Error;

    fn next_key_seed<K: de::DeserializeSeed<'de>>(&mut self, seed: K) -> Result<Option<K::Value>> {
        match self.de.next()? {
            Event::Key { name, span } => seed
                .deserialize(KeyDeserializer::new(name))
                .map(Some)
                .map_err(|e: Error| e.at(span)),
            end @ Event::End { .. } => {
                self.de.peeked = Some(end);
                Ok(None)
            }
            other => Err(Error::data(format!("unexpected event {other:?}"))),
        }
    }

    fn next_value_seed<V: de::DeserializeSeed<'de>>(&mut self, seed: V) -> Result<V::Value> {
        self.de.inherited = self.inherited.take();
        self.de.shares_first_line = false;
        let value = seed.deserialize(&mut *self.de);
        self.de.inherited = None;
        value
    }
}

// ── Enums ───────────────────────────────────────────────────────────

/// An externally tagged enum variant written as a single-entry mapping.
struct EnumAccess<'a, 'de, E> {
    de: &'a mut Deserializer<'de, E>,
    name: Cow<'de, str>,
    span: Span,
}

impl<'a, 'de, E: Events<'de>> de::EnumAccess<'de> for EnumAccess<'a, 'de, E> {
    type Error = Error;
    type Variant = VariantAccess<'a, 'de, E>;

    fn variant_seed<V: de::DeserializeSeed<'de>>(
        self,
        seed: V,
    ) -> Result<(V::Value, Self::Variant)> {
        let span = self.span;
        let variant = seed
            .deserialize(KeyDeserializer::new(self.name))
            .map_err(|e: Error| e.at(span))?;
        Ok((variant, VariantAccess { de: self.de, span }))
    }
}

struct VariantAccess<'a, 'de, E> {
    de: &'a mut Deserializer<'de, E>,
    /// The span of the variant name.
    span: Span,
}

impl<'de, E: Events<'de>> de::VariantAccess<'de> for VariantAccess<'_, 'de, E> {
    type Error = Error;

    fn unit_variant(self) -> Result<()> {
        Err(
            <Error as de::Error>::invalid_type(Unexpected::Map, &"a string for a unit variant")
                .at(self.span),
        )
    }

    fn newtype_variant_seed<T: de::DeserializeSeed<'de>>(self, seed: T) -> Result<T::Value> {
        self.de.shares_first_line = false;
        seed.deserialize(&mut *self.de)
    }

    fn tuple_variant<V: Visitor<'de>>(self, _len: usize, visitor: V) -> Result<V::Value> {
        self.de.shares_first_line = false;
        de::Deserializer::deserialize_seq(&mut *self.de, visitor)
    }

    fn struct_variant<V: Visitor<'de>>(
        self,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value> {
        self.de.shares_first_line = false;
        de::Deserializer::deserialize_map(&mut *self.de, visitor)
    }
}

// ── Commented<T> ────────────────────────────────────────────────────

struct CommentedAccess<'a, 'de, E> {
    de: &'a mut Deserializer<'de, E>,
    comment: Option<Cow<'de, str>>,
    completed: Option<Completed<'de>>,
    field: usize,
}

/// The order in which `CommentedAccess` presents fields:
/// comment, value, inline comment, trailing comment.
const COMMENTED_ORDER: [usize; 4] = [0, 3, 1, 2];

impl<'de, E: Events<'de>> de::MapAccess<'de> for CommentedAccess<'_, 'de, E> {
    type Error = Error;

    fn next_key_seed<K: de::DeserializeSeed<'de>>(&mut self, seed: K) -> Result<Option<K::Value>> {
        let Some(&index) = COMMENTED_ORDER.get(self.field) else {
            return Ok(None);
        };
        seed.deserialize(de::value::BorrowedStrDeserializer::new(
            COMMENTED_FIELDS[index],
        ))
        .map(Some)
    }

    fn next_value_seed<V: de::DeserializeSeed<'de>>(&mut self, seed: V) -> Result<V::Value> {
        let index = COMMENTED_ORDER[self.field];
        self.field += 1;
        match index {
            0 => seed.deserialize(CommentDeserializer(self.comment.take())),
            3 => {
                self.de.inherited = None;
                self.de.shares_first_line = false;
                let value = seed.deserialize(&mut *self.de)?;
                self.completed = Some(self.de.take_completed_comments()?);
                Ok(value)
            }
            1 => {
                let inline = self.completed.as_ref().and_then(|c| c.0);
                seed.deserialize(CommentDeserializer(inline.map(Cow::Borrowed)))
            }
            _ => {
                let trailing = self.completed.as_mut().and_then(|c| c.1.take());
                seed.deserialize(CommentDeserializer(trailing))
            }
        }
    }
}

/// Deserializes an optional comment.
struct CommentDeserializer<'de>(Option<Cow<'de, str>>);

impl<'de> de::Deserializer<'de> for CommentDeserializer<'de> {
    type Error = Error;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        match self.0 {
            Some(Cow::Borrowed(s)) => {
                visitor.visit_some(de::value::BorrowedStrDeserializer::new(s))
            }
            Some(Cow::Owned(s)) => visitor.visit_some(de::value::StringDeserializer::new(s)),
            None => visitor.visit_none(),
        }
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct map struct enum identifier ignored_any
    }
}

// ── Keys ────────────────────────────────────────────────────────────

/// Deserializes a mapping key.
pub(crate) struct KeyDeserializer<'de> {
    name: Cow<'de, str>,
}

impl<'de> KeyDeserializer<'de> {
    pub(crate) const fn new(name: Cow<'de, str>) -> Self {
        Self { name }
    }

    pub(crate) const fn owned(name: String) -> Self {
        Self {
            name: Cow::Owned(name),
        }
    }

    fn invalid(&self, expected: &str) -> Error {
        Error::data(format!(
            "invalid mapping key `{}`, expected {expected}",
            self.name
        ))
    }

    fn int(&self) -> Result<i64> {
        match scalar::parse_int(&self.name) {
            Some(Ok(i)) => Ok(i),
            Some(Err(())) => Err(self.invalid("an integer in the 64-bit signed range")),
            None => Err(self.invalid("an integer")),
        }
    }
}

macro_rules! deserialize_int_key {
    ($($method:ident)*) => {$(
        fn $method<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
            visitor.visit_i64(self.int()?)
        }
    )*};
}

impl<'de> de::Deserializer<'de> for KeyDeserializer<'de> {
    type Error = Error;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        match self.name {
            Cow::Borrowed(name) => visitor.visit_borrowed_str(name),
            Cow::Owned(name) => visitor.visit_string(name),
        }
    }

    fn deserialize_bool<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        match &*self.name {
            "true" => visitor.visit_bool(true),
            "false" => visitor.visit_bool(false),
            _ => Err(self.invalid("`true` or `false`")),
        }
    }

    deserialize_int_key! {
        deserialize_i8 deserialize_i16 deserialize_i32 deserialize_i64 deserialize_i128
        deserialize_u8 deserialize_u16 deserialize_u32 deserialize_u64 deserialize_u128
    }

    fn deserialize_f32<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.deserialize_f64(visitor)
    }

    fn deserialize_f64<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        if let Some(Ok(f)) = scalar::parse_float(&self.name) {
            return visitor.visit_f64(f);
        }
        visitor.visit_i64(self.int().map_err(|_| self.invalid("a number"))?)
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        visitor.visit_some(self)
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value> {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_enum<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value> {
        match self.name {
            Cow::Borrowed(name) => visitor.visit_enum(name.into_deserializer()),
            Cow::Owned(name) => visitor.visit_enum(name.into_deserializer()),
        }
    }

    serde::forward_to_deserialize_any! {
        char str string bytes byte_buf unit unit_struct seq tuple tuple_struct
        map struct identifier ignored_any
    }
}

impl<'de> IntoDeserializer<'de, Error> for KeyDeserializer<'de> {
    type Deserializer = Self;

    fn into_deserializer(self) -> Self {
        self
    }
}
