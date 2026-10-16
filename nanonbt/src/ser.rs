#[cfg(feature = "alloc")]
use alloc::borrow::{Cow, ToOwned};
#[cfg(feature = "alloc")]
use alloc::string::String;
#[cfg(feature = "alloc")]
use alloc::vec::Vec;

use crate::codec::{write_count, write_string};
use crate::route::{ArrayRoute, ByteArray, IntArray, LongArray};
use crate::wire::{F32Be, F64Be, I16Be, I32Be, I64Be, Wire};
use crate::{Error, Result, Tag, Write};

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

    /// Writes the elements of a `TAG_List<Self>` after its header.
    ///
    /// `Vec<T>` and the other list writers call this instead of looping over
    /// [`serialize_nbt`](Self::serialize_nbt) themselves, so an element type
    /// can batch its writes. The native integer and float types override it
    /// to byte-swap a whole 64-byte chunk at a time, which is the same bulk
    /// path the `TAG_Int_Array` and `TAG_Long_Array` routes use. The default
    /// implementation writes one element at a time.
    #[inline]
    fn serialize_nbt_elements<W: Write + ?Sized>(values: &[Self], writer: &mut W) -> Result<()>
    where
        Self: Sized,
    {
        for value in values {
            value.serialize_nbt(writer)?;
        }
        Ok(())
    }
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

/// Writes native integers and floats big-endian a whole chunk at a time.
///
/// The chunk becomes one fixed-size `[Wire; CHUNK]` of 64 bytes — 32 `i16`,
/// 16 `i32`/`f32` or 8 `i64`/`f64` — which keeps the byte swap in safe code
/// (the conversion is the plain `to_be_bytes` spelling) and gives the
/// compiler a fixed-size loop to turn into vector byte swaps, one shuffle per
/// register instead of a `bswap` per element. Where the target has no cheap
/// vector swap, the bulk write still replaces one writer call per element
/// with one per chunk. The ragged tail is written element by element.
/// `tests/simd.rs` pins the vectorized outcome down on the LLVM IR.
macro_rules! write_be_chunked {
    ($writer:expr, $values:expr, $native:ty, $wire:ty, $write:ident) => {{
        /// Elements per bulk write: 32 `i16`, 16 `i32`/`f32` or 8
        /// `i64`/`f64`, so a chunk is 64 bytes — one 512-bit register where
        /// the target has one, and a multiple of every smaller vector width.
        const CHUNK: usize = 64 / core::mem::size_of::<$native>();
        let (bulk, tail) = $values.as_chunks::<CHUNK>();
        for chunk in bulk {
            $writer.write_bytes(<$wire as Wire>::cast_to_bytes(&chunk.map(<$wire>::new)));
        }
        for &value in tail {
            $writer.$write(value);
        }
    }};
}

macro_rules! impl_serialize_primitive {
    ($ty:ty, $tag:ident, $write:ident, $wire:ty) => {
        impl SerializeNbt for $ty {
            const TAG: Tag = Tag::$tag;

            #[inline]
            fn serialize_nbt<W: Write + ?Sized>(&self, writer: &mut W) -> Result<()> {
                writer.$write(*self);
                Ok(())
            }

            #[inline]
            fn serialize_nbt_elements<W: Write + ?Sized>(
                values: &[Self],
                writer: &mut W,
            ) -> Result<()> {
                write_be_chunked!(writer, values, $ty, $wire, $write);
                Ok(())
            }
        }
    };
}

impl_serialize_primitive!(i16, Short, write_i16_be, I16Be);
impl_serialize_primitive!(i32, Int, write_i32_be, I32Be);
impl_serialize_primitive!(i64, Long, write_i64_be, I64Be);
impl_serialize_primitive!(f32, Float, write_f32_be, F32Be);
impl_serialize_primitive!(f64, Double, write_f64_be, F64Be);

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

impl SerializeNbt for nanocesu8::Cesu8 {
    const TAG: Tag = Tag::String;

    #[inline]
    fn serialize_nbt<W: Write + ?Sized>(&self, writer: &mut W) -> Result<()> {
        let len = u16::try_from(self.len()).map_err(|_| Error::InvalidLength)?;
        writer.write_u16_be(len);
        writer.write_bytes(self.as_bytes());
        Ok(())
    }
}

#[cfg(feature = "alloc")]
impl SerializeNbt for nanocesu8::Cesu8Buf {
    const TAG: Tag = Tag::String;

    #[inline]
    fn serialize_nbt<W: Write + ?Sized>(&self, writer: &mut W) -> Result<()> {
        let len = u16::try_from(self.len()).map_err(|_| Error::InvalidLength)?;
        writer.write_u16_be(len);
        writer.write_bytes(self.as_bytes());
        Ok(())
    }
}

#[cfg(feature = "alloc")]
impl<T: SerializeNbt + ?Sized> SerializeNbt for alloc::boxed::Box<T> {
    const TAG: Tag = T::TAG;

    #[inline]
    fn serialize_nbt<W: Write + ?Sized>(&self, writer: &mut W) -> Result<()> {
        (**self).serialize_nbt(writer)
    }
}

#[cfg(feature = "alloc")]
impl<T: SerializeNbt + ?Sized> SerializeNbt for alloc::sync::Arc<T> {
    const TAG: Tag = T::TAG;

    #[inline]
    fn serialize_nbt<W: Write + ?Sized>(&self, writer: &mut W) -> Result<()> {
        (**self).serialize_nbt(writer)
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
        T::serialize_nbt_elements(self, writer)?;
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

/// Serializes native integer arrays through the shared chunked byte swap.
macro_rules! impl_native_array_serialize {
    ($route:ty, $native:ty, $wire:ty, $write:ident) => {
        impl SerializeNbtArray<$route> for [$native] {
            #[inline]
            fn serialize_nbt_array<W: Write + ?Sized>(&self, writer: &mut W) -> Result<()> {
                write_count(writer, self.len())?;
                write_be_chunked!(writer, self, $native, $wire, $write);
                Ok(())
            }
        }
    };
}

impl_native_array_serialize!(IntArray, i32, I32Be, write_i32_be);
impl_native_array_serialize!(LongArray, i64, I64Be, write_i64_be);

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
