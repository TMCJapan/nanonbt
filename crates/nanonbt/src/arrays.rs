//! The NBT array traits, which serde's data model has no place for.
//!
//! [`ByteArray`], [`IntArray`] and [`LongArray`] write and read the payload
//! of an NBT array of each kind. They are implemented for anything that is
//! `AsRef<[E]>`, with `E` one of the element spellings the kind holds: `i8`
//! or `u8`, `i32` or `u32` and the big-endian [`I32Be`] and [`U32Be`], and
//! `i64` or `u64` and the big-endian [`I64Be`] and [`U64Be`]. A derived field
//! with `#[nbt(array = "...")]` goes through them, and
//! [`Write`](crate::Write)'s array methods dispatch to them, whichever
//! spelling the caller holds.
//!
//! The bytes of an unsigned element are the bytes of its signed element, as
//! they are everywhere else in the crate.

use alloc::vec::Vec;

use nanocesu8::Cesu8;

use crate::{
    be::{
        I32Be, I64Be, U32Be, U64Be, as_i32be, as_i64be, as_u8, as_u32be, as_u64be, i32be_as_bytes,
        i64be_as_bytes,
    },
    error::{Error, Result},
    read::Read,
    tag::{TAG_BYTE_ARRAY, TAG_INT_ARRAY, TAG_LONG_ARRAY},
    write::Write,
};

/// Decodes `len` big-endian elements of `SIZE` bytes each.
fn read_be<'de, T: Copy, const SIZE: usize, R: Read<'de>>(
    len: usize,
    reader: &mut R,
    decode: fn([u8; SIZE]) -> T,
) -> Result<Vec<T>> {
    let n = len
        .checked_mul(SIZE)
        .ok_or_else(crate::Error::array_too_large)?;
    let bytes = reader.read_bytes(n)?;
    #[cfg(feature = "simd")]
    {
        Ok(crate::simd::decode_be::<T, SIZE>(&bytes, decode))
    }
    #[cfg(not(feature = "simd"))]
    {
        Ok(bytes
            .as_chunks::<SIZE>()
            .0
            .iter()
            .map(|chunk| decode(*chunk))
            .collect())
    }
}

/// An NBT byte array, holding `i8` or `u8` elements.
///
/// Implemented for every `AsRef<[E]>` with one of those two element
/// spellings, so a `Vec<i8>`, a `[u8; N]` and a `&[u8]` all write and read a
/// `TAG_Byte_Array` through it. A derived field with
/// `#[nbt(array = "byte")]` and [`Write::write_byte_array`] are the usual
/// ways in.
pub trait ByteArray<E>: AsRef<[E]> {
    /// Writes the payload: the `i32` length, then the elements.
    fn write_payload<W: Write + ?Sized>(&self, writer: &mut W) -> Result<()>;

    /// Reads the payload, `len` elements.
    fn read_payload<'de, R: Read<'de>>(len: usize, reader: &mut R) -> Result<Vec<E>>;

    /// Writes a compound entry: the tag, the name, then the payload.
    ///
    /// The name is taken as its exact modified UTF-8 bytes, which is what
    /// the derive macros encode a field's name to while they expand; nothing
    /// is converted here.
    fn write_entry<W: Write>(&self, name: &Cesu8, writer: &mut W) -> Result<()> {
        writer.write_tag(TAG_BYTE_ARRAY)?;
        writer.write_name(name)?;
        self.write_payload(writer)
    }

    /// Reads an array, refusing a tag other than [`TAG_BYTE_ARRAY`].
    fn read<'de, R: Read<'de>>(tag: u8, reader: &mut R) -> Result<Vec<E>> {
        if tag != TAG_BYTE_ARRAY {
            return Err(Error::invalid_tag(tag));
        }
        let len = reader.read_len()?;
        Self::read_payload(len, reader)
    }
}

impl<T: AsRef<[i8]> + ?Sized> ByteArray<i8> for T {
    fn write_payload<W: Write + ?Sized>(&self, writer: &mut W) -> Result<()> {
        let elements: &[i8] = self.as_ref();
        writer.write_len(elements.len())?;
        writer.write_bytes(as_u8(elements))
    }
    fn read_payload<'de, R: Read<'de>>(len: usize, reader: &mut R) -> Result<Vec<i8>> {
        Ok(reader.read_byte_array(len)?.into_owned())
    }
}

/// The unsigned spelling of the element, sharing the same bits.
impl<T: AsRef<[u8]> + ?Sized> ByteArray<u8> for T {
    fn write_payload<W: Write + ?Sized>(&self, writer: &mut W) -> Result<()> {
        let elements: &[u8] = self.as_ref();
        writer.write_len(elements.len())?;
        writer.write_bytes(elements)
    }
    fn read_payload<'de, R: Read<'de>>(len: usize, reader: &mut R) -> Result<Vec<u8>> {
        let array = reader.read_byte_array(len)?;
        Ok(as_u8(&array).to_vec())
    }
}

/// An NBT int array, holding `i32`, `u32`, [`I32Be`] or [`U32Be`] elements.
///
/// Implemented for every `AsRef<[E]>` with one of those spellings, so a
/// `Vec<u32>`, an `[I32Be; N]` and a borrowed `&[I32Be]` all write and read
/// a `TAG_Int_Array`. A derived field with `#[nbt(array = "int")]` and
/// [`Write::write_int_array`] are the usual ways in.
pub trait IntArray<E>: AsRef<[E]> {
    /// Writes the payload: the `i32` length, then the elements.
    fn write_payload<W: Write + ?Sized>(&self, writer: &mut W) -> Result<()>;

    /// Reads the payload, `len` elements.
    fn read_payload<'de, R: Read<'de>>(len: usize, reader: &mut R) -> Result<Vec<E>>;

    /// Writes a compound entry: the tag, the name, then the payload.
    ///
    /// The name is taken as its exact modified UTF-8 bytes, which is what
    /// the derive macros encode a field's name to while they expand; nothing
    /// is converted here.
    fn write_entry<W: Write>(&self, name: &Cesu8, writer: &mut W) -> Result<()> {
        writer.write_tag(TAG_INT_ARRAY)?;
        writer.write_name(name)?;
        self.write_payload(writer)
    }

    /// Reads an array, refusing a tag other than [`TAG_INT_ARRAY`].
    fn read<'de, R: Read<'de>>(tag: u8, reader: &mut R) -> Result<Vec<E>> {
        if tag != TAG_INT_ARRAY {
            return Err(Error::invalid_tag(tag));
        }
        let len = reader.read_len()?;
        Self::read_payload(len, reader)
    }
}

impl<T: AsRef<[i32]> + ?Sized> IntArray<i32> for T {
    fn write_payload<W: Write + ?Sized>(&self, writer: &mut W) -> Result<()> {
        let elements: &[i32] = self.as_ref();
        writer.write_len(elements.len())?;
        #[cfg(feature = "simd")]
        {
            crate::simd::write_be::<i32, 4, W>(elements, writer)
        }
        #[cfg(not(feature = "simd"))]
        {
            for element in elements {
                writer.write_i32(*element)?;
            }
            Ok(())
        }
    }
    fn read_payload<'de, R: Read<'de>>(len: usize, reader: &mut R) -> Result<Vec<i32>> {
        read_be(len, reader, <i32>::from_be_bytes)
    }
}

/// The unsigned spelling of the element, sharing the same bits.
impl<T: AsRef<[u32]> + ?Sized> IntArray<u32> for T {
    fn write_payload<W: Write + ?Sized>(&self, writer: &mut W) -> Result<()> {
        let elements: &[u32] = self.as_ref();
        writer.write_len(elements.len())?;
        #[cfg(feature = "simd")]
        {
            crate::simd::write_be::<u32, 4, W>(elements, writer)
        }
        #[cfg(not(feature = "simd"))]
        {
            for element in elements {
                writer.write_i32((*element).cast_signed())?;
            }
            Ok(())
        }
    }
    fn read_payload<'de, R: Read<'de>>(len: usize, reader: &mut R) -> Result<Vec<u32>> {
        read_be(len, reader, <u32>::from_be_bytes)
    }
}

/// The element kept as the four big-endian bytes NBT stores.
impl<T: AsRef<[I32Be]> + ?Sized> IntArray<I32Be> for T {
    fn write_payload<W: Write + ?Sized>(&self, writer: &mut W) -> Result<()> {
        let elements: &[I32Be] = self.as_ref();
        writer.write_len(elements.len())?;
        writer.write_bytes(i32be_as_bytes(elements))
    }
    fn read_payload<'de, R: Read<'de>>(len: usize, reader: &mut R) -> Result<Vec<I32Be>> {
        Ok(reader.read_int_array(len)?.into_owned())
    }
}

/// The unsigned spelling of the element, sharing the same bits.
impl<T: AsRef<[U32Be]> + ?Sized> IntArray<U32Be> for T {
    fn write_payload<W: Write + ?Sized>(&self, writer: &mut W) -> Result<()> {
        let elements: &[U32Be] = self.as_ref();
        writer.write_len(elements.len())?;
        writer.write_bytes(i32be_as_bytes(as_i32be(elements)))
    }
    fn read_payload<'de, R: Read<'de>>(len: usize, reader: &mut R) -> Result<Vec<U32Be>> {
        let array = reader.read_int_array(len)?;
        Ok(as_u32be(&array).to_vec())
    }
}

/// An NBT long array, holding `i64`, `u64`, [`I64Be`] or [`U64Be`] elements.
///
/// Implemented for every `AsRef<[E]>` with one of those spellings, so a
/// `Vec<u64>`, an `[I64Be; N]` and a borrowed `&[I64Be]` all write and read
/// a `TAG_Long_Array`. A derived field with `#[nbt(array = "long")]` and
/// [`Write::write_long_array`] are the usual ways in.
pub trait LongArray<E>: AsRef<[E]> {
    /// Writes the payload: the `i32` length, then the elements.
    fn write_payload<W: Write + ?Sized>(&self, writer: &mut W) -> Result<()>;

    /// Reads the payload, `len` elements.
    fn read_payload<'de, R: Read<'de>>(len: usize, reader: &mut R) -> Result<Vec<E>>;

    /// Writes a compound entry: the tag, the name, then the payload.
    ///
    /// The name is taken as its exact modified UTF-8 bytes, which is what
    /// the derive macros encode a field's name to while they expand; nothing
    /// is converted here.
    fn write_entry<W: Write>(&self, name: &Cesu8, writer: &mut W) -> Result<()> {
        writer.write_tag(TAG_LONG_ARRAY)?;
        writer.write_name(name)?;
        self.write_payload(writer)
    }

    /// Reads an array, refusing a tag other than [`TAG_LONG_ARRAY`].
    fn read<'de, R: Read<'de>>(tag: u8, reader: &mut R) -> Result<Vec<E>> {
        if tag != TAG_LONG_ARRAY {
            return Err(Error::invalid_tag(tag));
        }
        let len = reader.read_len()?;
        Self::read_payload(len, reader)
    }
}

impl<T: AsRef<[i64]> + ?Sized> LongArray<i64> for T {
    fn write_payload<W: Write + ?Sized>(&self, writer: &mut W) -> Result<()> {
        let elements: &[i64] = self.as_ref();
        writer.write_len(elements.len())?;
        #[cfg(feature = "simd")]
        {
            crate::simd::write_be::<i64, 8, W>(elements, writer)
        }
        #[cfg(not(feature = "simd"))]
        {
            for element in elements {
                writer.write_i64(*element)?;
            }
            Ok(())
        }
    }
    fn read_payload<'de, R: Read<'de>>(len: usize, reader: &mut R) -> Result<Vec<i64>> {
        read_be(len, reader, <i64>::from_be_bytes)
    }
}

/// The unsigned spelling of the element, sharing the same bits.
impl<T: AsRef<[u64]> + ?Sized> LongArray<u64> for T {
    fn write_payload<W: Write + ?Sized>(&self, writer: &mut W) -> Result<()> {
        let elements: &[u64] = self.as_ref();
        writer.write_len(elements.len())?;
        #[cfg(feature = "simd")]
        {
            crate::simd::write_be::<u64, 8, W>(elements, writer)
        }
        #[cfg(not(feature = "simd"))]
        {
            for element in elements {
                writer.write_i64((*element).cast_signed())?;
            }
            Ok(())
        }
    }
    fn read_payload<'de, R: Read<'de>>(len: usize, reader: &mut R) -> Result<Vec<u64>> {
        read_be(len, reader, <u64>::from_be_bytes)
    }
}

/// The element kept as the eight big-endian bytes NBT stores.
impl<T: AsRef<[I64Be]> + ?Sized> LongArray<I64Be> for T {
    fn write_payload<W: Write + ?Sized>(&self, writer: &mut W) -> Result<()> {
        let elements: &[I64Be] = self.as_ref();
        writer.write_len(elements.len())?;
        writer.write_bytes(i64be_as_bytes(elements))
    }
    fn read_payload<'de, R: Read<'de>>(len: usize, reader: &mut R) -> Result<Vec<I64Be>> {
        Ok(reader.read_long_array(len)?.into_owned())
    }
}

/// The unsigned spelling of the element, sharing the same bits.
impl<T: AsRef<[U64Be]> + ?Sized> LongArray<U64Be> for T {
    fn write_payload<W: Write + ?Sized>(&self, writer: &mut W) -> Result<()> {
        let elements: &[U64Be] = self.as_ref();
        writer.write_len(elements.len())?;
        writer.write_bytes(i64be_as_bytes(as_i64be(elements)))
    }
    fn read_payload<'de, R: Read<'de>>(len: usize, reader: &mut R) -> Result<Vec<U64Be>> {
        let array = reader.read_long_array(len)?;
        Ok(as_u64be(&array).to_vec())
    }
}
