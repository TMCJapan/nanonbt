//! Optional [serde](https://serde.rs) support: serde's own derives over NBT.
//!
//! With the `serde` feature, any type implementing serde's
//! [`Serialize`] and [`Deserialize`] traits round-trips through the same wire
//! format as the native derives:
//!
//! ```
//! use nanonbt::serde::{from_bytes, to_bytes};
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Serialize, Deserialize, Debug, PartialEq)]
//! struct Player {
//!     name: String,
//!     health: f32,
//! }
//!
//! let player = Player {
//!     name: "Steve".to_owned(),
//!     health: 20.0,
//! };
//! let bytes = to_bytes(&player).unwrap();
//! let decoded: Player = from_bytes(&bytes).unwrap();
//! assert_eq!(player, decoded);
//! ```
//!
//! # Mapping
//!
//! Serde's data model is translated like this:
//!
//! * Structs and maps become `TAG_Compound`s. Field order follows the
//!   declaration (or the map's iteration order) when writing and is ignored
//!   when reading; unknown fields are skipped, unless the type asks serde to
//!   deny them.
//! * Sequences, tuples and tuples structs become `TAG_List`s. Every element
//!   must share one NBT tag, since a list stores it once.
//! * `serialize_bytes` (for example through
//!   [`serde_bytes`](https://crates.io/crates/serde_bytes)) becomes a
//!   `TAG_Byte_Array`. On the way back, the three array tags
//!   (`TAG_Byte_Array`, `TAG_Int_Array`, `TAG_Long_Array`) are accepted as
//!   sequences as well, so `Vec<i32>` decodes either representation.
//! * `bool`, `i8` and `u8` become `TAG_Byte`s; `i16`/`u16` and `i32`/`u32`
//!   and `i64`/`u64` become `TAG_Short`, `TAG_Int` and `TAG_Long`. An
//!   unsigned value that does not fit the signed tag is an error.
//! * `f32` and `f64` become `TAG_Float`/`TAG_Double`, and `char`, `str` and
//!   `String` become `TAG_String`s, encoded as Java modified UTF-8. `char`
//!   must decode from a string of exactly one character.
//! * Newtype structs are transparent. Unit structs are empty compounds; the
//!   unit type itself and enums are rejected, like the native derives.
//! * `Option<T>` reads a present value as `Some`; a missing compound field is
//!   `None`. NBT has no null, so writing `None` is an error: annotate the
//!   field with `#[serde(skip_serializing_if = "Option::is_none")]` to omit
//!   it.
//!
//! [`from_bytes`] decodes borrowed fields such as `&'de str` and
//! `&'de [u8]` (declared with `#[serde(borrow)]`) straight out of the input
//! when the modified UTF-8 spelling is also UTF-8; strings that only have a
//! modified spelling are decoded into owned values and are refused by a
//! borrowed target. [`from_reader`] is the owned counterpart for streaming
//! readers, with the same restriction as
//! [`DeserializeNbt`](crate::DeserializeNbt) versus
//! [`DeserializeNbtRef`](crate::DeserializeNbtRef).
//!
//! The [`Serializer`] and [`Deserializer`] types are public for advanced use;
//! unlike the helper traits, a [`Serializer`] writes only tag *payloads*, so a
//! complete document goes through [`to_writer`] and [`from_bytes`] or
//! [`from_reader`].

use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt;

use ::serde::de::value::{BorrowedStrDeserializer, StringDeserializer};
use ::serde::de::{
    self, Deserialize, DeserializeOwned, DeserializeSeed, MapAccess, SeqAccess, Visitor,
};
use ::serde::ser::{
    self, Impossible, Serialize, SerializeMap, SerializeSeq, SerializeStruct, SerializeTuple,
    SerializeTupleStruct,
};

use crate::codec::{read_count, write_count, write_string};
use crate::wire::{F32Be, F64Be, I16Be, I32Be, I64Be};
use crate::{Error, Read, ReadRef, Result, SliceReader, Tag, Write};

/// Encodes `value` into a complete NBT document with serde.
///
/// The document is framed like a Minecraft file: the root tag byte, an empty
/// root name, and the payload. The result is byte-for-byte identical to
/// [`to_bytes`](crate::to_bytes) for a value with an equivalent native derive.
pub fn to_bytes<T: Serialize + ?Sized>(value: &T) -> Result<Vec<u8>> {
    let mut buffer = Vec::new();
    to_writer(&mut buffer, value)?;
    Ok(buffer)
}

/// Encodes `value` into `writer` with serde; see [`to_bytes`] for the framing.
pub fn to_writer<W: Write + ?Sized, T: Serialize + ?Sized>(
    writer: &mut W,
    value: &T,
) -> Result<()> {
    let tag = value.serialize(TagProbe)?;
    writer.write_u8(tag.as_u8());
    write_string(writer, "")?;
    let written = value.serialize(Serializer { writer })?;
    if written != tag {
        return Err(Error::InvalidData(
            "value serialized with a different tag the second time around",
        ));
    }
    Ok(())
}

/// Decodes a complete NBT document with serde.
///
/// Values may borrow from `bytes`; see the module documentation for the
/// details.
pub fn from_bytes<'de, T: Deserialize<'de>>(bytes: &'de [u8]) -> Result<T> {
    let mut deserializer = Deserializer::from_slice(bytes)?;
    T::deserialize(&mut deserializer)
}

/// Decodes an owned value from a streaming reader such as
/// [`VecReader`](crate::VecReader).
///
/// Borrowed fields cannot be decoded this way; use [`from_bytes`] for a
/// borrowing decode.
pub fn from_reader<T: DeserializeOwned, R: Read>(reader: R) -> Result<T> {
    let mut deserializer = Deserializer::from_reader(reader)?;
    T::deserialize(&mut deserializer)
}

/// The input side of a serde decode: a borrowed slice or a boxed reader.
enum Source<'de> {
    Slice(SliceReader<'de>),
    Owned(Box<dyn Read + 'de>),
}

impl Read for Source<'_> {
    #[inline]
    fn read_exact(&mut self, buf: &mut [u8]) -> Result<()> {
        match self {
            Self::Slice(reader) => reader.read_exact(buf),
            Self::Owned(reader) => reader.read_exact(buf),
        }
    }

    #[inline]
    fn skip(&mut self, len: usize) -> Result<()> {
        match self {
            Self::Slice(reader) => reader.skip(len),
            Self::Owned(reader) => reader.skip(len),
        }
    }

    #[inline]
    fn remaining(&self) -> usize {
        match self {
            Self::Slice(reader) => reader.remaining(),
            Self::Owned(reader) => reader.remaining(),
        }
    }
}

impl<'de> Source<'de> {
    /// Reads `len` bytes, borrowing them when the input is a slice.
    fn read_bytes(&mut self, len: usize) -> Result<Bytes<'de>> {
        if len > self.remaining() {
            return Err(Error::UnexpectedEof);
        }
        match self {
            Self::Slice(reader) => Ok(Bytes::Borrowed(reader.read_borrowed(len)?)),
            Self::Owned(reader) => {
                let mut bytes = alloc::vec![0; len];
                reader.read_exact(&mut bytes)?;
                Ok(Bytes::Owned(bytes))
            }
        }
    }
}

/// Bytes that may point into the input buffer.
enum Bytes<'de> {
    Borrowed(&'de [u8]),
    Owned(Vec<u8>),
}

/// Text that may point into the input buffer.
enum Str<'de> {
    Borrowed(&'de str),
    Owned(String),
}

impl Str<'_> {
    fn as_str(&self) -> &str {
        match self {
            Self::Borrowed(text) => text,
            Self::Owned(text) => text,
        }
    }
}

/// Decodes modified UTF-8 (CESU-8) bytes, borrowing when possible.
fn decode(bytes: Bytes<'_>) -> Result<Str<'_>> {
    match bytes {
        Bytes::Borrowed(bytes) => {
            let text = nanocesu8::Cesu8::new(bytes).map_err(|_| Error::InvalidUtf8)?;
            text.try_decode().map_or_else(
                || Ok(Str::Owned(text.decode().into_owned())),
                |text| Ok(Str::Borrowed(text)),
            )
        }
        Bytes::Owned(bytes) => {
            let text = nanocesu8::Cesu8::new(&bytes).map_err(|_| Error::InvalidUtf8)?;
            Ok(Str::Owned(text.decode().into_owned()))
        }
    }
}

fn visit_str<'de, V: Visitor<'de>>(visitor: V, value: Str<'de>) -> Result<V::Value> {
    match value {
        Str::Borrowed(value) => visitor.visit_borrowed_str(value),
        Str::Owned(value) => visitor.visit_string(value),
    }
}

fn visit_bytes<'de, V: Visitor<'de>>(visitor: V, value: Bytes<'de>) -> Result<V::Value> {
    match value {
        Bytes::Borrowed(value) => visitor.visit_borrowed_bytes(value),
        Bytes::Owned(value) => visitor.visit_byte_buf(value),
    }
}

/// Rejects a length prefix that claims more elements than the input holds.
///
/// Elements are at least `size` bytes each, so this keeps a decoded length
/// from reserving (or reading) more than the remaining input could contain.
fn check_count<R: Read + ?Sized>(reader: &R, count: usize, size: usize) -> Result<()> {
    let bytes = count.checked_mul(size).ok_or(Error::InvalidLength)?;
    if bytes > reader.remaining() {
        return Err(Error::UnexpectedEof);
    }
    Ok(())
}

/// A serde deserializer over one NBT document.
///
/// It reads the document header (root tag and name) up front, so
/// `T::deserialize(&mut deserializer)` decodes the root payload. Prefer
/// [`from_bytes`] and [`from_reader`]; this type exists for callers that need
/// to drive serde themselves.
pub struct Deserializer<'de> {
    source: Source<'de>,
    /// The tag of the value that is about to be decoded, when a container
    /// already read it.
    tag: Option<Tag>,
}

impl<'de> Deserializer<'de> {
    /// Creates a deserializer over a complete NBT document in `bytes`.
    ///
    /// The returned value may borrow from `bytes`.
    pub fn from_slice(bytes: &'de [u8]) -> Result<Self> {
        let mut source = Source::Slice(SliceReader::new(bytes));
        let tag = read_root(&mut source)?;
        Ok(Self {
            source,
            tag: Some(tag),
        })
    }

    /// Creates a deserializer over a complete NBT document read from `reader`.
    ///
    /// The reader is boxed; decoded values are always owned.
    pub fn from_reader<R: Read + 'de>(reader: R) -> Result<Self> {
        let mut source = Source::Owned(Box::new(reader));
        let tag = read_root(&mut source)?;
        Ok(Self {
            source,
            tag: Some(tag),
        })
    }

    /// The tag the next value will be read as, consuming it or reading it
    /// from the input when a container has not done so already.
    fn take_tag(&mut self) -> Result<Tag> {
        match self.tag.take() {
            Some(tag) => Ok(tag),
            None => Tag::from_u8(self.source.read_u8()?),
        }
    }

    /// Consumes the next value's tag, requiring it to be `expected`.
    fn expect(&mut self, expected: Tag) -> Result<()> {
        let actual = self.take_tag()?;
        if actual != expected {
            return Err(Error::InvalidTag {
                expected,
                actual: actual.as_u8(),
            });
        }
        Ok(())
    }

    /// Reads a modified UTF-8 (CESU-8) string: a `u16` byte length followed by
    /// the bytes.
    fn read_string(&mut self) -> Result<Str<'de>> {
        let len = self.source.read_u16_be()? as usize;
        decode(self.source.read_bytes(len)?)
    }

    /// Reads the tag byte and length of a sequence, then hands its elements to
    /// `visitor`.
    fn read_sequence<'a, V: Visitor<'de>>(&'a mut self, tag: Tag, visitor: V) -> Result<V::Value> {
        let (element, size) = match tag {
            Tag::List => (Tag::from_u8(self.source.read_u8()?)?, 1),
            Tag::ByteArray => (Tag::Byte, 1),
            Tag::IntArray => (Tag::Int, 4),
            Tag::LongArray => (Tag::Long, 8),
            other => {
                return Err(Error::InvalidTag {
                    expected: Tag::List,
                    actual: other.as_u8(),
                });
            }
        };
        let count = read_count(&mut self.source)?;
        if element == Tag::End {
            if count != 0 {
                return Err(Error::InvalidData(
                    "non-empty list with the `End` element tag",
                ));
            }
        } else {
            check_count(&self.source, count, size)?;
        }
        visitor.visit_seq(Seq::new(self, element, count))
    }
}

/// Reads the root tag byte and skips the root name.
fn read_root(source: &mut Source<'_>) -> Result<Tag> {
    let tag = Tag::from_u8(source.read_u8()?)?;
    let len = source.read_u16_be()? as usize;
    source.skip(len)?;
    Ok(tag)
}

/// The element access of a `TAG_List` or one of the array tags.
struct Seq<'a, 'de> {
    de: &'a mut Deserializer<'de>,
    element: Tag,
    remaining: usize,
}

impl<'a, 'de> Seq<'a, 'de> {
    const fn new(de: &'a mut Deserializer<'de>, element: Tag, remaining: usize) -> Self {
        Self {
            de,
            element,
            remaining,
        }
    }
}

impl<'de> SeqAccess<'de> for Seq<'_, 'de> {
    type Error = Error;

    fn next_element_seed<T: DeserializeSeed<'de>>(&mut self, seed: T) -> Result<Option<T::Value>> {
        if self.remaining == 0 {
            return Ok(None);
        }
        self.remaining -= 1;
        self.de.tag = Some(self.element);
        seed.deserialize(&mut *self.de).map(Some)
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.remaining)
    }
}

/// The field access of a `TAG_Compound`.
struct Compound<'a, 'de> {
    de: &'a mut Deserializer<'de>,
    pending: Option<Tag>,
    done: bool,
}

impl<'a, 'de> Compound<'a, 'de> {
    const fn new(de: &'a mut Deserializer<'de>) -> Self {
        Self {
            de,
            pending: None,
            done: false,
        }
    }
}

impl<'de> MapAccess<'de> for Compound<'_, 'de> {
    type Error = Error;

    fn next_key_seed<K: DeserializeSeed<'de>>(&mut self, seed: K) -> Result<Option<K::Value>> {
        if self.done {
            return Ok(None);
        }
        let tag = Tag::from_u8(self.de.source.read_u8()?)?;
        if tag == Tag::End {
            self.done = true;
            return Ok(None);
        }
        self.pending = Some(tag);
        let value = match self.de.read_string()? {
            Str::Borrowed(name) => seed.deserialize(BorrowedStrDeserializer::<Error>::new(name))?,
            Str::Owned(name) => seed.deserialize(StringDeserializer::<Error>::new(name))?,
        };
        Ok(Some(value))
    }

    fn next_value_seed<V: DeserializeSeed<'de>>(&mut self, seed: V) -> Result<V::Value> {
        let Some(tag) = self.pending.take() else {
            return Err(Error::InvalidData("compound value without a field name"));
        };
        self.de.tag = Some(tag);
        seed.deserialize(&mut *self.de)
    }
}

/// The scalar methods share one shape: check the tag, read, visit.
macro_rules! deserialize_number {
    ($method:ident, $tag:ident, $read:ident, $visit:ident) => {
        fn $method<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
            self.expect(Tag::$tag)?;
            visitor.$visit(self.source.$read()?)
        }
    };
}

impl<'de> de::Deserializer<'de> for &mut Deserializer<'de> {
    type Error = Error;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        let tag = self.take_tag()?;
        match tag {
            Tag::Byte => visitor.visit_i8(self.source.read_u8()?.cast_signed()),
            Tag::Short => visitor.visit_i16(self.source.read_i16_be()?),
            Tag::Int => visitor.visit_i32(self.source.read_i32_be()?),
            Tag::Long => visitor.visit_i64(self.source.read_i64_be()?),
            Tag::Float => visitor.visit_f32(self.source.read_f32_be()?),
            Tag::Double => visitor.visit_f64(self.source.read_f64_be()?),
            Tag::String => {
                let value = self.read_string()?;
                visit_str(visitor, value)
            }
            Tag::ByteArray => {
                let len = read_count(&mut self.source)?;
                let value = self.source.read_bytes(len)?;
                visit_bytes(visitor, value)
            }
            Tag::List | Tag::IntArray | Tag::LongArray => self.read_sequence(tag, visitor),
            Tag::Compound => visitor.visit_map(Compound::new(self)),
            Tag::End => Err(Error::InvalidData("unexpected TAG_End")),
        }
    }

    fn deserialize_bool<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.expect(Tag::Byte)?;
        visitor.visit_bool(self.source.read_u8()? != 0)
    }

    fn deserialize_i8<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.expect(Tag::Byte)?;
        visitor.visit_i8(self.source.read_u8()?.cast_signed())
    }

    deserialize_number!(deserialize_i16, Short, read_i16_be, visit_i16);
    deserialize_number!(deserialize_i32, Int, read_i32_be, visit_i32);
    deserialize_number!(deserialize_i64, Long, read_i64_be, visit_i64);

    fn deserialize_u8<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.expect(Tag::Byte)?;
        visitor.visit_u8(self.source.read_u8()?)
    }

    fn deserialize_u16<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.expect(Tag::Short)?;
        let value = self.source.read_i16_be()?;
        let value = u16::try_from(value)
            .map_err(|_| Error::InvalidData("negative value for an unsigned integer"))?;
        visitor.visit_u16(value)
    }

    fn deserialize_u32<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.expect(Tag::Int)?;
        let value = self.source.read_i32_be()?;
        let value = u32::try_from(value)
            .map_err(|_| Error::InvalidData("negative value for an unsigned integer"))?;
        visitor.visit_u32(value)
    }

    fn deserialize_u64<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.expect(Tag::Long)?;
        let value = self.source.read_i64_be()?;
        let value = u64::try_from(value)
            .map_err(|_| Error::InvalidData("negative value for an unsigned integer"))?;
        visitor.visit_u64(value)
    }

    deserialize_number!(deserialize_f32, Float, read_f32_be, visit_f32);
    deserialize_number!(deserialize_f64, Double, read_f64_be, visit_f64);

    fn deserialize_char<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.expect(Tag::String)?;
        let text = self.read_string()?;
        let mut chars = text.as_str().chars();
        let Some(value) = chars.next() else {
            return Err(Error::InvalidData("empty string for a `char` value"));
        };
        if chars.next().is_some() {
            return Err(Error::InvalidData(
                "string of more than one character for a `char` value",
            ));
        }
        visitor.visit_char(value)
    }

    fn deserialize_str<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.expect(Tag::String)?;
        let value = self.read_string()?;
        visit_str(visitor, value)
    }

    fn deserialize_string<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.deserialize_str(visitor)
    }

    fn deserialize_bytes<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.expect(Tag::ByteArray)?;
        let len = read_count(&mut self.source)?;
        let value = self.source.read_bytes(len)?;
        visit_bytes(visitor, value)
    }

    fn deserialize_byte_buf<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.deserialize_bytes(visitor)
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        // A value is on the wire, so it is always `Some`; a missing compound
        // field never reaches this method, serde fills in `None` itself.
        visitor.visit_some(self)
    }

    fn deserialize_unit<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        let _ = visitor;
        Err(Error::InvalidData("NBT cannot represent the unit type"))
    }

    fn deserialize_unit_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value> {
        self.expect(Tag::Compound)?;
        if Tag::from_u8(self.source.read_u8()?)? != Tag::End {
            return Err(Error::InvalidData(
                "expected an empty compound for a unit struct",
            ));
        }
        visitor.visit_unit()
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value> {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_seq<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        let tag = self.take_tag()?;
        self.read_sequence(tag, visitor)
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
        self.expect(Tag::Compound)?;
        visitor.visit_map(Compound::new(self))
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
        let _ = visitor;
        Err(Error::InvalidData("NBT does not support serde enums"))
    }

    fn deserialize_identifier<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.deserialize_str(visitor)
    }

    fn deserialize_ignored_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        let tag = self.take_tag()?;
        crate::skip_nbt(&mut self.source, tag)?;
        visitor.visit_unit()
    }
}

// ---------------------------------------------------------------- serialize

/// A serde serializer that writes NBT tag payloads.
///
/// The tag itself is written by the caller: [`to_writer`] frames the root, and
/// the compound and list serializers write the tags of the fields and elements
/// they contain. The returned [`Tag`] is the tag the value just serialized as.
pub struct Serializer<'a, W: ?Sized> {
    writer: &'a mut W,
}

impl<'a, W: Write + ?Sized> Serializer<'a, W> {
    /// Creates a serializer that writes payloads to `writer`.
    pub const fn new(writer: &'a mut W) -> Self {
        Self { writer }
    }
}

impl<'a, W: Write + ?Sized> ser::Serializer for Serializer<'a, W> {
    type Ok = Tag;
    type Error = Error;
    type SerializeSeq = SeqSerializer<'a, W>;
    type SerializeTuple = SeqSerializer<'a, W>;
    type SerializeTupleStruct = SeqSerializer<'a, W>;
    type SerializeTupleVariant = Impossible<Tag, Error>;
    type SerializeMap = CompoundSerializer<'a, W>;
    type SerializeStruct = CompoundSerializer<'a, W>;
    type SerializeStructVariant = Impossible<Tag, Error>;

    fn serialize_bool(self, value: bool) -> Result<Tag> {
        self.writer.write_u8(u8::from(value));
        Ok(Tag::Byte)
    }

    fn serialize_i8(self, value: i8) -> Result<Tag> {
        self.writer.write_u8(value.cast_unsigned());
        Ok(Tag::Byte)
    }

    fn serialize_i16(self, value: i16) -> Result<Tag> {
        self.writer.write_i16_be(value);
        Ok(Tag::Short)
    }

    fn serialize_i32(self, value: i32) -> Result<Tag> {
        self.writer.write_i32_be(value);
        Ok(Tag::Int)
    }

    fn serialize_i64(self, value: i64) -> Result<Tag> {
        self.writer.write_i64_be(value);
        Ok(Tag::Long)
    }

    fn serialize_u8(self, value: u8) -> Result<Tag> {
        self.writer.write_u8(value);
        Ok(Tag::Byte)
    }

    fn serialize_u16(self, value: u16) -> Result<Tag> {
        let value = i16::try_from(value).map_err(|_| {
            Error::InvalidData("unsigned value does not fit the signed `TAG_Short`")
        })?;
        self.writer.write_i16_be(value);
        Ok(Tag::Short)
    }

    fn serialize_u32(self, value: u32) -> Result<Tag> {
        let value = i32::try_from(value)
            .map_err(|_| Error::InvalidData("unsigned value does not fit the signed `TAG_Int`"))?;
        self.writer.write_i32_be(value);
        Ok(Tag::Int)
    }

    fn serialize_u64(self, value: u64) -> Result<Tag> {
        let value = i64::try_from(value)
            .map_err(|_| Error::InvalidData("unsigned value does not fit the signed `TAG_Long`"))?;
        self.writer.write_i64_be(value);
        Ok(Tag::Long)
    }

    fn serialize_f32(self, value: f32) -> Result<Tag> {
        self.writer.write_f32_be(value);
        Ok(Tag::Float)
    }

    fn serialize_f64(self, value: f64) -> Result<Tag> {
        self.writer.write_f64_be(value);
        Ok(Tag::Double)
    }

    fn serialize_char(self, value: char) -> Result<Tag> {
        let mut buffer = [0u8; 4];
        write_string(self.writer, value.encode_utf8(&mut buffer))?;
        Ok(Tag::String)
    }

    fn serialize_str(self, value: &str) -> Result<Tag> {
        write_string(self.writer, value)?;
        Ok(Tag::String)
    }

    fn serialize_bytes(self, value: &[u8]) -> Result<Tag> {
        write_count(self.writer, value.len())?;
        self.writer.write_bytes(value);
        Ok(Tag::ByteArray)
    }

    fn serialize_none(self) -> Result<Tag> {
        Err(Error::InvalidData(
            "NBT cannot represent a missing value; use `skip_serializing_if`",
        ))
    }

    fn serialize_some<T: Serialize + ?Sized>(self, value: &T) -> Result<Tag> {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<Tag> {
        Err(Error::InvalidData("NBT cannot represent the unit type"))
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Tag> {
        self.writer.write_u8(Tag::End.as_u8());
        Ok(Tag::Compound)
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
    ) -> Result<Tag> {
        Err(Error::InvalidData("NBT does not support serde enums"))
    }

    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<Tag> {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Result<Tag> {
        Err(Error::InvalidData("NBT does not support serde enums"))
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq> {
        Ok(SeqSerializer::new(self.writer, len))
    }

    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple> {
        Ok(SeqSerializer::new(self.writer, Some(len)))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct> {
        Ok(SeqSerializer::new(self.writer, Some(len)))
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant> {
        Err(Error::InvalidData("NBT does not support serde enums"))
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap> {
        Ok(CompoundSerializer::new(self.writer))
    }

    fn serialize_struct(self, _name: &'static str, _len: usize) -> Result<Self::SerializeStruct> {
        Ok(CompoundSerializer::new(self.writer))
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant> {
        Err(Error::InvalidData("NBT does not support serde enums"))
    }
}

/// Serializes the elements of a `TAG_List`.
///
/// A list stores its element tag once, so the first element decides it and
/// every later element must match. When serde knows the length the header is
/// written before the first element and the rest stream through; when it does
/// not, the elements are buffered until the length is known.
#[doc(hidden)]
pub struct SeqSerializer<'a, W: ?Sized> {
    writer: &'a mut W,
    declared: usize,
    length_known: bool,
    started: bool,
    count: usize,
    element: Option<Tag>,
    buffer: Vec<u8>,
}

impl<'a, W: Write + ?Sized> SeqSerializer<'a, W> {
    fn new(writer: &'a mut W, len: Option<usize>) -> Self {
        Self {
            writer,
            declared: len.unwrap_or(0),
            length_known: len.is_some(),
            started: false,
            count: 0,
            element: None,
            buffer: Vec::new(),
        }
    }

    fn element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<()> {
        if self.length_known && self.count == self.declared {
            return Err(Error::InvalidData(
                "sequence yielded more elements than declared",
            ));
        }
        let tag = value.serialize(TagProbe)?;
        match self.element {
            Some(element) if element != tag => {
                return Err(Error::InvalidData(
                    "NBT lists require every element to share one tag",
                ));
            }
            _ => self.element = Some(tag),
        }
        if self.length_known {
            if !self.started {
                self.writer.write_u8(tag.as_u8());
                write_count(self.writer, self.declared)?;
                self.started = true;
            }
            let written = value.serialize(Serializer {
                writer: &mut *self.writer,
            })?;
            if written != tag {
                return Err(Error::InvalidData(
                    "element serialized with a different tag the second time around",
                ));
            }
        } else {
            value.serialize(Serializer {
                writer: &mut self.buffer,
            })?;
        }
        self.count += 1;
        Ok(())
    }

    fn finish(self) -> Result<Tag> {
        if self.length_known {
            if !self.started {
                if self.declared != 0 {
                    return Err(Error::InvalidData(
                        "sequence yielded fewer elements than declared",
                    ));
                }
                self.writer.write_u8(Tag::End.as_u8());
                write_count(self.writer, 0)?;
            } else if self.count != self.declared {
                return Err(Error::InvalidData(
                    "sequence yielded fewer elements than declared",
                ));
            }
        } else {
            match self.element {
                None => {
                    self.writer.write_u8(Tag::End.as_u8());
                    write_count(self.writer, 0)?;
                }
                Some(tag) => {
                    self.writer.write_u8(tag.as_u8());
                    write_count(self.writer, self.count)?;
                    self.writer.write_bytes(&self.buffer);
                }
            }
        }
        Ok(Tag::List)
    }
}

impl<W: Write + ?Sized> SerializeSeq for SeqSerializer<'_, W> {
    type Ok = Tag;
    type Error = Error;

    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<()> {
        self.element(value)
    }

    fn end(self) -> Result<Tag> {
        self.finish()
    }
}

impl<W: Write + ?Sized> SerializeTuple for SeqSerializer<'_, W> {
    type Ok = Tag;
    type Error = Error;

    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<()> {
        self.element(value)
    }

    fn end(self) -> Result<Tag> {
        self.finish()
    }
}

impl<W: Write + ?Sized> SerializeTupleStruct for SeqSerializer<'_, W> {
    type Ok = Tag;
    type Error = Error;

    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<()> {
        self.element(value)
    }

    fn end(self) -> Result<Tag> {
        self.finish()
    }
}

/// Serializes the fields of a `TAG_Compound`, used for both structs and maps.
///
/// The tag byte goes before the field name, so the value is probed for its tag
/// before the entry is written.
#[doc(hidden)]
pub struct CompoundSerializer<'a, W: ?Sized> {
    writer: &'a mut W,
    key: Option<String>,
}

impl<'a, W: Write + ?Sized> CompoundSerializer<'a, W> {
    const fn new(writer: &'a mut W) -> Self {
        Self { writer, key: None }
    }

    fn entry<T: Serialize + ?Sized>(&mut self, name: &str, value: &T) -> Result<()> {
        let tag = value.serialize(TagProbe)?;
        self.writer.write_u8(tag.as_u8());
        write_string(self.writer, name)?;
        let written = value.serialize(Serializer {
            writer: &mut *self.writer,
        })?;
        if written != tag {
            return Err(Error::InvalidData(
                "field serialized with a different tag the second time around",
            ));
        }
        Ok(())
    }

    fn finish(self) -> Result<Tag> {
        if self.key.is_some() {
            return Err(Error::InvalidData("compound field name without a value"));
        }
        self.writer.write_u8(Tag::End.as_u8());
        Ok(Tag::Compound)
    }
}

impl<W: Write + ?Sized> SerializeMap for CompoundSerializer<'_, W> {
    type Ok = Tag;
    type Error = Error;

    fn serialize_key<K: Serialize + ?Sized>(&mut self, key: &K) -> Result<()> {
        if self.key.is_some() {
            return Err(Error::InvalidData("compound field name without a value"));
        }
        let mut name = String::new();
        key.serialize(KeySerializer { name: &mut name })?;
        self.key = Some(name);
        Ok(())
    }

    fn serialize_value<V: Serialize + ?Sized>(&mut self, value: &V) -> Result<()> {
        let Some(name) = self.key.take() else {
            return Err(Error::InvalidData("compound value without a field name"));
        };
        self.entry(&name, value)
    }

    fn end(self) -> Result<Tag> {
        self.finish()
    }
}

impl<W: Write + ?Sized> SerializeStruct for CompoundSerializer<'_, W> {
    type Ok = Tag;
    type Error = Error;

    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        name: &'static str,
        value: &T,
    ) -> Result<()> {
        self.entry(name, value)
    }

    fn skip_field(&mut self, _name: &'static str) -> Result<()> {
        Ok(())
    }

    fn end(self) -> Result<Tag> {
        self.finish()
    }
}

/// Collects a map key: NBT compound field names are strings, so other key
/// types are rejected.
struct KeySerializer<'a> {
    name: &'a mut String,
}

const fn key_error() -> Error {
    Error::InvalidData("NBT compound field names must be strings")
}

impl ser::Serializer for KeySerializer<'_> {
    type Ok = ();
    type Error = Error;
    type SerializeSeq = Impossible<(), Error>;
    type SerializeTuple = Impossible<(), Error>;
    type SerializeTupleStruct = Impossible<(), Error>;
    type SerializeTupleVariant = Impossible<(), Error>;
    type SerializeMap = Impossible<(), Error>;
    type SerializeStruct = Impossible<(), Error>;
    type SerializeStructVariant = Impossible<(), Error>;

    fn serialize_str(self, value: &str) -> Result<()> {
        self.name.push_str(value);
        Ok(())
    }

    fn serialize_char(self, value: char) -> Result<()> {
        self.name.push(value);
        Ok(())
    }

    fn serialize_bool(self, _value: bool) -> Result<()> {
        Err(key_error())
    }

    fn serialize_i8(self, _value: i8) -> Result<()> {
        Err(key_error())
    }

    fn serialize_i16(self, _value: i16) -> Result<()> {
        Err(key_error())
    }

    fn serialize_i32(self, _value: i32) -> Result<()> {
        Err(key_error())
    }

    fn serialize_i64(self, _value: i64) -> Result<()> {
        Err(key_error())
    }

    fn serialize_u8(self, _value: u8) -> Result<()> {
        Err(key_error())
    }

    fn serialize_u16(self, _value: u16) -> Result<()> {
        Err(key_error())
    }

    fn serialize_u32(self, _value: u32) -> Result<()> {
        Err(key_error())
    }

    fn serialize_u64(self, _value: u64) -> Result<()> {
        Err(key_error())
    }

    fn serialize_f32(self, _value: f32) -> Result<()> {
        Err(key_error())
    }

    fn serialize_f64(self, _value: f64) -> Result<()> {
        Err(key_error())
    }

    fn serialize_bytes(self, _value: &[u8]) -> Result<()> {
        Err(key_error())
    }

    fn serialize_none(self) -> Result<()> {
        Err(key_error())
    }

    fn serialize_some<T: Serialize + ?Sized>(self, _value: &T) -> Result<()> {
        Err(key_error())
    }

    fn serialize_unit(self) -> Result<()> {
        Err(key_error())
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<()> {
        Err(key_error())
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
    ) -> Result<()> {
        Err(key_error())
    }

    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        _value: &T,
    ) -> Result<()> {
        Err(key_error())
    }

    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Result<()> {
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
        _variant_index: u32,
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
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant> {
        Err(key_error())
    }
}

// -------------------------------------------------------------- tag probing

/// Determines the tag of a value without writing anything.
///
/// NBT puts the tag byte before a value's payload, so a compound field or a
/// list header must know the tag before the value itself is serialized.
/// Probing walks only the outermost value: containers report their tag without
/// visiting their contents.
struct TagProbe;

impl ser::Serializer for TagProbe {
    type Ok = Tag;
    type Error = Error;
    type SerializeSeq = ProbeList;
    type SerializeTuple = ProbeList;
    type SerializeTupleStruct = ProbeList;
    type SerializeTupleVariant = Impossible<Tag, Error>;
    type SerializeMap = ProbeCompound;
    type SerializeStruct = ProbeCompound;
    type SerializeStructVariant = Impossible<Tag, Error>;

    fn serialize_bool(self, _value: bool) -> Result<Tag> {
        Ok(Tag::Byte)
    }

    fn serialize_i8(self, _value: i8) -> Result<Tag> {
        Ok(Tag::Byte)
    }

    fn serialize_i16(self, _value: i16) -> Result<Tag> {
        Ok(Tag::Short)
    }

    fn serialize_i32(self, _value: i32) -> Result<Tag> {
        Ok(Tag::Int)
    }

    fn serialize_i64(self, _value: i64) -> Result<Tag> {
        Ok(Tag::Long)
    }

    fn serialize_u8(self, _value: u8) -> Result<Tag> {
        Ok(Tag::Byte)
    }

    fn serialize_u16(self, _value: u16) -> Result<Tag> {
        Ok(Tag::Short)
    }

    fn serialize_u32(self, _value: u32) -> Result<Tag> {
        Ok(Tag::Int)
    }

    fn serialize_u64(self, _value: u64) -> Result<Tag> {
        Ok(Tag::Long)
    }

    fn serialize_f32(self, _value: f32) -> Result<Tag> {
        Ok(Tag::Float)
    }

    fn serialize_f64(self, _value: f64) -> Result<Tag> {
        Ok(Tag::Double)
    }

    fn serialize_char(self, _value: char) -> Result<Tag> {
        Ok(Tag::String)
    }

    fn serialize_str(self, _value: &str) -> Result<Tag> {
        Ok(Tag::String)
    }

    fn serialize_bytes(self, _value: &[u8]) -> Result<Tag> {
        Ok(Tag::ByteArray)
    }

    fn serialize_none(self) -> Result<Tag> {
        Err(Error::InvalidData(
            "NBT cannot represent a missing value; use `skip_serializing_if`",
        ))
    }

    fn serialize_some<T: Serialize + ?Sized>(self, value: &T) -> Result<Tag> {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<Tag> {
        Err(Error::InvalidData("NBT cannot represent the unit type"))
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Tag> {
        Ok(Tag::Compound)
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
    ) -> Result<Tag> {
        Err(Error::InvalidData("NBT does not support serde enums"))
    }

    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<Tag> {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Result<Tag> {
        Err(Error::InvalidData("NBT does not support serde enums"))
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq> {
        Ok(ProbeList)
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple> {
        Ok(ProbeList)
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct> {
        Ok(ProbeList)
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant> {
        Err(Error::InvalidData("NBT does not support serde enums"))
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap> {
        Ok(ProbeCompound)
    }

    fn serialize_struct(self, _name: &'static str, _len: usize) -> Result<Self::SerializeStruct> {
        Ok(ProbeCompound)
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant> {
        Err(Error::InvalidData("NBT does not support serde enums"))
    }
}

/// Consumes a probed sequence without looking at its elements.
struct ProbeList;

impl SerializeSeq for ProbeList {
    type Ok = Tag;
    type Error = Error;

    fn serialize_element<T: Serialize + ?Sized>(&mut self, _value: &T) -> Result<()> {
        Ok(())
    }

    fn end(self) -> Result<Tag> {
        Ok(Tag::List)
    }
}

impl SerializeTuple for ProbeList {
    type Ok = Tag;
    type Error = Error;

    fn serialize_element<T: Serialize + ?Sized>(&mut self, _value: &T) -> Result<()> {
        Ok(())
    }

    fn end(self) -> Result<Tag> {
        Ok(Tag::List)
    }
}

impl SerializeTupleStruct for ProbeList {
    type Ok = Tag;
    type Error = Error;

    fn serialize_field<T: Serialize + ?Sized>(&mut self, _value: &T) -> Result<()> {
        Ok(())
    }

    fn end(self) -> Result<Tag> {
        Ok(Tag::List)
    }
}

/// Consumes a probed compound without looking at its fields.
struct ProbeCompound;

impl SerializeMap for ProbeCompound {
    type Ok = Tag;
    type Error = Error;

    fn serialize_key<K: Serialize + ?Sized>(&mut self, _key: &K) -> Result<()> {
        Ok(())
    }

    fn serialize_value<V: Serialize + ?Sized>(&mut self, _value: &V) -> Result<()> {
        Ok(())
    }

    fn end(self) -> Result<Tag> {
        Ok(Tag::Compound)
    }
}

impl SerializeStruct for ProbeCompound {
    type Ok = Tag;
    type Error = Error;

    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        _name: &'static str,
        _value: &T,
    ) -> Result<()> {
        Ok(())
    }

    fn skip_field(&mut self, _name: &'static str) -> Result<()> {
        Ok(())
    }

    fn end(self) -> Result<Tag> {
        Ok(Tag::Compound)
    }
}

// ------------------------------------------------------------ error and wire

impl ser::Error for Error {
    fn custom<T: fmt::Display>(message: T) -> Self {
        Self::Message(message.to_string())
    }
}

impl de::Error for Error {
    fn custom<T: fmt::Display>(message: T) -> Self {
        Self::Message(message.to_string())
    }
}

impl Serialize for I16Be {
    #[inline]
    fn serialize<S: ser::Serializer>(
        &self,
        serializer: S,
    ) -> core::result::Result<S::Ok, S::Error> {
        serializer.serialize_i16(self.get())
    }
}

impl<'de> Deserialize<'de> for I16Be {
    #[inline]
    fn deserialize<D: de::Deserializer<'de>>(
        deserializer: D,
    ) -> core::result::Result<Self, D::Error> {
        i16::deserialize(deserializer).map(Self::new)
    }
}

impl Serialize for I32Be {
    #[inline]
    fn serialize<S: ser::Serializer>(
        &self,
        serializer: S,
    ) -> core::result::Result<S::Ok, S::Error> {
        serializer.serialize_i32(self.get())
    }
}

impl<'de> Deserialize<'de> for I32Be {
    #[inline]
    fn deserialize<D: de::Deserializer<'de>>(
        deserializer: D,
    ) -> core::result::Result<Self, D::Error> {
        i32::deserialize(deserializer).map(Self::new)
    }
}

impl Serialize for I64Be {
    #[inline]
    fn serialize<S: ser::Serializer>(
        &self,
        serializer: S,
    ) -> core::result::Result<S::Ok, S::Error> {
        serializer.serialize_i64(self.get())
    }
}

impl<'de> Deserialize<'de> for I64Be {
    #[inline]
    fn deserialize<D: de::Deserializer<'de>>(
        deserializer: D,
    ) -> core::result::Result<Self, D::Error> {
        i64::deserialize(deserializer).map(Self::new)
    }
}

impl Serialize for F32Be {
    #[inline]
    fn serialize<S: ser::Serializer>(
        &self,
        serializer: S,
    ) -> core::result::Result<S::Ok, S::Error> {
        serializer.serialize_f32(self.get())
    }
}

impl<'de> Deserialize<'de> for F32Be {
    #[inline]
    fn deserialize<D: de::Deserializer<'de>>(
        deserializer: D,
    ) -> core::result::Result<Self, D::Error> {
        f32::deserialize(deserializer).map(Self::new)
    }
}

impl Serialize for F64Be {
    #[inline]
    fn serialize<S: ser::Serializer>(
        &self,
        serializer: S,
    ) -> core::result::Result<S::Ok, S::Error> {
        serializer.serialize_f64(self.get())
    }
}

impl<'de> Deserialize<'de> for F64Be {
    #[inline]
    fn deserialize<D: de::Deserializer<'de>>(
        deserializer: D,
    ) -> core::result::Result<Self, D::Error> {
        f64::deserialize(deserializer).map(Self::new)
    }
}
