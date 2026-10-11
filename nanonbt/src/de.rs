#[cfg(feature = "alloc")]
use alloc::borrow::Cow;
#[cfg(feature = "alloc")]
use alloc::string::String;
#[cfg(feature = "alloc")]
use alloc::vec::Vec;

use crate::codec::read_count;
#[cfg(feature = "alloc")]
use crate::codec::read_string;
use crate::route::{ArrayRoute, ByteArray, IntArray, LongArray};
use crate::wire::{I32Be, I64Be, Wire};
use crate::{Error, Read, Result, Tag};

/// Decodes an owned value from the payload of an NBT tag.
///
/// Implementations read only the *payload* of [`Self::TAG`]; the tag byte and
/// compound field names are handled by the caller. A complete document can be
/// decoded with [`from_bytes`](crate::from_bytes).
///
/// Types that need to borrow from the input implement
/// [`DeserializeNbtRef`](crate::DeserializeNbtRef) instead, which additionally
/// requires a [`ReadRef`](crate::ReadRef) reader.
pub trait DeserializeNbt: Sized {
    /// The NBT tag this type expects.
    const TAG: Tag;

    /// Reads the payload of [`Self::TAG`] from `reader`.
    fn deserialize_nbt<R: Read + ?Sized>(reader: &mut R) -> Result<Self>;
}

/// Decodes an owned sequence from an NBT array route such as [`IntArray`].
///
/// The `#[nbt(array = ...)]` attribute makes the derive call this trait
/// instead of [`DeserializeNbt`].
pub trait DeserializeNbtArray<K: ArrayRoute>: Sized {
    /// Reads the array payload (length prefix + elements) from `reader`.
    fn deserialize_nbt_array<R: Read + ?Sized>(reader: &mut R) -> Result<Self>;
}

macro_rules! impl_owned_primitive {
    ($ty:ty, $tag:ident, $read:ident) => {
        impl DeserializeNbt for $ty {
            const TAG: Tag = Tag::$tag;

            #[inline]
            fn deserialize_nbt<R: Read + ?Sized>(reader: &mut R) -> Result<Self> {
                reader.$read()
            }
        }
    };
}

impl_owned_primitive!(i16, Short, read_i16_be);
impl_owned_primitive!(i32, Int, read_i32_be);
impl_owned_primitive!(i64, Long, read_i64_be);
impl_owned_primitive!(f32, Float, read_f32_be);
impl_owned_primitive!(f64, Double, read_f64_be);

impl DeserializeNbt for i8 {
    const TAG: Tag = Tag::Byte;

    #[inline]
    fn deserialize_nbt<R: Read + ?Sized>(reader: &mut R) -> Result<Self> {
        Ok(reader.read_u8()?.cast_signed())
    }
}

impl DeserializeNbt for u8 {
    const TAG: Tag = Tag::Byte;

    #[inline]
    fn deserialize_nbt<R: Read + ?Sized>(reader: &mut R) -> Result<Self> {
        reader.read_u8()
    }
}

impl DeserializeNbt for bool {
    const TAG: Tag = Tag::Byte;

    #[inline]
    fn deserialize_nbt<R: Read + ?Sized>(reader: &mut R) -> Result<Self> {
        Ok(reader.read_u8()? != 0)
    }
}

#[cfg(feature = "alloc")]
impl DeserializeNbt for String {
    const TAG: Tag = Tag::String;

    #[inline]
    fn deserialize_nbt<R: Read + ?Sized>(reader: &mut R) -> Result<Self> {
        read_string(reader)
    }
}

/// `Vec<T>` is a `TAG_List<T>` by default; see the [`SerializeNbt`]
/// implementation for details.
///
/// [`SerializeNbt`]: crate::SerializeNbt
#[cfg(feature = "alloc")]
impl<T: DeserializeNbt> DeserializeNbt for Vec<T> {
    const TAG: Tag = Tag::List;

    fn deserialize_nbt<R: Read + ?Sized>(reader: &mut R) -> Result<Self> {
        let element = Tag::from_u8(reader.read_u8()?)?;
        let len = read_count(reader)?;
        if element == Tag::End {
            return if len == 0 {
                Ok(Self::new())
            } else {
                Err(Error::InvalidData(
                    "non-empty list with the `End` element tag",
                ))
            };
        }
        if element != T::TAG {
            return Err(Error::InvalidTag {
                expected: T::TAG,
                actual: element.as_u8(),
            });
        }
        // Every list element consumes at least one byte, so a count that
        // exceeds the remaining input can never be satisfied. Rejecting it
        // here keeps the length prefix from reserving more than the input
        // could possibly contain. `element` is not `End` at this point.
        if len > reader.remaining() {
            return Err(Error::UnexpectedEof);
        }
        let mut out = Self::new();
        out.try_reserve(len).map_err(|_| Error::InvalidLength)?;
        for _ in 0..len {
            out.push(T::deserialize_nbt(reader)?);
        }
        Ok(out)
    }
}

#[cfg(feature = "alloc")]
fn reserve<T>(len: usize) -> Result<Vec<T>> {
    let mut out = Vec::new();
    out.try_reserve(len).map_err(|_| Error::InvalidLength)?;
    Ok(out)
}

/// Rejects an array length prefix that claims more bytes than the reader holds.
///
/// This runs before any allocation so a short document cannot request a huge
/// buffer for elements it does not contain.
fn check_array_len<R: Read + ?Sized>(reader: &R, len: usize, element_size: usize) -> Result<()> {
    let byte_len = len.checked_mul(element_size).ok_or(Error::InvalidLength)?;
    if byte_len > reader.remaining() {
        return Err(Error::UnexpectedEof);
    }
    Ok(())
}

#[cfg(feature = "alloc")]
impl DeserializeNbtArray<ByteArray> for Vec<u8> {
    fn deserialize_nbt_array<R: Read + ?Sized>(reader: &mut R) -> Result<Self> {
        let len = read_count(reader)?;
        check_array_len(reader, len, 1)?;
        let mut out = reserve(len)?;
        out.resize(len, 0);
        reader.read_exact(&mut out)?;
        Ok(out)
    }
}

macro_rules! impl_wire_array_decode {
    ($route:ty, $elem:ty) => {
        impl DeserializeNbtArray<$route> for Vec<$elem> {
            fn deserialize_nbt_array<R: Read + ?Sized>(reader: &mut R) -> Result<Self> {
                let len = read_count(reader)?;
                check_array_len(reader, len, <$elem as Wire>::SIZE)?;
                let mut out = reserve(len)?;
                for _ in 0..len {
                    out.push(<$elem as Wire>::read_from(reader)?);
                }
                Ok(out)
            }
        }

        impl<'a> DeserializeNbtArray<$route> for Cow<'a, [$elem]> {
            #[inline]
            fn deserialize_nbt_array<R: Read + ?Sized>(reader: &mut R) -> Result<Self> {
                Ok(Cow::Owned(
                    <Vec<$elem> as DeserializeNbtArray<$route>>::deserialize_nbt_array(reader)?,
                ))
            }
        }
    };
}

#[cfg(feature = "alloc")]
impl_wire_array_decode!(ByteArray, i8);
#[cfg(feature = "alloc")]
impl_wire_array_decode!(IntArray, I32Be);
#[cfg(feature = "alloc")]
impl_wire_array_decode!(LongArray, I64Be);

macro_rules! impl_native_array_decode {
    ($route:ty, $native:ty, $wire:ty) => {
        impl DeserializeNbtArray<$route> for Vec<$native> {
            fn deserialize_nbt_array<R: Read + ?Sized>(reader: &mut R) -> Result<Self> {
                let len = read_count(reader)?;
                check_array_len(reader, len, <$wire as Wire>::SIZE)?;
                let mut out = reserve(len)?;
                for _ in 0..len {
                    out.push(<$wire as Wire>::read_from(reader)?.get());
                }
                Ok(out)
            }
        }

        impl<'a> DeserializeNbtArray<$route> for Cow<'a, [$native]> {
            #[inline]
            fn deserialize_nbt_array<R: Read + ?Sized>(reader: &mut R) -> Result<Self> {
                Ok(Cow::Owned(
                    <Vec<$native> as DeserializeNbtArray<$route>>::deserialize_nbt_array(reader)?,
                ))
            }
        }
    };
}

#[cfg(feature = "alloc")]
impl_native_array_decode!(IntArray, i32, I32Be);
#[cfg(feature = "alloc")]
impl_native_array_decode!(LongArray, i64, I64Be);
