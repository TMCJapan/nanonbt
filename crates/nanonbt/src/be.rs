//! Big-endian views of NBT numbers.
//!
//! NBT stores numbers big-endian. Each wrapper here keeps the bytes as
//! written, so a reference into the input is the number: `&'de U64Be` is one
//! `TAG_Long` and `&'de [U64Be]` a whole `TAG_Long_Array`, read without
//! copying or decoding. [`U64Be::get`] yields the value.
//!
//! The wrappers are `#[repr(transparent)]` over `[u8; N]`, so their alignment
//! is 1 and a value can start at any byte of the input. Bytes have no
//! endianness and so no wrapper: `&'de u8`, `&'de i8`, `&'de [u8]` and
//! `&'de [i8]` borrow a `TAG_Byte` or a `TAG_Byte_Array` as it is.
//!
//! Reading accepts an array's tag and a list of the same element, so a
//! `&'de [U64Be]` reads a `TAG_Long_Array` or a `TAG_List` of `TAG_Long`.
//! Writing a borrowed slice goes through the sequence implementations, which
//! write a list rather than an array; [`LongArray`](crate::LongArray),
//! [`IntArray`](crate::IntArray) and [`ByteArray`](crate::ByteArray) are the
//! types that write arrays.
//!
//! ```
//! use nanonbt::{FromNBT, LongArray, ToNBT, U64Be};
//!
//! #[derive(ToNBT)]
//! struct Source {
//!     data: LongArray,
//! }
//!
//! #[derive(FromNBT)]
//! struct Borrowed<'a> {
//!     data: &'a [U64Be],
//! }
//!
//! let bytes = nanonbt::to_bytes(&Source {
//!     data: LongArray::new(vec![1, 2]),
//! })
//! .unwrap();
//! let borrowed = nanonbt::from_bytes::<Borrowed<'_>>(&bytes).unwrap();
//! assert_eq!(borrowed.data[0].get(), 1);
//! assert_eq!(borrowed.data[1].get(), 2);
//! ```

use alloc::borrow::Cow;

use crate::{
    error::{Error, Result},
    read::{FromNBT, Read},
    tag::{
        TAG_BYTE, TAG_BYTE_ARRAY, TAG_DOUBLE, TAG_END, TAG_FLOAT, TAG_INT, TAG_INT_ARRAY, TAG_LIST,
        TAG_LONG, TAG_LONG_ARRAY, TAG_SHORT,
    },
    write::{ToNBT, Write},
};

/// `Some(tag)` for a type with an array tag, `None` for one without.
macro_rules! array_tag {
    () => {
        None
    };
    ($tag:ident) => {
        Some($tag)
    };
}

/// The implementations every big-endian wrapper shares.
macro_rules! be_impls {
    ($name:ident, $native:ty, $tag:ident, $size:literal $(, $array_tag:ident)?) => {
        impl $name {
            /// The value as the bytes NBT stores.
            pub const fn new(value: $native) -> Self {
                Self(value.to_be_bytes())
            }

            /// The number the bytes encode.
            pub const fn get(self) -> $native {
                <$native>::from_be_bytes(self.0)
            }

            /// The bytes as written, without decoding them.
            pub const fn as_bytes(&self) -> &[u8; $size] {
                &self.0
            }

            /// A value from its big-endian bytes.
            pub const fn from_bytes(bytes: [u8; $size]) -> Self {
                Self(bytes)
            }

            /// Borrows one big-endian value from its bytes.
            pub const fn from_bytes_ref(bytes: &[u8; $size]) -> &Self {
                // SAFETY: `$name` is `repr(transparent)` over `[u8; $size]`,
                // so the cast keeps the address, the size and the alignment
                // (1), and every bit pattern is a valid value.
                unsafe { &*core::ptr::from_ref(bytes).cast::<Self>() }
            }

            /// Reinterprets a byte slice as big-endian values.
            ///
            /// `None` when the length is not a multiple of the element size.
            pub const fn slice_from_bytes(bytes: &[u8]) -> Option<&[Self]> {
                if bytes.len() % $size != 0 {
                    return None;
                }
                // SAFETY: as in `from_bytes_ref`; the length was just checked
                // to be a whole number of elements.
                Some(unsafe {
                    core::slice::from_raw_parts(bytes.as_ptr().cast::<Self>(), bytes.len() / $size)
                })
            }
        }

        impl From<$native> for $name {
            fn from(value: $native) -> Self {
                Self::new(value)
            }
        }

        impl From<$name> for $native {
            fn from(value: $name) -> Self {
                value.get()
            }
        }

        impl core::fmt::Debug for $name {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                f.debug_tuple(stringify!($name)).field(&self.get()).finish()
            }
        }

        impl ToNBT for $name {
            fn tag(&self) -> u8 {
                $tag
            }

            fn write<W: Write>(&self, writer: &mut W) -> Result<()> {
                writer.write_bytes(self.as_bytes())
            }
        }

        impl<'de> FromNBT<'de> for $name {
            fn read<R: Read<'de>>(tag: u8, reader: &mut R) -> Result<Self> {
                if tag != $tag {
                    return Err(Error::invalid_tag(tag));
                }
                let bytes = reader.read_bytes($size)?;
                let bytes: [u8; $size] = bytes
                    .as_ref()
                    .try_into()
                    .map_err(|_| Error::unexpected_eof())?;
                Ok(Self::from_bytes(bytes))
            }
        }

        impl<'de> FromNBT<'de> for &'de $name {
            fn read<R: Read<'de>>(tag: u8, reader: &mut R) -> Result<Self> {
                if tag != $tag {
                    return Err(Error::invalid_tag(tag));
                }
                match reader.read_bytes($size)? {
                    Cow::Borrowed(bytes) => {
                        let bytes: &[u8; $size] =
                            bytes.try_into().map_err(|_| Error::unexpected_eof())?;
                        Ok($name::from_bytes_ref(bytes))
                    }
                    Cow::Owned(_) => Err(Error::borrowed_bytes()),
                }
            }
        }

        impl<'de> FromNBT<'de> for &'de [$name] {
            fn read<R: Read<'de>>(tag: u8, reader: &mut R) -> Result<Self> {
                let bytes = read_be_slice(reader, tag, $size, array_tag!($($array_tag)?), $tag)?;
                match bytes {
                    Cow::Borrowed(bytes) => {
                        $name::slice_from_bytes(bytes).ok_or_else(Error::unexpected_eof)
                    }
                    Cow::Owned(_) => Err(Error::borrowed_bytes()),
                }
            }
        }
    };
}

/// An integer kept as its big-endian bytes.
macro_rules! be_int {
    ($(#[$doc:meta])* $name:ident, $native:ty, $tag:ident, $size:literal $(, $array_tag:ident)?) => {
        $(#[$doc])*
        #[repr(transparent)]
        #[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
        pub struct $name([u8; $size]);

        impl PartialOrd for $name {
            fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
                Some(self.cmp(other))
            }
        }

        impl Ord for $name {
            fn cmp(&self, other: &Self) -> core::cmp::Ordering {
                self.get().cmp(&other.get())
            }
        }

        be_impls!($name, $native, $tag, $size $(, $array_tag)?);
    };
}

/// A float kept as its big-endian bytes.
macro_rules! be_float {
    ($(#[$doc:meta])* $name:ident, $native:ty, $tag:ident, $size:literal) => {
        $(#[$doc])*
        #[repr(transparent)]
        #[derive(Clone, Copy, Default)]
        pub struct $name([u8; $size]);

        impl PartialEq for $name {
            #[allow(clippy::float_cmp)] // float semantics, not bit equality
            fn eq(&self, other: &Self) -> bool {
                self.get() == other.get()
            }
        }

        impl PartialOrd for $name {
            fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
                self.get().partial_cmp(&other.get())
            }
        }

        be_impls!($name, $native, $tag, $size);
    };
}

/// The raw payload of an array, or of a list whose elements are
/// `element_tag`, for a slice of `size`-byte elements.
fn read_be_slice<'de, R: Read<'de>>(
    reader: &mut R,
    tag: u8,
    size: usize,
    array_tag: Option<u8>,
    element_tag: u8,
) -> Result<Cow<'de, [u8]>> {
    let len = if array_tag == Some(tag) {
        reader.read_len()?
    } else if tag == TAG_LIST {
        let (element, len) = reader.read_list_header()?;
        if element == TAG_END {
            if len != 0 {
                return Err(Error::list_of_end());
            }
            return reader.read_bytes(0);
        }
        if element != element_tag {
            return Err(Error::invalid_tag(element));
        }
        len
    } else {
        return Err(Error::invalid_tag(tag));
    };
    let n = len.checked_mul(size).ok_or_else(Error::array_too_large)?;
    reader.read_bytes(n)
}

/// A `TAG_Byte`, borrowed from the input.
impl<'de> FromNBT<'de> for &'de u8 {
    fn read<R: Read<'de>>(tag: u8, reader: &mut R) -> Result<Self> {
        if tag != TAG_BYTE {
            return Err(Error::invalid_tag(tag));
        }
        match reader.read_bytes(1)? {
            Cow::Borrowed(bytes) => bytes.first().ok_or_else(Error::unexpected_eof),
            Cow::Owned(_) => Err(Error::borrowed_bytes()),
        }
    }
}

/// A `TAG_Byte`, borrowed from the input.
impl<'de> FromNBT<'de> for &'de i8 {
    fn read<R: Read<'de>>(tag: u8, reader: &mut R) -> Result<Self> {
        if tag != TAG_BYTE {
            return Err(Error::invalid_tag(tag));
        }
        match reader.read_bytes(1)? {
            Cow::Borrowed(bytes) => as_i8(bytes).first().ok_or_else(Error::unexpected_eof),
            Cow::Owned(_) => Err(Error::borrowed_bytes()),
        }
    }
}

/// A `TAG_Byte_Array`, borrowed from the input.
impl<'de> FromNBT<'de> for &'de [u8] {
    fn read<R: Read<'de>>(tag: u8, reader: &mut R) -> Result<Self> {
        match read_be_slice(reader, tag, 1, Some(TAG_BYTE_ARRAY), TAG_BYTE)? {
            Cow::Borrowed(bytes) => Ok(bytes),
            Cow::Owned(_) => Err(Error::borrowed_bytes()),
        }
    }
}

/// A `TAG_Byte_Array`, borrowed from the input.
impl<'de> FromNBT<'de> for &'de [i8] {
    fn read<R: Read<'de>>(tag: u8, reader: &mut R) -> Result<Self> {
        match read_be_slice(reader, tag, 1, Some(TAG_BYTE_ARRAY), TAG_BYTE)? {
            Cow::Borrowed(bytes) => Ok(as_i8(bytes)),
            Cow::Owned(_) => Err(Error::borrowed_bytes()),
        }
    }
}

/// Reinterprets bytes as signed bytes.
const fn as_i8(bytes: &[u8]) -> &[i8] {
    // SAFETY: `i8` and `u8` have the same size and alignment, and every bit
    // pattern of one is a valid value of the other.
    unsafe { core::slice::from_raw_parts(bytes.as_ptr().cast(), bytes.len()) }
}

be_int! {
    /// A `TAG_Short` kept as the two big-endian bytes NBT stores.
    ///
    /// [`U16Be::get`] reads them as a `u16`.
    U16Be, u16, TAG_SHORT, 2
}

be_int! {
    /// A `TAG_Short` kept as the two big-endian bytes NBT stores.
    ///
    /// [`I16Be::get`] reads them as an `i16`.
    I16Be, i16, TAG_SHORT, 2
}

be_int! {
    /// A `TAG_Int` kept as the four big-endian bytes NBT stores.
    ///
    /// [`U32Be::get`] reads them as a `u32`.
    U32Be, u32, TAG_INT, 4, TAG_INT_ARRAY
}

be_int! {
    /// A `TAG_Int` kept as the four big-endian bytes NBT stores.
    ///
    /// [`I32Be::get`] reads them as an `i32`.
    I32Be, i32, TAG_INT, 4, TAG_INT_ARRAY
}

be_int! {
    /// A `TAG_Long` kept as the eight big-endian bytes NBT stores.
    ///
    /// [`U64Be::get`] reads them as a `u64`.
    U64Be, u64, TAG_LONG, 8, TAG_LONG_ARRAY
}

be_int! {
    /// A `TAG_Long` kept as the eight big-endian bytes NBT stores.
    ///
    /// [`I64Be::get`] reads them as an `i64`.
    I64Be, i64, TAG_LONG, 8, TAG_LONG_ARRAY
}

be_float! {
    /// A `TAG_Float` kept as the four big-endian bytes NBT stores.
    ///
    /// [`F32Be::get`] reads them as an `f32`.
    F32Be, f32, TAG_FLOAT, 4
}

be_float! {
    /// A `TAG_Double` kept as the eight big-endian bytes NBT stores.
    ///
    /// [`F64Be::get`] reads them as an `f64`.
    F64Be, f64, TAG_DOUBLE, 8
}
