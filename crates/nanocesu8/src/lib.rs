//! Java's modified UTF-8 (CESU-8), zero-copy.
//!
//! This is the string encoding of NBT and of Java's `DataInput`. It differs
//! from UTF-8 in two ways: NUL is written as the two bytes `C0 80`, and
//! characters outside the Basic Multilingual Plane are written as a UTF-16
//! surrogate pair, each half encoded as three bytes.
//!
//! [`Cesu8`] is a validated, unsized view over such bytes, like [`str`] over
//! UTF-8, and [`Cesu8Buf`] is its owned counterpart. They borrow: a string
//! that is not UTF-8 still needs no allocation to be held or iterated.
//! [`from_java_cesu8`] and [`to_java_cesu8`] are the buffer-in, buffer-out
//! pair for callers that want the text or the canonical bytes.
//!
//! Plain UTF-8 is accepted as is, raw NUL and four-byte sequences included,
//! and so are `C0 80` and surrogate pairs. [`to_java_cesu8`] always writes
//! the canonical modified UTF-8 spelling, so bytes that mixed the two forms
//! do not round trip byte for byte.
//!
//! The `simd` feature vectorizes the two hot scans: deciding whether bytes
//! are UTF-8 runs through `simdutf8`, and deciding whether encoding can
//! borrow runs 32 bytes at a time through the `wide` crate. What is
//! accepted, borrowed and decoded does not change.
//!
//! ```
//! use nanocesu8::Cesu8;
//!
//! let bytes = b"a\xc0\x80b";
//! let text = Cesu8::new(bytes).unwrap();
//! assert_eq!(text.chars().collect::<String>(), "a\0b");
//! assert_eq!(&*text.decode(), "a\0b");
//! // Not UTF-8, so the text had to be decoded, but the bytes were borrowed.
//! assert_eq!(text.as_bytes(), bytes);
//! ```

#![no_std]

extern crate alloc;

mod borrowed;
mod error;
mod modified;
mod owned;
#[cfg(feature = "simd")]
mod simd;
mod utf8;

use alloc::borrow::Cow;

pub use borrowed::{Cesu8, Chars};
pub use error::DecodeError;
pub use owned::Cesu8Buf;

/// Decodes `bytes`, borrowing them when they are already UTF-8.
///
/// Plain UTF-8 is accepted as is, raw NUL and four-byte sequences included.
/// Anything else must be modified UTF-8 throughout.
pub fn from_java_cesu8(bytes: &[u8]) -> Result<Cow<'_, str>, DecodeError> {
    if let Some(text) = utf8::to_str(bytes) {
        return Ok(Cow::Borrowed(text));
    }
    modified::validate(bytes)?;
    Ok(Cow::Owned(modified::decode(bytes)))
}

/// Encodes `text`, borrowing it when it is already valid modified UTF-8.
pub fn to_java_cesu8(text: &str) -> Cow<'_, [u8]> {
    modified::encode(text)
}
