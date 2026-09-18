//! Reading NBT through the [`Read`] trait.
//!
//! [`FromNBT`] implementations read a value's payload only;
//! the tag has already been consumed by whoever owns the entry and is passed
//! along as an argument, so that a type can dispatch on it. Strings decode
//! from modified UTF-8, borrowing from the input when they are plain UTF-8;
//! [`Read::read_cesu8`] borrows the raw bytes instead, so a string that is
//! not UTF-8 borrows too.

use alloc::borrow::Cow;

use nanocesu8::{Cesu8, from_java_cesu8};

use crate::{
    DeOpts,
    error::{Error, Result},
    tag::{
        TAG_BYTE, TAG_BYTE_ARRAY, TAG_COMPOUND, TAG_DOUBLE, TAG_END, TAG_FLOAT, TAG_INT,
        TAG_INT_ARRAY, TAG_LIST, TAG_LONG, TAG_LONG_ARRAY, TAG_MAX, TAG_SHORT, TAG_STRING,
    },
};

/// A source of NBT bytes.
pub trait Read<'de> {
    /// Reads one tag byte, refusing bytes that name no type.
    fn read_tag(&mut self) -> Result<u8>;

    /// Reads a compound entry's name, which is encoded like a string.
    fn read_name(&mut self) -> Result<Cow<'de, str>> {
        self.read_str()
    }

    /// Skips a compound entry's name without decoding it.
    ///
    /// The root compound's name is skipped this way, so a document whose
    /// root name is not valid modified UTF-8 is still accepted.
    fn skip_name(&mut self) -> Result<()> {
        self.read_name().map(drop)
    }

    /// Reads a length-prefixed modified UTF-8 string.
    fn read_str(&mut self) -> Result<Cow<'de, str>>;

    /// Reads a length-prefixed modified UTF-8 string without decoding it.
    ///
    /// The bytes borrow from the input whenever the reader can, so a string
    /// that is not UTF-8 — a NUL as `C0 80`, a non-BMP character as a
    /// surrogate pair — can be kept as it was written.
    fn read_cesu8(&mut self) -> Result<Cow<'de, Cesu8>>;

    fn read_i8(&mut self) -> Result<i8>;

    fn read_i16(&mut self) -> Result<i16>;

    fn read_i32(&mut self) -> Result<i32>;

    fn read_i64(&mut self) -> Result<i64>;

    fn read_f32(&mut self) -> Result<f32>;

    fn read_f64(&mut self) -> Result<f64>;

    /// Reads a list header: the element tag, then the `i32` length.
    fn read_list_header(&mut self) -> Result<(u8, usize)>;

    /// Reads an array's `i32` length, which must be non-negative and within
    /// [`DeOpts::max_seq_len`].
    fn read_len(&mut self) -> Result<usize>;

    /// Reads `len` raw payload bytes.
    fn read_bytes(&mut self, len: usize) -> Result<Cow<'de, [u8]>>;

    /// Skips a value by its structure alone, decoding nothing.
    fn skip(&mut self, tag: u8) -> Result<()>;

    /// Runs `f` one level deeper, refusing input nested deeper than
    /// [`DeOpts::max_depth`].
    fn nest<T>(&mut self, f: impl FnOnce(&mut Self) -> Result<T>) -> Result<T>;
}

/// A value that can be read from NBT.
///
/// [`read`](FromNBT::read) receives the value's tag, which the owner of the
/// entry has already consumed, and reads the payload that follows.
pub trait FromNBT<'de>: Sized {
    fn read<R: Read<'de>>(tag: u8, reader: &mut R) -> Result<Self>;
}

/// A [`Read`] from a byte slice, borrowing from it where possible.
pub struct Reader<'de> {
    input: &'de [u8],
    /// How many lists and compounds are open, which bounds the recursion.
    depth: usize,
    opts: DeOpts,
}

impl<'de> Reader<'de> {
    pub const fn new(input: &'de [u8], opts: DeOpts) -> Self {
        Self {
            input,
            depth: 0,
            opts,
        }
    }

    /// The bytes not read yet.
    pub const fn remaining(&self) -> &'de [u8] {
        self.input
    }

    const fn take(&mut self, n: usize) -> Result<&'de [u8]> {
        if n > self.input.len() {
            return Err(Error::unexpected_eof());
        }
        let (taken, rest) = self.input.split_at(n);
        self.input = rest;
        Ok(taken)
    }

    fn take_array<const N: usize>(&mut self) -> Result<[u8; N]> {
        let bytes = self.take(N)?;
        let mut array = [0; N];
        array.copy_from_slice(bytes);
        Ok(array)
    }

    /// Reads a list or compound one level deeper, refusing input that nests
    /// too deeply.
    fn nested<T>(&mut self, f: impl FnOnce(&mut Self) -> Result<T>) -> Result<T> {
        if self.depth >= self.opts.max_depth {
            return Err(Error::too_deep());
        }
        self.depth += 1;
        let result = f(self);
        self.depth -= 1;
        result
    }

    /// Skips a string without decoding it.
    fn skip_str(&mut self) -> Result<()> {
        self.take_str_bytes().map(drop)
    }

    /// Skips a value by its structure alone, decoding nothing.
    ///
    /// Unlike reading a value, lengths are not bounded by `max_seq_len`, and
    /// a negative list length skips no elements.
    fn skip_value(&mut self, tag: u8) -> Result<()> {
        match tag {
            TAG_BYTE => self.take(1).map(drop),
            TAG_SHORT => self.take(2).map(drop),
            TAG_INT | TAG_FLOAT => self.take(4).map(drop),
            TAG_LONG | TAG_DOUBLE => self.take(8).map(drop),
            TAG_STRING => self.skip_str(),
            TAG_BYTE_ARRAY => self.skip_array(1),
            TAG_INT_ARRAY => self.skip_array(4),
            TAG_LONG_ARRAY => self.skip_array(8),
            TAG_COMPOUND => self.nested(|reader| {
                loop {
                    let tag = reader.read_tag()?;
                    if tag == TAG_END {
                        return Ok(());
                    }
                    reader.skip_str()?;
                    reader.skip_value(tag)?;
                }
            }),
            TAG_LIST => {
                let element = self.read_tag()?;
                let len = i32::from_be_bytes(self.take_array()?);
                self.nested(|reader| {
                    for _ in 0..len {
                        reader.skip_value(element)?;
                    }
                    Ok(())
                })
            }
            // fastnbt panics here, skipping a list of End with elements.
            _ => Err(Error::list_of_end()),
        }
    }

    fn skip_array(&mut self, size: usize) -> Result<()> {
        let len = i32::from_be_bytes(self.take_array()?);
        let len = usize::try_from(len).map_err(|_| Error::negative_len())?;
        self.take_elements(len, size).map(drop)
    }

    /// `len` elements of `size` bytes each.
    fn take_elements(&mut self, len: usize, size: usize) -> Result<&'de [u8]> {
        let n = len.checked_mul(size).ok_or_else(Error::array_too_large)?;
        self.take(n)
    }

    /// The bytes of a length-prefixed string, decoded by the caller.
    fn take_str_bytes(&mut self) -> Result<&'de [u8]> {
        let len = u16::from_be_bytes(self.take_array()?);
        self.take(usize::from(len))
    }

    fn str(&mut self) -> Result<Cow<'de, str>> {
        from_java_cesu8(self.take_str_bytes()?).map_err(|_| Error::nonunicode_string())
    }

    fn cesu8(&mut self) -> Result<&'de Cesu8> {
        Cesu8::new(self.take_str_bytes()?).map_err(|_| Error::nonunicode_string())
    }
}

impl<'de> Read<'de> for Reader<'de> {
    fn read_tag(&mut self) -> Result<u8> {
        let [tag] = self.take_array()?;
        if tag > TAG_MAX {
            return Err(Error::invalid_tag(tag));
        }
        Ok(tag)
    }

    fn read_str(&mut self) -> Result<Cow<'de, str>> {
        self.str()
    }

    fn read_cesu8(&mut self) -> Result<Cow<'de, Cesu8>> {
        self.cesu8().map(Cow::Borrowed)
    }

    fn skip_name(&mut self) -> Result<()> {
        self.skip_str()
    }

    fn read_i8(&mut self) -> Result<i8> {
        Ok(i8::from_be_bytes(self.take_array()?))
    }

    fn read_i16(&mut self) -> Result<i16> {
        Ok(i16::from_be_bytes(self.take_array()?))
    }

    fn read_i32(&mut self) -> Result<i32> {
        Ok(i32::from_be_bytes(self.take_array()?))
    }

    fn read_i64(&mut self) -> Result<i64> {
        Ok(i64::from_be_bytes(self.take_array()?))
    }

    fn read_f32(&mut self) -> Result<f32> {
        Ok(f32::from_be_bytes(self.take_array()?))
    }

    fn read_f64(&mut self) -> Result<f64> {
        Ok(f64::from_be_bytes(self.take_array()?))
    }

    fn read_list_header(&mut self) -> Result<(u8, usize)> {
        let element = self.read_tag()?;
        let len = self.read_len()?;
        Ok((element, len))
    }

    fn read_len(&mut self) -> Result<usize> {
        let len = i32::from_be_bytes(self.take_array()?);
        let len = usize::try_from(len).map_err(|_| Error::negative_len())?;
        if len > self.opts.max_seq_len {
            return Err(Error::seq_too_long());
        }
        Ok(len)
    }

    fn read_bytes(&mut self, len: usize) -> Result<Cow<'de, [u8]>> {
        self.take(len).map(Cow::Borrowed)
    }

    fn skip(&mut self, tag: u8) -> Result<()> {
        self.skip_value(tag)
    }

    fn nest<T>(&mut self, f: impl FnOnce(&mut Self) -> Result<T>) -> Result<T> {
        self.nested(f)
    }
}
