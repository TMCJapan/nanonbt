#[cfg(feature = "alloc")]
use alloc::borrow::Cow;

use crate::codec::read_count;
use crate::route::{ByteArray, IntArray, LongArray};
use crate::wire::{I32Be, I64Be, Wire};
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
