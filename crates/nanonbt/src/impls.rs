//! The [`ToNBT`] and [`FromNBT`] implementations for the standard types.
//!
//! Tags are matched strictly: a value reads its own tag only. An unsigned
//! integer shares its tag with the signed integer of the same width and
//! reinterprets the bits, so every value of either type round trips.

use alloc::{borrow::Cow, collections::BTreeMap, string::String, vec::Vec};

use nanocesu8::{Cesu8, Cesu8Buf};

use crate::{
    error::{Error, Result},
    read::{FromNBT, Read},
    tag::{
        TAG_BYTE, TAG_DOUBLE, TAG_END, TAG_FLOAT, TAG_INT, TAG_INT_ARRAY, TAG_LIST, TAG_LONG,
        TAG_SHORT, TAG_STRING,
    },
    write::{ToNBT, Write},
};

/// Refuses a tag other than the one the target expects.
const fn expect(tag: u8, expected: u8) -> Result<()> {
    if tag == expected {
        Ok(())
    } else {
        Err(Error::invalid_tag(tag))
    }
}

macro_rules! scalar {
    ($($ty:ty: $tag:ident, $write:ident, $read:ident;)*) => {
        $(
            impl ToNBT for $ty {
                fn tag(&self) -> u8 {
                    $tag
                }

                fn write<W: Write>(&self, writer: &mut W) -> Result<()> {
                    writer.$write(*self)
                }
            }

            impl<'de> FromNBT<'de> for $ty {
                fn read<R: Read<'de>>(tag: u8, reader: &mut R) -> Result<Self> {
                    expect(tag, $tag)?;
                    reader.$read()
                }
            }
        )*
    };
}

scalar! {
    i8: TAG_BYTE, write_i8, read_i8;
    i16: TAG_SHORT, write_i16, read_i16;
    i32: TAG_INT, write_i32, read_i32;
    i64: TAG_LONG, write_i64, read_i64;
    f32: TAG_FLOAT, write_f32, read_f32;
    f64: TAG_DOUBLE, write_f64, read_f64;
}

macro_rules! unsigned {
    ($($ty:ty => $signed:ty: $tag:ident, $write:ident, $read:ident;)*) => {
        $(
            impl ToNBT for $ty {
                fn tag(&self) -> u8 {
                    $tag
                }

                fn write<W: Write>(&self, writer: &mut W) -> Result<()> {
                    writer.$write(self.cast_signed())
                }
            }

            impl<'de> FromNBT<'de> for $ty {
                fn read<R: Read<'de>>(tag: u8, reader: &mut R) -> Result<Self> {
                    expect(tag, $tag)?;
                    reader.$read().map(<$signed>::cast_unsigned)
                }
            }
        )*
    };
}

unsigned! {
    u8 => i8: TAG_BYTE, write_i8, read_i8;
    u16 => i16: TAG_SHORT, write_i16, read_i16;
    u32 => i32: TAG_INT, write_i32, read_i32;
    u64 => i64: TAG_LONG, write_i64, read_i64;
}

impl ToNBT for bool {
    fn tag(&self) -> u8 {
        TAG_BYTE
    }

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
    fn tag(&self) -> u8 {
        TAG_INT
    }

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

macro_rules! wide {
    ($($ty:ty;)*) => {
        $(
            impl ToNBT for $ty {
                fn tag(&self) -> u8 {
                    TAG_INT_ARRAY
                }

                fn write<W: Write>(&self, writer: &mut W) -> Result<()> {
                    writer.write_len(4)?;
                    writer.write_bytes(&self.to_be_bytes())
                }
            }

            impl<'de> FromNBT<'de> for $ty {
                fn read<R: Read<'de>>(tag: u8, reader: &mut R) -> Result<Self> {
                    expect(tag, TAG_INT_ARRAY)?;
                    Ok(Self::from_be_bytes(int_array_128(reader)?))
                }
            }
        )*
    };
}

wide! {
    i128;
    u128;
}

impl ToNBT for str {
    fn tag(&self) -> u8 {
        TAG_STRING
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_str(self)
    }
}

impl ToNBT for String {
    fn tag(&self) -> u8 {
        TAG_STRING
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_str(self)
    }
}

impl ToNBT for Cow<'_, str> {
    fn tag(&self) -> u8 {
        TAG_STRING
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_str(self)
    }
}

impl ToNBT for Cesu8 {
    fn tag(&self) -> u8 {
        TAG_STRING
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_cesu8(self)
    }
}

impl ToNBT for Cesu8Buf {
    fn tag(&self) -> u8 {
        TAG_STRING
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_cesu8(self)
    }
}

impl ToNBT for Cow<'_, Cesu8> {
    fn tag(&self) -> u8 {
        TAG_STRING
    }

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
    fn tag(&self) -> u8 {
        (**self).tag()
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<()> {
        (**self).write(writer)
    }
}

/// A list's element tag comes from its first element; an empty list is a list
/// of End, the way fastnbt writes one.
pub(crate) fn write_list<T: ToNBT, W: Write>(items: &[T], writer: &mut W) -> Result<()> {
    let element = items.first().map_or(TAG_END, ToNBT::tag);
    writer.write_tag(element)?;
    writer.write_len(items.len())?;
    for item in items {
        item.write(writer)?;
    }
    Ok(())
}

pub(crate) fn read_list<'de, T: FromNBT<'de>, R: Read<'de>>(reader: &mut R) -> Result<Vec<T>> {
    let (element, len) = reader.read_list_header()?;
    if element == TAG_END && len != 0 {
        return Err(Error::list_of_end());
    }
    reader.nest(|reader| {
        let mut out = Vec::new();
        for _ in 0..len {
            out.push(T::read(element, reader)?);
        }
        Ok(out)
    })
}

impl<T: ToNBT> ToNBT for [T] {
    fn tag(&self) -> u8 {
        TAG_LIST
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<()> {
        write_list(self, writer)
    }
}

impl<T: ToNBT, const N: usize> ToNBT for [T; N] {
    fn tag(&self) -> u8 {
        TAG_LIST
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<()> {
        write_list(self, writer)
    }
}

impl<T: ToNBT> ToNBT for Vec<T> {
    fn tag(&self) -> u8 {
        TAG_LIST
    }

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
    fn tag(&self) -> u8 {
        crate::tag::TAG_COMPOUND
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<()> {
        for (key, value) in self {
            value.write_entry(key.as_ref(), writer)?;
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
                map.insert(name.into_owned(), V::read(tag, reader)?);
            }
        })?;
        Ok(map)
    }
}
