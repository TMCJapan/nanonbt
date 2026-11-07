#[cfg(feature = "alloc")]
use alloc::borrow::Cow;
#[cfg(feature = "alloc")]
use alloc::vec::Vec;

use crate::codec::{read_count, read_list_header};
use crate::route::{ByteArray, IntArray, LongArray};
use crate::wire::{F32Be, F64Be, I16Be, I32Be, I64Be, Wire};
use crate::{Error, ReadRef, Result, Tag};

/// Decodes a value that borrows from the input buffer.
///
/// This is the zero-copy counterpart of
/// [`DeserializeNbt`](crate::DeserializeNbt): the reader must implement
/// [`ReadRef`], which guarantees that the decoded value can point into the
/// backing buffer. The `#[derive(DeserializeNbt)]` macro emits an
/// implementation of this trait instead of
/// [`DeserializeNbt`](crate::DeserializeNbt) when a struct
/// has borrowed fields such as `&'de [u8]` or `&'de [I32Be]`.
///
/// Use [`from_slice`](crate::from_slice) to decode a complete document this
/// way.
#[diagnostic::on_unimplemented(
    message = "`{Self}` cannot be decoded by borrowing from the NBT input"
)]
pub trait DeserializeNbtRef<'de>: Sized {
    /// The NBT tag this type expects at the root.
    const TAG: Tag;

    /// Reads the payload of [`Self::TAG`] from `reader`.
    fn deserialize_nbt_ref<R: ReadRef<'de> + ?Sized>(reader: &mut R) -> Result<Self>;
}

/// Decodes a borrowed sequence from an NBT array route such as [`IntArray`].
///
/// Implemented for `&'de [u8]`, `&'de [I32Be]`, `&'de [I64Be]` (and `i8`)
/// plus the corresponding `Cow<'de, [_]>` forms, which decode as
/// `Cow::Borrowed`. Array elements are never byte-swapped, which is why the
/// borrowed element type is the wire type rather than a native integer.
pub trait DeserializeNbtArrayRef<'de, K>: Sized {
    /// Reads the array payload (length prefix + elements) from `reader`.
    fn deserialize_nbt_array_ref<R: ReadRef<'de> + ?Sized>(reader: &mut R) -> Result<Self>;
}

fn read_array_borrowed<'de, R, P>(reader: &mut R) -> Result<&'de [P]>
where
    R: ReadRef<'de> + ?Sized,
    P: Wire,
{
    let len = read_count(reader)?;
    borrow_elements(reader, len)
}

/// Borrows `len` back-to-back wire elements from `reader`.
fn borrow_elements<'de, R, P>(reader: &mut R, len: usize) -> Result<&'de [P]>
where
    R: ReadRef<'de> + ?Sized,
    P: Wire,
{
    let byte_len = len.checked_mul(P::SIZE).ok_or(Error::InvalidLength)?;
    let bytes = reader.read_borrowed(byte_len)?;
    P::cast_from_bytes(bytes)
}

macro_rules! impl_borrowed_array {
    ($route:ty, $elem:ty) => {
        impl<'de> DeserializeNbtArrayRef<'de, $route> for &'de [$elem] {
            #[inline]
            fn deserialize_nbt_array_ref<R: ReadRef<'de> + ?Sized>(
                reader: &mut R,
            ) -> Result<Self> {
                read_array_borrowed::<R, $elem>(reader)
            }
        }

        #[cfg(feature = "alloc")]
        impl<'de> DeserializeNbtArrayRef<'de, $route> for Cow<'de, [$elem]> {
            #[inline]
            fn deserialize_nbt_array_ref<R: ReadRef<'de> + ?Sized>(
                reader: &mut R,
            ) -> Result<Self> {
                let slice = <&'de [$elem] as DeserializeNbtArrayRef<'de, $route>>::deserialize_nbt_array_ref(reader)?;
                Ok(Cow::Borrowed(slice))
            }
        }
    };
}

impl_borrowed_array!(ByteArray, u8);
impl_borrowed_array!(ByteArray, i8);
impl_borrowed_array!(IntArray, I32Be);
impl_borrowed_array!(LongArray, I64Be);

/// Implements [`DeserializeNbtRef`] for a fixed-width `TAG_List` element.
///
/// List elements carry no per-element tag, so after checking the element tag
/// the payload borrows like an array. Any fixed-width [`Wire`] element can
/// use this path. `u8`, `i8`, `I32Be` and `I64Be` also have array routes; the
/// derive selects those instead when `#[nbt(array = ...)]` is present.
macro_rules! impl_borrowed_list {
    ($tag:ident, $elem:ty) => {
        impl<'de> DeserializeNbtRef<'de> for &'de [$elem] {
            const TAG: Tag = Tag::List;

            #[inline]
            fn deserialize_nbt_ref<R: ReadRef<'de> + ?Sized>(reader: &mut R) -> Result<Self> {
                let len = read_list_header(reader, Tag::$tag)?;
                borrow_elements(reader, len)
            }
        }

        #[cfg(feature = "alloc")]
        impl<'de> DeserializeNbtRef<'de> for Cow<'de, [$elem]> {
            const TAG: Tag = Tag::List;

            #[inline]
            fn deserialize_nbt_ref<R: ReadRef<'de> + ?Sized>(reader: &mut R) -> Result<Self> {
                <&'de [$elem]>::deserialize_nbt_ref(reader).map(Cow::Borrowed)
            }
        }
    };
}

impl_borrowed_list!(Byte, u8);
impl_borrowed_list!(Byte, i8);
impl_borrowed_list!(Short, I16Be);
impl_borrowed_list!(Int, I32Be);
impl_borrowed_list!(Long, I64Be);
impl_borrowed_list!(Float, F32Be);
impl_borrowed_list!(Double, F64Be);

impl<'de> DeserializeNbtRef<'de> for &'de nanocesu8::Cesu8 {
    const TAG: Tag = Tag::String;

    #[inline]
    fn deserialize_nbt_ref<R: ReadRef<'de> + ?Sized>(reader: &mut R) -> Result<Self> {
        let len = reader.read_u16_be()? as usize;
        let bytes = reader.read_borrowed(len)?;
        nanocesu8::Cesu8::new(bytes).map_err(|_| Error::InvalidUtf8)
    }
}

#[cfg(feature = "alloc")]
impl<'de> DeserializeNbtRef<'de> for Cow<'de, nanocesu8::Cesu8> {
    const TAG: Tag = Tag::String;

    #[inline]
    fn deserialize_nbt_ref<R: ReadRef<'de> + ?Sized>(reader: &mut R) -> Result<Self> {
        let len = reader.read_u16_be()? as usize;
        let bytes = reader.read_borrowed(len)?;
        let cesu8 = nanocesu8::Cesu8::new(bytes).map_err(|_| Error::InvalidUtf8)?;
        Ok(Cow::Borrowed(cesu8))
    }
}

impl<'de> DeserializeNbtRef<'de> for &'de str {
    const TAG: Tag = Tag::String;

    #[inline]
    fn deserialize_nbt_ref<R: ReadRef<'de> + ?Sized>(reader: &mut R) -> Result<Self> {
        let len = reader.read_u16_be()? as usize;
        let bytes = reader.read_borrowed(len)?;
        let cesu8 = nanocesu8::Cesu8::new(bytes).map_err(|_| Error::InvalidUtf8)?;
        cesu8.try_decode().ok_or(Error::InvalidUtf8)
    }
}

/// Zero-copy variant of [`DeserializeNbtSelf`](crate::DeserializeNbtSelf): the
/// bytes handed to it point directly into the input buffer.
pub trait DeserializeNbtSelfRef<'de>: Sized {
    /// Builds `Self` from the compound's raw payload bytes.
    fn deserialize_nbt_self_ref(bytes: &'de [u8]) -> Result<Self>;
}

impl<'de> DeserializeNbtSelfRef<'de> for &'de [u8] {
    #[inline]
    fn deserialize_nbt_self_ref(bytes: &'de [u8]) -> Result<Self> {
        Ok(bytes)
    }
}

#[cfg(feature = "alloc")]
impl<'de> DeserializeNbtSelfRef<'de> for alloc::borrow::Cow<'de, [u8]> {
    #[inline]
    fn deserialize_nbt_self_ref(bytes: &'de [u8]) -> Result<Self> {
        Ok(alloc::borrow::Cow::Borrowed(bytes))
    }
}

#[cfg(feature = "alloc")]
impl<'de, T: DeserializeNbtRef<'de>> DeserializeNbtRef<'de> for Vec<T> {
    const TAG: Tag = Tag::List;

    fn deserialize_nbt_ref<R: ReadRef<'de> + ?Sized>(reader: &mut R) -> Result<Self> {
        let len = read_list_header(reader, T::TAG)?;
        if len > reader.remaining() {
            return Err(Error::UnexpectedEof);
        }
        let mut out = Self::new();
        out.try_reserve(len).map_err(|_| Error::InvalidLength)?;
        for _ in 0..len {
            out.push(T::deserialize_nbt_ref(reader)?);
        }
        Ok(out)
    }
}
