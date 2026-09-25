//! The [`ToNBT`] and [`FromNBT`] implementations for the standard types.
//!
//! Tags are matched strictly: a value reads its own tag only. An unsigned
//! integer shares its tag with the signed integer of the same width and
//! reinterprets the bits, so every value of either type round trips.
use crate::{
    error::{Error, Result},
    read::{FromNBT, Read},
    tag::{
        TAG_BYTE, TAG_DOUBLE, TAG_END, TAG_FLOAT, TAG_INT, TAG_INT_ARRAY, TAG_LIST, TAG_LONG,
        TAG_SHORT, TAG_STRING,
    },
    write::{ToNBT, Write},
};

use alloc::{borrow::Cow, collections::BTreeMap, string::String, vec::Vec};
use nanocesu8::{Cesu8, Cesu8Buf};
/// Refuses a tag other than the one the target expects.
const fn expect(tag: u8, expected: u8) -> Result<()> {
    if tag == expected {
        Ok(())
    } else {
        Err(Error::invalid_tag(tag))
    }
}
impl ToNBT for i8 {
    const TAG: u8 = TAG_BYTE;
    fn write<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_i8(*self)
    }

    // Bytes have no endianness: the slice is one run of NBT payload.
    fn write_elements<W: Write>(elements: &[Self], writer: &mut W) -> Result<()> {
        writer.write_bytes(crate::be::as_u8(elements))
    }
}
impl<'de> FromNBT<'de> for i8 {
    fn read<R: Read<'de>>(tag: u8, reader: &mut R) -> Result<Self> {
        expect(tag, TAG_BYTE)?;
        reader.read_i8()
    }

    #[cfg(feature = "simd")]
    fn read_elements<R: Read<'de>>(element: u8, len: usize, reader: &mut R) -> Result<Vec<Self>> {
        expect(element, TAG_BYTE)?;
        let bytes = reader.read_bytes(len)?;
        if bytes.len() != len {
            return Err(Error::unexpected_eof());
        }
        Ok(crate::be::as_i8(&bytes).to_vec())
    }
}
impl ToNBT for i16 {
    const TAG: u8 = TAG_SHORT;
    fn write<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_i16(*self)
    }

    #[cfg(feature = "simd")]
    fn write_elements<W: Write>(elements: &[Self], writer: &mut W) -> Result<()> {
        crate::simd::write_be::<Self, 2, W>(elements, writer)
    }
}
impl<'de> FromNBT<'de> for i16 {
    fn read<R: Read<'de>>(tag: u8, reader: &mut R) -> Result<Self> {
        expect(tag, TAG_SHORT)?;
        reader.read_i16()
    }

    #[cfg(feature = "simd")]
    fn read_elements<R: Read<'de>>(element: u8, len: usize, reader: &mut R) -> Result<Vec<Self>> {
        expect(element, TAG_SHORT)?;
        crate::simd::read_be_elements::<Self, 2, R>(len, reader, <Self>::from_be_bytes)
    }
}
impl ToNBT for i32 {
    const TAG: u8 = TAG_INT;
    fn write<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_i32(*self)
    }

    #[cfg(feature = "simd")]
    fn write_elements<W: Write>(elements: &[Self], writer: &mut W) -> Result<()> {
        crate::simd::write_be::<Self, 4, W>(elements, writer)
    }
}
impl<'de> FromNBT<'de> for i32 {
    fn read<R: Read<'de>>(tag: u8, reader: &mut R) -> Result<Self> {
        expect(tag, TAG_INT)?;
        reader.read_i32()
    }

    #[cfg(feature = "simd")]
    fn read_elements<R: Read<'de>>(element: u8, len: usize, reader: &mut R) -> Result<Vec<Self>> {
        expect(element, TAG_INT)?;
        crate::simd::read_be_elements::<Self, 4, R>(len, reader, <Self>::from_be_bytes)
    }
}
impl ToNBT for i64 {
    const TAG: u8 = TAG_LONG;
    fn write<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_i64(*self)
    }

    #[cfg(feature = "simd")]
    fn write_elements<W: Write>(elements: &[Self], writer: &mut W) -> Result<()> {
        crate::simd::write_be::<Self, 8, W>(elements, writer)
    }
}
impl<'de> FromNBT<'de> for i64 {
    fn read<R: Read<'de>>(tag: u8, reader: &mut R) -> Result<Self> {
        expect(tag, TAG_LONG)?;
        reader.read_i64()
    }

    #[cfg(feature = "simd")]
    fn read_elements<R: Read<'de>>(element: u8, len: usize, reader: &mut R) -> Result<Vec<Self>> {
        expect(element, TAG_LONG)?;
        crate::simd::read_be_elements::<Self, 8, R>(len, reader, <Self>::from_be_bytes)
    }
}
impl ToNBT for f32 {
    const TAG: u8 = TAG_FLOAT;
    fn write<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_f32(*self)
    }

    #[cfg(feature = "simd")]
    fn write_elements<W: Write>(elements: &[Self], writer: &mut W) -> Result<()> {
        crate::simd::write_be::<Self, 4, W>(elements, writer)
    }
}
impl<'de> FromNBT<'de> for f32 {
    fn read<R: Read<'de>>(tag: u8, reader: &mut R) -> Result<Self> {
        expect(tag, TAG_FLOAT)?;
        reader.read_f32()
    }

    #[cfg(feature = "simd")]
    fn read_elements<R: Read<'de>>(element: u8, len: usize, reader: &mut R) -> Result<Vec<Self>> {
        expect(element, TAG_FLOAT)?;
        crate::simd::read_be_elements::<Self, 4, R>(len, reader, <Self>::from_be_bytes)
    }
}
impl ToNBT for f64 {
    const TAG: u8 = TAG_DOUBLE;
    fn write<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_f64(*self)
    }

    #[cfg(feature = "simd")]
    fn write_elements<W: Write>(elements: &[Self], writer: &mut W) -> Result<()> {
        crate::simd::write_be::<Self, 8, W>(elements, writer)
    }
}
impl<'de> FromNBT<'de> for f64 {
    fn read<R: Read<'de>>(tag: u8, reader: &mut R) -> Result<Self> {
        expect(tag, TAG_DOUBLE)?;
        reader.read_f64()
    }

    #[cfg(feature = "simd")]
    fn read_elements<R: Read<'de>>(element: u8, len: usize, reader: &mut R) -> Result<Vec<Self>> {
        expect(element, TAG_DOUBLE)?;
        crate::simd::read_be_elements::<Self, 8, R>(len, reader, <Self>::from_be_bytes)
    }
}
impl ToNBT for u8 {
    #[allow(clippy::use_self)]
    const TAG: u8 = TAG_BYTE;
    fn write<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_i8(self.cast_signed())
    }

    // Bytes have no endianness: the slice is one run of NBT payload.
    fn write_elements<W: Write>(elements: &[Self], writer: &mut W) -> Result<()> {
        writer.write_bytes(elements)
    }
}
impl<'de> FromNBT<'de> for u8 {
    fn read<R: Read<'de>>(tag: u8, reader: &mut R) -> Result<Self> {
        expect(tag, TAG_BYTE)?;
        reader.read_i8().map(<i8>::cast_unsigned)
    }

    #[cfg(feature = "simd")]
    fn read_elements<R: Read<'de>>(element: u8, len: usize, reader: &mut R) -> Result<Vec<Self>> {
        expect(element, TAG_BYTE)?;
        let bytes = reader.read_bytes(len)?;
        if bytes.len() != len {
            return Err(Error::unexpected_eof());
        }
        Ok(bytes.into_owned())
    }
}
impl ToNBT for u16 {
    const TAG: u8 = TAG_SHORT;
    fn write<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_i16(self.cast_signed())
    }

    #[cfg(feature = "simd")]
    fn write_elements<W: Write>(elements: &[Self], writer: &mut W) -> Result<()> {
        crate::simd::write_be::<Self, 2, W>(elements, writer)
    }
}
impl<'de> FromNBT<'de> for u16 {
    fn read<R: Read<'de>>(tag: u8, reader: &mut R) -> Result<Self> {
        expect(tag, TAG_SHORT)?;
        reader.read_i16().map(<i16>::cast_unsigned)
    }

    #[cfg(feature = "simd")]
    fn read_elements<R: Read<'de>>(element: u8, len: usize, reader: &mut R) -> Result<Vec<Self>> {
        expect(element, TAG_SHORT)?;
        crate::simd::read_be_elements::<Self, 2, R>(len, reader, <Self>::from_be_bytes)
    }
}
impl ToNBT for u32 {
    const TAG: u8 = TAG_INT;
    fn write<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_i32(self.cast_signed())
    }

    #[cfg(feature = "simd")]
    fn write_elements<W: Write>(elements: &[Self], writer: &mut W) -> Result<()> {
        crate::simd::write_be::<Self, 4, W>(elements, writer)
    }
}
impl<'de> FromNBT<'de> for u32 {
    fn read<R: Read<'de>>(tag: u8, reader: &mut R) -> Result<Self> {
        expect(tag, TAG_INT)?;
        reader.read_i32().map(<i32>::cast_unsigned)
    }

    #[cfg(feature = "simd")]
    fn read_elements<R: Read<'de>>(element: u8, len: usize, reader: &mut R) -> Result<Vec<Self>> {
        expect(element, TAG_INT)?;
        crate::simd::read_be_elements::<Self, 4, R>(len, reader, <Self>::from_be_bytes)
    }
}
impl ToNBT for u64 {
    const TAG: u8 = TAG_LONG;
    fn write<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_i64(self.cast_signed())
    }

    #[cfg(feature = "simd")]
    fn write_elements<W: Write>(elements: &[Self], writer: &mut W) -> Result<()> {
        crate::simd::write_be::<Self, 8, W>(elements, writer)
    }
}
impl<'de> FromNBT<'de> for u64 {
    fn read<R: Read<'de>>(tag: u8, reader: &mut R) -> Result<Self> {
        expect(tag, TAG_LONG)?;
        reader.read_i64().map(<i64>::cast_unsigned)
    }

    #[cfg(feature = "simd")]
    fn read_elements<R: Read<'de>>(element: u8, len: usize, reader: &mut R) -> Result<Vec<Self>> {
        expect(element, TAG_LONG)?;
        crate::simd::read_be_elements::<Self, 8, R>(len, reader, <Self>::from_be_bytes)
    }
}
impl ToNBT for bool {
    const TAG: u8 = TAG_BYTE;
    fn write<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_i8(i8::from(*self))
    }
}
/// Any non-zero byte is true.
impl<'de> FromNBT<'de> for bool {
    fn read<R: Read<'de>>(tag: u8, reader: &mut R) -> Result<Self> {
        expect(tag, TAG_BYTE)?;
        Ok(reader.read_i8()? != 0)
    }
}
impl ToNBT for char {
    const TAG: u8 = TAG_INT;
    fn write<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_i32(u32::from(*self).cast_signed())
    }
}
impl<'de> FromNBT<'de> for char {
    fn read<R: Read<'de>>(tag: u8, reader: &mut R) -> Result<Self> {
        expect(tag, TAG_INT)?;
        Self::from_u32(reader.read_i32()?.cast_unsigned()).ok_or_else(Error::invalid_char)
    }
}
/// Reads the four big-endian ints a 128-bit integer is stored as.
fn int_array_128<'de, R: Read<'de>>(reader: &mut R) -> Result<[u8; 16]> {
    if reader.read_len()? != 4 {
        return Err(Error::expected_int_array());
    }
    reader
        .read_bytes(16)?
        .as_ref()
        .try_into()
        .map_err(|_| Error::expected_int_array())
}
impl ToNBT for i128 {
    const TAG: u8 = TAG_INT_ARRAY;
    fn write<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_len(4)?;
        writer.write_bytes(&self.to_be_bytes())
    }
}
impl<'de> FromNBT<'de> for i128 {
    fn read<R: Read<'de>>(tag: u8, reader: &mut R) -> Result<Self> {
        expect(tag, TAG_INT_ARRAY)?;
        Ok(Self::from_be_bytes(int_array_128(reader)?))
    }
}
impl ToNBT for u128 {
    const TAG: u8 = TAG_INT_ARRAY;
    fn write<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_len(4)?;
        writer.write_bytes(&self.to_be_bytes())
    }
}
impl<'de> FromNBT<'de> for u128 {
    fn read<R: Read<'de>>(tag: u8, reader: &mut R) -> Result<Self> {
        expect(tag, TAG_INT_ARRAY)?;
        Ok(Self::from_be_bytes(int_array_128(reader)?))
    }
}
impl ToNBT for str {
    const TAG: u8 = TAG_STRING;
    fn write<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_str(self)
    }
}
impl ToNBT for String {
    const TAG: u8 = TAG_STRING;
    fn write<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_str(self)
    }
}
impl ToNBT for Cow<'_, str> {
    const TAG: u8 = TAG_STRING;
    fn write<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_str(self)
    }
}
impl ToNBT for Cesu8 {
    const TAG: u8 = TAG_STRING;
    fn write<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_cesu8(self)
    }
}
impl ToNBT for Cesu8Buf {
    const TAG: u8 = TAG_STRING;
    fn write<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_cesu8(self)
    }
}
impl ToNBT for Cow<'_, Cesu8> {
    const TAG: u8 = TAG_STRING;
    fn write<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_cesu8(self)
    }
}
/// Only a string the input already spells as UTF-8 can be borrowed.
impl<'de> FromNBT<'de> for &'de str {
    fn read<R: Read<'de>>(tag: u8, reader: &mut R) -> Result<Self> {
        expect(tag, TAG_STRING)?;
        match reader.read_str()? {
            Cow::Borrowed(text) => Ok(text),
            Cow::Owned(_) => Err(Error::borrowed_string()),
        }
    }
}
impl<'de> FromNBT<'de> for String {
    fn read<R: Read<'de>>(tag: u8, reader: &mut R) -> Result<Self> {
        expect(tag, TAG_STRING)?;
        Ok(reader.read_str()?.into_owned())
    }
}
impl<'de> FromNBT<'de> for Cow<'de, str> {
    fn read<R: Read<'de>>(tag: u8, reader: &mut R) -> Result<Self> {
        expect(tag, TAG_STRING)?;
        reader.read_str()
    }
}
/// A `Cesu8` borrows whenever the reader can lend its bytes.
impl<'de> FromNBT<'de> for &'de Cesu8 {
    fn read<R: Read<'de>>(tag: u8, reader: &mut R) -> Result<Self> {
        expect(tag, TAG_STRING)?;
        match reader.read_cesu8()? {
            Cow::Borrowed(text) => Ok(text),
            Cow::Owned(_) => Err(Error::borrowed_cesu8()),
        }
    }
}
impl<'de> FromNBT<'de> for Cesu8Buf {
    fn read<R: Read<'de>>(tag: u8, reader: &mut R) -> Result<Self> {
        expect(tag, TAG_STRING)?;
        Ok(reader.read_cesu8()?.into_owned())
    }
}
impl<'de> FromNBT<'de> for Cow<'de, Cesu8> {
    fn read<R: Read<'de>>(tag: u8, reader: &mut R) -> Result<Self> {
        expect(tag, TAG_STRING)?;
        reader.read_cesu8()
    }
}
/// A reference writes like the value it points at.
impl<T: ToNBT + ?Sized> ToNBT for &T {
    const TAG: u8 = T::TAG;
    fn write<W: Write>(&self, writer: &mut W) -> Result<()> {
        (**self).write(writer)
    }
}
/// A list's element tag comes from its first element; an empty list is a list
/// of End, the way fastnbt writes one.
pub(crate) fn write_list<T: ToNBT, W: Write>(items: &[T], writer: &mut W) -> Result<()> {
    let element = items.first().map_or(TAG_END, |_| T::TAG);
    writer.write_tag(element)?;
    writer.write_len(items.len())?;
    T::write_elements(items, writer)
}
pub(crate) fn read_list<'de, T: FromNBT<'de>, R: Read<'de>>(reader: &mut R) -> Result<Vec<T>> {
    let (element, len) = reader.read_list_header()?;
    if element == TAG_END {
        // Old chunks store empty lists as lists of End; a longer one would
        // be a cheap way to allocate without bound.
        return if len == 0 {
            Ok(Vec::new())
        } else {
            Err(Error::list_of_end())
        };
    }
    reader.nest(|reader| T::read_elements(element, len, reader))
}
impl<T: ToNBT> ToNBT for [T] {
    const TAG: u8 = TAG_LIST;
    fn write<W: Write>(&self, writer: &mut W) -> Result<()> {
        write_list(self, writer)
    }
}
impl<T: ToNBT, const N: usize> ToNBT for [T; N] {
    const TAG: u8 = TAG_LIST;
    fn write<W: Write>(&self, writer: &mut W) -> Result<()> {
        write_list(self, writer)
    }
}
impl<T: ToNBT> ToNBT for Vec<T> {
    const TAG: u8 = TAG_LIST;
    fn write<W: Write>(&self, writer: &mut W) -> Result<()> {
        write_list(self, writer)
    }
}
impl<'de, T: FromNBT<'de>, const N: usize> FromNBT<'de> for [T; N] {
    fn read<R: Read<'de>>(tag: u8, reader: &mut R) -> Result<Self> {
        expect(tag, TAG_LIST)?;
        read_list::<T, R>(reader)?
            .try_into()
            .map_err(|_| Error::wrong_len())
    }
}
impl<'de, T: FromNBT<'de>> FromNBT<'de> for Vec<T> {
    fn read<R: Read<'de>>(tag: u8, reader: &mut R) -> Result<Self> {
        expect(tag, TAG_LIST)?;
        read_list(reader)
    }
}
impl<K: AsRef<str> + Ord, V: ToNBT> ToNBT for BTreeMap<K, V> {
    const TAG: u8 = crate::tag::TAG_COMPOUND;
    fn write<W: Write>(&self, writer: &mut W) -> Result<()> {
        for (key, value) in self {
            value.write_entry(&Cesu8::from_str(key.as_ref()), writer)?;
        }
        writer.write_end()
    }
}
impl<'de, V: FromNBT<'de>> FromNBT<'de> for BTreeMap<String, V> {
    fn read<R: Read<'de>>(tag: u8, reader: &mut R) -> Result<Self> {
        expect(tag, crate::tag::TAG_COMPOUND)?;
        let mut map = Self::new();
        reader.nest(|reader| {
            loop {
                let tag = reader.read_tag()?;
                if tag == TAG_END {
                    return Ok(());
                }
                let name = reader.read_name()?;
                map.insert(name.decode().into_owned(), V::read(tag, reader)?);
            }
        })?;
        Ok(map)
    }
}
