//! Serialization to AYML text or a [`Node`] tree.

use std::borrow::Cow;
use std::collections::HashSet;

use serde::Serialize;
use serde::ser::{self, Impossible};

use ayml_core::{Comments, Key, Mapping, Node, Scalar, Value, Writer, scalar};

use crate::commented::{self, COMMENTED_STRUCT};
use crate::error::{Error, Result};

/// Serialize a `T` to an AYML string.
///
/// # Errors
///
/// Returns an error if serialization fails.
pub fn to_string<T: ?Sized + Serialize>(value: &T) -> Result<String> {
    let mut ser = Serializer::new(Writer::new());
    value.serialize(&mut ser)?;
    Ok(ser.sink.finish())
}

/// Serialize a `T` to AYML bytes.
pub fn to_vec<T: ?Sized + Serialize>(value: &T) -> Result<Vec<u8>> {
    to_string(value).map(String::into_bytes)
}

/// Serialize a `T` to an AYML writer.
pub fn to_writer<W: std::io::Write, T: ?Sized + Serialize>(mut writer: W, value: &T) -> Result<()> {
    writer.write_all(to_string(value)?.as_bytes())?;
    Ok(())
}

/// Serialize a `T` into an AYML node tree.
pub fn to_node<T: ?Sized + Serialize>(value: &T) -> Result<Node> {
    let mut ser = Serializer::new(TreeBuilder::default());
    value.serialize(&mut ser)?;
    Ok(ser.sink.finish())
}

/// Receives the values of a document in order, like [`Writer`].
trait Sink {
    fn scalar(&mut self, key: Option<&str>, value: Scalar<'_>, comments: Comments<'_>);
    fn begin(&mut self, key: Option<&str>, is_map: bool, comments: Comments<'_>);
    fn end(&mut self);
}

impl Sink for Writer {
    fn scalar(&mut self, key: Option<&str>, value: Scalar<'_>, comments: Comments<'_>) {
        Self::scalar(self, key, value, comments);
    }

    fn begin(&mut self, key: Option<&str>, is_map: bool, comments: Comments<'_>) {
        if is_map {
            self.begin_map(key, comments);
        } else {
            self.begin_seq(key, comments);
        }
    }

    fn end(&mut self) {
        Self::end(self);
    }
}

/// A [`Sink`] that builds a [`Node`] tree.
#[derive(Default)]
struct TreeBuilder {
    stack: Vec<(Node, Option<Key>)>,
    root: Option<Node>,
}

impl TreeBuilder {
    fn finish(self) -> Node {
        assert!(self.stack.is_empty(), "unclosed collection");
        self.root.expect("a value was written")
    }

    fn add(&mut self, node: Node, key: Option<Key>) {
        let Some((parent, _)) = self.stack.last_mut() else {
            self.root = Some(node);
            return;
        };
        match &mut parent.value {
            Value::Seq(items) => items.push(node),
            Value::Map(map) => {
                map.insert(key.expect("mapping value without a key"), node);
            }
            _ => unreachable!("collections hold entries"),
        }
    }
}

fn node(value: Value, comments: Comments<'_>) -> Node {
    Node {
        value,
        comment: comments.comment.map(str::to_owned),
        inline_comment: comments.inline.map(str::to_owned),
        trailing_comment: comments.trailing.map(str::to_owned),
        span: ayml_core::Span::default(),
    }
}

impl Sink for TreeBuilder {
    fn scalar(&mut self, key: Option<&str>, value: Scalar<'_>, comments: Comments<'_>) {
        self.add(node(Value::from(value), comments), key.map(Key::new));
    }

    fn begin(&mut self, key: Option<&str>, is_map: bool, comments: Comments<'_>) {
        let value = if is_map {
            Value::Map(Mapping::new())
        } else {
            Value::Seq(Vec::new())
        };
        self.stack.push((node(value, comments), key.map(Key::new)));
    }

    fn end(&mut self) {
        let (node, key) = self.stack.pop().expect("open collection");
        self.add(node, key);
    }
}

/// Widen an `f32` to the `f64` nearest its shortest decimal representation,
/// so that it is written as `0.1` rather than `0.10000000149011612`.
fn widen_f32(v: f32) -> f64 {
    if v.is_finite() {
        ryu::Buffer::new()
            .format_finite(v)
            .parse()
            .unwrap_or(f64::from(v))
    } else {
        f64::from(v)
    }
}

fn int_out_of_range(v: impl std::fmt::Display) -> Error {
    Error::data(format!(
        "integer {v} is out of range (AYML integers are 64-bit signed)"
    ))
}

struct Serializer<S> {
    sink: S,
    /// The key of the next value, in a mapping.
    key: Option<Cow<'static, str>>,
    comments: Option<commented::Comments>,
    key_sets: Vec<HashSet<String>>,
}

impl<S: Sink> Serializer<S> {
    const fn new(sink: S) -> Self {
        Self {
            sink,
            key: None,
            comments: None,
            key_sets: Vec::new(),
        }
    }

    fn scalar(&mut self, value: Scalar<'_>) {
        let key = self.key.take();
        let comments = self.comments.take().unwrap_or_default();
        self.sink.scalar(key.as_deref(), value, comments.borrow());
    }

    fn begin(&mut self, is_map: bool) {
        let key = self.key.take();
        let comments = self.comments.take().unwrap_or_default();
        self.sink.begin(key.as_deref(), is_map, comments.borrow());
    }

    /// Start a single-entry mapping holding a tuple or struct variant.
    fn begin_variant(&mut self, variant: &'static str, is_map: bool) {
        self.begin(true);
        self.sink.begin(Some(variant), is_map, Comments::default());
    }
}

impl<'a, S: Sink> ser::Serializer for &'a mut Serializer<S> {
    type Ok = ();
    type Error = Error;

    type SerializeSeq = Compound<'a, S>;
    type SerializeTuple = Compound<'a, S>;
    type SerializeTupleStruct = Compound<'a, S>;
    type SerializeTupleVariant = Compound<'a, S>;
    type SerializeMap = Compound<'a, S>;
    type SerializeStruct = Compound<'a, S>;
    type SerializeStructVariant = Compound<'a, S>;

    fn serialize_bool(self, v: bool) -> Result<()> {
        self.scalar(Scalar::Bool(v));
        Ok(())
    }

    fn serialize_i8(self, v: i8) -> Result<()> {
        self.serialize_i64(i64::from(v))
    }

    fn serialize_i16(self, v: i16) -> Result<()> {
        self.serialize_i64(i64::from(v))
    }

    fn serialize_i32(self, v: i32) -> Result<()> {
        self.serialize_i64(i64::from(v))
    }

    fn serialize_i64(self, v: i64) -> Result<()> {
        self.scalar(Scalar::Int(v));
        Ok(())
    }

    fn serialize_i128(self, v: i128) -> Result<()> {
        let v = i64::try_from(v).map_err(|_| int_out_of_range(v))?;
        self.serialize_i64(v)
    }

    fn serialize_u8(self, v: u8) -> Result<()> {
        self.serialize_i64(i64::from(v))
    }

    fn serialize_u16(self, v: u16) -> Result<()> {
        self.serialize_i64(i64::from(v))
    }

    fn serialize_u32(self, v: u32) -> Result<()> {
        self.serialize_i64(i64::from(v))
    }

    fn serialize_u64(self, v: u64) -> Result<()> {
        let v = i64::try_from(v).map_err(|_| int_out_of_range(v))?;
        self.serialize_i64(v)
    }

    fn serialize_u128(self, v: u128) -> Result<()> {
        let v = i64::try_from(v).map_err(|_| int_out_of_range(v))?;
        self.serialize_i64(v)
    }

    /// Write `f32` with its shortest representation (`0.1` vs `0.10000000149011612`).
    fn serialize_f32(self, v: f32) -> Result<()> {
        self.serialize_f64(widen_f32(v))
    }

    fn serialize_f64(self, v: f64) -> Result<()> {
        self.scalar(Scalar::Float(v));
        Ok(())
    }

    fn serialize_char(self, v: char) -> Result<()> {
        let mut buf = [0u8; 4];
        self.serialize_str(v.encode_utf8(&mut buf))
    }

    fn serialize_str(self, v: &str) -> Result<()> {
        self.scalar(Scalar::Str(Cow::Borrowed(v)));
        Ok(())
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<()> {
        self.begin(false);
        for b in v {
            self.sink
                .scalar(None, Scalar::Int(i64::from(*b)), Comments::default());
        }
        self.sink.end();
        Ok(())
    }

    fn serialize_none(self) -> Result<()> {
        self.scalar(Scalar::Null);
        Ok(())
    }

    fn serialize_some<T: ?Sized + Serialize>(self, value: &T) -> Result<()> {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<()> {
        self.scalar(Scalar::Null);
        Ok(())
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<()> {
        self.serialize_unit()
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _index: u32,
        variant: &'static str,
    ) -> Result<()> {
        self.serialize_str(variant)
    }

    fn serialize_newtype_struct<T: ?Sized + Serialize>(
        self,
        name: &'static str,
        value: &T,
    ) -> Result<()> {
        if name == COMMENTED_STRUCT
            && let Some(inner) = commented::take_pending_comments()
        {
            self.comments = Some(match self.comments.take() {
                Some(outer) => inner.or(outer),
                None => inner,
            });
        }
        value.serialize(self)
    }

    fn serialize_newtype_variant<T: ?Sized + Serialize>(
        self,
        _name: &'static str,
        _index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<()> {
        self.begin(true);
        self.key = Some(Cow::Borrowed(variant));
        value.serialize(&mut *self)?;
        self.sink.end();
        Ok(())
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Compound<'a, S>> {
        self.begin(false);
        Ok(Compound::new(self, Kind::Plain))
    }

    fn serialize_tuple(self, len: usize) -> Result<Compound<'a, S>> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_struct(self, _name: &'static str, len: usize) -> Result<Compound<'a, S>> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Compound<'a, S>> {
        self.begin_variant(variant, false);
        Ok(Compound::new(self, Kind::Variant))
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Compound<'a, S>> {
        self.begin(true);
        self.key_sets.push(HashSet::new());
        Ok(Compound::new(self, Kind::Map))
    }

    fn serialize_struct(self, _name: &'static str, _len: usize) -> Result<Compound<'a, S>> {
        self.begin(true);
        Ok(Compound::new(self, Kind::Plain))
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Compound<'a, S>> {
        self.begin_variant(variant, true);
        Ok(Compound::new(self, Kind::Variant))
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// A sequence or struct.
    Plain,
    /// A serde map.
    Map,
    /// A tuple or struct variant.
    Variant,
}

/// A collection being serialized.
struct Compound<'a, S> {
    ser: &'a mut Serializer<S>,
    kind: Kind,
}

impl<'a, S: Sink> Compound<'a, S> {
    const fn new(ser: &'a mut Serializer<S>, kind: Kind) -> Self {
        Self { ser, kind }
    }

    fn element<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<()> {
        value.serialize(&mut *self.ser)
    }

    fn field<T: ?Sized + Serialize>(&mut self, key: &'static str, value: &T) -> Result<()> {
        self.ser.key = Some(Cow::Borrowed(key));
        value.serialize(&mut *self.ser)
    }

    fn finish(self) {
        self.ser.sink.end();
        match self.kind {
            Kind::Plain => {}
            Kind::Map => {
                self.ser.key_sets.pop();
            }
            Kind::Variant => self.ser.sink.end(),
        }
    }
}

impl<S: Sink> ser::SerializeSeq for Compound<'_, S> {
    type Ok = ();
    type Error = Error;

    fn serialize_element<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<()> {
        self.element(value)
    }

    fn end(self) -> Result<()> {
        self.finish();
        Ok(())
    }
}

impl<S: Sink> ser::SerializeTuple for Compound<'_, S> {
    type Ok = ();
    type Error = Error;

    fn serialize_element<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<()> {
        self.element(value)
    }

    fn end(self) -> Result<()> {
        self.finish();
        Ok(())
    }
}

impl<S: Sink> ser::SerializeTupleStruct for Compound<'_, S> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<()> {
        self.element(value)
    }

    fn end(self) -> Result<()> {
        self.finish();
        Ok(())
    }
}

impl<S: Sink> ser::SerializeTupleVariant for Compound<'_, S> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<()> {
        self.element(value)
    }

    fn end(self) -> Result<()> {
        self.finish();
        Ok(())
    }
}

impl<S: Sink> ser::SerializeMap for Compound<'_, S> {
    type Ok = ();
    type Error = Error;

    fn serialize_key<T: ?Sized + Serialize>(&mut self, key: &T) -> Result<()> {
        let key = key.serialize(KeySerializer)?;
        let keys = self.ser.key_sets.last_mut().expect("map key set");
        if !keys.insert(key.clone()) {
            return Err(Error::data(format!("duplicate mapping key `{key}`")));
        }
        self.ser.key = Some(Cow::Owned(key));
        Ok(())
    }

    fn serialize_value<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<()> {
        self.element(value)
    }

    fn end(self) -> Result<()> {
        self.finish();
        Ok(())
    }
}

impl<S: Sink> ser::SerializeStruct for Compound<'_, S> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: ?Sized + Serialize>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<()> {
        self.field(key, value)
    }

    fn end(self) -> Result<()> {
        self.finish();
        Ok(())
    }
}

impl<S: Sink> ser::SerializeStructVariant for Compound<'_, S> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: ?Sized + Serialize>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<()> {
        self.field(key, value)
    }

    fn end(self) -> Result<()> {
        self.finish();
        Ok(())
    }
}

// ── Keys ────────────────────────────────────────────────────────────

fn key_error() -> Error {
    Error::data("mapping keys must be strings, numbers, booleans, or unit enum variants")
}

/// Serializes a mapping key to its string form.
struct KeySerializer;

impl ser::Serializer for KeySerializer {
    type Ok = String;
    type Error = Error;

    type SerializeSeq = Impossible<String, Error>;
    type SerializeTuple = Impossible<String, Error>;
    type SerializeTupleStruct = Impossible<String, Error>;
    type SerializeTupleVariant = Impossible<String, Error>;
    type SerializeMap = Impossible<String, Error>;
    type SerializeStruct = Impossible<String, Error>;
    type SerializeStructVariant = Impossible<String, Error>;

    fn serialize_bool(self, v: bool) -> Result<String> {
        Ok(v.to_string())
    }

    fn serialize_i8(self, v: i8) -> Result<String> {
        Ok(v.to_string())
    }

    fn serialize_i16(self, v: i16) -> Result<String> {
        Ok(v.to_string())
    }

    fn serialize_i32(self, v: i32) -> Result<String> {
        Ok(v.to_string())
    }

    fn serialize_i64(self, v: i64) -> Result<String> {
        Ok(v.to_string())
    }

    fn serialize_i128(self, v: i128) -> Result<String> {
        i64::try_from(v)
            .map(|v| v.to_string())
            .map_err(|_| int_out_of_range(v))
    }

    fn serialize_u8(self, v: u8) -> Result<String> {
        Ok(v.to_string())
    }

    fn serialize_u16(self, v: u16) -> Result<String> {
        Ok(v.to_string())
    }

    fn serialize_u32(self, v: u32) -> Result<String> {
        Ok(v.to_string())
    }

    fn serialize_u64(self, v: u64) -> Result<String> {
        i64::try_from(v)
            .map(|v| v.to_string())
            .map_err(|_| int_out_of_range(v))
    }

    fn serialize_u128(self, v: u128) -> Result<String> {
        i64::try_from(v)
            .map(|v| v.to_string())
            .map_err(|_| int_out_of_range(v))
    }

    fn serialize_f32(self, v: f32) -> Result<String> {
        self.serialize_f64(widen_f32(v))
    }

    fn serialize_f64(self, v: f64) -> Result<String> {
        let mut out = String::new();
        scalar::write_float(&mut out, v);
        Ok(out)
    }

    fn serialize_char(self, v: char) -> Result<String> {
        Ok(v.to_string())
    }

    fn serialize_str(self, v: &str) -> Result<String> {
        Ok(v.to_owned())
    }

    fn serialize_bytes(self, _v: &[u8]) -> Result<String> {
        Err(key_error())
    }

    fn serialize_none(self) -> Result<String> {
        Err(key_error())
    }

    fn serialize_some<T: ?Sized + Serialize>(self, value: &T) -> Result<String> {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<String> {
        Err(key_error())
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<String> {
        Err(key_error())
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _index: u32,
        variant: &'static str,
    ) -> Result<String> {
        Ok(variant.to_owned())
    }

    fn serialize_newtype_struct<T: ?Sized + Serialize>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<String> {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T: ?Sized + Serialize>(
        self,
        _name: &'static str,
        _index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Result<String> {
        Err(key_error())
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq> {
        Err(key_error())
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple> {
        Err(key_error())
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct> {
        Err(key_error())
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant> {
        Err(key_error())
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap> {
        Err(key_error())
    }

    fn serialize_struct(self, _name: &'static str, _len: usize) -> Result<Self::SerializeStruct> {
        Err(key_error())
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant> {
        Err(key_error())
    }
}
