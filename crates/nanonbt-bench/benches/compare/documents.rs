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

impl Doc {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Small => "small",
            Self::Player => "player",
            Self::Chunk => "chunk",
            Self::ShortNames => "short-names",
            Self::LongNames => "long-names",
        }
    }
}

/// A serialized document.
pub struct Input {
    pub doc: Doc,
    pub bytes: Vec<u8>,
}

/// The documents, serialized once by `nanonbt`.
pub fn inputs() -> Vec<Input> {
    vec![
        input(Doc::Small, &model::sample_small()),
        input(Doc::Player, &model::sample_player()),
        input(Doc::Chunk, &model::sample_chunk(8)),
        input(Doc::ShortNames, &model::ShortNames::sample()),
        input(Doc::LongNames, &model::LongNames::sample()),
    ]
}

fn input<T: ToNBT>(doc: Doc, value: &T) -> Input {
    Input {
        doc,
        bytes: nanonbt::to_bytes(value).expect("documents are valid NBT"),
    }
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

impl Array {
    /// Every shape, in the order the report lists them.
    pub const ALL: [Self; 4] = [Self::Byte, Self::Short, Self::Int, Self::Long];

    /// The name in the report.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Byte => "byte-array",
            Self::Short => "short-list",
            Self::Int => "int-array",
            Self::Long => "long-array",
        }
    }
}

/// A serialized array document.
pub struct ArrayInput {
    pub kind: Array,
    pub bytes: Vec<u8>,
}

/// A single benchmark input: either a document or an array.
pub enum BenchInput<'a> {
    Doc(Doc, &'a [u8]),
    Array(Array, &'a [u8]),
}

/// The array documents, serialized once by `nanonbt`.
///
/// Each is a compound holding one entry, `data`, with [`ARRAY_LENGTH`]
/// elements, written by the owned unsigned struct of that kind.
pub fn array_inputs() -> Vec<ArrayInput> {
    Array::ALL
        .into_iter()
        .map(|kind| ArrayInput {
            kind,
            bytes: model::array_document(kind, ARRAY_LENGTH),
        })
        .collect()
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

impl Skip {
    /// Every shape, in the order the report lists them.
    pub const ALL: [Self; 11] = [
        Self::ByteList,
        Self::ShortList,
        Self::IntList,
        Self::LongList,
        Self::FloatList,
        Self::DoubleList,
        Self::ByteArray,
        Self::IntArray,
        Self::LongArray,
        Self::StringList,
        Self::CompoundList,
    ];

    /// The name in the report.
    pub const fn name(self) -> &'static str {
        match self {
            Self::ByteList => "byte-list",
            Self::ShortList => "short-list",
            Self::IntList => "int-list",
            Self::LongList => "long-list",
            Self::FloatList => "float-list",
            Self::DoubleList => "double-list",
            Self::ByteArray => "byte-array",
            Self::IntArray => "int-array",
            Self::LongArray => "long-array",
            Self::StringList => "string-list",
            Self::CompoundList => "compound-list",
        }
    }
}

/// A serialized skip document.
pub struct SkipInput {
    pub kind: Skip,
    pub bytes: Vec<u8>,
}

/// The skip documents, serialized once by `nanonbt`.
///
/// Each is a compound holding `kept`, one `i32` the skip targets read, and
/// `skipped`, one entry with [`SKIP_LENGTH`] elements, written by the owned
/// struct of that kind. Everything but `kept` is what a skip passes over.
pub fn skip_inputs() -> Vec<SkipInput> {
    Skip::ALL
        .into_iter()
        .map(|kind| SkipInput {
            kind,
            bytes: model::skip_document(kind, SKIP_LENGTH),
        })
        .collect()
}
