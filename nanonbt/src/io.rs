#[cfg(feature = "alloc")]
use alloc::vec::Vec;

use crate::{Error, Result};

/// A sink for encoded NBT bytes.
///
/// Writing is infallible by design: the usual target is an in-memory buffer and
/// NBT documents are small. Encode into a [`Vec<u8>`] with
/// [`to_bytes`](crate::to_bytes), which is itself a [`Write`] implementation,
/// or implement this trait for a custom sink.
pub trait Write {
    /// Appends `bytes` to the sink.
    fn write_bytes(&mut self, bytes: &[u8]);

    /// Appends a single byte.
    #[inline]
    fn write_u8(&mut self, value: u8) {
        self.write_bytes(&[value]);
    }

    /// Appends a big-endian `u16`.
    #[inline]
    fn write_u16_be(&mut self, value: u16) {
        self.write_bytes(&value.to_be_bytes());
    }

    /// Appends a big-endian `i16`.
    #[inline]
    fn write_i16_be(&mut self, value: i16) {
        self.write_bytes(&value.to_be_bytes());
    }

    /// Appends a big-endian `i32`.
    #[inline]
    fn write_i32_be(&mut self, value: i32) {
        self.write_bytes(&value.to_be_bytes());
    }

    /// Appends a big-endian `i64`.
    #[inline]
    fn write_i64_be(&mut self, value: i64) {
        self.write_bytes(&value.to_be_bytes());
    }

    /// Appends a big-endian IEEE-754 `f32`.
    #[inline]
    fn write_f32_be(&mut self, value: f32) {
        self.write_bytes(&value.to_bits().to_be_bytes());
    }

    /// Appends a big-endian IEEE-754 `f64`.
    #[inline]
    fn write_f64_be(&mut self, value: f64) {
        self.write_bytes(&value.to_bits().to_be_bytes());
    }
}

#[cfg(feature = "alloc")]
impl Write for Vec<u8> {
    #[inline]
    fn write_bytes(&mut self, bytes: &[u8]) {
        self.extend_from_slice(bytes);
    }
}

impl<W: Write + ?Sized> Write for &mut W {
    #[inline]
    fn write_bytes(&mut self, bytes: &[u8]) {
        (**self).write_bytes(bytes);
    }
}

/// A source of encoded NBT bytes.
///
/// This is the owned half of the reader split: any [`Read`] can produce owned
/// values, while zero-copy decoding additionally requires [`ReadRef`].
pub trait Read {
    /// Reads exactly `buf.len()` bytes into `buf`.
    ///
    /// Returns [`Error::UnexpectedEof`] if the input is too short.
    fn read_exact(&mut self, buf: &mut [u8]) -> Result<()>;

    /// Advances the reader by `len` bytes without buffering them.
    fn skip(&mut self, len: usize) -> Result<()>;

    /// Returns the number of bytes that can still be read.
    ///
    /// Decoders consult this before acting on a length prefix: a count that
    /// exceeds the remaining input is rejected up front, so a short document
    /// cannot force a large allocation for a value it does not contain.
    fn remaining(&self) -> usize;

    /// Reads a single byte.
    #[inline]
    fn read_u8(&mut self) -> Result<u8> {
        let mut buffer = [0u8; 1];
        self.read_exact(&mut buffer)?;
        Ok(buffer[0])
    }

    /// Reads a big-endian `u16`.
    #[inline]
    fn read_u16_be(&mut self) -> Result<u16> {
        let mut buffer = [0u8; 2];
        self.read_exact(&mut buffer)?;
        Ok(u16::from_be_bytes(buffer))
    }

    /// Reads a big-endian `i16`.
    #[inline]
    fn read_i16_be(&mut self) -> Result<i16> {
        let mut buffer = [0u8; 2];
        self.read_exact(&mut buffer)?;
        Ok(i16::from_be_bytes(buffer))
    }

    /// Reads a big-endian `i32`.
    #[inline]
    fn read_i32_be(&mut self) -> Result<i32> {
        let mut buffer = [0u8; 4];
        self.read_exact(&mut buffer)?;
        Ok(i32::from_be_bytes(buffer))
    }

    /// Reads a big-endian `i64`.
    #[inline]
    fn read_i64_be(&mut self) -> Result<i64> {
        let mut buffer = [0u8; 8];
        self.read_exact(&mut buffer)?;
        Ok(i64::from_be_bytes(buffer))
    }

    /// Reads a big-endian IEEE-754 `f32`.
    #[inline]
    fn read_f32_be(&mut self) -> Result<f32> {
        let mut buffer = [0u8; 4];
        self.read_exact(&mut buffer)?;
        Ok(f32::from_bits(u32::from_be_bytes(buffer)))
    }

    /// Reads a big-endian IEEE-754 `f64`.
    #[inline]
    fn read_f64_be(&mut self) -> Result<f64> {
        let mut buffer = [0u8; 8];
        self.read_exact(&mut buffer)?;
        Ok(f64::from_bits(u64::from_be_bytes(buffer)))
    }
}

#[cfg(not(feature = "std"))]
impl Read for &[u8] {
    #[inline]
    fn read_exact(&mut self, buf: &mut [u8]) -> Result<()> {
        if self.len() < buf.len() {
            return Err(Error::UnexpectedEof);
        }
        let bytes: &[u8] = self;
        let (head, tail) = bytes.split_at(buf.len());
        buf.copy_from_slice(head);
        *self = tail;
        Ok(())
    }

    #[inline]
    fn skip(&mut self, len: usize) -> Result<()> {
        if self.len() < len {
            return Err(Error::UnexpectedEof);
        }
        let bytes: &[u8] = self;
        *self = &bytes[len..];
        Ok(())
    }

    #[inline]
    fn remaining(&self) -> usize {
        self.len()
    }
}

#[cfg(not(feature = "std"))]
impl<R: Read + ?Sized> Read for &mut R {
    #[inline]
    fn read_exact(&mut self, buf: &mut [u8]) -> Result<()> {
        (**self).read_exact(buf)
    }

    #[inline]
    fn skip(&mut self, len: usize) -> Result<()> {
        (**self).skip(len)
    }

    #[inline]
    fn remaining(&self) -> usize {
        (**self).remaining()
    }
}

/// A [`Read`] that can hand out slices of its backing buffer.
///
/// The returned slice is tied to `'buf` — the lifetime of the backing buffer —
/// and *not* to the `&mut self` borrow. This is what makes it possible to
/// decode several borrowed fields from one reader:
///
/// ```
/// use nanonbt::{ReadRef, SliceReader};
///
/// let input = [0u8, 0, 0, 1, 2, 3, 4, 5];
/// let mut reader = SliceReader::new(&input);
/// let first = reader.read_borrowed(2).unwrap();
/// let second = reader.read_borrowed(2).unwrap();
/// assert_eq!(first, &[0, 0]);
/// assert_eq!(second, &[0, 1]);
/// ```
///
/// A reader that owns its buffer (such as [`VecReader`]) cannot implement this
/// trait, which restricts it to owned decoding at compile time:
///
/// ```compile_fail
/// use nanonbt::{DeserializeNbtArrayRef, VecReader, route::ByteArray};
///
/// let mut reader = VecReader::new(vec![0, 0, 0, 1, 7]);
/// let borrowed = <&[u8] as DeserializeNbtArrayRef<ByteArray>>::deserialize_nbt_array_ref(&mut reader);
/// ```
pub trait ReadRef<'buf>: Read {
    /// Returns the next `len` bytes as a slice of the backing buffer.
    fn read_borrowed(&mut self, len: usize) -> Result<&'buf [u8]>;
}

impl<'buf> ReadRef<'buf> for &'buf [u8] {
    #[inline]
    fn read_borrowed(&mut self, len: usize) -> Result<&'buf [u8]> {
        if self.len() < len {
            return Err(Error::UnexpectedEof);
        }
        let bytes: &'buf [u8] = self;
        let (head, tail) = bytes.split_at(len);
        *self = tail;
        Ok(head)
    }
}

#[cfg(not(feature = "std"))]
impl<'buf, R: ReadRef<'buf> + ?Sized> ReadRef<'buf> for &mut R {
    #[inline]
    fn read_borrowed(&mut self, len: usize) -> Result<&'buf [u8]> {
        (**self).read_borrowed(len)
    }
}

/// A [`Read`] / [`ReadRef`] over a borrowed byte slice.
#[derive(Debug, Clone, Copy)]
pub struct SliceReader<'buf> {
    bytes: &'buf [u8],
    pos: usize,
}

impl<'buf> SliceReader<'buf> {
    /// Creates a reader positioned at the start of `bytes`.
    #[inline]
    pub const fn new(bytes: &'buf [u8]) -> Self {
        Self { bytes, pos: 0 }
    }

    /// Returns the current offset into the backing buffer.
    #[inline]
    pub const fn position(&self) -> usize {
        self.pos
    }

    /// Returns `true` if the reader is at the end of the buffer.
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.pos >= self.bytes.len()
    }
}

impl Read for SliceReader<'_> {
    #[inline]
    fn read_exact(&mut self, buf: &mut [u8]) -> Result<()> {
        let end = self
            .pos
            .checked_add(buf.len())
            .ok_or(Error::UnexpectedEof)?;
        let source = self.bytes.get(self.pos..end).ok_or(Error::UnexpectedEof)?;
        buf.copy_from_slice(source);
        self.pos = end;
        Ok(())
    }

    #[inline]
    fn skip(&mut self, len: usize) -> Result<()> {
        let end = self.pos.checked_add(len).ok_or(Error::UnexpectedEof)?;
        if end > self.bytes.len() {
            return Err(Error::UnexpectedEof);
        }
        self.pos = end;
        Ok(())
    }

    #[inline]
    fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.pos)
    }
}

impl<'buf> ReadRef<'buf> for SliceReader<'buf> {
    #[inline]
    fn read_borrowed(&mut self, len: usize) -> Result<&'buf [u8]> {
        let end = self.pos.checked_add(len).ok_or(Error::UnexpectedEof)?;
        if end > self.bytes.len() {
            return Err(Error::UnexpectedEof);
        }
        let start = self.pos;
        self.pos = end;
        let bytes: &'buf [u8] = self.bytes;
        Ok(&bytes[start..end])
    }
}

/// A [`Read`]-only reader over an owned byte buffer.
///
/// [`VecReader`] deliberately does **not** implement [`ReadRef`]: because it
/// owns its buffer, no slice of that buffer can outlive a borrow, so
/// zero-copy array decoding is rejected at compile time. Use it to model
/// streaming inputs such as sockets or decompressed regions.
#[cfg(feature = "alloc")]
#[derive(Debug, Clone)]
pub struct VecReader {
    bytes: Vec<u8>,
    pos: usize,
}

#[cfg(feature = "alloc")]
impl VecReader {
    /// Creates a reader owning `bytes`, positioned at the start.
    #[inline]
    pub const fn new(bytes: Vec<u8>) -> Self {
        Self { bytes, pos: 0 }
    }

    /// Returns the current offset into the backing buffer.
    #[inline]
    pub const fn position(&self) -> usize {
        self.pos
    }

    /// Consumes the reader and returns the backing buffer.
    #[inline]
    pub fn into_inner(self) -> Vec<u8> {
        self.bytes
    }
}

#[cfg(feature = "alloc")]
impl Read for VecReader {
    #[inline]
    fn read_exact(&mut self, buf: &mut [u8]) -> Result<()> {
        let end = self
            .pos
            .checked_add(buf.len())
            .ok_or(Error::UnexpectedEof)?;
        let source = self.bytes.get(self.pos..end).ok_or(Error::UnexpectedEof)?;
        buf.copy_from_slice(source);
        self.pos = end;
        Ok(())
    }

    #[inline]
    fn skip(&mut self, len: usize) -> Result<()> {
        let end = self.pos.checked_add(len).ok_or(Error::UnexpectedEof)?;
        if end > self.bytes.len() {
            return Err(Error::UnexpectedEof);
        }
        self.pos = end;
        Ok(())
    }

    #[inline]
    fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.pos)
    }
}

#[cfg(feature = "std")]
impl<R: std::io::Read + ?Sized> crate::Read for R {
    fn read_exact(&mut self, buf: &mut [u8]) -> crate::Result<()> {
        std::io::Read::read_exact(self, buf).map_err(|_| crate::Error::UnexpectedEof)
    }

    fn skip(&mut self, len: usize) -> crate::Result<()> {
        let mut remaining = len;
        let mut tmp = [0u8; 1024];
        while remaining > 0 {
            let chunk = std::cmp::min(remaining, tmp.len());
            std::io::Read::read_exact(self, &mut tmp[..chunk])
                .map_err(|_| crate::Error::UnexpectedEof)?;
            remaining -= chunk;
        }
        Ok(())
    }

    fn remaining(&self) -> usize {
        // std::io::Read does not expose remaining bytes; treat as unknown.
        usize::MAX
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn slice_reader_tracks_position() {
        let mut reader = SliceReader::new(&[1, 2, 3, 4]);
        assert_eq!(reader.read_u8().unwrap(), 1);
        reader.skip(2).unwrap();
        assert_eq!(reader.read_u8().unwrap(), 4);
        assert!(reader.is_empty());
        assert_eq!(reader.read_u8(), Err(Error::UnexpectedEof));
    }

    #[test]
    fn slice_reader_borrows_independent_of_reader_borrow() {
        let input = [1u8, 2, 3, 4, 5, 6];
        let mut reader = SliceReader::new(&input);
        let first = reader.read_borrowed(3).unwrap();
        let second = reader.read_borrowed(3).unwrap();
        assert_eq!(first, &[1, 2, 3]);
        assert_eq!(second, &[4, 5, 6]);
    }

    #[test]
    fn slice_impl_reads() {
        let mut input: &[u8] = &[9, 8, 7];
        assert_eq!(input.read_u8().unwrap(), 9);
        assert_eq!(input.read_u16_be().unwrap(), u16::from_be_bytes([8, 7]));
    }

    #[test]
    fn vec_reader_decodes_owned() {
        let mut reader = VecReader::new(vec![0, 0, 0, 42]);
        assert_eq!(reader.read_i32_be().unwrap(), 42);
        assert_eq!(reader.remaining(), 0);
    }
}
