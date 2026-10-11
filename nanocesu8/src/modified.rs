//! The modified UTF-8 grammar, shared by validation, decoding and iteration.

use alloc::{borrow::Cow, string::String, vec::Vec};

use crate::DecodeError;

/// A cursor over modified UTF-8 bytes.
///
/// The grammar is the one Java writes: `0x01..=0x7f` as itself, NUL as
/// `C0 80`, two- and three-byte sequences as in UTF-8 but without overlong
/// forms or lone surrogates, and a supplementary character as a UTF-16
/// surrogate pair in two three-byte sequences.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Modified<'a> {
    rest: &'a [u8],
}

impl<'a> Modified<'a> {
    pub(crate) const fn new(bytes: &'a [u8]) -> Self {
        Self { rest: bytes }
    }

    pub(crate) const fn is_empty(&self) -> bool {
        self.rest.is_empty()
    }

    /// Decodes the next character, or refuses the bytes.
    pub(crate) fn next_char(&mut self) -> Result<char, DecodeError> {
        let (&first, tail) = self.rest.split_first().ok_or(DecodeError)?;
        self.rest = tail;
        match first {
            0x01..=0x7f => Ok(char::from(first)),
            0xc0 => {
                self.expect(0x80)?;
                Ok('\0')
            }
            0xc2..=0xdf => {
                let second = self.continuation()?;
                let code = (u32::from(first & 0x1f) << 6) | u32::from(second & 0x3f);
                char::from_u32(code).ok_or(DecodeError)
            }
            0xe0..=0xef => {
                let second = self.continuation()?;
                let third = self.continuation()?;
                match (first, second) {
                    (0xe0, 0xa0..=0xbf)
                    | (0xe1..=0xec | 0xee..=0xef, 0x80..=0xbf)
                    | (0xed, 0x80..=0x9f) => {
                        let code = (u32::from(first & 0x0f) << 12)
                            | (u32::from(second & 0x3f) << 6)
                            | u32::from(third & 0x3f);
                        char::from_u32(code).ok_or(DecodeError)
                    }
                    (0xed, 0xa0..=0xaf) => {
                        self.expect(0xed)?;
                        let fifth = self.continuation()?;
                        if !(0xb0..=0xbf).contains(&fifth) {
                            return Err(DecodeError);
                        }
                        let sixth = self.continuation()?;
                        let high = decode_surrogate(second, third);
                        let low = decode_surrogate(fifth, sixth);
                        let code = 0x1_0000 + (((high - 0xd800) << 10) | (low - 0xdc00));
                        char::from_u32(code).ok_or(DecodeError)
                    }
                    _ => Err(DecodeError),
                }
            }
            _ => Err(DecodeError),
        }
    }

    const fn expect(&mut self, byte: u8) -> Result<(), DecodeError> {
        match self.rest.split_first() {
            Some((&b, tail)) if b == byte => {
                self.rest = tail;
                Ok(())
            }
            _ => Err(DecodeError),
        }
    }

    const fn continuation(&mut self) -> Result<u8, DecodeError> {
        match self.rest.split_first() {
            Some((&b, tail)) if b & 0xc0 == 0x80 => {
                self.rest = tail;
                Ok(b)
            }
            _ => Err(DecodeError),
        }
    }
}

/// A surrogate code unit from the two continuation bytes of its sequence.
fn decode_surrogate(second: u8, third: u8) -> u32 {
    0xd000 | (u32::from(second & 0x3f) << 6) | u32::from(third & 0x3f)
}

/// Whether `bytes` are accepted by the modified UTF-8 grammar.
pub(crate) fn validate(bytes: &[u8]) -> Result<(), DecodeError> {
    let mut cursor = Modified::new(bytes);
    while !cursor.is_empty() {
        cursor.next_char()?;
    }
    Ok(())
}

/// Decodes modified UTF-8 into a `String`.
///
/// Only called with bytes [`validate`] accepted, where the error branch is
/// unreachable; on malformed input it stops rather than panicking.
pub(crate) fn decode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len());
    let mut cursor = Modified::new(bytes);
    while !cursor.is_empty() {
        match cursor.next_char() {
            Ok(c) => out.push(c),
            Err(_) => break,
        }
    }
    out
}

/// Encodes `text`, borrowing it when it is already valid modified UTF-8.
pub(crate) fn encode(text: &str) -> Cow<'_, [u8]> {
    if !needs_encoding(text.as_bytes()) {
        return Cow::Borrowed(text.as_bytes());
    }
    // NUL doubles and a four-byte sequence grows by half, so twice the
    // length is the exact bound and never has to grow.
    let mut out = Vec::with_capacity(text.len() * 2);
    for c in text.chars() {
        match c {
            '\0' => out.extend_from_slice(&[0xc0, 0x80]),
            c if c.len_utf16() == 2 => {
                let mut units = [0; 2];
                c.encode_utf16(&mut units);
                for unit in units {
                    out.extend_from_slice(&encode_surrogate(unit));
                }
            }
            c => out.extend_from_slice(c.encode_utf8(&mut [0; 4]).as_bytes()),
        }
    }
    Cow::Owned(out)
}

/// Whether a `str`'s UTF-8 bytes are not already modified UTF-8: only a NUL,
/// which must become `C0 80`, or a four-byte sequence, which must become a
/// surrogate pair, force a copy. [`Cesu8::new`] must not accept such bytes
/// as they are; `encode` rewrites them instead.
///
/// [`Cesu8::new`]: crate::Cesu8::new
#[cfg(feature = "simd")]
pub(crate) fn needs_encoding(bytes: &[u8]) -> bool {
    crate::simd::contains_null_or_utf8_4_byte_char_header(bytes)
}

/// Whether a `str`'s UTF-8 bytes are not already modified UTF-8: only a NUL,
/// which must become `C0 80`, or a four-byte sequence, which must become a
/// surrogate pair, force a copy. [`Cesu8::new`] must not accept such bytes
/// as they are; `encode` rewrites them instead.
///
/// [`Cesu8::new`]: crate::Cesu8::new
#[cfg(not(feature = "simd"))]
pub(crate) fn needs_encoding(bytes: &[u8]) -> bool {
    bytes.iter().any(|&byte| byte == 0 || byte >= 0xf0)
}

/// The three-byte form of a lone surrogate code unit.
const fn encode_surrogate(unit: u16) -> [u8; 3] {
    [
        0xe0 | (unit >> 12) as u8,
        0x80 | ((unit >> 6) & 0x3f) as u8,
        0x80 | (unit & 0x3f) as u8,
    ]
}
