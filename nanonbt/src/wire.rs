use core::fmt;

use crate::{Error, Read, Result, Write};

/// A type whose values are stored in NBT as raw big-endian bytes.
///
/// [`I32Be`] and [`I64Be`] are the reason this trait exists: they let an
/// integer array payload be reinterpreted as a slice without any per-element
/// work, which is what makes zero-copy decoding possible. Native integers such
/// as `i32` deliberately do **not** implement this trait, because their byte
/// representation is endian-dependent.
///
/// # Safety
///
/// Implementors must guarantee that:
///
/// * `Self` has the layout of `[u8; SIZE]`: no padding, and
///   `align_of::<Self>() == 1`;
/// * every bit pattern of that size is a valid value of `Self`.
///
/// This makes it sound to reinterpret slices of bytes as slices of `Self` and
/// vice versa. Implementations must use `unsafe impl` to acknowledge this.
pub unsafe trait Wire: Copy {
    /// Size of the wire representation in bytes.
    const SIZE: usize;

    /// Reads a single element.
    fn read_from<R: Read + ?Sized>(reader: &mut R) -> Result<Self>;

    /// Writes a single element.
    fn write_to<W: Write + ?Sized>(writer: &mut W, value: Self);

    /// Reinterprets a byte slice as a slice of elements.
    ///
    /// Fails with [`Error::InvalidLength`] if the length is not a multiple of
    /// [`Self::SIZE`] and with [`Error::InvalidData`] if the byte slice is not
    /// aligned for `Self`.
    fn cast_from_bytes(bytes: &[u8]) -> Result<&[Self]>;

    /// Reinterprets a slice of elements as a byte slice.
    fn cast_to_bytes(values: &[Self]) -> &[u8];
}

/// Shared implementation of [`Wire::cast_from_bytes`].
fn cast_slice<P: Wire>(bytes: &[u8]) -> Result<&[P]> {
    let size = P::SIZE;
    debug_assert_eq!(size, core::mem::size_of::<P>());
    debug_assert_eq!(core::mem::align_of::<P>(), 1);

    if size == 0 || !bytes.len().is_multiple_of(size) {
        return Err(Error::InvalidLength);
    }
    if !(bytes.as_ptr() as usize).is_multiple_of(core::mem::align_of::<P>()) {
        return Err(Error::InvalidData("misaligned array payload"));
    }
    // SAFETY: `P: Wire` guarantees that the layout matches `[u8; P::SIZE]`
    // and that every byte pattern is a valid `P`. The length is a multiple of
    // the element size and the pointer is aligned, both checked above.
    Ok(unsafe { core::slice::from_raw_parts(bytes.as_ptr().cast::<P>(), bytes.len() / size) })
}

/// Shared implementation of [`Wire::cast_to_bytes`].
const fn bytes_of<P: Wire>(values: &[P]) -> &[u8] {
    // SAFETY: `P: Wire` guarantees a padding-free, byte-compatible layout, so
    // the slice covers exactly `values.len() * P::SIZE` initialized bytes.
    unsafe {
        core::slice::from_raw_parts(values.as_ptr().cast::<u8>(), core::mem::size_of_val(values))
    }
}

unsafe impl Wire for u8 {
    const SIZE: usize = 1;

    #[inline]
    fn read_from<R: Read + ?Sized>(reader: &mut R) -> Result<Self> {
        reader.read_u8()
    }

    #[inline]
    fn write_to<W: Write + ?Sized>(writer: &mut W, value: Self) {
        writer.write_u8(value);
    }

    #[inline]
    fn cast_from_bytes(bytes: &[u8]) -> Result<&[Self]> {
        Ok(bytes)
    }

    #[inline]
    fn cast_to_bytes(values: &[Self]) -> &[u8] {
        values
    }
}

unsafe impl Wire for i8 {
    const SIZE: usize = 1;

    #[inline]
    fn read_from<R: Read + ?Sized>(reader: &mut R) -> Result<Self> {
        Ok(reader.read_u8()?.cast_signed())
    }

    #[inline]
    fn write_to<W: Write + ?Sized>(writer: &mut W, value: Self) {
        writer.write_u8(value.cast_unsigned());
    }

    #[inline]
    fn cast_from_bytes(bytes: &[u8]) -> Result<&[Self]> {
        cast_slice(bytes)
    }

    #[inline]
    fn cast_to_bytes(values: &[Self]) -> &[u8] {
        bytes_of(values)
    }
}

/// A big-endian `i32` in wire form.
///
/// The value is stored exactly as it appears on the wire, so a byte slice can
/// be reinterpreted as a slice of `I32Be` without copying or byte swapping.
/// The type has alignment 1, which is what makes that reinterpretation valid
/// for byte slices coming from anywhere. Use [`get`](Self::get) to obtain the
/// native `i32`.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct I32Be([u8; 4]);

const _: () = assert!(core::mem::size_of::<I32Be>() == 4);
const _: () = assert!(core::mem::align_of::<I32Be>() == 1);

impl I32Be {
    /// Creates a wire value from a native `i32`.
    #[inline]
    pub const fn new(value: i32) -> Self {
        Self(value.to_be_bytes())
    }

    /// Creates a wire value from its raw bytes.
    #[inline]
    pub const fn from_bytes(bytes: [u8; 4]) -> Self {
        Self(bytes)
    }

    /// Returns the value as a native `i32`.
    #[inline]
    pub const fn get(self) -> i32 {
        i32::from_be_bytes(self.0)
    }

    /// Returns the raw wire bytes.
    #[inline]
    pub const fn to_bytes(self) -> [u8; 4] {
        self.0
    }
}

impl From<i32> for I32Be {
    #[inline]
    fn from(value: i32) -> Self {
        Self::new(value)
    }
}

impl From<I32Be> for i32 {
    #[inline]
    fn from(value: I32Be) -> Self {
        value.get()
    }
}

impl fmt::Debug for I32Be {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "I32Be({})", self.get())
    }
}

unsafe impl Wire for I32Be {
    const SIZE: usize = 4;

    #[inline]
    fn read_from<R: Read + ?Sized>(reader: &mut R) -> Result<Self> {
        let mut bytes = [0u8; 4];
        reader.read_exact(&mut bytes)?;
        Ok(Self(bytes))
    }

    #[inline]
    fn write_to<W: Write + ?Sized>(writer: &mut W, value: Self) {
        writer.write_bytes(&value.0);
    }

    #[inline]
    fn cast_from_bytes(bytes: &[u8]) -> Result<&[Self]> {
        cast_slice(bytes)
    }

    #[inline]
    fn cast_to_bytes(values: &[Self]) -> &[u8] {
        bytes_of(values)
    }
}

/// A big-endian `i64` in wire form; see [`I32Be`] for the rationale.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct I64Be([u8; 8]);

const _: () = assert!(core::mem::size_of::<I64Be>() == 8);
const _: () = assert!(core::mem::align_of::<I64Be>() == 1);

impl I64Be {
    /// Creates a wire value from a native `i64`.
    #[inline]
    pub const fn new(value: i64) -> Self {
        Self(value.to_be_bytes())
    }

    /// Creates a wire value from its raw bytes.
    #[inline]
    pub const fn from_bytes(bytes: [u8; 8]) -> Self {
        Self(bytes)
    }

    /// Returns the value as a native `i64`.
    #[inline]
    pub const fn get(self) -> i64 {
        i64::from_be_bytes(self.0)
    }

    /// Returns the raw wire bytes.
    #[inline]
    pub const fn to_bytes(self) -> [u8; 8] {
        self.0
    }
}

impl From<i64> for I64Be {
    #[inline]
    fn from(value: i64) -> Self {
        Self::new(value)
    }
}

impl From<I64Be> for i64 {
    #[inline]
    fn from(value: I64Be) -> Self {
        value.get()
    }
}

impl fmt::Debug for I64Be {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "I64Be({})", self.get())
    }
}

unsafe impl Wire for I64Be {
    const SIZE: usize = 8;

    #[inline]
    fn read_from<R: Read + ?Sized>(reader: &mut R) -> Result<Self> {
        let mut bytes = [0u8; 8];
        reader.read_exact(&mut bytes)?;
        Ok(Self(bytes))
    }

    #[inline]
    fn write_to<W: Write + ?Sized>(writer: &mut W, value: Self) {
        writer.write_bytes(&value.0);
    }

    #[inline]
    fn cast_from_bytes(bytes: &[u8]) -> Result<&[Self]> {
        cast_slice(bytes)
    }

    #[inline]
    fn cast_to_bytes(values: &[Self]) -> &[u8] {
        bytes_of(values)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn i32be_roundtrips_native_values() {
        for value in [0, 1, -1, i32::MIN, i32::MAX, 123_456] {
            assert_eq!(I32Be::new(value).get(), value);
        }
    }

    #[test]
    fn casts_are_zero_copy() {
        let bytes = [0u8, 0, 0, 1, 0xff, 0xff, 0xff, 0xfe];
        let values = I32Be::cast_from_bytes(&bytes).unwrap();
        assert_eq!(values.len(), 2);
        assert_eq!(values[0].get(), 1);
        assert_eq!(values[1].get(), -2);
        assert_eq!(values.as_ptr().cast::<u8>(), bytes.as_ptr());
        assert_eq!(I32Be::cast_to_bytes(values), bytes);
    }

    #[test]
    fn casts_reject_ragged_lengths() {
        assert_eq!(I32Be::cast_from_bytes(&[0; 5]), Err(Error::InvalidLength));
        assert_eq!(I64Be::cast_from_bytes(&[0; 9]), Err(Error::InvalidLength));
    }
}
