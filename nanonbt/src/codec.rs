#[cfg(feature = "alloc")]
use alloc::string::String;
#[cfg(feature = "alloc")]
use alloc::vec;

use crate::{Error, Read, Result, Tag, Write};

/// Maximum nesting depth accepted by [`skip_nbt`].
const MAX_NESTING_DEPTH: usize = 512;

/// Reads a length prefix: a non-negative big-endian `i32`.
#[inline]
pub fn read_count<R: Read + ?Sized>(reader: &mut R) -> Result<usize> {
    let count = reader.read_i32_be()?;
    if count < 0 {
        return Err(Error::InvalidLength);
    }
    usize::try_from(count).map_err(|_| Error::InvalidLength)
}

/// Writes a length prefix as a big-endian `i32`.
#[inline]
pub fn write_count<W: Write + ?Sized>(writer: &mut W, len: usize) -> Result<()> {
    let len = i32::try_from(len).map_err(|_| Error::InvalidLength)?;
    writer.write_i32_be(len);
    Ok(())
}

#[cfg(feature = "alloc")]
/// Reads a modified UTF-8 (CESU-8) string: a `u16` byte length followed by the bytes.
pub fn read_string<R: Read + ?Sized>(reader: &mut R) -> Result<String> {
    let mut out = String::new();
    read_string_into(reader, &mut out)?;
    Ok(out)
}

#[cfg(feature = "alloc")]
/// Reads a modified UTF-8 (CESU-8) string into `out`, reusing its allocation.
///
/// [`read_string`] is the shorthand when the allocation is not already there
/// to reuse.
#[cfg(feature = "alloc")]
pub fn read_string_into<R: Read + ?Sized>(reader: &mut R, out: &mut String) -> Result<()> {
    /// Strings up to this length are read through a stack buffer.
    const INLINE: usize = 64;

    let len = reader.read_u16_be()? as usize;
    out.clear();
    if len <= INLINE {
        let mut buffer = [0u8; INLINE];
        reader.read_exact(&mut buffer[..len])?;
        let cesu8 = nanocesu8::Cesu8::new(&buffer[..len]).map_err(|_| Error::InvalidUtf8)?;
        out.push_str(&cesu8.decode());
    } else {
        let mut buffer = vec![0u8; len];
        reader.read_exact(&mut buffer)?;
        let cesu8 = nanocesu8::Cesu8::new(&buffer).map_err(|_| Error::InvalidUtf8)?;
        out.push_str(&cesu8.decode());
    }
    Ok(())
}

/// Reads a compound entry's name into `buf`, without decoding it.
///
/// On success the name's bytes are in `buf[..len]`, validated as modified
/// UTF-8 like any other string on the wire, so the caller can compare them
/// with the bytes its field names were encoded as. Nothing is decoded into
/// `str`: a name holding a NUL or a non-BMP character has a different
/// modified spelling than its UTF-8, and matching that spelling is what
/// keeps a read from converting it back.
///
/// Returns `None` when the name is longer than `buf`; it has been skipped,
/// and since the derive sizes `buf` to its longest field name, a name that
/// does not fit can equal none of them. Such a name is not validated.
pub fn read_name_into<R: Read + ?Sized>(reader: &mut R, buf: &mut [u8]) -> Result<Option<usize>> {
    let len = reader.read_u16_be()? as usize;
    if len > buf.len() {
        reader.skip(len)?;
        return Ok(None);
    }
    let name = &mut buf[..len];
    reader.read_exact(name)?;
    nanocesu8::Cesu8::new(name).map_err(|_| Error::InvalidUtf8)?;
    Ok(Some(len))
}

/// Writes a modified UTF-8 (CESU-8) string: a `u16` byte length followed by the bytes.
pub fn write_string<W: Write + ?Sized>(writer: &mut W, value: &str) -> Result<()> {
    let encoded = nanocesu8::Cesu8::from_str(value);
    let len = u16::try_from(encoded.len()).map_err(|_| Error::InvalidLength)?;
    writer.write_u16_be(len);
    writer.write_bytes(encoded.as_bytes());
    Ok(())
}

/// Skips the payload of `tag`, including arbitrarily nested compounds and
/// lists.
///
/// The derive macro uses this to ignore unknown compound fields, which makes
/// decoding forward compatible. Nesting is limited to protect against
/// maliciously deep inputs.
pub fn skip_nbt<R: Read + ?Sized>(reader: &mut R, tag: Tag) -> Result<()> {
    skip_nbt_depth(reader, tag, MAX_NESTING_DEPTH)
}

fn skip_nbt_depth<R: Read + ?Sized>(reader: &mut R, tag: Tag, depth: usize) -> Result<()> {
    if depth == 0 {
        return Err(Error::InvalidData("NBT nesting is too deep"));
    }
    match tag {
        Tag::End => Ok(()),
        Tag::Byte => reader.skip(1),
        Tag::Short => reader.skip(2),
        Tag::Int | Tag::Float => reader.skip(4),
        Tag::Long | Tag::Double => reader.skip(8),
        Tag::ByteArray => skip_sized(reader, 1),
        Tag::IntArray => skip_sized(reader, 4),
        Tag::LongArray => skip_sized(reader, 8),
        Tag::String => {
            let len = reader.read_u16_be()? as usize;
            reader.skip(len)
        }
        Tag::List => {
            let element = Tag::from_u8(reader.read_u8()?)?;
            let len = read_count(reader)?;
            if element == Tag::End {
                return if len == 0 {
                    Ok(())
                } else {
                    Err(Error::InvalidData(
                        "non-empty list with the `End` element tag",
                    ))
                };
            }
            match element {
                Tag::Byte => reader.skip(len),
                Tag::Short => reader.skip(checked_len(len, 2)?),
                Tag::Int | Tag::Float => reader.skip(checked_len(len, 4)?),
                Tag::Long | Tag::Double => reader.skip(checked_len(len, 8)?),
                _ => {
                    for _ in 0..len {
                        skip_nbt_depth(reader, element, depth - 1)?;
                    }
                    Ok(())
                }
            }
        }
        Tag::Compound => loop {
            let field = Tag::from_u8(reader.read_u8()?)?;
            if field == Tag::End {
                return Ok(());
            }
            let name_len = reader.read_u16_be()? as usize;
            reader.skip(name_len)?;
            skip_nbt_depth(reader, field, depth - 1)?;
        },
    }
}

fn skip_sized<R: Read + ?Sized>(reader: &mut R, size: usize) -> Result<()> {
    let len = read_count(reader)?;
    reader.skip(checked_len(len, size)?)
}

#[inline]
fn checked_len(len: usize, size: usize) -> Result<usize> {
    len.checked_mul(size).ok_or(Error::InvalidLength)
}

#[cfg(all(test, feature = "alloc"))]
mod tests {
    use crate::{ReadRef, SliceReader};
    #[cfg(feature = "alloc")]
    use alloc::vec::Vec;

    use super::*;

    #[test]
    fn strings_roundtrip() {
        for value in ["", "hello", "こんにちは"] {
            let mut buffer = Vec::new();
            write_string(&mut buffer, value).unwrap();
            let mut reader = SliceReader::new(&buffer);
            assert_eq!(read_string(&mut reader).unwrap(), value);
        }
    }

    #[test]
    fn names_are_read_as_bytes_and_long_ones_are_skipped() {
        // A name that fits is read as its raw bytes.
        let mut buffer = Vec::new();
        write_string(&mut buffer, "名前").unwrap();
        let mut reader = SliceReader::new(&buffer);
        let mut name = [0u8; 8];
        assert_eq!(read_name_into(&mut reader, &mut name), Ok(Some(6)));
        assert_eq!(&name[..6], "名前".as_bytes());

        // A name longer than the buffer is skipped, not decoded.
        let mut buffer = Vec::new();
        write_string(&mut buffer, "much longer").unwrap();
        let mut reader = SliceReader::new(&buffer);
        assert_eq!(read_name_into(&mut reader, &mut name), Ok(None));
        assert_eq!(reader.remaining(), 0);

        // A name that fits is still validated as modified UTF-8.
        let mut reader = SliceReader::new(&[0, 4, 0xf0, 0x9f, 0x98, 0x80]);
        assert_eq!(
            read_name_into(&mut reader, &mut name),
            Err(Error::InvalidUtf8)
        );
    }

    #[test]
    fn string_length_is_u16() {
        let mut buffer = Vec::new();
        let long = "a".repeat(usize::from(u16::MAX) + 1);
        assert_eq!(write_string(&mut buffer, &long), Err(Error::InvalidLength));
    }

    #[test]
    fn negative_counts_are_rejected() {
        let mut reader = SliceReader::new(&[0xff, 0xff, 0xff, 0xff]);
        assert_eq!(read_count(&mut reader), Err(Error::InvalidLength));
    }

    #[test]
    fn skip_walks_nested_payloads() {
        // A compound holding an int array, a list of strings, and a nested
        // compound; skipping must consume all of it.
        let mut writer = Vec::new();
        writer.write_i32_be(2); // int array length
        writer.write_i32_be(7);
        writer.write_i32_be(-7);
        writer.write_u8(Tag::String.as_u8()); // list element tag
        writer.write_i32_be(2); // list length
        write_string(&mut writer, "ab").unwrap();
        write_string(&mut writer, "c").unwrap();
        writer.write_u8(Tag::Byte.as_u8()); // nested compound: byte field ...
        writer.write_u16_be(3);
        writer.write_bytes(b"xyz");
        writer.write_u8(1);
        writer.write_u8(Tag::End.as_u8()); // ... terminated by End
        writer.write_bytes(b"trailer");

        let mut reader = SliceReader::new(&writer);

        skip_nbt(&mut reader, Tag::IntArray).unwrap();
        skip_nbt(&mut reader, Tag::List).unwrap();
        skip_nbt(&mut reader, Tag::Compound).unwrap();

        assert_eq!(reader.remaining(), 7);
        assert_eq!(reader.read_borrowed(7).unwrap(), b"trailer");
    }
}
