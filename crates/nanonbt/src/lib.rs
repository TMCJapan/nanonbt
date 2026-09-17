//! `nanonbt`: serde support for Minecraft's NBT format, without `std`.
//!
//! The encoding is byte-for-byte the one [fastnbt](https://docs.rs/fastnbt)
//! 2.6 produces and accepts, so that the two crates can be used
//! interchangeably: [`to_bytes`], [`from_bytes`], [`to_value`] and
//! [`from_value`] succeed exactly where fastnbt's do, with the same bytes or
//! values, and the array types use fastnbt's serde tokens, so either crate's
//! [`ByteArray`], [`IntArray`] and [`LongArray`] work with the other.
//! Error messages are not part of that promise.
//!
//! Agreement is checked two ways. `crates/nanonbt-kani` proves it with a
//! model checker, for fixed shapes with arbitrary values. This crate's
//! `differential` test suite checks it on seeded random documents, trees and
//! mutations, which is what covers malformed input, string contents,
//! compounds of several entries and lists inside a [`Value`] — the shapes
//! the proofs cannot reach. Of the `cesu8` module only the decoder is
//! proven, and only one byte at a time, which reaches no multi-byte form;
//! the encoder is not proven at all, so there the random tests carry the
//! whole weight.
//!
//! # Where fastnbt is not followed
//!
//! - Where fastnbt panics, this returns an error: skipping a list of End
//!   tags that has elements, and [`to_value`] of `None`, units, newtype
//!   variants or a malformed array wrapper.
//! - Documents nested deeper than [`DeOpts::max_depth`], 512 by default, are
//!   refused. Reading is recursive, so fastnbt instead overflows the stack
//!   and aborts, which no error can report and no `catch_unwind` can catch.
//! - Strings longer than 65535 bytes are refused; fastnbt truncates their
//!   length and writes corrupt NBT.
//! - [`Value::Compound`] is ordered by key, fastnbt's by hash, so compounds
//!   of several entries serialize in a different order. For the same reason
//!   [`from_value`] visits entries in key order, which matters only when one
//!   of several keys is an array token.
//! - [`from_value`] presents an array as a map of one entry; fastnbt's never
//!   runs out of entries.
//! - There is no `from_reader` or `to_writer`, as there is no `std::io`.
//!
//! # Where the two readers disagree
//!
//! [`from_bytes`] and [`from_value`] follow fastnbt separately, and fastnbt's
//! own two paths do not always agree, so neither does this:
//!
//! - A fixed-size target shorter than the list it reads — a tuple or an
//!   array — leaves the elements it did not take unread, and the next field
//!   is then read out of the list's payload rather than the compound. A
//!   document that claims a list of 11 and is deserialized into a 2-tuple
//!   therefore yields a following field built from bytes inside that list.
//!   [`from_value`] refuses the same tree as too long. Do not read untrusted
//!   documents into fixed-size targets; use [`alloc::vec::Vec`] or
//!   [`Value`], both of which consume the list.
//! - A `char` round trips through [`to_value`] and [`from_value`] but not
//!   through [`to_bytes`] and [`from_bytes`]: it is written as `TAG_Int` and
//!   read back as an integer, which serde's `char` visitor refuses. Bytes
//!   are the mirror image — a `serde_bytes` field reads from a byte array
//!   through [`from_bytes`] but not through [`from_value`].

#![no_std]

extern crate alloc;

mod arrays;
pub mod cesu8;
pub mod de;
pub mod error;
pub mod ser;
mod tag;
pub mod value;

use alloc::{string::String, vec::Vec};

use serde::{Deserialize, Serialize};

pub use arrays::{ByteArray, IntArray, LongArray};
pub use error::{Error, Result};
pub use value::{Value, from_value, to_value};

/// Serializes `value` as the root compound, with an empty name.
pub fn to_bytes<T: Serialize + ?Sized>(value: &T) -> Result<Vec<u8>> {
    to_bytes_with_opts(value, SerOpts::default())
}

/// Serializes `value` as the root compound, named as `opts` says.
pub fn to_bytes_with_opts<T: Serialize + ?Sized>(value: &T, opts: SerOpts) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    let root_name = opts.serialize_root_name.then_some(opts.root_name);
    value.serialize(&mut ser::Serializer::new(&mut out, root_name))?;
    Ok(out)
}

/// Options for serialization.
#[derive(Debug, Clone)]
pub struct SerOpts {
    root_name: String,
    serialize_root_name: bool,
}

impl Default for SerOpts {
    fn default() -> Self {
        Self::new()
    }
}

impl SerOpts {
    /// `const`, so that the builders below can be used in a `const` too.
    pub const fn new() -> Self {
        Self {
            root_name: String::new(),
            serialize_root_name: true,
        }
    }

    /// "Network NBT": the root compound has no name at all.
    pub const fn network_nbt() -> Self {
        Self::new().serialize_root_compound_name(false)
    }

    #[must_use]
    pub const fn serialize_root_compound_name(mut self, serialize_root_name: bool) -> Self {
        self.serialize_root_name = serialize_root_name;
        self
    }

    /// Names the root compound, which also turns its name on.
    #[must_use]
    pub fn root_name(mut self, root_name: impl Into<String>) -> Self {
        self.root_name = root_name.into();
        self.serialize_root_name = true;
        self
    }
}

/// Options for deserialization.
#[derive(Debug, Clone)]
pub struct DeOpts {
    max_seq_len: usize,
    max_depth: usize,
    expect_compound_names: bool,
}

impl Default for DeOpts {
    fn default() -> Self {
        Self::new()
    }
}

impl DeOpts {
    /// `const`, so that the builders below can be used in a `const` too.
    pub const fn new() -> Self {
        Self {
            max_seq_len: 100_000_000,
            max_depth: 512,
            expect_compound_names: true,
        }
    }

    /// "Network NBT": the root compound has no name at all.
    pub const fn network_nbt() -> Self {
        Self::new().expect_compound_names(false)
    }

    /// The longest list or array accepted.
    #[must_use]
    pub const fn max_seq_len(mut self, max_seq_len: usize) -> Self {
        self.max_seq_len = max_seq_len;
        self
    }

    /// The deepest nesting of lists and compounds accepted.
    ///
    /// Reading a document recurses once per level, so a bound is what keeps
    /// untrusted input from overflowing the stack; the default 512 is the
    /// depth Minecraft itself accepts, and fits a stack of about half a
    /// megabyte. A smaller stack needs a smaller bound: measured here, a
    /// level costs a few hundred bytes, so 64 KiB holds about 70 of them.
    ///
    /// This bounds reading only. [`to_bytes`], [`to_value`], [`from_value`]
    /// and dropping a [`Value`] all recurse once per level as well, with no
    /// bound of their own, so a tree deeper than this that was *not* built by
    /// [`from_bytes`] still overflows the stack and aborts. Reading costs the
    /// most stack per level, so a [`from_bytes`] then [`to_bytes`] round trip
    /// within this bound is safe.
    #[must_use]
    pub const fn max_depth(mut self, max_depth: usize) -> Self {
        self.max_depth = max_depth;
        self
    }

    /// Whether the root compound has a name to skip.
    #[must_use]
    pub const fn expect_compound_names(mut self, expect_compound_names: bool) -> Self {
        self.expect_compound_names = expect_compound_names;
        self
    }
}

/// Deserializes a `T` from a document whose root is a compound.
pub fn from_bytes<'de, T: Deserialize<'de>>(input: &'de [u8]) -> Result<T> {
    from_bytes_with_opts(input, DeOpts::default())
}

/// Deserializes a `T` from a document whose root is a compound.
pub fn from_bytes_with_opts<'de, T: Deserialize<'de>>(input: &'de [u8], opts: DeOpts) -> Result<T> {
    T::deserialize(&mut de::Deserializer::from_bytes(input, opts))
}
