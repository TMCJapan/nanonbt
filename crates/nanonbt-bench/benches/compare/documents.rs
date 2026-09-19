//! The documents every target parses and writes.
//!
//! Each document is built as the owned `nanonbt` derive model and serialized
//! by `nanonbt`, so every entry reads the same bytes.

use nanonbt::ToNBT;

use crate::targets::nanonbt as model;

/// One document shape.
#[derive(Clone, Copy)]
pub enum Doc {
    Small,
    Player,
    Chunk,
}

impl Doc {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Small => "small",
            Self::Player => "player",
            Self::Chunk => "chunk",
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
    ]
}

fn input<T: ToNBT>(doc: Doc, value: &T) -> Input {
    Input {
        doc,
        bytes: nanonbt::to_bytes(value).expect("documents are valid NBT"),
    }
}
