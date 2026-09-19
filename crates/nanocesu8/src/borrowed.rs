//! The borrowed, validated [`Cesu8`] type and its iterator.

use alloc::borrow::{Cow, ToOwned};
use core::{fmt, hash::Hash, iter::FusedIterator, str};

use crate::{DecodeError, modified, owned::Cesu8Buf, utf8};

/// Modified UTF-8 bytes, validated once and borrowed from their owner.
///
/// `Cesu8` is to modified UTF-8 what [`str`] is to UTF-8: an unsized view
/// that is held behind a pointer, usually `&Cesu8`. It never copies or
/// allocates, so a string that is not UTF-8 — a NUL as `C0 80`, a non-BMP
/// character as a surrogate pair — still borrows from the input.
/// [`chars`](Cesu8::chars) decodes it in place and
/// [`decode`](Cesu8::decode) yields the text.
///
/// # Equality
///
/// Modified UTF-8 is not canonical: `00` and `C0 80` are different bytes
/// that decode to the same text. [`PartialEq`] compares the bytes as
/// written, so those two are *not* equal; compare
/// [`decode`](Cesu8::decode) for text equality.
#[repr(transparent)]
pub struct Cesu8([u8]);

impl Cesu8 {
    /// Validates `bytes` as modified UTF-8 and borrows them.
    ///
    /// Plain UTF-8 is accepted as is, raw NUL and four-byte sequences
    /// included. Anything else must be modified UTF-8 throughout.
    pub fn new(bytes: &[u8]) -> Result<&Self, DecodeError> {
        if utf8::to_str(bytes).is_none() {
            modified::validate(bytes)?;
        }
        // SAFETY: `utf8::to_str` or `modified::validate` just accepted the
        // bytes, and UTF-8 is a subset of modified UTF-8.
        Ok(unsafe { Self::from_bytes_unchecked(bytes) })
    }

    /// Borrows the bytes of `text` without copying.
    ///
    /// UTF-8 is a subset of modified UTF-8, so this cannot fail.
    pub const fn from_str(text: &str) -> &Self {
        // SAFETY: UTF-8 is a subset of modified UTF-8.
        unsafe { Self::from_bytes_unchecked(text.as_bytes()) }
    }

    /// Borrows `bytes` without checking that they are modified UTF-8.
    ///
    /// # Safety
    ///
    /// The bytes must be accepted by [`Cesu8::new`]. Breaking this is not
    /// undefined behavior — the type holds only bytes — but
    /// [`chars`](Cesu8::chars) and [`decode`](Cesu8::decode) then return
    /// wrong text.
    pub const unsafe fn from_bytes_unchecked(bytes: &[u8]) -> &Self {
        // SAFETY: `Cesu8` is `repr(transparent)` over `[u8]`, so the cast
        // keeps the address and the slice metadata, and the caller promises
        // the bytes are modified UTF-8.
        unsafe { &*(core::ptr::from_ref::<[u8]>(bytes) as *const Self) }
    }

    /// The bytes as written, without decoding them.
    pub const fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    pub const fn len(&self) -> usize {
        self.0.len()
    }

    pub const fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Iterates the characters without allocating.
    ///
    /// `C0 80` yields one NUL, and a surrogate pair yields one supplementary
    /// character.
    pub fn chars(&self) -> Chars<'_> {
        utf8::to_str(&self.0).map_or_else(
            || Chars(CharsInner::Modified(modified::Modified::new(&self.0))),
            |text| Chars(CharsInner::Utf8(text.chars())),
        )
    }

    /// Decodes into UTF-8, borrowing when the bytes already are UTF-8.
    pub fn decode(&self) -> Cow<'_, str> {
        utf8::to_str(&self.0).map_or_else(|| Cow::Owned(modified::decode(&self.0)), Cow::Borrowed)
    }
}

impl fmt::Display for Cesu8 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.decode())
    }
}

impl fmt::Debug for Cesu8 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Cesu8").field(&self.as_bytes()).finish()
    }
}

impl PartialEq for Cesu8 {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Eq for Cesu8 {}

impl PartialOrd for Cesu8 {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Cesu8 {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.0.cmp(&other.0)
    }
}

impl Hash for Cesu8 {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl PartialEq<[u8]> for Cesu8 {
    fn eq(&self, other: &[u8]) -> bool {
        self.as_bytes() == other
    }
}

impl PartialEq<Cesu8> for [u8] {
    fn eq(&self, other: &Cesu8) -> bool {
        self == other.as_bytes()
    }
}

impl AsRef<[u8]> for Cesu8 {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl<'a> From<&'a str> for &'a Cesu8 {
    fn from(text: &'a str) -> Self {
        Cesu8::from_str(text)
    }
}

impl ToOwned for Cesu8 {
    type Owned = Cesu8Buf;

    fn to_owned(&self) -> Cesu8Buf {
        Cesu8Buf::from_validated(self.0.to_vec())
    }
}

/// The characters of a [`Cesu8`], decoded without allocating.
#[derive(Clone, Debug)]
pub struct Chars<'a>(CharsInner<'a>);

#[derive(Clone, Debug)]
enum CharsInner<'a> {
    Utf8(str::Chars<'a>),
    Modified(modified::Modified<'a>),
}

impl Iterator for Chars<'_> {
    type Item = char;

    fn next(&mut self) -> Option<char> {
        match &mut self.0 {
            CharsInner::Utf8(chars) => chars.next(),
            CharsInner::Modified(cursor) => cursor.next_char().map_or_else(
                |_| {
                    // Unreachable: a `Cesu8` was validated when it was made.
                    // Ending the iteration keeps this iterator fused instead
                    // of panicking.
                    *cursor = modified::Modified::new(&[]);
                    None
                },
                Some,
            ),
        }
    }
}

impl FusedIterator for Chars<'_> {}
