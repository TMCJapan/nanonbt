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
//! the proofs cannot reach. The `cesu8` module is proven only one byte at a
//! time, so there too the random tests carry the weight.
//!
//! # Where fastnbt is not followed
//!
//! - Where fastnbt panics, this returns an error: skipping a list of End
//!   tags that has elements, and [`to_value`] of `None`, units, newtype
//!   variants or a malformed array wrapper.
//! - Strings longer than 65535 bytes are refused; fastnbt truncates their
//!   length and writes corrupt NBT.
//! - [`Value::Compound`] is ordered by key, fastnbt's by hash, so compounds
//!   of several entries serialize in a different order. For the same reason
//!   [`from_value`] visits entries in key order, which matters only when one
//!   of several keys is an array token.
//! - [`from_value`] presents an array as a map of one entry; fastnbt's never
//!   runs out of entries.
//! - There is no `from_reader` or `to_writer`, as there is no `std::io`.

#![no_std]

extern crate alloc;

mod arrays;
pub mod cesu8;
pub mod de;
pub mod error;
pub mod ser;
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
        Self {
            root_name: String::new(),
            serialize_root_name: true,
        }
    }
}

impl SerOpts {
    pub fn new() -> Self {
        Self::default()
    }

    /// "Network NBT": the root compound has no name at all.
    pub fn network_nbt() -> Self {
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
    expect_compound_names: bool,
}

impl Default for DeOpts {
    fn default() -> Self {
        Self {
            max_seq_len: 100_000_000,
            expect_compound_names: true,
        }
    }
}

impl DeOpts {
    pub fn new() -> Self {
        Self::default()
    }

    /// "Network NBT": the root compound has no name at all.
    pub fn network_nbt() -> Self {
        Self::new().expect_compound_names(false)
    }

    /// The longest list or array accepted.
    #[must_use]
    pub const fn max_seq_len(mut self, max_seq_len: usize) -> Self {
        self.max_seq_len = max_seq_len;
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
