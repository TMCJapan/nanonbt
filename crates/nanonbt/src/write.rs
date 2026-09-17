//! Writing NBT through the [`Write`] trait.
//!
//! [`ToNBT`] implementations write a value's payload only; the
//! tag and the name around it are written by whoever owns the entry, usually
//! through [`ToNBT::write_entry`].  A list header is
//! written by the sequence itself, as an element tag followed by a length.

use alloc::vec::Vec;

use crate::{
    cesu8,
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
    fn write_name(&mut self, name: &str) -> Result<()> {
        self.write_str(name)
    }

    /// Writes a length-prefixed modified UTF-8 string.
    fn write_str(&mut self, v: &str) -> Result<()>;

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
}

impl Write for Writer<'_> {
    fn write_tag(&mut self, tag: u8) -> Result<()> {
        self.out.push(tag);
        Ok(())
    }

    fn write_str(&mut self, v: &str) -> Result<()> {
        let bytes = cesu8::to_java_cesu8(v);
        let len = u16::try_from(bytes.len()).map_err(|_| Error::string_too_long())?;
        self.out.extend_from_slice(&len.to_be_bytes());
        self.out.extend_from_slice(&bytes);
        Ok(())
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
    /// A method rather than a constant because a [`Value`](crate::Value) can
    /// hold any tag.
    fn tag(&self) -> u8;

    /// Writes the payload, without the tag or the name.
    fn write<W: Write>(&self, writer: &mut W) -> Result<()>;

    /// Writes a compound entry: the tag, the name, then the payload.
    fn write_entry<W: Write>(&self, name: &str, writer: &mut W) -> Result<()> {
        writer.write_tag(self.tag())?;
        writer.write_name(name)?;
        self.write(writer)
    }
}
