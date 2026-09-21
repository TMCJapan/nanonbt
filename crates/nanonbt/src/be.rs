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
use crate::{
    error::{Error, Result},
    read::{FromNBT, Read},
    tag::{
        TAG_BYTE, TAG_BYTE_ARRAY, TAG_DOUBLE, TAG_END, TAG_FLOAT, TAG_INT, TAG_INT_ARRAY, TAG_LIST,
        TAG_LONG, TAG_LONG_ARRAY, TAG_SHORT,
    },
    write::{ToNBT, Write},
};
use alloc::borrow::Cow;
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
pub(crate) const fn as_i8(bytes: &[u8]) -> &[i8] {
    unsafe { core::slice::from_raw_parts(bytes.as_ptr().cast(), bytes.len()) }
}

/// Reinterprets signed bytes as the bytes they are.
pub(crate) const fn as_u8(bytes: &[i8]) -> &[u8] {
    unsafe { core::slice::from_raw_parts(bytes.as_ptr().cast(), bytes.len()) }
}
/// A `TAG_Short` kept as the two big-endian bytes NBT stores.
///
/// [`U16Be::get`] reads them as a `u16`.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct U16Be([u8; 2]);
impl PartialOrd for U16Be {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for U16Be {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.get().cmp(&other.get())
    }
}
impl U16Be {
    /// The value as the bytes NBT stores.
    pub const fn new(value: u16) -> Self {
        Self(value.to_be_bytes())
    }
    /// The number the bytes encode.
    pub const fn get(self) -> u16 {
        <u16>::from_be_bytes(self.0)
    }
    /// The bytes as written, without decoding them.
    pub const fn as_bytes(&self) -> &[u8; 2] {
        &self.0
    }
    /// A value from its big-endian bytes.
    pub const fn from_bytes(bytes: [u8; 2]) -> Self {
        Self(bytes)
    }
    /// Borrows one big-endian value from its bytes.
    pub const fn from_bytes_ref(bytes: &[u8; 2]) -> &Self {
        unsafe { &*core::ptr::from_ref(bytes).cast::<Self>() }
    }
    /// Reinterprets a byte slice as big-endian values.
    ///
    /// `None` when the length is not a multiple of the element size.
    pub const fn slice_from_bytes(bytes: &[u8]) -> Option<&[Self]> {
        if !bytes.len().is_multiple_of(2) {
            return None;
        }
        Some(unsafe { core::slice::from_raw_parts(bytes.as_ptr().cast::<Self>(), bytes.len() / 2) })
    }
}
impl From<u16> for U16Be {
    fn from(value: u16) -> Self {
        Self::new(value)
    }
}
impl From<U16Be> for u16 {
    fn from(value: U16Be) -> Self {
        value.get()
    }
}
impl core::fmt::Debug for U16Be {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("U16Be").field(&self.get()).finish()
    }
}
impl ToNBT for U16Be {
    fn tag(&self) -> u8 {
        TAG_SHORT
    }
    fn write<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_bytes(self.as_bytes())
    }
}
impl<'de> FromNBT<'de> for U16Be {
    fn read<R: Read<'de>>(tag: u8, reader: &mut R) -> Result<Self> {
        if tag != TAG_SHORT {
            return Err(Error::invalid_tag(tag));
        }
        let bytes = reader.read_bytes(2)?;
        let bytes: [u8; 2] = bytes
            .as_ref()
            .try_into()
            .map_err(|_| Error::unexpected_eof())?;
        Ok(Self::from_bytes(bytes))
    }
}
impl<'de> FromNBT<'de> for &'de U16Be {
    fn read<R: Read<'de>>(tag: u8, reader: &mut R) -> Result<Self> {
        if tag != TAG_SHORT {
            return Err(Error::invalid_tag(tag));
        }
        match reader.read_bytes(2)? {
            Cow::Borrowed(bytes) => {
                let bytes: &[u8; 2] = bytes.try_into().map_err(|_| Error::unexpected_eof())?;
                Ok(U16Be::from_bytes_ref(bytes))
            }
            Cow::Owned(_) => Err(Error::borrowed_bytes()),
        }
    }
}
impl<'de> FromNBT<'de> for &'de [U16Be] {
    fn read<R: Read<'de>>(tag: u8, reader: &mut R) -> Result<Self> {
        let bytes = read_be_slice(reader, tag, 2, None, TAG_SHORT)?;
        match bytes {
            Cow::Borrowed(bytes) => {
                U16Be::slice_from_bytes(bytes).ok_or_else(Error::unexpected_eof)
            }
            Cow::Owned(_) => Err(Error::borrowed_bytes()),
        }
    }
}
/// A `TAG_Short` kept as the two big-endian bytes NBT stores.
///
/// [`I16Be::get`] reads them as an `i16`.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct I16Be([u8; 2]);
impl PartialOrd for I16Be {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for I16Be {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.get().cmp(&other.get())
    }
}
impl I16Be {
    /// The value as the bytes NBT stores.
    pub const fn new(value: i16) -> Self {
        Self(value.to_be_bytes())
    }
    /// The number the bytes encode.
    pub const fn get(self) -> i16 {
        <i16>::from_be_bytes(self.0)
    }
    /// The bytes as written, without decoding them.
    pub const fn as_bytes(&self) -> &[u8; 2] {
        &self.0
    }
    /// A value from its big-endian bytes.
    pub const fn from_bytes(bytes: [u8; 2]) -> Self {
        Self(bytes)
    }
    /// Borrows one big-endian value from its bytes.
    pub const fn from_bytes_ref(bytes: &[u8; 2]) -> &Self {
        unsafe { &*core::ptr::from_ref(bytes).cast::<Self>() }
    }
    /// Reinterprets a byte slice as big-endian values.
    ///
    /// `None` when the length is not a multiple of the element size.
    pub const fn slice_from_bytes(bytes: &[u8]) -> Option<&[Self]> {
        if !bytes.len().is_multiple_of(2) {
            return None;
        }
        Some(unsafe { core::slice::from_raw_parts(bytes.as_ptr().cast::<Self>(), bytes.len() / 2) })
    }
}
impl From<i16> for I16Be {
    fn from(value: i16) -> Self {
        Self::new(value)
    }
}
impl From<I16Be> for i16 {
    fn from(value: I16Be) -> Self {
        value.get()
    }
}
impl core::fmt::Debug for I16Be {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("I16Be").field(&self.get()).finish()
    }
}
impl ToNBT for I16Be {
    fn tag(&self) -> u8 {
        TAG_SHORT
    }
    fn write<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_bytes(self.as_bytes())
    }
}
impl<'de> FromNBT<'de> for I16Be {
    fn read<R: Read<'de>>(tag: u8, reader: &mut R) -> Result<Self> {
        if tag != TAG_SHORT {
            return Err(Error::invalid_tag(tag));
        }
        let bytes = reader.read_bytes(2)?;
        let bytes: [u8; 2] = bytes
            .as_ref()
            .try_into()
            .map_err(|_| Error::unexpected_eof())?;
        Ok(Self::from_bytes(bytes))
    }
}
impl<'de> FromNBT<'de> for &'de I16Be {
    fn read<R: Read<'de>>(tag: u8, reader: &mut R) -> Result<Self> {
        if tag != TAG_SHORT {
            return Err(Error::invalid_tag(tag));
        }
        match reader.read_bytes(2)? {
            Cow::Borrowed(bytes) => {
                let bytes: &[u8; 2] = bytes.try_into().map_err(|_| Error::unexpected_eof())?;
                Ok(I16Be::from_bytes_ref(bytes))
            }
            Cow::Owned(_) => Err(Error::borrowed_bytes()),
        }
    }
}
impl<'de> FromNBT<'de> for &'de [I16Be] {
    fn read<R: Read<'de>>(tag: u8, reader: &mut R) -> Result<Self> {
        let bytes = read_be_slice(reader, tag, 2, None, TAG_SHORT)?;
        match bytes {
            Cow::Borrowed(bytes) => {
                I16Be::slice_from_bytes(bytes).ok_or_else(Error::unexpected_eof)
            }
            Cow::Owned(_) => Err(Error::borrowed_bytes()),
        }
    }
}
/// A `TAG_Int` kept as the four big-endian bytes NBT stores.
///
/// [`U32Be::get`] reads them as a `u32`.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct U32Be([u8; 4]);
impl PartialOrd for U32Be {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for U32Be {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.get().cmp(&other.get())
    }
}
impl U32Be {
    /// The value as the bytes NBT stores.
    pub const fn new(value: u32) -> Self {
        Self(value.to_be_bytes())
    }
    /// The number the bytes encode.
    pub const fn get(self) -> u32 {
        <u32>::from_be_bytes(self.0)
    }
    /// The bytes as written, without decoding them.
    pub const fn as_bytes(&self) -> &[u8; 4] {
        &self.0
    }
    /// A value from its big-endian bytes.
    pub const fn from_bytes(bytes: [u8; 4]) -> Self {
        Self(bytes)
    }
    /// Borrows one big-endian value from its bytes.
    pub const fn from_bytes_ref(bytes: &[u8; 4]) -> &Self {
        unsafe { &*core::ptr::from_ref(bytes).cast::<Self>() }
    }
    /// Reinterprets a byte slice as big-endian values.
    ///
    /// `None` when the length is not a multiple of the element size.
    pub const fn slice_from_bytes(bytes: &[u8]) -> Option<&[Self]> {
        if !bytes.len().is_multiple_of(4) {
            return None;
        }
        Some(unsafe { core::slice::from_raw_parts(bytes.as_ptr().cast::<Self>(), bytes.len() / 4) })
    }
}
impl From<u32> for U32Be {
    fn from(value: u32) -> Self {
        Self::new(value)
    }
}
impl From<U32Be> for u32 {
    fn from(value: U32Be) -> Self {
        value.get()
    }
}
impl core::fmt::Debug for U32Be {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("U32Be").field(&self.get()).finish()
    }
}
impl ToNBT for U32Be {
    fn tag(&self) -> u8 {
        TAG_INT
    }
    fn write<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_bytes(self.as_bytes())
    }
}
impl<'de> FromNBT<'de> for U32Be {
    fn read<R: Read<'de>>(tag: u8, reader: &mut R) -> Result<Self> {
        if tag != TAG_INT {
            return Err(Error::invalid_tag(tag));
        }
        let bytes = reader.read_bytes(4)?;
        let bytes: [u8; 4] = bytes
            .as_ref()
            .try_into()
            .map_err(|_| Error::unexpected_eof())?;
        Ok(Self::from_bytes(bytes))
    }
}
impl<'de> FromNBT<'de> for &'de U32Be {
    fn read<R: Read<'de>>(tag: u8, reader: &mut R) -> Result<Self> {
        if tag != TAG_INT {
            return Err(Error::invalid_tag(tag));
        }
        match reader.read_bytes(4)? {
            Cow::Borrowed(bytes) => {
                let bytes: &[u8; 4] = bytes.try_into().map_err(|_| Error::unexpected_eof())?;
                Ok(U32Be::from_bytes_ref(bytes))
            }
            Cow::Owned(_) => Err(Error::borrowed_bytes()),
        }
    }
}
impl<'de> FromNBT<'de> for &'de [U32Be] {
    fn read<R: Read<'de>>(tag: u8, reader: &mut R) -> Result<Self> {
        let bytes = read_be_slice(reader, tag, 4, Some(TAG_INT_ARRAY), TAG_INT)?;
        match bytes {
            Cow::Borrowed(bytes) => {
                U32Be::slice_from_bytes(bytes).ok_or_else(Error::unexpected_eof)
            }
            Cow::Owned(_) => Err(Error::borrowed_bytes()),
        }
    }
}
/// A `TAG_Int` kept as the four big-endian bytes NBT stores.
///
/// [`I32Be::get`] reads them as an `i32`.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct I32Be([u8; 4]);
impl PartialOrd for I32Be {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for I32Be {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.get().cmp(&other.get())
    }
}
impl I32Be {
    /// The value as the bytes NBT stores.
    pub const fn new(value: i32) -> Self {
        Self(value.to_be_bytes())
    }
    /// The number the bytes encode.
    pub const fn get(self) -> i32 {
        <i32>::from_be_bytes(self.0)
    }
    /// The bytes as written, without decoding them.
    pub const fn as_bytes(&self) -> &[u8; 4] {
        &self.0
    }
    /// A value from its big-endian bytes.
    pub const fn from_bytes(bytes: [u8; 4]) -> Self {
        Self(bytes)
    }
    /// Borrows one big-endian value from its bytes.
    pub const fn from_bytes_ref(bytes: &[u8; 4]) -> &Self {
        unsafe { &*core::ptr::from_ref(bytes).cast::<Self>() }
    }
    /// Reinterprets a byte slice as big-endian values.
    ///
    /// `None` when the length is not a multiple of the element size.
    pub const fn slice_from_bytes(bytes: &[u8]) -> Option<&[Self]> {
        if !bytes.len().is_multiple_of(4) {
            return None;
        }
        Some(unsafe { core::slice::from_raw_parts(bytes.as_ptr().cast::<Self>(), bytes.len() / 4) })
    }
}
impl From<i32> for I32Be {
    fn from(value: i32) -> Self {
        Self::new(value)
    }
}
impl From<I32Be> for i32 {
    fn from(value: I32Be) -> Self {
        value.get()
    }
}
impl core::fmt::Debug for I32Be {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("I32Be").field(&self.get()).finish()
    }
}
impl ToNBT for I32Be {
    fn tag(&self) -> u8 {
        TAG_INT
    }
    fn write<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_bytes(self.as_bytes())
    }
}
impl<'de> FromNBT<'de> for I32Be {
    fn read<R: Read<'de>>(tag: u8, reader: &mut R) -> Result<Self> {
        if tag != TAG_INT {
            return Err(Error::invalid_tag(tag));
        }
        let bytes = reader.read_bytes(4)?;
        let bytes: [u8; 4] = bytes
            .as_ref()
            .try_into()
            .map_err(|_| Error::unexpected_eof())?;
        Ok(Self::from_bytes(bytes))
    }
}
impl<'de> FromNBT<'de> for &'de I32Be {
    fn read<R: Read<'de>>(tag: u8, reader: &mut R) -> Result<Self> {
        if tag != TAG_INT {
            return Err(Error::invalid_tag(tag));
        }
        match reader.read_bytes(4)? {
            Cow::Borrowed(bytes) => {
                let bytes: &[u8; 4] = bytes.try_into().map_err(|_| Error::unexpected_eof())?;
                Ok(I32Be::from_bytes_ref(bytes))
            }
            Cow::Owned(_) => Err(Error::borrowed_bytes()),
        }
    }
}
impl<'de> FromNBT<'de> for &'de [I32Be] {
    fn read<R: Read<'de>>(tag: u8, reader: &mut R) -> Result<Self> {
        let bytes = read_be_slice(reader, tag, 4, Some(TAG_INT_ARRAY), TAG_INT)?;
        match bytes {
            Cow::Borrowed(bytes) => {
                I32Be::slice_from_bytes(bytes).ok_or_else(Error::unexpected_eof)
            }
            Cow::Owned(_) => Err(Error::borrowed_bytes()),
        }
    }
}
/// A `TAG_Long` kept as the eight big-endian bytes NBT stores.
///
/// [`U64Be::get`] reads them as a `u64`.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct U64Be([u8; 8]);
impl PartialOrd for U64Be {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for U64Be {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.get().cmp(&other.get())
    }
}
impl U64Be {
    /// The value as the bytes NBT stores.
    pub const fn new(value: u64) -> Self {
        Self(value.to_be_bytes())
    }
    /// The number the bytes encode.
    pub const fn get(self) -> u64 {
        <u64>::from_be_bytes(self.0)
    }
    /// The bytes as written, without decoding them.
    pub const fn as_bytes(&self) -> &[u8; 8] {
        &self.0
    }
    /// A value from its big-endian bytes.
    pub const fn from_bytes(bytes: [u8; 8]) -> Self {
        Self(bytes)
    }
    /// Borrows one big-endian value from its bytes.
    pub const fn from_bytes_ref(bytes: &[u8; 8]) -> &Self {
        unsafe { &*core::ptr::from_ref(bytes).cast::<Self>() }
    }
    /// Reinterprets a byte slice as big-endian values.
    ///
    /// `None` when the length is not a multiple of the element size.
    pub const fn slice_from_bytes(bytes: &[u8]) -> Option<&[Self]> {
        if !bytes.len().is_multiple_of(8) {
            return None;
        }
        Some(unsafe { core::slice::from_raw_parts(bytes.as_ptr().cast::<Self>(), bytes.len() / 8) })
    }
}
impl From<u64> for U64Be {
    fn from(value: u64) -> Self {
        Self::new(value)
    }
}
impl From<U64Be> for u64 {
    fn from(value: U64Be) -> Self {
        value.get()
    }
}
impl core::fmt::Debug for U64Be {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("U64Be").field(&self.get()).finish()
    }
}
impl ToNBT for U64Be {
    fn tag(&self) -> u8 {
        TAG_LONG
    }
    fn write<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_bytes(self.as_bytes())
    }
}
impl<'de> FromNBT<'de> for U64Be {
    fn read<R: Read<'de>>(tag: u8, reader: &mut R) -> Result<Self> {
        if tag != TAG_LONG {
            return Err(Error::invalid_tag(tag));
        }
        let bytes = reader.read_bytes(8)?;
        let bytes: [u8; 8] = bytes
            .as_ref()
            .try_into()
            .map_err(|_| Error::unexpected_eof())?;
        Ok(Self::from_bytes(bytes))
    }
}
impl<'de> FromNBT<'de> for &'de U64Be {
    fn read<R: Read<'de>>(tag: u8, reader: &mut R) -> Result<Self> {
        if tag != TAG_LONG {
            return Err(Error::invalid_tag(tag));
        }
        match reader.read_bytes(8)? {
            Cow::Borrowed(bytes) => {
                let bytes: &[u8; 8] = bytes.try_into().map_err(|_| Error::unexpected_eof())?;
                Ok(U64Be::from_bytes_ref(bytes))
            }
            Cow::Owned(_) => Err(Error::borrowed_bytes()),
        }
    }
}
impl<'de> FromNBT<'de> for &'de [U64Be] {
    fn read<R: Read<'de>>(tag: u8, reader: &mut R) -> Result<Self> {
        let bytes = read_be_slice(reader, tag, 8, Some(TAG_LONG_ARRAY), TAG_LONG)?;
        match bytes {
            Cow::Borrowed(bytes) => {
                U64Be::slice_from_bytes(bytes).ok_or_else(Error::unexpected_eof)
            }
            Cow::Owned(_) => Err(Error::borrowed_bytes()),
        }
    }
}
/// A `TAG_Long` kept as the eight big-endian bytes NBT stores.
///
/// [`I64Be::get`] reads them as an `i64`.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct I64Be([u8; 8]);
impl PartialOrd for I64Be {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for I64Be {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.get().cmp(&other.get())
    }
}
impl I64Be {
    /// The value as the bytes NBT stores.
    pub const fn new(value: i64) -> Self {
        Self(value.to_be_bytes())
    }
    /// The number the bytes encode.
    pub const fn get(self) -> i64 {
        <i64>::from_be_bytes(self.0)
    }
    /// The bytes as written, without decoding them.
    pub const fn as_bytes(&self) -> &[u8; 8] {
        &self.0
    }
    /// A value from its big-endian bytes.
    pub const fn from_bytes(bytes: [u8; 8]) -> Self {
        Self(bytes)
    }
    /// Borrows one big-endian value from its bytes.
    pub const fn from_bytes_ref(bytes: &[u8; 8]) -> &Self {
        unsafe { &*core::ptr::from_ref(bytes).cast::<Self>() }
    }
    /// Reinterprets a byte slice as big-endian values.
    ///
    /// `None` when the length is not a multiple of the element size.
    pub const fn slice_from_bytes(bytes: &[u8]) -> Option<&[Self]> {
        if !bytes.len().is_multiple_of(8) {
            return None;
        }
        Some(unsafe { core::slice::from_raw_parts(bytes.as_ptr().cast::<Self>(), bytes.len() / 8) })
    }
}
impl From<i64> for I64Be {
    fn from(value: i64) -> Self {
        Self::new(value)
    }
}
impl From<I64Be> for i64 {
    fn from(value: I64Be) -> Self {
        value.get()
    }
}
impl core::fmt::Debug for I64Be {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("I64Be").field(&self.get()).finish()
    }
}
impl ToNBT for I64Be {
    fn tag(&self) -> u8 {
        TAG_LONG
    }
    fn write<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_bytes(self.as_bytes())
    }
}
impl<'de> FromNBT<'de> for I64Be {
    fn read<R: Read<'de>>(tag: u8, reader: &mut R) -> Result<Self> {
        if tag != TAG_LONG {
            return Err(Error::invalid_tag(tag));
        }
        let bytes = reader.read_bytes(8)?;
        let bytes: [u8; 8] = bytes
            .as_ref()
            .try_into()
            .map_err(|_| Error::unexpected_eof())?;
        Ok(Self::from_bytes(bytes))
    }
}
impl<'de> FromNBT<'de> for &'de I64Be {
    fn read<R: Read<'de>>(tag: u8, reader: &mut R) -> Result<Self> {
        if tag != TAG_LONG {
            return Err(Error::invalid_tag(tag));
        }
        match reader.read_bytes(8)? {
            Cow::Borrowed(bytes) => {
                let bytes: &[u8; 8] = bytes.try_into().map_err(|_| Error::unexpected_eof())?;
                Ok(I64Be::from_bytes_ref(bytes))
            }
            Cow::Owned(_) => Err(Error::borrowed_bytes()),
        }
    }
}
impl<'de> FromNBT<'de> for &'de [I64Be] {
    fn read<R: Read<'de>>(tag: u8, reader: &mut R) -> Result<Self> {
        let bytes = read_be_slice(reader, tag, 8, Some(TAG_LONG_ARRAY), TAG_LONG)?;
        match bytes {
            Cow::Borrowed(bytes) => {
                I64Be::slice_from_bytes(bytes).ok_or_else(Error::unexpected_eof)
            }
            Cow::Owned(_) => Err(Error::borrowed_bytes()),
        }
    }
}
/// A `TAG_Float` kept as the four big-endian bytes NBT stores.
///
/// [`F32Be::get`] reads them as an `f32`.
#[repr(transparent)]
#[derive(Clone, Copy, Default)]
pub struct F32Be([u8; 4]);
impl PartialEq for F32Be {
    #[allow(clippy::float_cmp)]
    fn eq(&self, other: &Self) -> bool {
        self.get() == other.get()
    }
}
impl PartialOrd for F32Be {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        self.get().partial_cmp(&other.get())
    }
}
impl F32Be {
    /// The value as the bytes NBT stores.
    pub const fn new(value: f32) -> Self {
        Self(value.to_be_bytes())
    }
    /// The number the bytes encode.
    pub const fn get(self) -> f32 {
        <f32>::from_be_bytes(self.0)
    }
    /// The bytes as written, without decoding them.
    pub const fn as_bytes(&self) -> &[u8; 4] {
        &self.0
    }
    /// A value from its big-endian bytes.
    pub const fn from_bytes(bytes: [u8; 4]) -> Self {
        Self(bytes)
    }
    /// Borrows one big-endian value from its bytes.
    pub const fn from_bytes_ref(bytes: &[u8; 4]) -> &Self {
        unsafe { &*core::ptr::from_ref(bytes).cast::<Self>() }
    }
    /// Reinterprets a byte slice as big-endian values.
    ///
    /// `None` when the length is not a multiple of the element size.
    pub const fn slice_from_bytes(bytes: &[u8]) -> Option<&[Self]> {
        if !bytes.len().is_multiple_of(4) {
            return None;
        }
        Some(unsafe { core::slice::from_raw_parts(bytes.as_ptr().cast::<Self>(), bytes.len() / 4) })
    }
}
impl From<f32> for F32Be {
    fn from(value: f32) -> Self {
        Self::new(value)
    }
}
impl From<F32Be> for f32 {
    fn from(value: F32Be) -> Self {
        value.get()
    }
}
impl core::fmt::Debug for F32Be {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("F32Be").field(&self.get()).finish()
    }
}
impl ToNBT for F32Be {
    fn tag(&self) -> u8 {
        TAG_FLOAT
    }
    fn write<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_bytes(self.as_bytes())
    }
}
impl<'de> FromNBT<'de> for F32Be {
    fn read<R: Read<'de>>(tag: u8, reader: &mut R) -> Result<Self> {
        if tag != TAG_FLOAT {
            return Err(Error::invalid_tag(tag));
        }
        let bytes = reader.read_bytes(4)?;
        let bytes: [u8; 4] = bytes
            .as_ref()
            .try_into()
            .map_err(|_| Error::unexpected_eof())?;
        Ok(Self::from_bytes(bytes))
    }
}
impl<'de> FromNBT<'de> for &'de F32Be {
    fn read<R: Read<'de>>(tag: u8, reader: &mut R) -> Result<Self> {
        if tag != TAG_FLOAT {
            return Err(Error::invalid_tag(tag));
        }
        match reader.read_bytes(4)? {
            Cow::Borrowed(bytes) => {
                let bytes: &[u8; 4] = bytes.try_into().map_err(|_| Error::unexpected_eof())?;
                Ok(F32Be::from_bytes_ref(bytes))
            }
            Cow::Owned(_) => Err(Error::borrowed_bytes()),
        }
    }
}
impl<'de> FromNBT<'de> for &'de [F32Be] {
    fn read<R: Read<'de>>(tag: u8, reader: &mut R) -> Result<Self> {
        let bytes = read_be_slice(reader, tag, 4, None, TAG_FLOAT)?;
        match bytes {
            Cow::Borrowed(bytes) => {
                F32Be::slice_from_bytes(bytes).ok_or_else(Error::unexpected_eof)
            }
            Cow::Owned(_) => Err(Error::borrowed_bytes()),
        }
    }
}
/// A `TAG_Double` kept as the eight big-endian bytes NBT stores.
///
/// [`F64Be::get`] reads them as an `f64`.
#[repr(transparent)]
#[derive(Clone, Copy, Default)]
pub struct F64Be([u8; 8]);
impl PartialEq for F64Be {
    #[allow(clippy::float_cmp)]
    fn eq(&self, other: &Self) -> bool {
        self.get() == other.get()
    }
}
impl PartialOrd for F64Be {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        self.get().partial_cmp(&other.get())
    }
}
impl F64Be {
    /// The value as the bytes NBT stores.
    pub const fn new(value: f64) -> Self {
        Self(value.to_be_bytes())
    }
    /// The number the bytes encode.
    pub const fn get(self) -> f64 {
        <f64>::from_be_bytes(self.0)
    }
    /// The bytes as written, without decoding them.
    pub const fn as_bytes(&self) -> &[u8; 8] {
        &self.0
    }
    /// A value from its big-endian bytes.
    pub const fn from_bytes(bytes: [u8; 8]) -> Self {
        Self(bytes)
    }
    /// Borrows one big-endian value from its bytes.
    pub const fn from_bytes_ref(bytes: &[u8; 8]) -> &Self {
        unsafe { &*core::ptr::from_ref(bytes).cast::<Self>() }
    }
    /// Reinterprets a byte slice as big-endian values.
    ///
    /// `None` when the length is not a multiple of the element size.
    pub const fn slice_from_bytes(bytes: &[u8]) -> Option<&[Self]> {
        if !bytes.len().is_multiple_of(8) {
            return None;
        }
        Some(unsafe { core::slice::from_raw_parts(bytes.as_ptr().cast::<Self>(), bytes.len() / 8) })
    }
}
impl From<f64> for F64Be {
    fn from(value: f64) -> Self {
        Self::new(value)
    }
}
impl From<F64Be> for f64 {
    fn from(value: F64Be) -> Self {
        value.get()
    }
}
impl core::fmt::Debug for F64Be {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("F64Be").field(&self.get()).finish()
    }
}
impl ToNBT for F64Be {
    fn tag(&self) -> u8 {
        TAG_DOUBLE
    }
    fn write<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_bytes(self.as_bytes())
    }
}
impl<'de> FromNBT<'de> for F64Be {
    fn read<R: Read<'de>>(tag: u8, reader: &mut R) -> Result<Self> {
        if tag != TAG_DOUBLE {
            return Err(Error::invalid_tag(tag));
        }
        let bytes = reader.read_bytes(8)?;
        let bytes: [u8; 8] = bytes
            .as_ref()
            .try_into()
            .map_err(|_| Error::unexpected_eof())?;
        Ok(Self::from_bytes(bytes))
    }
}
impl<'de> FromNBT<'de> for &'de F64Be {
    fn read<R: Read<'de>>(tag: u8, reader: &mut R) -> Result<Self> {
        if tag != TAG_DOUBLE {
            return Err(Error::invalid_tag(tag));
        }
        match reader.read_bytes(8)? {
            Cow::Borrowed(bytes) => {
                let bytes: &[u8; 8] = bytes.try_into().map_err(|_| Error::unexpected_eof())?;
                Ok(F64Be::from_bytes_ref(bytes))
            }
            Cow::Owned(_) => Err(Error::borrowed_bytes()),
        }
    }
}
impl<'de> FromNBT<'de> for &'de [F64Be] {
    fn read<R: Read<'de>>(tag: u8, reader: &mut R) -> Result<Self> {
        let bytes = read_be_slice(reader, tag, 8, None, TAG_DOUBLE)?;
        match bytes {
            Cow::Borrowed(bytes) => {
                F64Be::slice_from_bytes(bytes).ok_or_else(Error::unexpected_eof)
            }
            Cow::Owned(_) => Err(Error::borrowed_bytes()),
        }
    }
}
