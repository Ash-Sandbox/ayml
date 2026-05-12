use serde::Serialize;
use serde::ser;

use crate::error::{Error, Result};
use crate::fmt_helpers::looks_like_number;

/// Serialize a `T` to an AYML string.
///
/// # Errors
///
/// Returns an error if serialization fails.
pub fn to_string<T: Serialize>(value: &T) -> Result<String> {
    let mut buf = Vec::new();
    to_writer(&mut buf, value)?;
    Ok(String::from_utf8(buf)?)
}

/// Serialize a `T` to an AYML byte vector.
///
/// # Errors
///
/// Returns an error if serialization fails.
pub fn to_vec<T: Serialize>(value: &T) -> Result<Vec<u8>> {
    let mut buf = Vec::new();
    to_writer(&mut buf, value)?;
    Ok(buf)
}

/// Serialize a `T` as AYML into a writer.
///
/// # Errors
///
/// Returns an error if serialization or I/O fails.
pub fn to_writer<W: std::io::Write, T: Serialize>(writer: W, value: &T) -> Result<()> {
    let mut ser = Serializer::new(writer);
    value.serialize(&mut ser)?;
    ser.write_str("\n")?;
    Ok(())
}

// ── Serializer ──────────────────────────────────────────────────

#[allow(clippy::struct_excessive_bools)]
struct Serializer<W> {
    writer: W,
    indent: usize,
    /// After writing "key:", the next scalar gets " " prefix;
    /// the next collection gets "\n" and appropriate indent bump.
    after_key: bool,
    /// After "- " is pending, the next struct/map first entry goes inline
    /// (compact notation) — suppress indent for the first entry only.
    compact: bool,
    /// When true, "- " has not yet been written for the current sequence
    /// element. This allows `Commented<T>` to emit a top comment before
    /// the dash indicator.
    pending_seq_dash: bool,
    /// When true, we are serializing a mapping key. Null and float keys
    /// are rejected per the AYML spec.
    serializing_key: bool,
}

impl<W: std::io::Write> Serializer<W> {
    fn new(writer: W) -> Self {
        Self {
            writer,
            indent: 0,
            after_key: false,
            compact: false,
            pending_seq_dash: false,
            serializing_key: false,
        }
    }

    fn write_str(&mut self, s: &str) -> Result<()> {
        self.writer.write_all(s.as_bytes()).map_err(Error::from)
    }

    fn write_spaces(&mut self, count: usize) -> Result<()> {
        const SPACES: &[u8; 32] = b"                                ";
        let mut n = count;
        while n > 0 {
            let chunk = n.min(SPACES.len());
            self.writer
                .write_all(&SPACES[..chunk])
                .map_err(Error::from)?;
            n -= chunk;
        }
        Ok(())
    }

    fn write_indent(&mut self) -> Result<()> {
        if self.indent > 0 {
            self.write_spaces(self.indent)?;
        }
        Ok(())
    }

    /// Write a triple-quoted string. The closing `"""` and content lines
    /// are indented to `self.indent + 2`.
    fn write_triple_quoted(&mut self, v: &str) -> Result<()> {
        // If after_key, the `"""` follows the key on the same line
        if self.after_key {
            self.write_str(" ")?;
            self.after_key = false;
        }
        self.write_str("\"\"\"\n")?;
        let content_indent = self.indent + 2;
        for line in v.split('\n') {
            if line.is_empty() {
                self.write_str("\n")?;
            } else {
                self.write_spaces(content_indent)?;
                // Escape characters that need escaping in triple-quoted strings
                // (same as double-quoted, except `"` doesn't need escaping
                // and `\n` is represented by actual newlines)
                write_triple_quoted_line(&mut self.writer, line)?;
                self.write_str("\n")?;
            }
        }
        self.write_spaces(content_indent)?;
        self.write_str("\"\"\"")
    }

    /// If a sequence dash is pending, write "- " now. The caller is
    /// responsible for any preceding indentation.
    fn flush_pending_dash(&mut self) -> Result<()> {
        if self.pending_seq_dash {
            self.pending_seq_dash = false;
            self.write_str("- ")?;
        }
        Ok(())
    }

    /// Write the inline prefix for a scalar value that follows "key:".
    fn scalar_prefix(&mut self) -> Result<()> {
        self.flush_pending_dash()?;
        if self.after_key {
            self.write_str(" ")?;
            self.after_key = false;
        }
        Ok(())
    }

    /// Write the prefix for an enum variant key (Variant: ...).
    /// Handles `after_key`/`compact` like `Compound::write_key_prefix`.
    /// Returns true if indent was bumped by 2.
    fn variant_key_prefix(&mut self, variant: &str) -> Result<bool> {
        self.flush_pending_dash()?;
        let bumped = if self.after_key {
            self.write_str("\n")?;
            self.after_key = false;
            self.indent += 2;
            self.write_indent()?;
            true
        } else if self.compact {
            self.compact = false;
            false
        } else {
            false
        };
        write_key(&mut self.writer, variant)?;
        self.write_str(":")?;
        self.after_key = true;
        Ok(bumped)
    }
}

impl<'a, W: std::io::Write> ser::Serializer for &'a mut Serializer<W> {
    type Ok = ();
    type Error = Error;

    type SerializeSeq = SeqState<'a, W>;
    type SerializeTuple = SeqState<'a, W>;
    type SerializeTupleStruct = SeqState<'a, W>;
    type SerializeTupleVariant = SeqState<'a, W>;
    type SerializeMap = Compound<'a, W>;
    type SerializeStruct = Compound<'a, W>;
    type SerializeStructVariant = Compound<'a, W>;

    fn serialize_bool(self, v: bool) -> Result<()> {
        self.scalar_prefix()?;
        self.write_str(if v { "true" } else { "false" })
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
        self.scalar_prefix()?;
        let mut buf = itoa::Buffer::new();
        self.write_str(buf.format(v))
    }

    fn serialize_u8(self, v: u8) -> Result<()> {
        self.serialize_u64(u64::from(v))
    }

    fn serialize_u16(self, v: u16) -> Result<()> {
        self.serialize_u64(u64::from(v))
    }

    fn serialize_u32(self, v: u32) -> Result<()> {
        self.serialize_u64(u64::from(v))
    }

    fn serialize_u64(self, v: u64) -> Result<()> {
        self.scalar_prefix()?;
        let mut buf = itoa::Buffer::new();
        self.write_str(buf.format(v))
    }

    fn serialize_f32(self, v: f32) -> Result<()> {
        self.serialize_f64(f64::from(v))
    }

    fn serialize_f64(self, v: f64) -> Result<()> {
        if self.serializing_key {
            return Err(Error::Message(
                "float values are not allowed as mapping keys".into(),
            ));
        }
        self.scalar_prefix()?;
        if v.is_nan() {
            self.write_str("nan")
        } else if v.is_infinite() {
            if v.is_sign_positive() {
                self.write_str("inf")
            } else {
                self.write_str("-inf")
            }
        } else {
            let mut buf = ryu::Buffer::new();
            let s = buf.format_finite(v);
            self.write_str(s)
        }
    }

    fn serialize_char(self, v: char) -> Result<()> {
        let mut tmp = [0u8; 4];
        let s = v.encode_utf8(&mut tmp);
        self.serialize_str(s)
    }

    fn serialize_str(self, v: &str) -> Result<()> {
        self.scalar_prefix()?;
        if v.is_empty() {
            return self.write_str(r#""""#);
        }
        if v.contains('\n') && !self.serializing_key {
            return self.write_triple_quoted(v);
        }
        if v.contains('\n') || needs_quoting(v) {
            write_quoted(&mut self.writer, v).map_err(Error::from)
        } else {
            self.write_str(v)
        }
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<()> {
        self.scalar_prefix()?;
        self.write_str("[")?;
        for (i, &byte) in v.iter().enumerate() {
            if i > 0 {
                self.write_str(", ")?;
            }
            let mut buf = itoa::Buffer::new();
            self.write_str(buf.format(byte))?;
        }
        self.write_str("]")
    }

    fn serialize_none(self) -> Result<()> {
        if self.serializing_key {
            return Err(Error::Message(
                "null values are not allowed as mapping keys".into(),
            ));
        }
        self.scalar_prefix()?;
        self.write_str("null")
    }

    fn serialize_some<T: ?Sized + Serialize>(self, value: &T) -> Result<()> {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<()> {
        if self.serializing_key {
            return Err(Error::Message(
                "null values are not allowed as mapping keys".into(),
            ));
        }
        self.scalar_prefix()?;
        self.write_str("null")
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<()> {
        if self.serializing_key {
            return Err(Error::Message(
                "null values are not allowed as mapping keys".into(),
            ));
        }
        self.scalar_prefix()?;
        self.write_str("null")
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<()> {
        self.serialize_str(variant)
    }

    fn serialize_newtype_struct<T: ?Sized + Serialize>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<()> {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T: ?Sized + Serialize>(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<()> {
        let bumped = self.variant_key_prefix(variant)?;
        value.serialize(&mut *self)?;
        if bumped {
            self.indent -= 2;
        }
        Ok(())
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq> {
        Ok(SeqState {
            ser: self,
            first: true,
            bumped: false,
        })
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple> {
        self.serialize_seq(None)
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct> {
        self.serialize_seq(None)
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant> {
        let bumped = self.variant_key_prefix(variant)?;
        let mut seq = self.serialize_seq(None)?;
        seq.bumped = bumped;
        Ok(seq)
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap> {
        Ok(Compound {
            ser: self,
            first: true,
            bumped: false,
            variant_bumped: false,
            commented: false,
            top_comment: None,
            inline_comment: None,
        })
    }

    fn serialize_struct(self, name: &'static str, _len: usize) -> Result<Self::SerializeStruct> {
        if name == crate::commented::COMMENTED_STRUCT {
            return Ok(Compound {
                ser: self,
                first: true,
                bumped: false,
                variant_bumped: false,
                commented: true,
                top_comment: None,
                inline_comment: None,
            });
        }
        self.serialize_map(None)
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant> {
        let bumped = self.variant_key_prefix(variant)?;
        let mut compound = self.serialize_map(None)?;
        compound.variant_bumped = bumped;
        Ok(compound)
    }
}

// ── SeqState (SerializeSeq / SerializeTuple) ────────────────────

struct SeqState<'a, W> {
    ser: &'a mut Serializer<W>,
    first: bool,
    /// True if we bumped indent for a variant key prefix.
    bumped: bool,
}

impl<W: std::io::Write> ser::SerializeSeq for SeqState<'_, W> {
    type Ok = ();
    type Error = Error;

    fn serialize_element<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<()> {
        if self.first {
            if self.ser.after_key {
                self.ser.write_str("\n")?;
                self.ser.after_key = false;
                self.ser.indent += 2;
                self.bumped = true;
            }
            if self.ser.compact {
                self.ser.compact = false;
            } else {
                self.ser.write_indent()?;
            }
            self.first = false;
        } else {
            self.ser.write_str("\n")?;
            self.ser.write_indent()?;
        }
        self.ser.flush_pending_dash()?;
        self.ser.indent += 2;
        self.ser.pending_seq_dash = true;
        self.ser.compact = true;
        value.serialize(&mut *self.ser)?;
        self.ser.pending_seq_dash = false;
        self.ser.compact = false;
        self.ser.indent -= 2;
        Ok(())
    }

    fn end(self) -> Result<()> {
        if self.first {
            // Empty sequence — use flow style
            self.ser.flush_pending_dash()?;
            if self.ser.after_key {
                self.ser.write_str(" ")?;
                self.ser.after_key = false;
            }
            self.ser.write_str("[]")?;
        }
        if self.bumped {
            self.ser.indent -= 2;
        }
        Ok(())
    }
}

impl<W: std::io::Write> ser::SerializeTuple for SeqState<'_, W> {
    type Ok = ();
    type Error = Error;

    fn serialize_element<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<()> {
        ser::SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<()> {
        ser::SerializeSeq::end(self)
    }
}

impl<W: std::io::Write> ser::SerializeTupleStruct for SeqState<'_, W> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<()> {
        ser::SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<()> {
        ser::SerializeSeq::end(self)
    }
}

impl<W: std::io::Write> ser::SerializeTupleVariant for SeqState<'_, W> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<()> {
        ser::SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<()> {
        ser::SerializeSeq::end(self)
    }
}

// ── Compound (SerializeMap / SerializeStruct) ───────────────────

#[allow(clippy::struct_excessive_bools)]
struct Compound<'a, W> {
    ser: &'a mut Serializer<W>,
    first: bool,
    /// True if we bumped indent by 2 for a nested mapping after "key:".
    bumped: bool,
    /// True if `variant_key_prefix` already bumped indent (for struct variants).
    variant_bumped: bool,
    /// True when this Compound represents a `Commented<T>`.
    commented: bool,
    /// Buffered top comment for commented mode.
    top_comment: Option<String>,
    /// Buffered inline comment for commented mode.
    inline_comment: Option<String>,
}

impl<W: std::io::Write> Compound<'_, W> {
    fn write_key_prefix(&mut self) -> Result<()> {
        self.ser.flush_pending_dash()?;
        if self.first {
            if self.ser.after_key {
                self.ser.write_str("\n")?;
                self.ser.after_key = false;
                self.ser.indent += 2;
                self.bumped = true;
            }
            if self.ser.compact {
                self.ser.compact = false;
            } else {
                self.ser.write_indent()?;
            }
            self.first = false;
        } else {
            self.ser.write_str("\n")?;
            self.ser.write_indent()?;
        }
        Ok(())
    }

    /// Handle a field within a `Commented<T>` struct.
    fn serialize_commented_field<T: ?Sized + Serialize>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<()> {
        use crate::commented::{FIELD_INLINE_COMMENT, FIELD_TOP_COMMENT, FIELD_VALUE};
        match key {
            FIELD_TOP_COMMENT => {
                self.top_comment = capture_option_string(value);
                Ok(())
            }
            FIELD_INLINE_COMMENT => {
                self.inline_comment = capture_option_string(value);
                Ok(())
            }
            FIELD_VALUE => {
                // Emit top comment if present
                if let Some(comment) = self.top_comment.as_ref().filter(|c| !c.is_empty()) {
                    if self.ser.after_key {
                        self.ser.write_str("\n")?;
                        self.ser.after_key = false;
                        self.ser.indent += 2;
                        self.bumped = true;
                    }
                    // Write comment lines before the pending "- " (if any),
                    // at the current indent minus the dash offset.
                    let in_seq = self.ser.pending_seq_dash;
                    let comment_indent = if in_seq {
                        self.ser.indent.saturating_sub(2)
                    } else {
                        self.ser.indent
                    };
                    // The comment goes on its own line(s), so compact
                    // (which means "indent already written for this line")
                    // no longer applies.
                    self.ser.compact = false;
                    for line in comment.lines() {
                        if line.is_empty() {
                            self.ser.write_str("\n")?;
                        } else {
                            self.ser.write_spaces(comment_indent)?;
                            self.ser.write_str("# ")?;
                            self.ser.write_str(line)?;
                            self.ser.write_str("\n")?;
                        }
                    }
                    // Now position for the value to start inline.
                    // Write indent at the appropriate level, then flush
                    // the pending "- " if we're in a sequence.
                    let value_indent = if self.ser.pending_seq_dash {
                        self.ser.indent.saturating_sub(2)
                    } else {
                        self.ser.indent
                    };
                    self.ser.write_spaces(value_indent)?;
                    self.ser.flush_pending_dash()?;
                    self.ser.compact = true;
                }
                // Serialize the actual value
                value.serialize(&mut *self.ser)?;
                // Clear compact in case the value was a scalar (collections
                // consume it in write_key_prefix, but scalars don't).
                self.ser.compact = false;
                // Emit inline comment if present
                if let Some(ref comment) = self.inline_comment
                    && !comment.is_empty()
                {
                    self.ser.write_str(" # ")?;
                    self.ser.write_str(comment)?;
                }
                Ok(())
            }
            _ => {
                // Unknown field — serialize normally
                value.serialize(&mut *self.ser)
            }
        }
    }

    fn end_compound(self) -> Result<()> {
        if self.commented {
            // Commented compound — just undo any indent bumps
            if self.bumped {
                self.ser.indent -= 2;
            }
            return Ok(());
        }
        if self.first {
            // Empty map — use flow style
            self.ser.flush_pending_dash()?;
            if self.ser.after_key {
                self.ser.write_str(" ")?;
                self.ser.after_key = false;
            }
            self.ser.write_str("{}")?;
        }
        if self.bumped {
            self.ser.indent -= 2;
        }
        if self.variant_bumped {
            self.ser.indent -= 2;
        }
        Ok(())
    }
}

impl<W: std::io::Write> ser::SerializeMap for Compound<'_, W> {
    type Ok = ();
    type Error = Error;

    fn serialize_key<T: ?Sized + Serialize>(&mut self, key: &T) -> Result<()> {
        self.write_key_prefix()?;
        self.ser.serializing_key = true;
        let result = key.serialize(&mut *self.ser);
        self.ser.serializing_key = false;
        result?;
        self.ser.write_str(":")?;
        self.ser.after_key = true;
        Ok(())
    }

    fn serialize_value<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<()> {
        value.serialize(&mut *self.ser)
    }

    fn end(self) -> Result<()> {
        self.end_compound()
    }
}

impl<W: std::io::Write> ser::SerializeStruct for Compound<'_, W> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: ?Sized + Serialize>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<()> {
        if self.commented {
            return self.serialize_commented_field(key, value);
        }
        self.write_key_prefix()?;
        write_key(&mut self.ser.writer, key)?;
        self.ser.write_str(":")?;
        self.ser.after_key = true;
        value.serialize(&mut *self.ser)
    }

    fn end(self) -> Result<()> {
        self.end_compound()
    }
}

impl<W: std::io::Write> ser::SerializeStructVariant for Compound<'_, W> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T: ?Sized + Serialize>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<()> {
        ser::SerializeStruct::serialize_field(self, key, value)
    }

    fn end(self) -> Result<()> {
        self.end_compound()
    }
}

// ── Commented helpers ───────────────────────────────────────────

/// Serialize a value (expected to be `Option<String>`) and capture it.
#[allow(clippy::too_many_lines)]
fn capture_option_string<T: ?Sized + Serialize>(value: &T) -> Option<String> {
    use serde::ser::Impossible;

    struct Capturer;

    impl serde::Serializer for Capturer {
        type Ok = Option<String>;
        type Error = Error;
        type SerializeSeq = Impossible<Self::Ok, Self::Error>;
        type SerializeTuple = Impossible<Self::Ok, Self::Error>;
        type SerializeTupleStruct = Impossible<Self::Ok, Self::Error>;
        type SerializeTupleVariant = Impossible<Self::Ok, Self::Error>;
        type SerializeMap = Impossible<Self::Ok, Self::Error>;
        type SerializeStruct = Impossible<Self::Ok, Self::Error>;
        type SerializeStructVariant = Impossible<Self::Ok, Self::Error>;

        fn serialize_none(self) -> Result<Self::Ok> {
            Ok(None)
        }

        fn serialize_some<V: ?Sized + Serialize>(self, value: &V) -> Result<Self::Ok> {
            value.serialize(self)
        }

        fn serialize_str(self, v: &str) -> Result<Self::Ok> {
            Ok(Some(v.to_string()))
        }

        fn serialize_unit(self) -> Result<Self::Ok> {
            Ok(None)
        }

        fn serialize_bool(self, _: bool) -> Result<Self::Ok> {
            Ok(None)
        }
        fn serialize_i8(self, _: i8) -> Result<Self::Ok> {
            Ok(None)
        }
        fn serialize_i16(self, _: i16) -> Result<Self::Ok> {
            Ok(None)
        }
        fn serialize_i32(self, _: i32) -> Result<Self::Ok> {
            Ok(None)
        }
        fn serialize_i64(self, _: i64) -> Result<Self::Ok> {
            Ok(None)
        }
        fn serialize_u8(self, _: u8) -> Result<Self::Ok> {
            Ok(None)
        }
        fn serialize_u16(self, _: u16) -> Result<Self::Ok> {
            Ok(None)
        }
        fn serialize_u32(self, _: u32) -> Result<Self::Ok> {
            Ok(None)
        }
        fn serialize_u64(self, _: u64) -> Result<Self::Ok> {
            Ok(None)
        }
        fn serialize_f32(self, _: f32) -> Result<Self::Ok> {
            Ok(None)
        }
        fn serialize_f64(self, _: f64) -> Result<Self::Ok> {
            Ok(None)
        }
        fn serialize_char(self, _: char) -> Result<Self::Ok> {
            Ok(None)
        }
        fn serialize_bytes(self, _: &[u8]) -> Result<Self::Ok> {
            Ok(None)
        }
        fn serialize_unit_struct(self, _: &'static str) -> Result<Self::Ok> {
            Ok(None)
        }
        fn serialize_unit_variant(
            self,
            _: &'static str,
            _: u32,
            _: &'static str,
        ) -> Result<Self::Ok> {
            Ok(None)
        }
        fn serialize_newtype_struct<V: ?Sized + Serialize>(
            self,
            _: &'static str,
            _: &V,
        ) -> Result<Self::Ok> {
            Ok(None)
        }
        fn serialize_newtype_variant<V: ?Sized + Serialize>(
            self,
            _: &'static str,
            _: u32,
            _: &'static str,
            _: &V,
        ) -> Result<Self::Ok> {
            Ok(None)
        }
        fn serialize_seq(self, _: Option<usize>) -> Result<Self::SerializeSeq> {
            Err(Error::Unexpected)
        }
        fn serialize_tuple(self, _: usize) -> Result<Self::SerializeTuple> {
            Err(Error::Unexpected)
        }
        fn serialize_tuple_struct(
            self,
            _: &'static str,
            _: usize,
        ) -> Result<Self::SerializeTupleStruct> {
            Err(Error::Unexpected)
        }
        fn serialize_tuple_variant(
            self,
            _: &'static str,
            _: u32,
            _: &'static str,
            _: usize,
        ) -> Result<Self::SerializeTupleVariant> {
            Err(Error::Unexpected)
        }
        fn serialize_map(self, _: Option<usize>) -> Result<Self::SerializeMap> {
            Err(Error::Unexpected)
        }
        fn serialize_struct(self, _: &'static str, _: usize) -> Result<Self::SerializeStruct> {
            Err(Error::Unexpected)
        }
        fn serialize_struct_variant(
            self,
            _: &'static str,
            _: u32,
            _: &'static str,
            _: usize,
        ) -> Result<Self::SerializeStructVariant> {
            Err(Error::Unexpected)
        }
    }

    value.serialize(Capturer).unwrap_or(None)
}

// ── String quoting helpers ──────────────────────────────────────

/// Returns true if the string must be double-quoted in AYML output.
fn needs_quoting(s: &str) -> bool {
    let bytes = s.as_bytes();
    if bytes.is_empty() {
        return true;
    }

    let first = bytes[0];
    let last = bytes[bytes.len() - 1];

    // Trailing `:`, space, or tab always needs quoting
    if last == b':' || last == b' ' || last == b'\t' {
        return true;
    }

    // Dispatch on first byte: handle reserved words, indicators, numbers,
    // and leading whitespace without redundant branching.
    match first {
        // Leading whitespace, control characters, indicators that are never valid bare starters
        b' '
        | b'\t'
        | 0x00..=0x08
        | 0x0B..=0x1F
        | 0x7F
        | b'"'
        | b'\\'
        | b','
        | b'['
        | b']'
        | b'{'
        | b'}'
        | b'#' => return true,
        // `-` and `:` require a following ns-char; also check for numbers/reserved words
        b'-' => {
            if bytes.len() < 2 || bytes[1] == b' ' || bytes[1] == b'\t' {
                return true;
            }
            if s == "-inf" || looks_like_number(s) {
                return true;
            }
        }
        b':' => {
            if bytes.len() < 2 || bytes[1] == b' ' || bytes[1] == b'\t' {
                return true;
            }
        }
        // `+` can start `+inf` or a number
        b'+' => {
            if s == "+inf" || looks_like_number(s) {
                return true;
            }
        }
        // Digits can start numbers
        b'0'..=b'9' => {
            if looks_like_number(s) {
                return true;
            }
        }
        // Reserved words by first letter
        b'n' if s == "null" || s == "nan" => return true,
        b't' if s == "true" => return true,
        b'f' if s == "false" => return true,
        b'i' if s == "inf" => return true,
        // Multi-byte UTF-8 first byte: check c-printable
        0x80.. => {
            let ch = s.chars().next().unwrap();
            if !is_printable_for_bare(ch) {
                return true;
            }
        }
        _ => {}
    }

    // Scan remaining bytes for problematic characters.
    // Skip past the first character (which may be multi-byte).
    let mut i = if first < 0x80 {
        1
    } else {
        s.chars().next().unwrap().len_utf8()
    };
    while i < bytes.len() {
        let b = bytes[i];
        match b {
            // ASCII control characters, escaping chars, flow indicators, '#'
            0x00..=0x08
            | 0x0B..=0x1F
            | 0x7F
            | b'"'
            | b'\\'
            | b','
            | b'['
            | b']'
            | b'{'
            | b'}'
            | b'#' => return true,
            // `: ` or `:\t` mid-string
            b':' if i + 1 < bytes.len() && (bytes[i + 1] == b' ' || bytes[i + 1] == b'\t') => {
                return true;
            }
            // Multi-byte UTF-8: decode and check c-printable
            0x80.. => {
                let ch = s[i..].chars().next().unwrap();
                if !is_printable_for_bare(ch) {
                    return true;
                }
                i += ch.len_utf8();
                continue;
            }
            _ => {}
        }
        i += 1;
    }

    false
}

/// Check if a character is safe in a bare string. This is the c-printable set
/// minus line break characters (LF, CR) which are already handled by the
/// byte-level checks. NEL (U+0085) is c-printable and nb-char per the spec.
fn is_printable_for_bare(ch: char) -> bool {
    let cp = ch as u32;
    matches!(
        cp,
        0x09 |
        0x20..=0x7E |
        0x85 |
        0xA0..=0xD7FF |
        0xE000..=0xFFFD |
        0x10000..=0x10_FFFF
    )
}

/// Write a single escaped character to the writer. Handles all AYML escape
/// sequences. When `escape_double_quote` is true, `"` is escaped as `\"`;
/// otherwise it is written literally (for triple-quoted strings).
fn write_escaped_char<W: std::io::Write>(
    w: &mut W,
    ch: char,
    escape_double_quote: bool,
) -> std::io::Result<()> {
    match ch {
        '\0' => w.write_all(b"\\0"),
        '\x07' => w.write_all(b"\\a"),
        '\x08' => w.write_all(b"\\b"),
        '\t' => w.write_all(b"\\t"),
        '\n' => w.write_all(b"\\n"),
        '\x0B' => w.write_all(b"\\v"),
        '\x0C' => w.write_all(b"\\f"),
        '\r' => w.write_all(b"\\r"),
        '\x1B' => w.write_all(b"\\e"),
        '"' if escape_double_quote => w.write_all(b"\\\""),
        '\\' => w.write_all(b"\\\\"),
        c if !is_printable_for_bare(c) => {
            let cp = c as u32;
            if cp <= 0xFF {
                write!(w, "\\x{cp:02x}")
            } else if cp <= 0xFFFF {
                write!(w, "\\u{cp:04x}")
            } else {
                write!(w, "\\U{cp:08x}")
            }
        }
        c => {
            let mut buf = [0u8; 4];
            let s = c.encode_utf8(&mut buf);
            w.write_all(s.as_bytes())
        }
    }
}

/// Write a double-quoted AYML string with escaping.
fn write_quoted<W: std::io::Write>(w: &mut W, s: &str) -> std::io::Result<()> {
    w.write_all(b"\"")?;
    for ch in s.chars() {
        write_escaped_char(w, ch, true)?;
    }
    w.write_all(b"\"")?;
    Ok(())
}

/// Write a single line of triple-quoted string content, escaping control
/// characters but allowing `"` and `#` to pass through literally.
fn write_triple_quoted_line<W: std::io::Write>(w: &mut W, line: &str) -> std::io::Result<()> {
    let mut consecutive_quotes = 0u32;
    for ch in line.chars() {
        if ch == '"' {
            consecutive_quotes += 1;
            if consecutive_quotes == 3 {
                // Break the `"""` sequence by escaping this quote
                w.write_all(b"\\\"")?;
                consecutive_quotes = 0;
            } else {
                w.write_all(b"\"")?;
            }
        } else {
            consecutive_quotes = 0;
            write_escaped_char(w, ch, false)?;
        }
    }
    Ok(())
}

/// Write a mapping key, quoting if necessary.
fn write_key<W: std::io::Write>(w: &mut W, key: &str) -> Result<()> {
    if key.is_empty() || needs_quoting(key) {
        write_quoted(w, key).map_err(Error::from)
    } else {
        w.write_all(key.as_bytes()).map_err(Error::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Scalar tests ────────────────────────────────────────────

    #[test]
    fn test_bool() {
        assert_eq!(to_string(&true).unwrap(), "true\n");
        assert_eq!(to_string(&false).unwrap(), "false\n");
    }

    #[test]
    fn test_integers() {
        assert_eq!(to_string(&0i32).unwrap(), "0\n");
        assert_eq!(to_string(&42i32).unwrap(), "42\n");
        assert_eq!(to_string(&-17i64).unwrap(), "-17\n");
        assert_eq!(to_string(&255u8).unwrap(), "255\n");
        assert_eq!(to_string(&u64::MAX).unwrap(), "18446744073709551615\n");
    }

    #[test]
    fn test_floats() {
        assert_eq!(to_string(&3.25f64).unwrap(), "3.25\n");
        assert_eq!(to_string(&-0.5f64).unwrap(), "-0.5\n");
        assert_eq!(to_string(&f64::INFINITY).unwrap(), "inf\n");
        assert_eq!(to_string(&f64::NEG_INFINITY).unwrap(), "-inf\n");
        assert_eq!(to_string(&f64::NAN).unwrap(), "nan\n");
    }

    #[test]
    fn test_float_whole_number() {
        let s = to_string(&1.0f64).unwrap();
        assert!(s.trim_end().contains('.') || s.contains('e') || s.contains('E'));
    }

    #[test]
    fn test_string_bare() {
        assert_eq!(to_string(&"hello").unwrap(), "hello\n");
        assert_eq!(to_string(&"hello world").unwrap(), "hello world\n");
        assert_eq!(
            to_string(&"https://example.com").unwrap(),
            "https://example.com\n"
        );
    }

    #[test]
    fn test_string_empty() {
        assert_eq!(to_string(&"").unwrap(), "\"\"\n");
    }

    #[test]
    fn test_string_needs_quoting() {
        assert_eq!(to_string(&"null").unwrap(), "\"null\"\n");
        assert_eq!(to_string(&"true").unwrap(), "\"true\"\n");
        assert_eq!(to_string(&"false").unwrap(), "\"false\"\n");
        assert_eq!(to_string(&"inf").unwrap(), "\"inf\"\n");
        assert_eq!(to_string(&"nan").unwrap(), "\"nan\"\n");
        assert_eq!(to_string(&"42").unwrap(), "\"42\"\n");
        assert_eq!(to_string(&"3.25").unwrap(), "\"3.25\"\n");
        assert_eq!(to_string(&"0xFF").unwrap(), "\"0xFF\"\n");
    }

    #[test]
    fn test_string_escapes() {
        assert_eq!(
            to_string(&"line\nbreak").unwrap(),
            "\"\"\"\n  line\n  break\n  \"\"\"\n"
        );
        assert_eq!(to_string(&"say \"hi\"").unwrap(), "\"say \\\"hi\\\"\"\n");
        assert_eq!(to_string(&"back\\slash").unwrap(), "\"back\\\\slash\"\n");
        assert_eq!(to_string(&"\x00null").unwrap(), "\"\\0null\"\n");
    }

    #[test]
    fn test_string_indicator_chars() {
        assert_eq!(to_string(&"[array").unwrap(), "\"[array\"\n");
        assert_eq!(to_string(&"{map").unwrap(), "\"{map\"\n");
        assert_eq!(to_string(&"#comment").unwrap(), "\"#comment\"\n");
        assert_eq!(to_string(&"key: value").unwrap(), "\"key: value\"\n");
        assert_eq!(
            to_string(&"value #comment").unwrap(),
            "\"value #comment\"\n"
        );
    }

    #[test]
    fn test_string_leading_trailing_whitespace() {
        assert_eq!(to_string(&" leading").unwrap(), "\" leading\"\n");
        assert_eq!(to_string(&"trailing ").unwrap(), "\"trailing \"\n");
    }

    #[test]
    fn test_null() {
        assert_eq!(to_string::<()>(&()).unwrap(), "null\n");
    }

    #[test]
    fn test_option() {
        assert_eq!(to_string(&None::<i32>).unwrap(), "null\n");
        assert_eq!(to_string(&Some(42)).unwrap(), "42\n");
        assert_eq!(to_string(&Some("hello")).unwrap(), "hello\n");
    }

    #[test]
    fn test_newtype_struct() {
        #[derive(serde::Serialize)]
        struct Meters(f64);
        assert_eq!(to_string(&Meters(3.5)).unwrap(), "3.5\n");
    }

    #[test]
    fn test_unit_struct() {
        #[derive(serde::Serialize)]
        struct Marker;
        assert_eq!(to_string(&Marker).unwrap(), "null\n");
    }

    #[test]
    fn test_unit_variant() {
        #[derive(serde::Serialize)]
        enum Color {
            Red,
            Green,
        }
        assert_eq!(to_string(&Color::Red).unwrap(), "Red\n");
        assert_eq!(to_string(&Color::Green).unwrap(), "Green\n");
    }

    #[test]
    fn test_char() {
        assert_eq!(to_string(&'a').unwrap(), "a\n");
        // char '\n' is a single-char string containing a newline → triple-quoted
        assert_eq!(to_string(&'\n').unwrap(), "\"\"\"\n\n\n  \"\"\"\n");
        assert_eq!(to_string(&'"').unwrap(), "\"\\\"\"\n");
    }

    #[test]
    fn test_bytes() {
        let bytes: &[u8] = &[1, 2, 3];
        assert_eq!(
            to_string(&serde_bytes::Bytes::new(bytes)).unwrap(),
            "[1, 2, 3]\n"
        );
    }

    #[test]
    fn test_to_vec() {
        let v = to_vec(&42i32).unwrap();
        assert_eq!(v, b"42\n");
    }

    #[test]
    fn test_to_writer() {
        let mut buf = Vec::new();
        to_writer(&mut buf, &"hello").unwrap();
        assert_eq!(buf, b"hello\n");
    }

    // ── Collection tests ────────────────────────────────────────

    #[test]
    fn test_seq_integers() {
        let v = vec![1, 2, 3];
        assert_eq!(to_string(&v).unwrap(), "- 1\n- 2\n- 3\n");
    }

    #[test]
    fn test_seq_strings() {
        let v = vec!["hello", "world"];
        assert_eq!(to_string(&v).unwrap(), "- hello\n- world\n");
    }

    #[test]
    fn test_seq_empty() {
        let v: Vec<i32> = vec![];
        assert_eq!(to_string(&v).unwrap(), "[]\n");
    }

    #[test]
    fn test_struct_simple() {
        #[derive(serde::Serialize)]
        struct Config {
            name: String,
            port: u16,
        }
        let c = Config {
            name: "myapp".into(),
            port: 8080,
        };
        assert_eq!(to_string(&c).unwrap(), "name: myapp\nport: 8080\n");
    }

    #[test]
    fn test_struct_nested() {
        #[derive(serde::Serialize)]
        struct Outer {
            inner: Inner,
        }
        #[derive(serde::Serialize)]
        struct Inner {
            value: i32,
        }
        let o = Outer {
            inner: Inner { value: 42 },
        };
        assert_eq!(to_string(&o).unwrap(), "inner:\n  value: 42\n");
    }

    #[test]
    fn test_struct_with_seq() {
        #[derive(serde::Serialize)]
        struct Config {
            items: Vec<String>,
        }
        let c = Config {
            items: vec!["alpha".into(), "beta".into()],
        };
        assert_eq!(to_string(&c).unwrap(), "items:\n  - alpha\n  - beta\n");
    }

    #[test]
    fn test_struct_with_empty_seq() {
        #[derive(serde::Serialize)]
        struct Config {
            items: Vec<String>,
        }
        let c = Config { items: vec![] };
        assert_eq!(to_string(&c).unwrap(), "items: []\n");
    }

    #[test]
    fn test_struct_with_empty_map() {
        #[derive(serde::Serialize)]
        struct Config {
            tags: std::collections::BTreeMap<String, String>,
        }
        let c = Config {
            tags: std::collections::BTreeMap::new(),
        };
        assert_eq!(to_string(&c).unwrap(), "tags: {}\n");
    }

    #[test]
    fn test_seq_of_structs() {
        #[derive(serde::Serialize)]
        struct Item {
            name: String,
            qty: u32,
        }
        let items = vec![
            Item {
                name: "Widget".into(),
                qty: 5,
            },
            Item {
                name: "Gadget".into(),
                qty: 3,
            },
        ];
        assert_eq!(
            to_string(&items).unwrap(),
            "- name: Widget\n  qty: 5\n- name: Gadget\n  qty: 3\n"
        );
    }

    #[test]
    fn test_deeply_nested() {
        #[derive(serde::Serialize)]
        struct A {
            b: B,
        }
        #[derive(serde::Serialize)]
        struct B {
            c: C,
        }
        #[derive(serde::Serialize)]
        struct C {
            value: i32,
        }
        let a = A {
            b: B { c: C { value: 99 } },
        };
        assert_eq!(to_string(&a).unwrap(), "b:\n  c:\n    value: 99\n");
    }

    #[test]
    fn test_map() {
        let mut m = std::collections::BTreeMap::new();
        m.insert("a", 1);
        m.insert("b", 2);
        assert_eq!(to_string(&m).unwrap(), "a: 1\nb: 2\n");
    }

    #[test]
    fn test_tuple() {
        let t = (42, "hello", true);
        assert_eq!(to_string(&t).unwrap(), "- 42\n- hello\n- true\n");
    }

    #[test]
    fn test_struct_mixed() {
        #[derive(serde::Serialize)]
        struct Config {
            name: String,
            debug: bool,
            inner: Inner,
            ports: Vec<u16>,
        }
        #[derive(serde::Serialize)]
        struct Inner {
            host: String,
        }
        let c = Config {
            name: "app".into(),
            debug: true,
            inner: Inner {
                host: "localhost".into(),
            },
            ports: vec![8080, 9090],
        };
        let expected = "\
name: app
debug: true
inner:
  host: localhost
ports:
  - 8080
  - 9090
";
        assert_eq!(to_string(&c).unwrap(), expected);
    }

    #[test]
    fn test_nested_seq() {
        let v = vec![vec![1, 2], vec![3, 4]];
        let expected = "\
- - 1
  - 2
- - 3
  - 4
";
        assert_eq!(to_string(&v).unwrap(), expected);
    }

    #[test]
    fn test_seq_of_maps() {
        let mut m1 = std::collections::BTreeMap::new();
        m1.insert("x", 1);
        m1.insert("y", 2);
        let mut m2 = std::collections::BTreeMap::new();
        m2.insert("x", 3);
        m2.insert("y", 4);
        let v = vec![m1, m2];
        let expected = "\
- x: 1
  y: 2
- x: 3
  y: 4
";
        assert_eq!(to_string(&v).unwrap(), expected);
    }

    #[test]
    fn test_option_in_struct() {
        #[derive(serde::Serialize)]
        struct Config {
            name: String,
            label: Option<String>,
        }
        let c = Config {
            name: "app".into(),
            label: None,
        };
        assert_eq!(to_string(&c).unwrap(), "name: app\nlabel: null\n");

        let c2 = Config {
            name: "app".into(),
            label: Some("prod".into()),
        };
        assert_eq!(to_string(&c2).unwrap(), "name: app\nlabel: prod\n");
    }

    // ── Enum tests ──────────────────────────────────────────────

    #[test]
    fn test_enum_newtype_variant() {
        #[derive(serde::Serialize)]
        enum Shape {
            Circle(f64),
            Label(String),
        }
        assert_eq!(to_string(&Shape::Circle(3.25)).unwrap(), "Circle: 3.25\n");
        assert_eq!(
            to_string(&Shape::Label("hi".into())).unwrap(),
            "Label: hi\n"
        );
    }

    #[test]
    fn test_enum_tuple_variant() {
        #[derive(serde::Serialize)]
        enum Cmd {
            Move(i32, i32),
        }
        assert_eq!(
            to_string(&Cmd::Move(10, 20)).unwrap(),
            "Move:\n  - 10\n  - 20\n"
        );
    }

    #[test]
    fn test_enum_struct_variant() {
        #[derive(serde::Serialize)]
        enum Shape {
            Rect { w: u32, h: u32 },
        }
        assert_eq!(
            to_string(&Shape::Rect { w: 10, h: 20 }).unwrap(),
            "Rect:\n  w: 10\n  h: 20\n"
        );
    }

    #[test]
    fn test_enum_in_struct() {
        #[derive(serde::Serialize)]
        #[allow(dead_code)]
        enum Status {
            Active,
            Inactive,
        }
        #[derive(serde::Serialize)]
        struct User {
            name: String,
            status: Status,
        }
        let u = User {
            name: "Alice".into(),
            status: Status::Active,
        };
        assert_eq!(to_string(&u).unwrap(), "name: Alice\nstatus: Active\n");
    }

    #[test]
    fn test_enum_unit_in_seq() {
        #[derive(serde::Serialize)]
        enum Color {
            Red,
            Blue,
        }
        let v = vec![Color::Red, Color::Blue];
        assert_eq!(to_string(&v).unwrap(), "- Red\n- Blue\n");
    }

    #[test]
    fn test_enum_newtype_in_struct() {
        #[derive(serde::Serialize)]
        enum Value {
            Num(i32),
        }
        #[derive(serde::Serialize)]
        struct Config {
            val: Value,
        }
        let c = Config {
            val: Value::Num(42),
        };
        assert_eq!(to_string(&c).unwrap(), "val:\n  Num: 42\n");
    }

    #[test]
    fn test_float_key_rejected() {
        // Manually serialize a map with a float key via SerializeMap
        use serde::ser::SerializeMap;
        let mut buf = Vec::new();
        let mut ser = Serializer::new(&mut buf);
        let mut map = ser::Serializer::serialize_map(&mut ser, Some(1)).unwrap();
        let err = map.serialize_entry(&1.5f64, &"x").unwrap_err();
        assert!(
            err.to_string().contains("float"),
            "expected float key error, got: {err}"
        );
    }

    #[test]
    fn test_null_key_rejected() {
        use serde::ser::SerializeMap;
        let mut buf = Vec::new();
        let mut ser = Serializer::new(&mut buf);
        let mut map = ser::Serializer::serialize_map(&mut ser, Some(1)).unwrap();
        let err = map.serialize_entry(&(), &"value").unwrap_err();
        assert!(
            err.to_string().contains("null"),
            "expected null key error, got: {err}"
        );
    }
}
