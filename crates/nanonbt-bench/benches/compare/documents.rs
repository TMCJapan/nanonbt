//! The documents every target parses and writes.
//!
//! Each document is built as the owned `nanonbt` derive model and serialized
//! by `nanonbt`, so every entry reads the same bytes. Besides the five
//! structs there are four array shapes, each one compound holding one huge
//! `data` array or list. `short-names` and `long-names` hold the same 64
//! `i32` fields and differ only in key length, so their numbers read as a
//! pair; the `random_names` attribute draws their keys.

use nanonbt::ToNBT;

use crate::targets::nanonbt as model;

/// The length of every array document: half a million elements, far past
/// cache and within the 512,000 elements `pumpkin-nbt` accepts.
pub const ARRAY_LENGTH: usize = 500_000;

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
