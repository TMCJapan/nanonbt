//! A small NBT (Named Binary Tag) serializer.
//!
//! `nanonbt` encodes and decodes the binary NBT format used by Minecraft and
//! many other games. It is built around three ideas:
//!
//! 1. **Derived compounds.** `#[derive(SerializeNbt, DeserializeNbt)]` turns a
//!    struct into a `TAG_Compound`. Fields are matched by name on decode, so
//!    field order does not matter and unknown fields are skipped, which keeps
//!    decoding forward compatible.
//! 2. **Explicit array routes.** `TAG_List` is the default encoding for
//!    `Vec<T>` and `[T]`; the `#[nbt(array = byte | int | long)]` attribute
//!    (or `#[nbt(array)]`, whose kind is inferred from the element type)
//!    switches a field to `TAG_Byte_Array`, `TAG_Int_Array` or
//!    `TAG_Long_Array`.
//! 3. **Zero-copy decoding.** Readers that can hand out slices of their
//!    backing buffer implement [`ReadRef`]. Array fields typed as
//!    `&'de [u8]`, `&'de [I32Be]` or `&'de [I64Be]` then borrow directly from
//!    the input. Readers that only implement [`Read`] (such as
//!    [`VecReader`]) are restricted to owned decoding at compile time.
//!
//! With the `hashify` feature (enable `nanonbt/hashify`), a derived read
//! dispatches the names it sees through
//! [hashify](https://crates.io/crates/hashify)'s perfect hash lookups
//! instead of a `match`, which pays off for types with many fields; it too
//! stays `no_std`.
//!
//! With the `serde` feature, serde's own derives work over NBT as well: the
//! `serde` module encodes and decodes any type implementing serde's
//! `Serialize` / `Deserialize` traits with the same wire format as the native
//! derives.
//!
//! # Example
//!
//! ```
//! extern crate alloc;
//! use alloc::{string::String, vec::Vec};
//! use nanonbt::{from_bytes, to_bytes, DeserializeNbt, SerializeNbt};
//!
//! #[derive(SerializeNbt, DeserializeNbt, Debug, PartialEq)]
//! struct Player {
//!     name: String,
//!     health: f32,
//!     #[nbt(array = byte)]
//!     skin: Vec<u8>,
//!     #[nbt(array = int)]
//!     scores: Vec<i32>,
//! }
//!
//! let player = Player {
//!     name: "Steve".to_owned(),
//!     health: 20.0,
//!     skin: vec![1, 2, 3],
//!     scores: vec![10, -20],
//! };
//! let bytes = to_bytes(&player).unwrap();
//! let decoded: Player = from_bytes(&bytes).unwrap();
//! assert_eq!(player, decoded);
//! ```
//!
//! # Wire types
//!
//! Zero-copy array fields are exposed as [`I32Be`] / [`I64Be`], which store
//! the big-endian bytes of the value and have alignment 1. Borrowing native
//! `&[i32]` would require either an alignment guarantee on the input buffer or
//! a byte swap, so `nanonbt` does not do it; decode into `Vec<i32>` when
//! native integers are needed. [`I16Be`], [`F32Be`] and [`F64Be`] round out
//! the wire representations (NBT has no short or float array tag); the
//! `TAG_List` writers use all five to byte-swap a whole chunk of
//! `i16`/`i32`/`i64`/`f32`/`f64` elements in one conversion. A list whose
//! elements are already wire form — a `*Be` type, or plain `u8`/`i8` — needs
//! no conversion and is written in a single call.
//!
//! # Encoding notes
//!
//! * `Vec<T>` and `[T]` are a `TAG_List<T>`, including `Vec<u8>`; slices are
//!   what `Box<[T]>`, `&[T]` and `Cow<[T]>` serialize through. Annotate with
//!   `#[nbt(array = byte)]` to get a `TAG_Byte_Array`. Empty lists are written
//!   with the `End` element tag.
//! * Strings are written as Java modified UTF-8 (CESU-8). Raw NUL bytes or
//!   four-byte UTF-8 sequences are rejected on read, and their correct
//!   modified spelling (`C0 80` and surrogate pairs) is produced on write.
//! * [`to_bytes`] frames the root tag with an empty root name, matching
//!   Minecraft's file format; [`from_bytes`] reads and ignores the root name.
//! * Integers keep their signed NBT tags: `i8`/`i16`/`i32`/`i64` map to
//!   `Byte`/`Short`/`Int`/`Long`, `u8` maps to `Byte`.
//! * Enums, `Option`, maps, and borrowed strings are not supported yet by the
//!   native derives; see the derive crate for the supported field attribute
//!   set. The `serde` feature covers most of them through serde's derives.

#![no_std]
#![warn(missing_docs)]

#[cfg(feature = "std")]
extern crate std;

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "alloc")]
use alloc::vec::Vec;

mod codec;
mod de;
mod de_ref;
mod error;
mod io;
pub mod route;
mod ser;
#[cfg(feature = "serde")]
pub mod serde;
mod tag;
mod wire;

pub use codec::{read_count, read_name_into, skip_nbt, write_count, write_string};
#[cfg(feature = "alloc")]
pub use codec::{read_string, read_string_into};
pub use de::{DeserializeNbt, DeserializeNbtArray, DeserializeNbtSelf};
pub use de_ref::{DeserializeNbtArrayRef, DeserializeNbtRef, DeserializeNbtSelfRef};
pub use error::{Error, Result};
#[cfg(feature = "alloc")]
pub use io::TeeReader;
#[cfg(feature = "alloc")]
pub use io::VecReader;
pub use io::{Read, ReadRef, SliceReader, Write};
pub use route::{ArrayElem, ArrayRoute, ArrayRouteFor};
pub use ser::{SerializeNbt, SerializeNbtArray};
pub use tag::Tag;
pub use wire::{F32Be, F64Be, I16Be, I32Be, I64Be, Wire};

pub use nanonbt_derive::{DeserializeNbt, SerializeNbt};

pub use nanocesu8::{Cesu8, Cesu8Buf};
/// Implementation details the derive macros rely on.
///
/// Nothing here is stable; it follows what the generated code needs.
#[doc(hidden)]
pub mod __private {
    /// The macro the `hashify` feature's generated reads call.
    #[cfg(feature = "hashify")]
    pub use hashify;
}

fn read_root_header<R: Read + ?Sized>(reader: &mut R, expected: Tag) -> Result<()> {
    let actual = Tag::from_u8(reader.read_u8()?)?;
    if actual != expected {
        return Err(Error::InvalidTag {
            expected,
            actual: actual.as_u8(),
        });
    }
    let len = reader.read_u16_be()? as usize;
    reader.skip(len)?;
    Ok(())
}

/// Encodes `value` into a complete NBT document.
///
/// The document is framed like a Minecraft file: the root tag byte, an empty
/// root name, and the payload.
#[cfg(feature = "alloc")]
pub fn to_bytes<T: SerializeNbt + ?Sized>(value: &T) -> Result<Vec<u8>> {
    let mut buffer = Vec::new();
    to_writer(&mut buffer, value)?;
    Ok(buffer)
}

/// Encodes `value` into `writer`; see [`to_bytes`] for the framing.
pub fn to_writer<W: Write + ?Sized, T: SerializeNbt + ?Sized>(
    writer: &mut W,
    value: &T,
) -> Result<()> {
    writer.write_u8(T::TAG.as_u8());
    write_string(writer, "")?;
    value.serialize_nbt(writer)
}

/// Decodes an owned value from a complete NBT document; see [`to_bytes`] for
/// the expected framing.
pub fn from_bytes<T: DeserializeNbt>(bytes: &[u8]) -> Result<T> {
    let mut reader = SliceReader::new(bytes);
    read_root_header(&mut reader, T::TAG)?;
    T::deserialize_nbt(&mut reader)
}

/// Decodes a value that borrows from `bytes`.
///
/// The returned value may point directly into `bytes`, which is how array
/// fields avoid copying. See [`to_bytes`] for the expected framing.
pub fn from_slice<'de, T: DeserializeNbtRef<'de>>(bytes: &'de [u8]) -> Result<T> {
    let mut reader = SliceReader::new(bytes);
    read_root_header(&mut reader, T::TAG)?;
    T::deserialize_nbt_ref(&mut reader)
}

#[cfg(all(test, feature = "alloc"))]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn documents_roundtrip() {
        let bytes = to_bytes(&vec![1i32, 2, 3]).unwrap();
        assert_eq!(
            bytes,
            [
                Tag::List.as_u8(),
                0,
                0,
                Tag::Int.as_u8(),
                0,
                0,
                0,
                3,
                0,
                0,
                0,
                1,
                0,
                0,
                0,
                2,
                0,
                0,
                0,
                3,
            ]
        );
        let decoded: Vec<i32> = from_bytes(&bytes).unwrap();
        assert_eq!(decoded, [1, 2, 3]);
    }

    #[test]
    fn root_tag_is_validated() {
        let bytes = to_bytes(&1i32).unwrap();
        assert_eq!(
            from_bytes::<i64>(&bytes),
            Err(Error::InvalidTag {
                expected: Tag::Long,
                actual: Tag::Int.as_u8(),
            })
        );
    }

    #[test]
    fn empty_input_fails_cleanly() {
        assert_eq!(from_bytes::<i32>(&[]), Err(Error::UnexpectedEof));
    }
}
