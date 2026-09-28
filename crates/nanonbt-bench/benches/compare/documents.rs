//! The documents every target parses and writes.
//!
//! Each document is built as the owned `nanonbt` derive model and serialized
//! by `nanonbt`, so every entry reads the same bytes. Besides the five
//! structs there are four array shapes, each one compound holding one huge
//! `data` array or list, and eleven skip shapes, each one compound holding a
//! small `kept` entry and one huge `skipped` entry the targets do not keep.
//! `short-names` and `long-names` hold the same 64 `i32` fields and
//! differ only in key length, so their numbers read as a pair; the
//! `random_names` attribute draws their keys.
//!
//! The benchmark entries ask for their document by name — `doc(Doc::Chunk)` —
//! and the bytes are built once per run, outside the measurement.

use nanonbt::ToNBT;

use crate::targets::nanonbt as model;

/// The length of every array document: half a million elements, far past
/// cache and within the 512,000 elements `pumpkin-nbt` accepts.
pub const ARRAY_LENGTH: usize = 500_000;

/// The length of every skip document's `skipped` entry.
///
/// A fifth of [`ARRAY_LENGTH`]: a skip itself is length-independent, but the
/// entries that cannot skip read the whole payload, and a compound or string
/// walk allocates per element, so half a million of those would not fit the
/// skip group's measurement.
pub const SKIP_LENGTH: usize = 100_000;

/// One document shape.
#[derive(Clone, Copy)]
pub enum Doc {
    Small,
    Player,
    Chunk,
    ShortNames,
    LongNames,
}

/// The bytes of one document, serialized by `nanonbt`.
pub fn doc(doc: Doc) -> Vec<u8> {
    match doc {
        Doc::Small => bytes(&model::sample_small()),
        Doc::Player => bytes(&model::sample_player()),
        Doc::Chunk => bytes(&model::sample_chunk(8)),
        Doc::ShortNames => bytes(&model::ShortNames::sample()),
        Doc::LongNames => bytes(&model::LongNames::sample()),
    }
}

fn bytes<T: ToNBT>(value: &T) -> Vec<u8> {
    nanonbt::to_bytes(value).expect("documents are valid NBT")
}

/// One array shape: a byte, int or long array, or a short list.
///
/// NBT has no short array, so 16-bit elements go through lists; the other
/// three are the arrays of their tags.
#[derive(Clone, Copy)]
pub enum Array {
    Byte,
    Short,
    Int,
    Long,
}

/// The bytes of one array document.
///
/// Each is a compound holding one entry, `data`, with [`ARRAY_LENGTH`]
/// elements, written by the owned unsigned struct of that kind.
pub fn array_doc(kind: Array) -> Vec<u8> {
    model::array_document(kind, ARRAY_LENGTH)
}

/// One skip shape: a compound with one `kept` entry and one huge `skipped`
/// entry the targets do not keep.
///
/// The six list shapes are the lists whose elements all take the same number
/// of bytes, which a skip can pass over without walking them; the three
/// arrays are the arrays of their tags. The last two shapes are the ones a
/// skip must walk element by element: variable-length strings, and compounds
/// with an entry each. The names match the array family's, `short-list`
/// included, since NBT has no short array.
#[derive(Clone, Copy)]
pub enum Skip {
    ByteList,
    ShortList,
    IntList,
    LongList,
    FloatList,
    DoubleList,
    ByteArray,
    IntArray,
    LongArray,
    StringList,
    CompoundList,
}

/// The bytes of one skip document.
///
/// Each is a compound holding `kept`, one `i32` the skip targets read, and
/// `skipped`, one entry with [`SKIP_LENGTH`] elements, written by the owned
/// struct of that kind. Everything but `kept` is what a skip passes over.
pub fn skip_doc(kind: Skip) -> Vec<u8> {
    model::skip_document(kind, SKIP_LENGTH)
}
