//! Java's modified UTF-8 (CESU-8), zero-copy.
//!
//! This is the string encoding of NBT and of Java's `DataInput`. It differs
//! from UTF-8 in two ways: NUL is written as the two bytes `C0 80`, and
//! characters outside the Basic Multilingual Plane are written as a UTF-16
//! surrogate pair, each half encoded as three bytes.
//!
//! [`Cesu8`] is a validated, unsized view over such bytes, like [`str`](prim@str) over
//! UTF-8, and [`Cesu8Buf`] is its owned counterpart. They borrow: a string
//! that is not UTF-8 still needs no allocation to be held or iterated.
//! [`Cesu8::from_str`] and [`Cesu8::decode`] convert between the two, in
//! either direction and borrowing when the text is already spelled the way
//! the target wants it.
//!
//! Java's spelling is the only one accepted: a raw NUL, a four-byte
//! sequence, an overlong form or a lone surrogate is refused. A character
//! then has one spelling only, so a [`Cesu8`]'s bytes decode to text and
//! encode back to exactly the same bytes.
//!
//! The `simd` feature vectorizes the two hot scans: the accept fast path
//! checks UTF-8 with `simdutf8` and looks for what UTF-8 does not spell the
//! modified way with `wide`, and deciding whether encoding can borrow runs
//! 32 bytes at a time through `wide` as well. What is accepted, borrowed and
//! decoded does not change.
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
//!
//! // The other direction: a NUL forces the modified spelling.
//! let encoded = Cesu8::from_str("a\0b");
//! assert_eq!(encoded.as_bytes(), bytes);
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

/// Decodes modified UTF-8, borrowing when the bytes are already UTF-8.
///
/// Only called with bytes [`Cesu8::new`] accepted, where decoding cannot
/// fail; on any other input it stops at the first bad byte rather than
/// panicking.
pub(crate) fn from_java_cesu8(bytes: &[u8]) -> Cow<'_, str> {
    utf8::to_str(bytes).map_or_else(|| Cow::Owned(modified::decode(bytes)), Cow::Borrowed)
}

/// Encodes `text`, borrowing it when it is already valid modified UTF-8.
pub(crate) fn to_java_cesu8(text: &str) -> Cow<'_, [u8]> {
    modified::encode(text)
}
