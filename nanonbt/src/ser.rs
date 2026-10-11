#[cfg(feature = "alloc")]
use alloc::borrow::{Cow, ToOwned};
#[cfg(feature = "alloc")]
use alloc::string::String;
#[cfg(feature = "alloc")]
use alloc::vec::Vec;

use crate::codec::{write_count, write_string};
use crate::route::{ArrayRoute, ByteArray, IntArray, LongArray};
use crate::wire::{I32Be, I64Be, Wire};
use crate::{Result, Tag, Write};

/// Serializes a value as an NBT tag.
///
/// Implementations write only the *payload* of [`Self::TAG`]; the tag byte and
/// compound field names are written by the caller. For compounds the payload
/// is the list of fields terminated by `TAG_End`. A complete document can be
/// produced with [`to_bytes`](crate::to_bytes).
pub trait SerializeNbt {
    /// The NBT tag this value represents.
    const TAG: Tag;

    /// Writes the payload of [`Self::TAG`] to `writer`.
    fn serialize_nbt<W: Write + ?Sized>(&self, writer: &mut W) -> Result<()>;
}

/// Serializes a sequence using an NBT array route such as [`IntArray`].
///
/// `Vec<i32>`, `&[i32]`, `[u8]` and the wire types all implement this trait
/// for the routes they can represent. The `#[nbt(array = ...)]` attribute
/// makes the derive call it instead of [`SerializeNbt`].
pub trait SerializeNbtArray<K: ArrayRoute> {
    /// Writes the array payload (length prefix + elements) to `writer`.
    fn serialize_nbt_array<W: Write + ?Sized>(&self, writer: &mut W) -> Result<()>;
}

macro_rules! impl_serialize_primitive {
    ($ty:ty, $tag:ident, $write:ident) => {
        impl SerializeNbt for $ty {
            const TAG: Tag = Tag::$tag;

            #[inline]
            fn serialize_nbt<W: Write + ?Sized>(&self, writer: &mut W) -> Result<()> {
                writer.$write(*self);
                Ok(())
            }
        }
    };
}

impl_serialize_primitive!(i16, Short, write_i16_be);
impl_serialize_primitive!(i32, Int, write_i32_be);
impl_serialize_primitive!(i64, Long, write_i64_be);
impl_serialize_primitive!(f32, Float, write_f32_be);
impl_serialize_primitive!(f64, Double, write_f64_be);

impl SerializeNbt for i8 {
    const TAG: Tag = Tag::Byte;

    #[inline]
    fn serialize_nbt<W: Write + ?Sized>(&self, writer: &mut W) -> Result<()> {
        writer.write_u8(self.cast_unsigned());
        Ok(())
    }
}

impl SerializeNbt for u8 {
    const TAG: Tag = Tag::Byte;

    #[inline]
    fn serialize_nbt<W: Write + ?Sized>(&self, writer: &mut W) -> Result<()> {
        writer.write_u8(*self);
        Ok(())
    }
}

impl SerializeNbt for bool {
    const TAG: Tag = Tag::Byte;

    #[inline]
    fn serialize_nbt<W: Write + ?Sized>(&self, writer: &mut W) -> Result<()> {
        writer.write_u8(u8::from(*self));
        Ok(())
    }
}

impl SerializeNbt for str {
    const TAG: Tag = Tag::String;

    #[inline]
    fn serialize_nbt<W: Write + ?Sized>(&self, writer: &mut W) -> Result<()> {
        write_string(writer, self)
    }
}

#[cfg(feature = "alloc")]
impl SerializeNbt for String {
    const TAG: Tag = Tag::String;

    #[inline]
    fn serialize_nbt<W: Write + ?Sized>(&self, writer: &mut W) -> Result<()> {
        write_string(writer, self)
    }
}

impl<T: SerializeNbt + ?Sized> SerializeNbt for &T {
    const TAG: Tag = T::TAG;

    #[inline]
    fn serialize_nbt<W: Write + ?Sized>(&self, writer: &mut W) -> Result<()> {
        (**self).serialize_nbt(writer)
    }
}

#[cfg(feature = "alloc")]
impl<T: SerializeNbt + ToOwned + ?Sized> SerializeNbt for Cow<'_, T> {
    const TAG: Tag = T::TAG;

    #[inline]
    fn serialize_nbt<W: Write + ?Sized>(&self, writer: &mut W) -> Result<()> {
        (**self).serialize_nbt(writer)
    }
}

/// `Vec<T>` is a `TAG_List<T>` by default.
///
/// This includes `Vec<u8>`: to get a `TAG_Byte_Array`, annotate the field with
/// `#[nbt(array = byte)]`. Empty lists are written with the `End` element
/// tag, which is the conventional representation.
#[cfg(feature = "alloc")]
impl<T: SerializeNbt> SerializeNbt for Vec<T> {
    const TAG: Tag = Tag::List;

    fn serialize_nbt<W: Write + ?Sized>(&self, writer: &mut W) -> Result<()> {
        if self.is_empty() {
            writer.write_u8(Tag::End.as_u8());
            return write_count(writer, 0);
        }
        writer.write_u8(T::TAG.as_u8());
        write_count(writer, self.len())?;
        for value in self {
            value.serialize_nbt(writer)?;
        }
        Ok(())
    }
}

macro_rules! impl_wire_array_serialize {
    ($route:ty, $elem:ty) => {
        impl SerializeNbtArray<$route> for [$elem] {
            #[inline]
            fn serialize_nbt_array<W: Write + ?Sized>(&self, writer: &mut W) -> Result<()> {
                write_count(writer, self.len())?;
                writer.write_bytes(<$elem as Wire>::cast_to_bytes(self));
                Ok(())
            }
        }
    };
}

impl_wire_array_serialize!(ByteArray, u8);
impl_wire_array_serialize!(ByteArray, i8);
impl_wire_array_serialize!(IntArray, I32Be);
impl_wire_array_serialize!(LongArray, I64Be);

macro_rules! impl_native_array_serialize {
    ($route:ty, $elem:ty, $write:ident) => {
        impl SerializeNbtArray<$route> for [$elem] {
            fn serialize_nbt_array<W: Write + ?Sized>(&self, writer: &mut W) -> Result<()> {
                write_count(writer, self.len())?;
                for &value in self {
                    writer.$write(value);
                }
                Ok(())
            }
        }
    };
}

impl_native_array_serialize!(IntArray, i32, write_i32_be);
impl_native_array_serialize!(LongArray, i64, write_i64_be);

impl<K: ArrayRoute, T: SerializeNbtArray<K> + ?Sized> SerializeNbtArray<K> for &T {
    #[inline]
    fn serialize_nbt_array<W: Write + ?Sized>(&self, writer: &mut W) -> Result<()> {
        (**self).serialize_nbt_array(writer)
    }
}

#[cfg(feature = "alloc")]
impl<K: ArrayRoute, T> SerializeNbtArray<K> for Vec<T>
where
    [T]: SerializeNbtArray<K>,
{
    #[inline]
    fn serialize_nbt_array<W: Write + ?Sized>(&self, writer: &mut W) -> Result<()> {
        self.as_slice().serialize_nbt_array(writer)
    }
}

#[cfg(feature = "alloc")]
impl<K: ArrayRoute, T: Clone> SerializeNbtArray<K> for Cow<'_, [T]>
where
    [T]: SerializeNbtArray<K>,
{
    #[inline]
    fn serialize_nbt_array<W: Write + ?Sized>(&self, writer: &mut W) -> Result<()> {
        let slice: &[T] = self;
        slice.serialize_nbt_array(writer)
    }
}
