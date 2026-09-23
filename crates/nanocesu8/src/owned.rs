//! The owned, growable counterpart of [`Cesu8`].

use alloc::{borrow::Cow, string::String, vec::Vec};
use core::{borrow::Borrow, fmt, ops::Deref};

use crate::{Cesu8, DecodeError, to_java_cesu8};

/// Owned modified UTF-8 bytes, the [`String`] to [`Cesu8`]'s [`str`](prim@str).
///
/// The bytes are always accepted by [`Cesu8::new`]. Unlike [`String`] they
/// need not be UTF-8: they may spell NUL as `C0 80` or a non-BMP character
/// as a surrogate pair, which is how a document's string is kept exactly as
/// it was read. [`From<&str>`](Self::from) and [`push_str`](Self::push_str)
/// encode the modified spelling.
///
/// Equality is byte equality, as for [`Cesu8`].
#[derive(Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Cesu8Buf(Vec<u8>);

impl Cesu8Buf {
    pub const fn new() -> Self {
        Self(Vec::new())
    }

    /// Validates `bytes` and copies them.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, DecodeError> {
        Self::from_vec(bytes.to_vec())
    }

    /// Validates `bytes` without copying.
    pub fn from_vec(bytes: Vec<u8>) -> Result<Self, DecodeError> {
        Cesu8::new(&bytes)?;
        Ok(Self(bytes))
    }

    /// Appends `text`, encoding it as modified UTF-8.
    pub fn push_str(&mut self, text: &str) {
        self.0.extend_from_slice(&to_java_cesu8(text));
    }

    /// The bytes as written, without decoding them.
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// Borrows the bytes as a validated [`Cesu8`].
    pub fn as_cesu8(&self) -> &Cesu8 {
        // SAFETY: the buffer was validated when it was made, and every
        // method that changes it keeps it valid.
        unsafe { Cesu8::from_bytes_unchecked(&self.0) }
    }

    /// The bytes, giving up validation.
    pub fn into_bytes(self) -> Vec<u8> {
        self.0
    }

    /// Wraps bytes that are already known to be valid.
    pub(crate) const fn from_validated(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }
}

impl Deref for Cesu8Buf {
    type Target = Cesu8;

    fn deref(&self) -> &Cesu8 {
        self.as_cesu8()
    }
}

impl Borrow<Cesu8> for Cesu8Buf {
    fn borrow(&self) -> &Cesu8 {
        self.as_cesu8()
    }
}

impl AsRef<Cesu8> for Cesu8Buf {
    fn as_ref(&self) -> &Cesu8 {
        self.as_cesu8()
    }
}

impl AsRef<[u8]> for Cesu8Buf {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl From<&str> for Cesu8Buf {
    /// Encodes `text` as modified UTF-8.
    fn from(text: &str) -> Self {
        Self::from_validated(to_java_cesu8(text).into_owned())
    }
}

impl From<String> for Cesu8Buf {
    /// Encodes `text` as modified UTF-8, keeping its bytes when they are
    /// already spelled that way.
    fn from(text: String) -> Self {
        match to_java_cesu8(&text) {
            Cow::Borrowed(_) => Self::from_validated(text.into_bytes()),
            Cow::Owned(bytes) => Self::from_validated(bytes),
        }
    }
}

impl From<Cesu8Buf> for Vec<u8> {
    fn from(buf: Cesu8Buf) -> Self {
        buf.into_bytes()
    }
}

impl fmt::Display for Cesu8Buf {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&**self, f)
    }
}

impl fmt::Debug for Cesu8Buf {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Cesu8Buf").field(&self.0).finish()
    }
}

impl PartialEq<Cesu8> for Cesu8Buf {
    fn eq(&self, other: &Cesu8) -> bool {
        self.as_cesu8() == other
    }
}

impl PartialEq<Cesu8Buf> for Cesu8 {
    fn eq(&self, other: &Cesu8Buf) -> bool {
        self == other.as_cesu8()
    }
}
