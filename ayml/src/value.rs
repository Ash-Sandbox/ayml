//! An untyped AYML value, analogous to `serde_json::Value`.

use indexmap::IndexMap;
use serde::Serialize;
use serde::de::{self, Deserializer, Visitor};
use std::fmt;

use ayml_core::{Key, Node, Value as NodeValue};

/// An untyped AYML value. Use [`ayml_core::Node`] to add comments.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum Value {
    /// AYML `null`.
    Null,
    /// AYML `true` or `false`.
    Bool(bool),
    /// AYML integer (64-bit signed).
    Int(i64),
    /// AYML float (64-bit IEEE 754).
    Float(f64),
    /// AYML string.
    Str(String),
    /// AYML sequence.
    Seq(Vec<Value>),
    /// AYML mapping.
    Map(IndexMap<String, Value>),
}

impl<'de> serde::Deserialize<'de> for Value {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(ValueVisitor)
    }
}

struct ValueVisitor;

impl<'de> Visitor<'de> for ValueVisitor {
    type Value = Value;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "any AYML value")
    }

    fn visit_unit<E: de::Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_none<E: de::Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_some<D: Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        deserializer.deserialize_any(self)
    }

    fn visit_bool<E: de::Error>(self, v: bool) -> Result<Value, E> {
        Ok(Value::Bool(v))
    }

    fn visit_i64<E: de::Error>(self, v: i64) -> Result<Value, E> {
        Ok(Value::Int(v))
    }

    fn visit_u64<E: de::Error>(self, v: u64) -> Result<Value, E> {
        i64::try_from(v)
            .map(Value::Int)
            .map_err(|_| E::custom(format_args!("integer {v} exceeds the 64-bit signed range")))
    }

    fn visit_f64<E: de::Error>(self, v: f64) -> Result<Value, E> {
        Ok(Value::Float(v))
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<Value, E> {
        Ok(Value::Str(v.to_owned()))
    }

    fn visit_string<E: de::Error>(self, v: String) -> Result<Value, E> {
        Ok(Value::Str(v))
    }

    fn visit_seq<A: de::SeqAccess<'de>>(self, mut seq: A) -> Result<Value, A::Error> {
        let mut items = Vec::with_capacity(seq.size_hint().unwrap_or(0));
        while let Some(item) = seq.next_element()? {
            items.push(item);
        }
        Ok(Value::Seq(items))
    }

    fn visit_map<A: de::MapAccess<'de>>(self, mut map: A) -> Result<Value, A::Error> {
        let mut entries = IndexMap::with_capacity(map.size_hint().unwrap_or(0));
        while let Some((key, value)) = map.next_entry::<String, Value>()? {
            match entries.entry(key) {
                indexmap::map::Entry::Occupied(e) => {
                    return Err(de::Error::custom(format_args!(
                        "duplicate mapping key `{}`",
                        e.key()
                    )));
                }
                indexmap::map::Entry::Vacant(e) => {
                    e.insert(value);
                }
            }
        }
        Ok(Value::Map(entries))
    }
}

// ── Deserializer ────────────────────────────────────────────────────

impl<'de> Deserializer<'de> for Value {
    type Error = crate::Error;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> crate::Result<V::Value> {
        match self {
            Value::Null => visitor.visit_unit(),
            Value::Bool(b) => visitor.visit_bool(b),
            Value::Int(i) => visitor.visit_i64(i),
            Value::Float(f) => visitor.visit_f64(f),
            Value::Str(s) => visitor.visit_string(s),
            Value::Seq(items) => {
                let mut seq = de::value::SeqDeserializer::new(items.into_iter());
                let value = visitor.visit_seq(&mut seq)?;
                seq.end()?;
                Ok(value)
            }
            Value::Map(map) => {
                let mut access = de::value::MapDeserializer::new(keyed(map));
                let value = visitor.visit_map(&mut access)?;
                access.end()?;
                Ok(value)
            }
        }
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> crate::Result<V::Value> {
        match self {
            Value::Null => visitor.visit_none(),
            other => visitor.visit_some(other),
        }
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> crate::Result<V::Value> {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_enum<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> crate::Result<V::Value> {
        match self {
            Value::Str(s) => visitor.visit_enum(de::IntoDeserializer::into_deserializer(s)),
            Value::Map(map) if map.len() == 1 => visitor.visit_enum(
                de::value::MapAccessDeserializer::new(de::value::MapDeserializer::new(keyed(map))),
            ),
            other => Err(de::Error::invalid_type(
                de::Unexpected::Other(other.type_name()),
                &"a string (unit variant) or a mapping with a single key (other variants)",
            )),
        }
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf unit unit_struct seq tuple tuple_struct map struct
        identifier ignored_any
    }
}

fn keyed<'de>(
    map: IndexMap<String, Value>,
) -> impl Iterator<Item = (crate::de::KeyDeserializer<'de>, Value)> {
    map.into_iter()
        .map(|(k, v)| (crate::de::KeyDeserializer::owned(k), v))
}

impl de::IntoDeserializer<'_, crate::Error> for Value {
    type Deserializer = Self;

    fn into_deserializer(self) -> Self {
        self
    }
}

impl Value {
    /// Type name, for error messages.
    #[must_use]
    pub const fn type_name(&self) -> &'static str {
        match self {
            Value::Null => "null",
            Value::Bool(_) => "bool",
            Value::Int(_) => "int",
            Value::Float(_) => "float",
            Value::Str(_) => "string",
            Value::Seq(_) => "sequence",
            Value::Map(_) => "mapping",
        }
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Null, Value::Null) => true,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Float(a), Value::Float(b)) => (a.is_nan() && b.is_nan()) || a == b,
            (Value::Str(a), Value::Str(b)) => a == b,
            (Value::Seq(a), Value::Seq(b)) => a == b,
            (Value::Map(a), Value::Map(b)) => a == b,
            _ => false,
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&ayml_core::emit_flow(&NodeValue::from(self)))
    }
}

impl From<&Value> for NodeValue {
    fn from(value: &Value) -> Self {
        match value {
            Value::Null => Self::Null,
            Value::Bool(b) => Self::Bool(*b),
            Value::Int(i) => Self::Int(*i),
            Value::Float(f) => Self::Float(*f),
            Value::Str(s) => Self::Str(s.clone()),
            Value::Seq(items) => Self::Seq(items.iter().map(Node::from).collect()),
            Value::Map(map) => Self::Map(
                map.iter()
                    .map(|(k, v)| (Key::new(k.clone()), Node::from(v)))
                    .collect(),
            ),
        }
    }
}

impl From<&Value> for Node {
    fn from(value: &Value) -> Self {
        Self::new(NodeValue::from(value))
    }
}

impl From<&Node> for Value {
    fn from(node: &Node) -> Self {
        match &node.value {
            NodeValue::Null => Self::Null,
            NodeValue::Bool(b) => Self::Bool(*b),
            NodeValue::Int(i) => Self::Int(*i),
            NodeValue::Float(f) => Self::Float(*f),
            NodeValue::Str(s) => Self::Str(s.clone()),
            NodeValue::Seq(items) => Self::Seq(items.iter().map(Self::from).collect()),
            NodeValue::Map(map) => Self::Map(
                map.iter()
                    .map(|(k, v)| (k.name.clone(), Self::from(v)))
                    .collect(),
            ),
        }
    }
}

#[cfg(feature = "schemars")]
impl schemars::JsonSchema for Value {
    fn schema_name() -> String {
        "Value".to_owned()
    }

    fn json_schema(generator: &mut schemars::r#gen::SchemaGenerator) -> schemars::schema::Schema {
        use schemars::schema::{InstanceType, Schema, SchemaObject, SingleOrVec};

        let any_of = vec![
            generator.subschema_for::<()>(),
            generator.subschema_for::<bool>(),
            generator.subschema_for::<i64>(),
            generator.subschema_for::<f64>(),
            generator.subschema_for::<String>(),
            Schema::Object(SchemaObject {
                instance_type: Some(SingleOrVec::Single(Box::new(InstanceType::Array))),
                array: Some(Box::new(schemars::schema::ArrayValidation {
                    items: Some(schemars::schema::SingleOrVec::Single(Box::new(
                        generator.subschema_for::<Self>(),
                    ))),
                    ..Default::default()
                })),
                ..Default::default()
            }),
            Schema::Object(SchemaObject {
                instance_type: Some(SingleOrVec::Single(Box::new(InstanceType::Object))),
                object: Some(Box::new(schemars::schema::ObjectValidation {
                    additional_properties: Some(Box::new(generator.subschema_for::<Self>())),
                    ..Default::default()
                })),
                ..Default::default()
            }),
        ];

        Schema::Object(SchemaObject {
            subschemas: Some(Box::new(schemars::schema::SubschemaValidation {
                any_of: Some(any_of),
                ..Default::default()
            })),
            ..Default::default()
        })
    }
}
