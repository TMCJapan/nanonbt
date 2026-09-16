//! Java's "modified UTF-8", the string encoding of NBT.
//!
//! It differs from UTF-8 in two ways: NUL is written as the two bytes
//! `C0 80`, and characters outside the Basic Multilingual Plane are written
//! as a UTF-16 surrogate pair, each half encoded as three bytes.

use alloc::{borrow::Cow, string::String, vec::Vec};
use core::fmt;

/// The bytes are neither UTF-8 nor modified UTF-8.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecodeError;

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("invalid modified UTF-8")
    }
}

impl core::error::Error for DecodeError {}

/// Decodes `bytes`, borrowing them when they are already UTF-8.
///
/// Plain UTF-8 is accepted as is, raw NUL and four-byte sequences included.
/// Anything else must be modified UTF-8 throughout.
pub fn from_java_cesu8(bytes: &[u8]) -> Result<Cow<'_, str>, DecodeError> {
    if let Ok(text) = core::str::from_utf8(bytes) {
        return Ok(Cow::Borrowed(text));
    }
    let mut out = Vec::with_capacity(bytes.len());
    let mut rest = bytes;
    while let Some((&first, tail)) = rest.split_first() {
        rest = tail;
        // NUL must be written as C0 80, and the rest of 80..FF cannot lead.
        match first {
            0x01..=0x7f => out.push(first),
            0xc0 => {
                expect(&mut rest, |b| b == 0x80)?;
                out.push(0);
            }
            0xc2..=0xdf => {
                let second = continuation(&mut rest)?;
                out.extend_from_slice(&[first, second]);
            }
            0xe0..=0xef => {
                let second = continuation(&mut rest)?;
                let third = continuation(&mut rest)?;
                match (first, second) {
                    (0xe0, 0xa0..=0xbf)
                    | (0xe1..=0xec | 0xee..=0xef, 0x80..=0xbf)
                    | (0xed, 0x80..=0x9f) => out.extend_from_slice(&[first, second, third]),
                    (0xed, 0xa0..=0xaf) => {
                        expect(&mut rest, |b| b == 0xed)?;
                        let fifth = continuation(&mut rest)?;
                        if !(0xb0..=0xbf).contains(&fifth) {
                            return Err(DecodeError);
                        }
                        let sixth = continuation(&mut rest)?;
                        let high = decode_surrogate(second, third);
                        let low = decode_surrogate(fifth, sixth);
                        let c = 0x1_0000 + (((high - 0xd800) << 10) | (low - 0xdc00));
                        let c = char::from_u32(c).ok_or(DecodeError)?;
                        out.extend_from_slice(c.encode_utf8(&mut [0; 4]).as_bytes());
                    }
                    _ => return Err(DecodeError),
                }
            }
            _ => return Err(DecodeError),
        }
    }
    String::from_utf8(out)
        .map(Cow::Owned)
        .map_err(|_| DecodeError)
}

fn expect(rest: &mut &[u8], accept: impl Fn(u8) -> bool) -> Result<u8, DecodeError> {
    match rest.split_first() {
        Some((&b, tail)) if accept(b) => {
            *rest = tail;
            Ok(b)
        }
        _ => Err(DecodeError),
    }
}

fn continuation(rest: &mut &[u8]) -> Result<u8, DecodeError> {
    expect(rest, |b| b & 0xc0 == 0x80)
}

fn decode_surrogate(second: u8, third: u8) -> u32 {
    0xd000 | (u32::from(second & 0x3f) << 6) | u32::from(third & 0x3f)
}

/// Encodes `text`, borrowing it when it is already valid modified UTF-8.
pub fn to_java_cesu8(text: &str) -> Cow<'_, [u8]> {
    if text.bytes().all(|b| b != 0 && b < 0xf0) {
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

/// The three-byte form of a lone surrogate code unit.
const fn encode_surrogate(unit: u16) -> [u8; 3] {
    [
        0xe0 | (unit >> 12) as u8,
        0x80 | ((unit >> 6) & 0x3f) as u8,
        0x80 | (unit & 0x3f) as u8,
    ]
}
