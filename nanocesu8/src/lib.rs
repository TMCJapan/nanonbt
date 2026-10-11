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
//! modified way with a vector loop, and deciding whether encoding can borrow
//! runs that loop as well — on the target's own vector where the compiler
//! already has it, and through `pulp`'s run-time dispatch where the widest
//! vector is a choice. What is accepted, borrowed and decoded does not
//! change.
//!
//! ```
//! use nanocesu8::Cesu8;
//!
//! let bytes = b"a\xc0\x80b";
//! let text = Cesu8::new(bytes).unwrap();
//! assert!(text.chars().eq("a\0b".chars()));
//! assert_eq!(text.as_bytes(), bytes);
//! ```
//!
//! With the `alloc` feature, conversion from/to `str` and owned buffers are
//! available:
//! ```
//! # #[cfg(feature = "alloc")]
//! # {
//! use nanocesu8::Cesu8;
//! let bytes = b"a\xc0\x80b";
//! let text = Cesu8::new(bytes).unwrap();
//! assert_eq!(text.chars().collect::<String>(), "a\0b");
//! assert_eq!(&*text.decode(), "a\0b");
//! let encoded = Cesu8::from_str("a\0b");
//! assert_eq!(encoded.as_bytes(), bytes);
//! # }
//! ```

#![no_std]

#[cfg(feature = "alloc")]
extern crate alloc;

mod borrowed;
mod error;
mod modified;
#[cfg(feature = "alloc")]
mod owned;
#[cfg(feature = "simd")]
mod simd;
mod utf8;

pub use borrowed::{Cesu8, Chars};
pub use error::DecodeError;
#[cfg(feature = "alloc")]
pub use owned::Cesu8Buf;
