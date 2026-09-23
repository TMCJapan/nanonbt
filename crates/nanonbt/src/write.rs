//! Writing NBT through the [`Write`] trait.
//!
//! [`ToNBT`] implementations write a value's payload only; the
//! tag and the name around it are written by whoever owns the entry, usually
//! through [`ToNBT::write_entry`].  A list header is
//! written by the sequence itself, as an element tag followed by a length.

use alloc::vec::Vec;

use nanocesu8::Cesu8;

use crate::{
    arrays::{ByteArray, IntArray, LongArray},
    error::{Error, Result},
    tag::TAG_END,
};

/// A sink of NBT bytes.
///
/// The methods write big-endian payloads, exactly as NBT stores them; nothing
/// here adds a tag, a name or a length the caller did not ask for.
pub trait Write {
    /// Writes one tag byte.
    fn write_tag(&mut self, tag: u8) -> Result<()>;

    /// Writes the End tag that closes a compound.
    ///
    /// The default is [`write_tag`](Write::write_tag); a writer that tracks
    /// structure, like the one behind `to_value`, overrides it.
    fn write_end(&mut self) -> Result<()> {
        self.write_tag(TAG_END)
    }

    /// Writes a compound entry's name, which is encoded like a string.
    fn write_name(&mut self, name: &Cesu8) -> Result<()> {
        self.write_cesu8(name)
    }

    /// Writes a length-prefixed modified UTF-8 string.
    fn write_str(&mut self, v: &str) -> Result<()>;

    /// Writes a length-prefixed string as its exact modified UTF-8 bytes.
    ///
    /// [`write_str`](Write::write_str) encodes to the same spelling, so the
    /// two write the same bytes for the same text.
    fn write_cesu8(&mut self, v: &Cesu8) -> Result<()>;

    fn write_i8(&mut self, v: i8) -> Result<()>;

    fn write_i16(&mut self, v: i16) -> Result<()>;

    fn write_i32(&mut self, v: i32) -> Result<()>;

    fn write_i64(&mut self, v: i64) -> Result<()>;

    fn write_f32(&mut self, v: f32) -> Result<()>;

    fn write_f64(&mut self, v: f64) -> Result<()>;

    /// Writes a list's or array's `i32` length.
    fn write_len(&mut self, len: usize) -> Result<()>;

    /// Writes raw payload bytes, as an array's elements already encoded.
    fn write_bytes(&mut self, bytes: &[u8]) -> Result<()>;

    /// Writes a byte array's payload: the `i32` length, then the elements.
    ///
    /// Any element spelling [`ByteArray`] holds goes in — an `i8` or `u8`
    /// slice, array or `Vec` — and bytes have no endianness to settle, so
    /// they go out as they are. The tag and the name around them are the
    /// caller's, as they are for any other payload.
    fn write_byte_array<E, T: ByteArray<E>>(&mut self, array: T) -> Result<()> {
        array.write_payload(self)
    }

    /// Writes an int array's payload: the `i32` length, then the elements.
    ///
    /// Any element spelling [`IntArray`] holds goes in: `i32`, `u32`,
    /// [`I32Be`](crate::I32Be) or [`U32Be`](crate::U32Be).
    fn write_int_array<E, T: IntArray<E>>(&mut self, array: T) -> Result<()> {
        array.write_payload(self)
    }

    /// Writes a long array's payload: the `i32` length, then the elements.
    ///
    /// Any element spelling [`LongArray`] holds goes in: `i64`, `u64`,
    /// [`I64Be`](crate::I64Be) or [`U64Be`](crate::U64Be).
    fn write_long_array<E, T: LongArray<E>>(&mut self, array: T) -> Result<()> {
        array.write_payload(self)
    }
}

/// A [`Write`] into a byte vector.
pub struct Writer<'w> {
    out: &'w mut Vec<u8>,
}

impl<'w> Writer<'w> {
    pub const fn new(out: &'w mut Vec<u8>) -> Self {
        Self { out }
    }

    pub const fn into_inner(self) -> &'w mut Vec<u8> {
        self.out
    }

    /// Writes bytes after their `u16` length.
    fn write_prefixed(&mut self, bytes: &[u8]) -> Result<()> {
        let len = u16::try_from(bytes.len()).map_err(|_| Error::string_too_long())?;
        self.out.extend_from_slice(&len.to_be_bytes());
        self.out.extend_from_slice(bytes);
        Ok(())
    }
}

impl Write for Writer<'_> {
    fn write_tag(&mut self, tag: u8) -> Result<()> {
        self.out.push(tag);
        Ok(())
    }

    fn write_str(&mut self, v: &str) -> Result<()> {
        self.write_prefixed(Cesu8::from_str(v).as_bytes())
    }

    fn write_cesu8(&mut self, v: &Cesu8) -> Result<()> {
        self.write_prefixed(v.as_bytes())
    }

    fn write_i8(&mut self, v: i8) -> Result<()> {
        self.out.extend_from_slice(&v.to_be_bytes());
        Ok(())
    }

    fn write_i16(&mut self, v: i16) -> Result<()> {
        self.out.extend_from_slice(&v.to_be_bytes());
        Ok(())
    }

    fn write_i32(&mut self, v: i32) -> Result<()> {
        self.out.extend_from_slice(&v.to_be_bytes());
        Ok(())
    }

    fn write_i64(&mut self, v: i64) -> Result<()> {
        self.out.extend_from_slice(&v.to_be_bytes());
        Ok(())
    }

    fn write_f32(&mut self, v: f32) -> Result<()> {
        self.out.extend_from_slice(&v.to_be_bytes());
        Ok(())
    }

    fn write_f64(&mut self, v: f64) -> Result<()> {
        self.out.extend_from_slice(&v.to_be_bytes());
        Ok(())
    }

    fn write_len(&mut self, len: usize) -> Result<()> {
        let len = i32::try_from(len).map_err(|_| Error::len_too_large())?;
        self.out.extend_from_slice(&len.to_be_bytes());
        Ok(())
    }

    fn write_bytes(&mut self, bytes: &[u8]) -> Result<()> {
        self.out.extend_from_slice(bytes);
        Ok(())
    }
}

/// A value that can be written as NBT.
///
/// [`write`](ToNBT::write) writes the value's payload only. The tag and the
/// name around it are written by the owner of the entry, either
/// [`write_entry`](ToNBT::write_entry) or the root of [`to_bytes`].
///
/// [`to_bytes`]: crate::to_bytes
pub trait ToNBT {
    /// The tag byte that precedes this value.
    ///
    /// A method rather than a constant because a value can hold any tag.
    const TAG: u8;

    /// Writes the payload, without the tag or the name.
    fn write<W: Write>(&self, writer: &mut W) -> Result<()>;

    /// Writes a list's elements, the header already written.
    ///
    /// The default writes them one at a time. The numeric types override
    /// this to write the whole payload in a few bulk writes, which settles
    /// the byte order a vector at a time; an override must write exactly
    /// `elements.len()` elements, each in the spelling its own
    /// [`write`](ToNBT::write) uses.
    fn write_elements<W: Write>(elements: &[Self], writer: &mut W) -> Result<()>
    where
        Self: Sized,
    {
        for element in elements {
            element.write(writer)?;
        }
        Ok(())
    }

    /// Writes a compound entry: the tag, the name, then the payload.
    ///
    /// The name is taken as its exact modified UTF-8 bytes, which is what
    /// the derive macros encode a field's name to while they expand; nothing
    /// is converted here.
    fn write_entry<W: Write>(&self, name: &Cesu8, writer: &mut W) -> Result<()> {
        writer.write_tag(Self::TAG)?;
        writer.write_name(name)?;
        self.write(writer)
    }
}
