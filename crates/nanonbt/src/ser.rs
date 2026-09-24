//! A serde serializer writing NBT.
//!
//! NBT writes a value's tag before the value's name, but serde only reveals
//! the type once the value itself is serialized. So the name, or a list's
//! length, is held back as a header until then.
use crate::{
    error::{Error, Result},
    serde_arrays::ArrayKind,
    tag::{
        TAG_BYTE, TAG_COMPOUND, TAG_DOUBLE, TAG_END, TAG_FLOAT, TAG_INT, TAG_INT_ARRAY, TAG_LIST,
        TAG_LONG, TAG_SHORT, TAG_STRING,
    },
};
use alloc::{string::String, vec::Vec};
use nanocesu8::Cesu8;
use serde::ser::{self, Impossible, Serialize};
/// What precedes a value, written once the value's tag is known.
enum Header {
    /// The root compound, which must be a compound, and its name if any.
    Root(Option<String>),
    /// A compound entry: the tag, then the already encoded name.
    Entry(Vec<u8>),
    /// A list: the element tag, then the number of elements.
    List { len: usize },
}
fn write_header(out: &mut Vec<u8>, header: Header, tag: u8) -> Result<()> {
    match header {
        Header::Root(name) => {
            if tag != TAG_COMPOUND {
                return Err(Error::no_root_compound());
            }
            out.push(TAG_COMPOUND);
            if let Some(name) = name {
                write_str(out, &name)?;
            }
        }
        Header::Entry(name) => {
            out.push(tag);
            write_prefixed(out, &name)?;
        }
        Header::List { len } => {
            out.push(tag);
            write_len(out, len)?;
        }
    }
    Ok(())
}
/// Writes the `i32` length of a list or array.
fn write_len(out: &mut Vec<u8>, len: usize) -> Result<()> {
    let len = i32::try_from(len).map_err(|_| Error::len_too_large())?;
    out.extend_from_slice(&len.to_be_bytes());
    Ok(())
}
/// Writes a length-prefixed modified UTF-8 string.
fn write_str(out: &mut Vec<u8>, text: &str) -> Result<()> {
    write_prefixed(out, Cesu8::from_str(text).as_bytes())
}
/// Writes bytes after their `u16` length.
///
/// fastnbt silently truncates the length of longer strings; this refuses them.
fn write_prefixed(out: &mut Vec<u8>, bytes: &[u8]) -> Result<()> {
    let len = u16::try_from(bytes.len()).map_err(|_| Error::string_too_long())?;
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(bytes);
    Ok(())
}
/// Serializes the root of a document, which must be a compound.
pub struct Serializer<'w> {
    out: &'w mut Vec<u8>,
    /// The root compound's name, or `None` to leave it out entirely.
    root_name: Option<String>,
}
impl<'w> Serializer<'w> {
    pub const fn new(out: &'w mut Vec<u8>, root_name: Option<String>) -> Self {
        Self { out, root_name }
    }
}
impl<'a> ser::Serializer for &'a mut Serializer<'_> {
    type Ok = ();
    type Error = Error;
    type SerializeSeq = Impossible<(), Error>;
    type SerializeTuple = Impossible<(), Error>;
    type SerializeTupleStruct = Impossible<(), Error>;
    type SerializeTupleVariant = Impossible<(), Error>;
    type SerializeMap = Compound<'a>;
    type SerializeStruct = Compound<'a>;
    type SerializeStructVariant = Impossible<(), Error>;
    fn serialize_map(self, _len: Option<usize>) -> Result<Compound<'a>> {
        let name = self.root_name.take();
        Ok(Compound::new(self.out, Some(Header::Root(name))))
    }
    fn serialize_struct(self, _name: &'static str, len: usize) -> Result<Compound<'a>> {
        self.serialize_map(Some(len))
    }
    fn serialize_bool(self, _: bool) -> Result<()> {
        Err(Error::no_root_compound())
    }
    fn serialize_i8(self, _: i8) -> Result<()> {
        Err(Error::no_root_compound())
    }
    fn serialize_i16(self, _: i16) -> Result<()> {
        Err(Error::no_root_compound())
    }
    fn serialize_i32(self, _: i32) -> Result<()> {
        Err(Error::no_root_compound())
    }
    fn serialize_i64(self, _: i64) -> Result<()> {
        Err(Error::no_root_compound())
    }
    fn serialize_i128(self, _: i128) -> Result<()> {
        Err(Error::no_root_compound())
    }
    fn serialize_u8(self, _: u8) -> Result<()> {
        Err(Error::no_root_compound())
    }
    fn serialize_u16(self, _: u16) -> Result<()> {
        Err(Error::no_root_compound())
    }
    fn serialize_u32(self, _: u32) -> Result<()> {
        Err(Error::no_root_compound())
    }
    fn serialize_u64(self, _: u64) -> Result<()> {
        Err(Error::no_root_compound())
    }
    fn serialize_u128(self, _: u128) -> Result<()> {
        Err(Error::no_root_compound())
    }
    fn serialize_f32(self, _: f32) -> Result<()> {
        Err(Error::no_root_compound())
    }
    fn serialize_f64(self, _: f64) -> Result<()> {
        Err(Error::no_root_compound())
    }
    fn serialize_char(self, _: char) -> Result<()> {
        Err(Error::no_root_compound())
    }
    fn serialize_str(self, _: &str) -> Result<()> {
        Err(Error::no_root_compound())
    }
    fn serialize_bytes(self, _: &[u8]) -> Result<()> {
        Err(Error::no_root_compound())
    }
    fn serialize_none(self) -> Result<()> {
        Err(Error::no_root_compound())
    }
    fn serialize_unit(self) -> Result<()> {
        Err(Error::no_root_compound())
    }
    fn serialize_unit_struct(self, _: &'static str) -> Result<()> {
        Err(Error::no_root_compound())
    }
    fn serialize_unit_variant(self, _: &'static str, _: u32, _: &'static str) -> Result<()> {
        Err(Error::no_root_compound())
    }
    fn serialize_seq(self, _: Option<usize>) -> Result<Self::SerializeSeq> {
        Err(Error::no_root_compound())
    }
    fn serialize_tuple(self, _: usize) -> Result<Self::SerializeTuple> {
        Err(Error::no_root_compound())
    }
    fn serialize_tuple_struct(
        self,
        _: &'static str,
        _: usize,
    ) -> Result<Self::SerializeTupleStruct> {
        Err(Error::no_root_compound())
    }
    fn serialize_tuple_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: usize,
    ) -> Result<Self::SerializeTupleVariant> {
        Err(Error::no_root_compound())
    }
    fn serialize_struct_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: usize,
    ) -> Result<Self::SerializeStructVariant> {
        Err(Error::no_root_compound())
    }
    fn serialize_some<T: Serialize + ?Sized>(self, _value: &T) -> Result<()> {
        Err(Error::no_root_compound())
    }
    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<()> {
        value.serialize(self)
    }
    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Result<()> {
        Err(Error::no_root_compound())
    }
}
/// The entries of a compound being serialized.
pub struct Compound<'a> {
    out: &'a mut Vec<u8>,
    /// The compound's own header, until its first entry reveals the tag.
    header: Option<Header>,
    key: Option<Vec<u8>>,
}
impl<'a> Compound<'a> {
    const fn new(out: &'a mut Vec<u8>, header: Option<Header>) -> Self {
        Self {
            out,
            header,
            key: None,
        }
    }
}
impl ser::SerializeMap for Compound<'_> {
    type Ok = ();
    type Error = Error;
    fn serialize_key<T: Serialize + ?Sized>(&mut self, key: &T) -> Result<()> {
        let mut name = Vec::new();
        key.serialize(NameSerializer { name: &mut name })?;
        self.key = Some(name);
        Ok(())
    }
    fn serialize_value<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<()> {
        let name = self.key.take().ok_or_else(Error::value_before_key)?;
        if let Some(header) = self.header.take() {
            write_header(self.out, header, TAG_COMPOUND)?;
        }
        value.serialize(Delayed {
            out: self.out,
            header: Some(Header::Entry(name)),
            in_list: false,
        })
    }
    fn end(self) -> Result<()> {
        if let Some(header) = self.header {
            write_header(self.out, header, TAG_COMPOUND)?;
        }
        self.out.push(TAG_END);
        Ok(())
    }
}
impl ser::SerializeStruct for Compound<'_> {
    type Ok = ();
    type Error = Error;
    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<()> {
        ser::SerializeMap::serialize_entry(self, key, value)
    }
    fn end(self) -> Result<()> {
        ser::SerializeMap::end(self)
    }
}
/// Serializes a value whose header has not been written yet.
struct Delayed<'a> {
    out: &'a mut Vec<u8>,
    header: Option<Header>,
    /// Whether this is a list element, which cannot be left out.
    in_list: bool,
}
impl Delayed<'_> {
    fn write_header(&mut self, tag: u8) -> Result<()> {
        match self.header.take() {
            Some(header) => write_header(self.out, header, tag),
            None => Ok(()),
        }
    }
    fn scalar(mut self, tag: u8, payload: &[u8]) -> Result<()> {
        self.write_header(tag)?;
        self.out.extend_from_slice(payload);
        Ok(())
    }
}
impl<'a> ser::Serializer for Delayed<'a> {
    type Ok = ();
    type Error = Error;
    type SerializeSeq = List<'a>;
    type SerializeTuple = List<'a>;
    type SerializeTupleStruct = List<'a>;
    type SerializeTupleVariant = List<'a>;
    type SerializeMap = Compound<'a>;
    type SerializeStruct = Compound<'a>;
    type SerializeStructVariant = Impossible<(), Error>;
    fn serialize_bool(self, v: bool) -> Result<()> {
        self.serialize_u8(u8::from(v))
    }
    fn serialize_i8(self, v: i8) -> Result<()> {
        self.scalar(TAG_BYTE, &v.to_be_bytes())
    }
    fn serialize_i16(self, v: i16) -> Result<()> {
        self.scalar(TAG_SHORT, &v.to_be_bytes())
    }
    fn serialize_i32(self, v: i32) -> Result<()> {
        self.scalar(TAG_INT, &v.to_be_bytes())
    }
    fn serialize_i64(self, v: i64) -> Result<()> {
        self.scalar(TAG_LONG, &v.to_be_bytes())
    }
    #[allow(clippy::cast_sign_loss)]
    fn serialize_i128(self, v: i128) -> Result<()> {
        self.serialize_u128(v as u128)
    }
    fn serialize_u8(self, v: u8) -> Result<()> {
        self.scalar(TAG_BYTE, &v.to_be_bytes())
    }
    fn serialize_u16(self, v: u16) -> Result<()> {
        self.scalar(TAG_SHORT, &v.to_be_bytes())
    }
    fn serialize_u32(self, v: u32) -> Result<()> {
        self.scalar(TAG_INT, &v.to_be_bytes())
    }
    fn serialize_u64(self, v: u64) -> Result<()> {
        self.scalar(TAG_LONG, &v.to_be_bytes())
    }
    /// A UUID-style int array of length 4, most significant int first.
    fn serialize_u128(mut self, v: u128) -> Result<()> {
        self.write_header(TAG_INT_ARRAY)?;
        self.out.extend_from_slice(&4i32.to_be_bytes());
        self.out.extend_from_slice(&v.to_be_bytes());
        Ok(())
    }
    fn serialize_f32(self, v: f32) -> Result<()> {
        self.scalar(TAG_FLOAT, &v.to_be_bytes())
    }
    fn serialize_f64(self, v: f64) -> Result<()> {
        self.scalar(TAG_DOUBLE, &v.to_be_bytes())
    }
    fn serialize_char(self, v: char) -> Result<()> {
        self.serialize_u32(u32::from(v))
    }
    fn serialize_str(mut self, v: &str) -> Result<()> {
        self.write_header(TAG_STRING)?;
        write_str(self.out, v)
    }
    fn serialize_map(self, _len: Option<usize>) -> Result<Compound<'a>> {
        Ok(Compound::new(self.out, self.header))
    }
    fn serialize_struct(self, _name: &'static str, _len: usize) -> Result<Compound<'a>> {
        Ok(Compound::new(self.out, self.header))
    }
    fn serialize_seq(self, len: Option<usize>) -> Result<List<'a>> {
        self.serialize_tuple(len.ok_or_else(Error::unknown_len)?)
    }
    fn serialize_tuple(mut self, len: usize) -> Result<List<'a>> {
        self.write_header(TAG_LIST)?;
        if len == 0 {
            self.out.push(TAG_END);
            self.out.extend_from_slice(&0i32.to_be_bytes());
        }
        Ok(List {
            out: self.out,
            len,
            first: true,
        })
    }
    fn serialize_tuple_struct(self, _name: &'static str, len: usize) -> Result<List<'a>> {
        self.serialize_tuple(len)
    }
    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        len: usize,
    ) -> Result<List<'a>> {
        self.serialize_seq(Some(len))
    }
    /// A list of bytes, since NBT has no dedicated type for them.
    fn serialize_bytes(mut self, v: &[u8]) -> Result<()> {
        self.write_header(TAG_LIST)?;
        self.out.push(TAG_BYTE);
        write_len(self.out, v.len())?;
        self.out.extend_from_slice(v);
        Ok(())
    }
    /// Leaves a compound entry out; lists have no way to.
    fn serialize_none(self) -> Result<()> {
        if self.in_list {
            Err(Error::none_in_list())
        } else {
            Ok(())
        }
    }
    fn serialize_some<T: Serialize + ?Sized>(self, value: &T) -> Result<()> {
        value.serialize(self)
    }
    fn serialize_unit(self) -> Result<()> {
        Err(Error::unit())
    }
    fn serialize_unit_struct(self, _name: &'static str) -> Result<()> {
        Err(Error::unit())
    }
    /// The variant's name, as a string.
    fn serialize_unit_variant(
        mut self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<()> {
        self.write_header(TAG_STRING)?;
        write_str(self.out, variant)
    }
    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        name: &'static str,
        value: &T,
    ) -> Result<()> {
        if let Some(kind) = ArrayKind::from_token(name) {
            return value.serialize(ArrayWriter {
                out: self.out,
                header: self.header,
                kind,
            });
        }
        value.serialize(self)
    }
    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Result<()> {
        Err(Error::variant())
    }
    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Impossible<(), Error>> {
        Err(Error::variant())
    }
}

/// Writes an NBT array from the bytes a `#[serde(with = ...)]` module
/// offers, in NBT's big-endian order.
///
/// The module wraps its value in a newtype struct named by the array kind,
/// and the value offers itself through [`Serialize::serialize_bytes`]. Every
/// other method is unreachable — only this crate's array modules know a
/// token — and refuses.
struct ArrayWriter<'a> {
    out: &'a mut Vec<u8>,
    header: Option<Header>,
    kind: ArrayKind,
}

/// Reorders a payload of native-endian elements to big-endian.
fn swap_elements(kind: ArrayKind, bytes: &mut [u8]) {
    if cfg!(target_endian = "big") || kind == ArrayKind::Byte {
        return;
    }
    #[cfg(feature = "simd")]
    match kind {
        ArrayKind::Int => crate::simd::swap_bytes_in_place::<4>(bytes),
        ArrayKind::Long => crate::simd::swap_bytes_in_place::<8>(bytes),
        ArrayKind::Byte => {}
    }
    #[cfg(not(feature = "simd"))]
    for element in bytes.chunks_mut(kind.size()) {
        element.reverse();
    }
}

impl ser::Serializer for ArrayWriter<'_> {
    type Ok = ();
    type Error = Error;
    type SerializeSeq = Impossible<(), Error>;
    type SerializeTuple = Impossible<(), Error>;
    type SerializeTupleStruct = Impossible<(), Error>;
    type SerializeTupleVariant = Impossible<(), Error>;
    type SerializeMap = Impossible<(), Error>;
    type SerializeStruct = Impossible<(), Error>;
    type SerializeStructVariant = Impossible<(), Error>;
    /// The array's payload, as the elements lie in memory.
    fn serialize_bytes(mut self, v: &[u8]) -> Result<()> {
        let size = self.kind.size();
        if !v.len().is_multiple_of(size) {
            return Err(Error::array_payload());
        }
        if let Some(header) = self.header.take() {
            write_header(self.out, header, self.kind.tag())?;
        }
        write_len(self.out, v.len() / size)?;
        let start = self.out.len();
        self.out.extend_from_slice(v);
        swap_elements(self.kind, &mut self.out[start..]);
        Ok(())
    }
    fn serialize_bool(self, _: bool) -> Result<()> {
        Err(Error::array_payload())
    }
    fn serialize_i8(self, _: i8) -> Result<()> {
        Err(Error::array_payload())
    }
    fn serialize_i16(self, _: i16) -> Result<()> {
        Err(Error::array_payload())
    }
    fn serialize_i32(self, _: i32) -> Result<()> {
        Err(Error::array_payload())
    }
    fn serialize_i64(self, _: i64) -> Result<()> {
        Err(Error::array_payload())
    }
    fn serialize_i128(self, _: i128) -> Result<()> {
        Err(Error::array_payload())
    }
    fn serialize_u8(self, _: u8) -> Result<()> {
        Err(Error::array_payload())
    }
    fn serialize_u16(self, _: u16) -> Result<()> {
        Err(Error::array_payload())
    }
    fn serialize_u32(self, _: u32) -> Result<()> {
        Err(Error::array_payload())
    }
    fn serialize_u64(self, _: u64) -> Result<()> {
        Err(Error::array_payload())
    }
    fn serialize_u128(self, _: u128) -> Result<()> {
        Err(Error::array_payload())
    }
    fn serialize_f32(self, _: f32) -> Result<()> {
        Err(Error::array_payload())
    }
    fn serialize_f64(self, _: f64) -> Result<()> {
        Err(Error::array_payload())
    }
    fn serialize_char(self, _: char) -> Result<()> {
        Err(Error::array_payload())
    }
    fn serialize_str(self, _: &str) -> Result<()> {
        Err(Error::array_payload())
    }
    fn serialize_none(self) -> Result<()> {
        Err(Error::array_payload())
    }
    fn serialize_some<T: Serialize + ?Sized>(self, _value: &T) -> Result<()> {
        Err(Error::array_payload())
    }
    fn serialize_unit(self) -> Result<()> {
        Err(Error::array_payload())
    }
    fn serialize_unit_struct(self, _name: &'static str) -> Result<()> {
        Err(Error::array_payload())
    }
    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
    ) -> Result<()> {
        Err(Error::array_payload())
    }
    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        _value: &T,
    ) -> Result<()> {
        Err(Error::array_payload())
    }
    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Result<()> {
        Err(Error::array_payload())
    }
    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq> {
        Err(Error::array_payload())
    }
    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple> {
        Err(Error::array_payload())
    }
    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct> {
        Err(Error::array_payload())
    }
    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant> {
        Err(Error::array_payload())
    }
    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap> {
        Err(Error::array_payload())
    }
    fn serialize_struct(self, _name: &'static str, _len: usize) -> Result<Self::SerializeStruct> {
        Err(Error::array_payload())
    }
    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant> {
        Err(Error::array_payload())
    }
}

/// The elements of a list being serialized.
///
/// Only the first element writes the list header, so a list whose elements
/// have different types produces the same malformed NBT fastnbt does.
pub struct List<'a> {
    out: &'a mut Vec<u8>,
    len: usize,
    first: bool,
}
impl ser::SerializeSeq for List<'_> {
    type Ok = ();
    type Error = Error;
    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<()> {
        let header = self.first.then_some(Header::List { len: self.len });
        value.serialize(Delayed {
            out: self.out,
            header,
            in_list: true,
        })?;
        self.first = false;
        Ok(())
    }
    fn end(self) -> Result<()> {
        Ok(())
    }
}
impl ser::SerializeTuple for List<'_> {
    type Ok = ();
    type Error = Error;
    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<()> {
        ser::SerializeSeq::serialize_element(self, value)
    }
    fn end(self) -> Result<()> {
        Ok(())
    }
}
impl ser::SerializeTupleStruct for List<'_> {
    type Ok = ();
    type Error = Error;
    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<()> {
        ser::SerializeSeq::serialize_element(self, value)
    }
    fn end(self) -> Result<()> {
        Ok(())
    }
}
impl ser::SerializeTupleVariant for List<'_> {
    type Ok = ();
    type Error = Error;
    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<()> {
        ser::SerializeSeq::serialize_element(self, value)
    }
    fn end(self) -> Result<()> {
        Ok(())
    }
}
/// Encodes a compound key, which must be string-like.
struct NameSerializer<'a> {
    name: &'a mut Vec<u8>,
}
impl ser::Serializer for NameSerializer<'_> {
    type Ok = ();
    type Error = Error;
    type SerializeSeq = Impossible<(), Error>;
    type SerializeTuple = Impossible<(), Error>;
    type SerializeTupleStruct = Impossible<(), Error>;
    type SerializeTupleVariant = Impossible<(), Error>;
    type SerializeMap = Impossible<(), Error>;
    type SerializeStruct = Impossible<(), Error>;
    type SerializeStructVariant = Impossible<(), Error>;
    fn serialize_str(self, v: &str) -> Result<()> {
        self.name.extend_from_slice(Cesu8::from_str(v).as_bytes());
        Ok(())
    }
    fn serialize_char(self, v: char) -> Result<()> {
        self.serialize_str(v.encode_utf8(&mut [0; 4]))
    }
    fn serialize_bytes(self, v: &[u8]) -> Result<()> {
        self.name.extend_from_slice(v);
        Ok(())
    }
    fn serialize_bool(self, _: bool) -> Result<()> {
        Err(Error::key_not_string())
    }
    fn serialize_i8(self, _: i8) -> Result<()> {
        Err(Error::key_not_string())
    }
    fn serialize_i16(self, _: i16) -> Result<()> {
        Err(Error::key_not_string())
    }
    fn serialize_i32(self, _: i32) -> Result<()> {
        Err(Error::key_not_string())
    }
    fn serialize_i64(self, _: i64) -> Result<()> {
        Err(Error::key_not_string())
    }
    fn serialize_i128(self, _: i128) -> Result<()> {
        Err(Error::key_not_string())
    }
    fn serialize_u8(self, _: u8) -> Result<()> {
        Err(Error::key_not_string())
    }
    fn serialize_u16(self, _: u16) -> Result<()> {
        Err(Error::key_not_string())
    }
    fn serialize_u32(self, _: u32) -> Result<()> {
        Err(Error::key_not_string())
    }
    fn serialize_u64(self, _: u64) -> Result<()> {
        Err(Error::key_not_string())
    }
    fn serialize_u128(self, _: u128) -> Result<()> {
        Err(Error::key_not_string())
    }
    fn serialize_f32(self, _: f32) -> Result<()> {
        Err(Error::key_not_string())
    }
    fn serialize_f64(self, _: f64) -> Result<()> {
        Err(Error::key_not_string())
    }
    fn serialize_none(self) -> Result<()> {
        Err(Error::key_not_string())
    }
    fn serialize_unit(self) -> Result<()> {
        Err(Error::key_not_string())
    }
    fn serialize_unit_struct(self, _: &'static str) -> Result<()> {
        Err(Error::key_not_string())
    }
    fn serialize_unit_variant(self, _: &'static str, _: u32, _: &'static str) -> Result<()> {
        Err(Error::key_not_string())
    }
    fn serialize_seq(self, _: Option<usize>) -> Result<Self::SerializeSeq> {
        Err(Error::key_not_string())
    }
    fn serialize_tuple(self, _: usize) -> Result<Self::SerializeTuple> {
        Err(Error::key_not_string())
    }
    fn serialize_tuple_struct(
        self,
        _: &'static str,
        _: usize,
    ) -> Result<Self::SerializeTupleStruct> {
        Err(Error::key_not_string())
    }
    fn serialize_tuple_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: usize,
    ) -> Result<Self::SerializeTupleVariant> {
        Err(Error::key_not_string())
    }
    fn serialize_map(self, _: Option<usize>) -> Result<Self::SerializeMap> {
        Err(Error::key_not_string())
    }
    fn serialize_struct(self, _: &'static str, _: usize) -> Result<Self::SerializeStruct> {
        Err(Error::key_not_string())
    }
    fn serialize_struct_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: usize,
    ) -> Result<Self::SerializeStructVariant> {
        Err(Error::key_not_string())
    }
    fn serialize_some<T: Serialize + ?Sized>(self, _value: &T) -> Result<()> {
        Err(Error::key_not_string())
    }
    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        _value: &T,
    ) -> Result<()> {
        Err(Error::key_not_string())
    }
    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Result<()> {
        Err(Error::key_not_string())
    }
}
