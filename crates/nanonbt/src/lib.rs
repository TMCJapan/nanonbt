//! `nanonbt`: Minecraft's NBT format, without `std` and without serde.
//!
//! Values implement [`ToNBT`] and [`FromNBT`], and the derive macros write
//! those implementations for structs, newtypes and unit enums:
//!
//! ```
//! use nanonbt::{FromNBT, ToNBT};
//!
//! #[derive(FromNBT, ToNBT, PartialEq, Debug)]
//! struct Player {
//!     health: i32,
//!     #[nbt(rename = "Name")]
//!     name: String,
//!     #[nbt(ignore)]
//!     cached: u64,
//!     #[nbt(array = "int")]
//!     uuid: Vec<u32>,
//! }
//!
//! let player = Player { health: 20, name: "topi".into(), cached: 0, uuid: vec![1, 2] };
//! let bytes = nanonbt::to_bytes(&player).unwrap();
//! let back: Player = nanonbt::from_bytes(&bytes).unwrap();
//! assert_eq!(back, player);
//! ```
//!
//! The encoding is byte-for-byte the one [fastnbt](https://docs.rs/fastnbt)
//! 2.6 produces for the same values, and the array types are fastnbt's. With
//! the `serde` feature, the old `Serialize`/`Deserialize` implementations are
//! kept as well, and `serde_compat` holds their entry points. The `simd`
//! feature settles array and numeric-list byte order a vector at a time, on
//! x86, aarch64 and wasm, and stays `no_std`.
//!
//! # The data model
//!
//! Tags are matched strictly: a value reads its own tag only, so a `TAG_Int`
//! does not become an `i64`, and a float tag does not become an integer. An
//! unsigned integer shares its tag with the signed integer of the same width
//! and reinterprets the bits. A [`char`] is a `TAG_Int`, an `i128` or `u128`
//! a `TAG_Int_Array` of four ints, and a string a length-prefixed modified
//! UTF-8 (Java CESU-8) `TAG_String`.
//!
//! [`Option`] has no tag of its own: in a derived struct a `None` field is
//! left out of the compound and an absent entry reads back as `None`. It
//! cannot appear in a list. [`Vec`], arrays and slices are lists; the empty
//! list is written as a list of End, the way fastnbt writes one.
//!
//! A field with `#[nbt(array = "byte")]`, `"int"` or `"long"` writes a
//! `Vec<T>`, `[T; N]` or `&[T]` as the NBT array of that kind instead of a
//! list, and reads one back; `T` is `i8` or `u8`, `i32` or `u32`, `i64` or
//! `u64` to match the kind. The elements go straight to and from the
//! document — [`ArrayOf`] is the trait behind it — so nothing is copied into
//! an array type first. A borrowed `&[T]` reads through its own
//! [`FromNBT`], which byte slices have and wider integers do not, their
//! bytes being big-endian: use `Vec<T>` or `[T; N]` to read those.
//!
//! Strings borrow: [`FromNBT`] is implemented for `&'de str`,
//! `Cow<'de, str>`, `&'de Cesu8`, [`Cesu8Buf`] and `Cow<'de, Cesu8>`. A
//! string that needs no decoding borrows from the input; one written in
//! modified UTF-8 (a NUL or a non-BMP character) decodes into an owned
//! `Cow<str>`, and reading it into a `&'de str` is an error. A [`Cesu8`]
//! keeps the bytes as they were written instead, so even that string
//! borrows; [`Cesu8::decode`] yields the text on demand.
//!
//! Numbers and byte arrays borrow too. [`U64Be`] is a `TAG_Long` kept as the
//! eight bytes NBT wrote, and `&'de [U64Be]` a whole long array; both read
//! without copying, and [`U64Be::get`] decodes one value. The other widths
//! have the same wrappers, `I16Be` through `F64Be`, and bytes need none:
//! `&'de u8`, `&'de i8`, `&'de [u8]` and `&'de [i8]` borrow a `TAG_Byte` or a
//! `TAG_Byte_Array` as it is.
//!
//! A borrowed slice reads an array's tag or a list of the same element, so a
//! `&'de [U64Be]` reads a `TAG_Long_Array` or a `TAG_List` of `TAG_Long`.
//! Writing one writes a list, not an array: [`LongArray`], [`IntArray`] and
//! [`ByteArray`] write arrays, and so does a field with `#[nbt(array = ...)]`.
//! `from_value` cannot lend bytes, so borrowed numbers and byte arrays are
//! errors there, as a `&'de str` is for a string that needs decoding.
//!
//! # Where fastnbt is not followed
//!
//! - Where fastnbt panics, this returns an error: skipping a list of End
//!   tags that has elements.
//! - Documents nested deeper than [`DeOpts::max_depth`], 512 by default, are
//!   refused. Reading is recursive, so fastnbt instead overflows the stack
//!   and aborts, which no error can report and no `catch_unwind` can catch.
//! - Strings longer than 65535 bytes are refused; fastnbt truncates their
//!   length and writes corrupt NBT.
//! - Values convert between tags only where fastnbt's serde visitors happen
//!   to; this crate's rule is simpler and stricter, and `char` round trips
//!   through bytes, which fastnbt's does not.
//! - A borrowed slice such as `&'de [U64Be]` writes as a list, where
//!   fastnbt's `borrow::LongArray` writes a long array. [`LongArray`], the
//!   other array types, and derived fields with `#[nbt(array = ...)]` are the
//!   ones that write arrays.
//! - There is no `from_reader` or `to_writer`, as there is no `std::io`.

#![no_std]

extern crate alloc;

mod arrays;
mod be;
#[cfg(feature = "serde")]
pub mod de;
pub mod error;
mod impls;
pub mod read;
#[cfg(feature = "serde")]
pub mod ser;
#[cfg(feature = "simd")]
mod simd;
mod tag;
pub mod write;

use alloc::{string::String, vec::Vec};

pub use arrays::{ArrayOf, ByteArray, IntArray, LongArray};
pub use be::{F32Be, F64Be, I16Be, I32Be, I64Be, U16Be, U32Be, U64Be};
pub use error::{Error, Result};
pub use nanocesu8::{Cesu8, Cesu8Buf};
#[cfg(feature = "derive")]
pub use nanonbt_derive::{FromNBT, ToNBT};
pub use read::{FromNBT, Read, Reader};
pub use tag::{
    TAG_BYTE, TAG_BYTE_ARRAY, TAG_COMPOUND, TAG_DOUBLE, TAG_END, TAG_FLOAT, TAG_INT, TAG_INT_ARRAY,
    TAG_LIST, TAG_LONG, TAG_LONG_ARRAY, TAG_SHORT, TAG_STRING,
};
pub use write::{ToNBT, Write, Writer};

/// Serializes `value` as the root compound, with an empty name.
pub fn to_bytes<T: ToNBT + ?Sized>(value: &T) -> Result<Vec<u8>> {
    to_bytes_with_opts(value, SerOpts::default())
}

/// Serializes `value` as the root compound, named as `opts` says.
pub fn to_bytes_with_opts<T: ToNBT + ?Sized>(value: &T, opts: SerOpts) -> Result<Vec<u8>> {
    if const { T::TAG != TAG_COMPOUND } {
        return Err(Error::no_root_compound());
    }
    let SerOpts {
        root_name,
        serialize_root_name,
    } = opts;
    let mut out = Vec::new();
    let mut writer = Writer::new(&mut out);
    writer.write_tag(TAG_COMPOUND)?;
    if serialize_root_name {
        writer.write_name(&root_name)?;
    }
    value.write(&mut writer)?;
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
    /// This bounds reading only. [`to_bytes`] recurses once per level as
    /// well, with no bound of its own, so a tree deeper than this that was
    /// *not* built by [`from_bytes`] still overflows the stack and aborts.
    /// Reading costs the most stack per level, so a [`from_bytes`] then
    /// [`to_bytes`] round trip within this bound is safe.
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
pub fn from_bytes<'de, T: FromNBT<'de>>(input: &'de [u8]) -> Result<T> {
    from_bytes_with_opts(input, DeOpts::default())
}

/// Deserializes a `T` from a document whose root is a compound.
pub fn from_bytes_with_opts<'de, T: FromNBT<'de>>(input: &'de [u8], opts: DeOpts) -> Result<T> {
    let expect_names = opts.expect_compound_names;
    let mut reader = Reader::new(input, opts);
    let tag = reader.read_tag()?;
    if tag != TAG_COMPOUND {
        return Err(Error::no_root_compound());
    }
    if expect_names {
        reader.skip_name()?;
    }
    T::read(tag, &mut reader)
}

/// The serde entry points, for callers that use `Serialize`/`Deserialize`.
///
/// Available only with the `serde` feature, alongside the trait
/// implementations the old versions of this crate provided.
#[cfg(feature = "serde")]
pub mod serde_compat {
    use alloc::vec::Vec;

    use serde::{Deserialize, Serialize};

    use crate::{DeOpts, Result, SerOpts, de, ser};

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

    /// Deserializes a `T` from a document whose root is a compound.
    pub fn from_bytes<'de, T: Deserialize<'de>>(input: &'de [u8]) -> Result<T> {
        from_bytes_with_opts(input, DeOpts::default())
    }

    /// Deserializes a `T` from a document whose root is a compound.
    pub fn from_bytes_with_opts<'de, T: Deserialize<'de>>(
        input: &'de [u8],
        opts: DeOpts,
    ) -> Result<T> {
        T::deserialize(&mut de::Deserializer::from_bytes(input, opts))
    }
}
