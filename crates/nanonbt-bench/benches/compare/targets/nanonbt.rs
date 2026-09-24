//! The `nanonbt` entries: `serde`, `derive` and borrowed `derive`.
//!
//! The owned model below doubles as the input source: `documents` serializes
//! [`sample_small`], [`sample_player`], [`sample_chunk`], [`ShortNames`] and
//! [`LongNames`], and every target then parses those same bytes. The name pair
//! carries only `i32` fields, so it has no `nanonbt-borrow` entry: the
//! borrowed struct would compile to the owned one. Their field names are drawn
//! by the `random_names` attribute, so no name appears in this source.
//!
//! Field names are the NBT keys, so neither `serde` nor the derive needs a
//! rename attribute; the parser rejects unknown fields, not unknown attributes.
//!
//! The skip model at the end writes what the `skip` group parses: a compound
//! holding `kept`, the one field [`Sparse`] declares, and `skipped`, one huge
//! entry of that shape. Everything past `kept` is passed over, the serde side
//! through `IgnoredAny` and the derive through `Read::skip`.

use std::borrow::Cow;

use criterion::{BenchmarkGroup, measurement::WallTime};
use nanonbt::{
    ByteArray, F32Be, F64Be, FromNBT, I16Be, I32Be, I64Be, IntArray, LongArray, ToNBT, U16Be,
    U32Be, U64Be, serde_compat,
};
use random_names::random_names;
use serde::{Deserialize, Serialize};

use crate::documents::{Array, BenchInput, Doc, Skip};
use crate::{bench_parse, bench_write};

// ---------------------------------------------------------------------------
// The owned model, which is also what generates the input documents.
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, FromNBT, ToNBT)]
pub struct Small {
    pub dimension: String,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub yaw: f32,
    pub pitch: f32,
    pub on_ground: i8,
    pub health: i16,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, FromNBT, ToNBT)]
pub struct Player {
    pub DataVersion: i32,
    pub Health: f32,
    pub foodLevel: i32,
    pub XpLevel: i32,
    pub playerGameType: i32,
    pub UUID: IntArray,
    pub Pos: Vec<f64>,
    pub Motion: Vec<f64>,
    pub Rotation: Vec<f32>,
    pub Inventory: Vec<InventoryItem>,
    pub Attributes: Vec<Attribute>,
    pub EnderItems: Vec<InventoryItem>,
    pub abilities: Abilities,
    pub recipeBook: RecipeBook,
    pub Tags: Vec<String>,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, FromNBT, ToNBT)]
pub struct InventoryItem {
    pub Slot: i8,
    pub id: String,
    pub Count: i8,
    pub tag: ItemTag,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, FromNBT, ToNBT)]
pub struct ItemTag {
    pub Damage: i32,
    pub display: Display,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, FromNBT, ToNBT)]
pub struct Display {
    pub Name: String,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, FromNBT, ToNBT)]
pub struct Attribute {
    pub Name: String,
    pub Base: f64,
    pub Modifiers: Vec<Modifier>,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, FromNBT, ToNBT)]
pub struct Modifier {
    pub Amount: f64,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, FromNBT, ToNBT)]
pub struct Abilities {
    pub flying: i8,
    pub mayfly: i8,
    pub invulnerable: i8,
    pub walkSpeed: f32,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, FromNBT, ToNBT)]
pub struct RecipeBook {
    pub isFilteringCraftable: i8,
    pub recipes: Vec<String>,
    pub toBeDisplayed: Vec<String>,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, FromNBT, ToNBT)]
pub struct Chunk {
    pub DataVersion: i32,
    pub xPos: i32,
    pub zPos: i32,
    pub Status: String,
    pub LastUpdate: i64,
    pub InhabitedTime: i64,
    pub sections: Vec<Section>,
    pub block_entities: Vec<BlockEntity>,
    pub Heightmaps: Heightmaps,
    pub PostProcessing: Vec<Vec<i16>>,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, FromNBT, ToNBT)]
pub struct Section {
    pub Y: i8,
    pub block_states: BlockStates,
    pub biomes: Biomes,
    pub BlockLight: ByteArray,
    pub SkyLight: ByteArray,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, FromNBT, ToNBT)]
pub struct BlockStates {
    pub palette: Vec<PaletteEntry>,
    pub data: LongArray,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, FromNBT, ToNBT)]
pub struct PaletteEntry {
    pub Name: String,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, FromNBT, ToNBT)]
pub struct Biomes {
    pub palette: Vec<String>,
    pub data: LongArray,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, FromNBT, ToNBT)]
pub struct Heightmaps {
    pub MOTION_BLOCKING: LongArray,
    pub WORLD_SURFACE: LongArray,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, FromNBT, ToNBT)]
pub struct BlockEntity {
    pub id: String,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub KeepPacked: i8,
}

/// A compound of 64 `i32` fields whose keys are a fixed 60-byte prefix plus
/// four random bytes, drawn by the `random_names` attribute.
#[random_names(
    64,
    len = 64,
    prefix = "benchmark_field_with_a_deliberately_long_name_shared_prefix_"
)]
#[random_names(
    64,
    len = 64,
    prefix = "benchmark_field_with_a_deliberately_long_name_shared_prefix_"
)]
#[derive(Serialize, Deserialize, FromNBT, ToNBT)]
pub struct LongNames;

/// The same 64 `i32` fields as [`LongNames`], named by three random bytes.
#[random_names(64, len = 3)]
#[derive(Serialize, Deserialize, FromNBT, ToNBT)]
pub struct ShortNames;

/// The attributes of [`sample_player`], as `(name, base value)`.
const ATTRIBUTES: [(&str, f64); 5] = [
    ("generic.max_health", 20.0),
    ("generic.movement_speed", 0.1),
    ("generic.attack_damage", 1.0),
    ("generic.armor", 0.0),
    ("generic.luck", 0.0),
];

pub fn sample_small() -> Small {
    Small {
        dimension: "minecraft:overworld".to_owned(),
        x: 1234,
        y: 64,
        z: -5678,
        yaw: 90.0,
        pitch: -12.5,
        on_ground: 1,
        health: 20,
    }
}

pub fn sample_player() -> Player {
    Player {
        DataVersion: 3953,
        Health: 20.0,
        foodLevel: 20,
        XpLevel: 30,
        playerGameType: 1,
        UUID: IntArray::new(vec![1_234_567, -2_345_678, 345_678_901, -456_789_012]),
        Pos: vec![12.5, 64.0, -8.25],
        Motion: vec![0.0; 3],
        Rotation: vec![90.0, 0.0],
        Inventory: (0i8..36)
            .map(|slot| InventoryItem {
                Slot: slot,
                id: "minecraft:diamond_sword".to_owned(),
                Count: 1,
                tag: ItemTag {
                    Damage: i32::from(slot),
                    display: Display {
                        Name: "{\"text\":\"🦀 Rusty Blade\"}".to_owned(),
                    },
                },
            })
            .collect(),
        Attributes: ATTRIBUTES
            .iter()
            .map(|(name, base)| Attribute {
                Name: (*name).to_owned(),
                Base: *base,
                Modifiers: Vec::new(),
            })
            .collect(),
        EnderItems: Vec::new(),
        abilities: Abilities {
            flying: 0,
            mayfly: 1,
            invulnerable: 0,
            walkSpeed: 0.1,
        },
        recipeBook: RecipeBook {
            isFilteringCraftable: 0,
            recipes: vec![
                "minecraft:diamond_sword".to_owned(),
                "minecraft:diamond_pickaxe".to_owned(),
            ],
            toBeDisplayed: Vec::new(),
        },
        Tags: vec!["🦀".to_owned()],
    }
}

pub fn sample_chunk(sections: usize) -> Chunk {
    Chunk {
        DataVersion: 3953,
        xPos: 12,
        zPos: -34,
        Status: "minecraft:full".to_owned(),
        LastUpdate: 1_234_567_890,
        InhabitedTime: -9_876_543_210,
        sections: (0i8..)
            .take(sections)
            .map(|section| Section {
                Y: section,
                block_states: BlockStates {
                    palette: (0..16)
                        .map(|block| PaletteEntry {
                            Name: format!("minecraft:block_{block}"),
                        })
                        .collect(),
                    data: LongArray::new(vec![i64::from(section); 256]),
                },
                biomes: Biomes {
                    palette: vec!["minecraft:plains".to_owned()],
                    data: LongArray::new(vec![0; 64]),
                },
                BlockLight: ByteArray::new(vec![0; 2048]),
                SkyLight: ByteArray::new(vec![-1; 2048]),
            })
            .collect(),
        block_entities: (0i8..4)
            .map(|i| BlockEntity {
                id: "minecraft:chest".to_owned(),
                x: 12,
                y: i32::from(i) * 16,
                z: -34,
                KeepPacked: 0,
            })
            .collect(),
        Heightmaps: Heightmaps {
            MOTION_BLOCKING: LongArray::new(vec![0; 37]),
            WORLD_SURFACE: LongArray::new(vec![-1; 37]),
        },
        PostProcessing: (0..24).map(|_| vec![0; 16]).collect(),
    }
}

// ---------------------------------------------------------------------------
// The borrowed model, which lends strings and arrays from the input.
// ---------------------------------------------------------------------------

#[allow(non_snake_case)]
#[derive(FromNBT, ToNBT)]
pub struct SmallRef<'a> {
    pub dimension: Cow<'a, str>,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub yaw: f32,
    pub pitch: f32,
    pub on_ground: i8,
    pub health: i16,
}

#[allow(non_snake_case)]
#[derive(FromNBT, ToNBT)]
pub struct PlayerRef<'a> {
    pub DataVersion: i32,
    pub Health: f32,
    pub foodLevel: i32,
    pub XpLevel: i32,
    pub playerGameType: i32,
    pub UUID: &'a [I32Be],
    pub Pos: &'a [F64Be],
    pub Motion: &'a [F64Be],
    pub Rotation: &'a [F32Be],
    pub Inventory: Vec<InventoryItemRef<'a>>,
    pub Attributes: Vec<AttributeRef<'a>>,
    pub EnderItems: Vec<InventoryItemRef<'a>>,
    pub abilities: Abilities,
    pub recipeBook: RecipeBookRef<'a>,
    pub Tags: Vec<Cow<'a, str>>,
}

#[allow(non_snake_case)]
#[derive(FromNBT, ToNBT)]
pub struct InventoryItemRef<'a> {
    pub Slot: i8,
    pub id: Cow<'a, str>,
    pub Count: i8,
    pub tag: ItemTagRef<'a>,
}

#[allow(non_snake_case)]
#[derive(FromNBT, ToNBT)]
pub struct ItemTagRef<'a> {
    pub Damage: i32,
    pub display: DisplayRef<'a>,
}

#[allow(non_snake_case)]
#[derive(FromNBT, ToNBT)]
pub struct DisplayRef<'a> {
    pub Name: Cow<'a, str>,
}

#[allow(non_snake_case)]
#[derive(FromNBT, ToNBT)]
pub struct AttributeRef<'a> {
    pub Name: Cow<'a, str>,
    pub Base: f64,
    pub Modifiers: Vec<Modifier>,
}

#[allow(non_snake_case)]
#[derive(FromNBT, ToNBT)]
pub struct RecipeBookRef<'a> {
    pub isFilteringCraftable: i8,
    pub recipes: Vec<Cow<'a, str>>,
    pub toBeDisplayed: Vec<Cow<'a, str>>,
}

#[allow(non_snake_case)]
#[derive(FromNBT, ToNBT)]
pub struct ChunkRef<'a> {
    pub DataVersion: i32,
    pub xPos: i32,
    pub zPos: i32,
    pub Status: Cow<'a, str>,
    pub LastUpdate: i64,
    pub InhabitedTime: i64,
    pub sections: Vec<SectionRef<'a>>,
    pub block_entities: Vec<BlockEntityRef<'a>>,
    pub Heightmaps: HeightmapsRef<'a>,
    pub PostProcessing: Vec<Vec<i16>>,
}

#[allow(non_snake_case)]
#[derive(FromNBT, ToNBT)]
pub struct SectionRef<'a> {
    pub Y: i8,
    pub block_states: BlockStatesRef<'a>,
    pub biomes: BiomesRef<'a>,
    pub BlockLight: &'a [i8],
    pub SkyLight: &'a [i8],
}

#[allow(non_snake_case)]
#[derive(FromNBT, ToNBT)]
pub struct BlockStatesRef<'a> {
    pub palette: Vec<PaletteEntryRef<'a>>,
    pub data: &'a [U64Be],
}

#[allow(non_snake_case)]
#[derive(FromNBT, ToNBT)]
pub struct PaletteEntryRef<'a> {
    pub Name: Cow<'a, str>,
}

#[allow(non_snake_case)]
#[derive(FromNBT, ToNBT)]
pub struct BiomesRef<'a> {
    pub palette: Vec<Cow<'a, str>>,
    pub data: &'a [U64Be],
}

#[allow(non_snake_case)]
#[derive(FromNBT, ToNBT)]
pub struct HeightmapsRef<'a> {
    pub MOTION_BLOCKING: &'a [U64Be],
    pub WORLD_SURFACE: &'a [U64Be],
}

#[allow(non_snake_case)]
#[derive(FromNBT, ToNBT)]
pub struct BlockEntityRef<'a> {
    pub id: Cow<'a, str>,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub KeepPacked: i8,
}

#[allow(dead_code)]
pub fn sample_long_names() -> LongNames {
    LongNames::sample()
}

#[allow(dead_code)]
pub fn sample_short_names() -> ShortNames {
    ShortNames::sample()
}

// ---------------------------------------------------------------------------
// The array model, one compound per array kind.
// ---------------------------------------------------------------------------

/// One array kind's four structs and its entries: the unsigned and signed
/// `Vec<T>` a derived field writes and reads, and the unsigned and signed
/// slice it lends.
///
/// Bytes keep the array attribute on both sides, so a borrowed byte slice
/// round trips as an array; the wider borrowed slices write lists, as
/// borrowed slices do everywhere.
macro_rules! array_kinds {
    ($(
        $(#[$owned:meta])?
        $module:ident, $usuffix:literal, $ssuffix:literal,
        $unsigned:ty, $signed:ty, $uview:ty, $sview:ty $(, #[$borrowed:meta])?;
    )*) => {
        $(
            pub mod $module {
                use super::*;

                /// The owned spelling, unsigned.
                #[derive(FromNBT, ToNBT)]
                pub struct OwnedU {
                    $(#[$owned])?
                    data: Vec<$unsigned>,
                }

                /// The owned spelling, signed.
                #[derive(FromNBT, ToNBT)]
                pub struct OwnedS {
                    $(#[$owned])?
                    data: Vec<$signed>,
                }

                /// The borrowed spelling, unsigned.
                #[derive(FromNBT, ToNBT)]
                pub struct BorrowedU<'a> {
                    $(#[$borrowed])?
                    data: &'a [$uview],
                }

                /// The borrowed spelling, signed.
                #[derive(FromNBT, ToNBT)]
                pub struct BorrowedS<'a> {
                    $(#[$borrowed])?
                    data: &'a [$sview],
                }

                /// The document: a compound holding one huge `data` array or
                /// list, written by the owned unsigned struct.
                pub fn document(len: usize) -> Vec<u8> {
                    let value = OwnedU {
                        data: (0..len).map(|i| i as $unsigned).collect(),
                    };
                    ::nanonbt::to_bytes(&value).expect("the document writes")
                }

                /// The parse entries of this kind.
                pub fn parse(group: &mut BenchmarkGroup<'_, WallTime>, kind: Array, bytes: &[u8]) {
                    bench_parse(group, "nanonbt-derive", kind.name(), bytes, |b| {
                        ::nanonbt::from_bytes::<OwnedU>(b).expect("the document parses")
                    });
                    bench_parse(group, "nanonbt-derive-signed", kind.name(), bytes, |b| {
                        ::nanonbt::from_bytes::<OwnedS>(b).expect("the document parses")
                    });
                    bench_parse(group, "nanonbt-borrow", kind.name(), bytes, |b| {
                        ::nanonbt::from_bytes::<BorrowedU<'_>>(b).expect("the document parses")
                    });
                    bench_parse(group, "nanonbt-borrow-signed", kind.name(), bytes, |b| {
                        ::nanonbt::from_bytes::<BorrowedS<'_>>(b).expect("the document parses")
                    });
                }

                /// The write entries of this kind.
                pub fn write(group: &mut BenchmarkGroup<'_, WallTime>, kind: Array, bytes: &[u8]) {
                    bench_write(
                        group,
                        "nanonbt-derive",
                        kind.name(),
                        bytes,
                        |b| ::nanonbt::from_bytes::<OwnedU>(b).expect("the document parses"),
                        |v: &OwnedU| ::nanonbt::to_bytes(v).expect("the document writes"),
                    );
                    bench_write(
                        group,
                        "nanonbt-derive-signed",
                        kind.name(),
                        bytes,
                        |b| ::nanonbt::from_bytes::<OwnedS>(b).expect("the document parses"),
                        |v: &OwnedS| ::nanonbt::to_bytes(v).expect("the document writes"),
                    );
                    bench_write(
                        group,
                        "nanonbt-borrow",
                        kind.name(),
                        bytes,
                        |b| ::nanonbt::from_bytes::<BorrowedU<'_>>(b).expect("the document parses"),
                        |v: &BorrowedU<'_>| ::nanonbt::to_bytes(v).expect("the document writes"),
                    );
                    bench_write(
                        group,
                        "nanonbt-borrow-signed",
                        kind.name(),
                        bytes,
                        |b| ::nanonbt::from_bytes::<BorrowedS<'_>>(b).expect("the document parses"),
                        |v: &BorrowedS<'_>| ::nanonbt::to_bytes(v).expect("the document writes"),
                    );
                }
            }
        )*
    };
}

array_kinds! {
    #[nbt(array = "byte")]
    byte, "u8", "i8", u8, i8, u8, i8, #[nbt(array = "byte")];
    short, "u16", "i16", u16, i16, U16Be, I16Be;
    #[nbt(array = "int")]
    int, "u32", "i32", u32, i32, U32Be, I32Be;
    #[nbt(array = "long")]
    long, "u64", "i64", u64, i64, U64Be, I64Be;
}

/// The document bytes of one array kind.
pub fn array_document(kind: Array, len: usize) -> Vec<u8> {
    match kind {
        Array::Byte => byte::document(len),
        Array::Short => short::document(len),
        Array::Int => int::document(len),
        Array::Long => long::document(len),
    }
}

// ---------------------------------------------------------------------------
// The skip model, one compound per skipped shape.
// ---------------------------------------------------------------------------

/// The six lists whose elements all take the same number of bytes, one
/// module per element type.
///
/// A structural skip can pass over these without walking them, so their
/// documents are what the `skip` group measures against targets that walk
/// every element.
macro_rules! skip_lists {
    ($($module:ident, $ty:ty;)*) => {
        $(
            pub mod $module {
                use super::*;

                /// The document: a compound holding `kept` and the skipped
                /// list.
                #[derive(ToNBT)]
                pub struct Source {
                    pub kept: i32,
                    pub skipped: Vec<$ty>,
                }

                /// A document of `len` skipped elements.
                pub fn document(len: usize) -> Vec<u8> {
                    let value = Source {
                        kept: 1,
                        skipped: (0..len).map(|i| i as $ty).collect(),
                    };
                    ::nanonbt::to_bytes(&value).expect("the document writes")
                }
            }
        )*
    };
}

skip_lists! {
    byte_list, i8;
    short_list, i16;
    int_list, i32;
    long_list, i64;
    float_list, f32;
    double_list, f64;
}

/// The three arrays, one module per kind.
macro_rules! skip_arrays {
    ($($module:ident, $kind:literal, $ty:ty;)*) => {
        $(
            pub mod $module {
                use super::*;

                /// The document: a compound holding `kept` and the skipped
                /// array.
                #[derive(ToNBT)]
                pub struct Source {
                    pub kept: i32,
                    #[nbt(array = $kind)]
                    pub skipped: Vec<$ty>,
                }

                /// A document of `len` skipped elements.
                pub fn document(len: usize) -> Vec<u8> {
                    let value = Source {
                        kept: 1,
                        skipped: (0..len).map(|i| i as $ty).collect(),
                    };
                    ::nanonbt::to_bytes(&value).expect("the document writes")
                }
            }
        )*
    };
}

skip_arrays! {
    byte_array, "byte", i8;
    int_array, "int", i32;
    long_array, "long", i64;
}

/// The skipped list of strings.
pub mod string_list {
    use super::*;

    /// The document: a compound holding `kept` and the skipped list.
    #[derive(ToNBT)]
    pub struct Source {
        pub kept: i32,
        pub skipped: Vec<String>,
    }

    /// A document of `len` skipped elements.
    pub fn document(len: usize) -> Vec<u8> {
        let value = Source {
            kept: 1,
            skipped: (0..len).map(|i| format!("minecraft:string_{i}")).collect(),
        };
        ::nanonbt::to_bytes(&value).expect("the document writes")
    }
}

/// The skipped list of compounds.
pub mod compound_list {
    use super::*;

    /// One element of the skipped list.
    #[derive(ToNBT)]
    pub struct Leaf {
        pub value: i64,
    }

    /// The document: a compound holding `kept` and the skipped list.
    #[derive(ToNBT)]
    pub struct Source {
        pub kept: i32,
        pub skipped: Vec<Leaf>,
    }

    /// A document of `len` skipped elements.
    pub fn document(len: usize) -> Vec<u8> {
        let value = Source {
            kept: 1,
            skipped: (0..len).map(|i| Leaf { value: i as i64 }).collect(),
        };
        ::nanonbt::to_bytes(&value).expect("the document writes")
    }
}

/// The document bytes of one skip shape.
pub fn skip_document(kind: Skip, len: usize) -> Vec<u8> {
    match kind {
        Skip::ByteList => byte_list::document(len),
        Skip::ShortList => short_list::document(len),
        Skip::IntList => int_list::document(len),
        Skip::LongList => long_list::document(len),
        Skip::FloatList => float_list::document(len),
        Skip::DoubleList => double_list::document(len),
        Skip::ByteArray => byte_array::document(len),
        Skip::IntArray => int_array::document(len),
        Skip::LongArray => long_array::document(len),
        Skip::StringList => string_list::document(len),
        Skip::CompoundList => compound_list::document(len),
    }
}

// ---------------------------------------------------------------------------
// The entries.
// ---------------------------------------------------------------------------

pub fn parse(group: &mut BenchmarkGroup<'_, WallTime>, input: BenchInput) {
    match input {
        BenchInput::Doc(doc, bytes) => match doc {
            Doc::Small => {
                bench_parse(group, "nanonbt-serde", doc.name(), bytes, |b: &[u8]| {
                    serde_compat::from_bytes::<Small>(b).expect("document parses")
                });
                bench_parse(group, "nanonbt-derive", doc.name(), bytes, |b: &[u8]| {
                    nanonbt::from_bytes::<Small>(b).expect("document parses")
                });
                bench_parse(group, "nanonbt-borrow", doc.name(), bytes, |b: &[u8]| {
                    nanonbt::from_bytes::<SmallRef<'_>>(b).expect("document parses")
                });
            }
            Doc::Player => {
                bench_parse(group, "nanonbt-serde", doc.name(), bytes, |b: &[u8]| {
                    serde_compat::from_bytes::<Player>(b).expect("document parses")
                });
                bench_parse(group, "nanonbt-derive", doc.name(), bytes, |b: &[u8]| {
                    nanonbt::from_bytes::<Player>(b).expect("document parses")
                });
                bench_parse(group, "nanonbt-borrow", doc.name(), bytes, |b: &[u8]| {
                    nanonbt::from_bytes::<PlayerRef<'_>>(b).expect("document parses")
                });
            }
            Doc::Chunk => {
                bench_parse(group, "nanonbt-serde", doc.name(), bytes, |b: &[u8]| {
                    serde_compat::from_bytes::<Chunk>(b).expect("document parses")
                });
                bench_parse(group, "nanonbt-derive", doc.name(), bytes, |b: &[u8]| {
                    nanonbt::from_bytes::<Chunk>(b).expect("document parses")
                });
                bench_parse(group, "nanonbt-borrow", doc.name(), bytes, |b: &[u8]| {
                    nanonbt::from_bytes::<ChunkRef<'_>>(b).expect("document parses")
                });
            }
            Doc::ShortNames => {
                bench_parse(group, "nanonbt-serde", doc.name(), bytes, |b: &[u8]| {
                    serde_compat::from_bytes::<ShortNames>(b).expect("document parses")
                });
                bench_parse(group, "nanonbt-derive", doc.name(), bytes, |b: &[u8]| {
                    nanonbt::from_bytes::<ShortNames>(b).expect("document parses")
                });
            }
            Doc::LongNames => {
                bench_parse(group, "nanonbt-serde", doc.name(), bytes, |b: &[u8]| {
                    serde_compat::from_bytes::<LongNames>(b).expect("document parses")
                });
                bench_parse(group, "nanonbt-derive", doc.name(), bytes, |b: &[u8]| {
                    nanonbt::from_bytes::<LongNames>(b).expect("document parses")
                });
            }
        },
        BenchInput::Array(kind, bytes) => match kind {
            Array::Byte => byte::parse(group, kind, bytes),
            Array::Short => short::parse(group, kind, bytes),
            Array::Int => int::parse(group, kind, bytes),
            Array::Long => long::parse(group, kind, bytes),
        },
    }
}

pub fn write(group: &mut BenchmarkGroup<'_, WallTime>, input: BenchInput) {
    match input {
        BenchInput::Doc(doc, bytes) => match doc {
            Doc::Small => {
                bench_write(
                    group,
                    "nanonbt-serde",
                    doc.name(),
                    bytes,
                    |b: &[u8]| serde_compat::from_bytes::<Small>(b).expect("document parses"),
                    |v: &Small| serde_compat::to_bytes(v).expect("struct writes"),
                );
                bench_write(
                    group,
                    "nanonbt-derive",
                    doc.name(),
                    bytes,
                    |b: &[u8]| nanonbt::from_bytes::<Small>(b).expect("document parses"),
                    |v: &Small| nanonbt::to_bytes(v).expect("struct writes"),
                );
                bench_write(
                    group,
                    "nanonbt-borrow",
                    doc.name(),
                    bytes,
                    |b: &[u8]| nanonbt::from_bytes::<SmallRef<'_>>(b).expect("document parses"),
                    |v: &SmallRef<'_>| nanonbt::to_bytes(v).expect("struct writes"),
                );
            }
            Doc::Player => {
                bench_write(
                    group,
                    "nanonbt-serde",
                    doc.name(),
                    bytes,
                    |b: &[u8]| serde_compat::from_bytes::<Player>(b).expect("document parses"),
                    |v: &Player| serde_compat::to_bytes(v).expect("struct writes"),
                );
                bench_write(
                    group,
                    "nanonbt-derive",
                    doc.name(),
                    bytes,
                    |b: &[u8]| nanonbt::from_bytes::<Player>(b).expect("document parses"),
                    |v: &Player| nanonbt::to_bytes(v).expect("struct writes"),
                );
                bench_write(
                    group,
                    "nanonbt-borrow",
                    doc.name(),
                    bytes,
                    |b: &[u8]| nanonbt::from_bytes::<PlayerRef<'_>>(b).expect("document parses"),
                    |v: &PlayerRef<'_>| nanonbt::to_bytes(v).expect("struct writes"),
                );
            }
            Doc::Chunk => {
                bench_write(
                    group,
                    "nanonbt-serde",
                    doc.name(),
                    bytes,
                    |b: &[u8]| serde_compat::from_bytes::<Chunk>(b).expect("document parses"),
                    |v: &Chunk| serde_compat::to_bytes(v).expect("struct writes"),
                );
                bench_write(
                    group,
                    "nanonbt-derive",
                    doc.name(),
                    bytes,
                    |b: &[u8]| nanonbt::from_bytes::<Chunk>(b).expect("document parses"),
                    |v: &Chunk| nanonbt::to_bytes(v).expect("struct writes"),
                );
                bench_write(
                    group,
                    "nanonbt-borrow",
                    doc.name(),
                    bytes,
                    |b: &[u8]| nanonbt::from_bytes::<ChunkRef<'_>>(b).expect("document parses"),
                    |v: &ChunkRef<'_>| nanonbt::to_bytes(v).expect("struct writes"),
                );
            }
            Doc::ShortNames => {
                bench_write(
                    group,
                    "nanonbt-serde",
                    doc.name(),
                    bytes,
                    |b: &[u8]| serde_compat::from_bytes::<ShortNames>(b).expect("document parses"),
                    |v: &ShortNames| serde_compat::to_bytes(v).expect("struct writes"),
                );
                bench_write(
                    group,
                    "nanonbt-derive",
                    doc.name(),
                    bytes,
                    |b: &[u8]| nanonbt::from_bytes::<ShortNames>(b).expect("document parses"),
                    |v: &ShortNames| nanonbt::to_bytes(v).expect("struct writes"),
                );
            }
            Doc::LongNames => {
                bench_write(
                    group,
                    "nanonbt-serde",
                    doc.name(),
                    bytes,
                    |b: &[u8]| serde_compat::from_bytes::<LongNames>(b).expect("document parses"),
                    |v: &LongNames| serde_compat::to_bytes(v).expect("struct writes"),
                );
                bench_write(
                    group,
                    "nanonbt-derive",
                    doc.name(),
                    bytes,
                    |b: &[u8]| nanonbt::from_bytes::<LongNames>(b).expect("document parses"),
                    |v: &LongNames| nanonbt::to_bytes(v).expect("struct writes"),
                );
            }
        },
        BenchInput::Array(kind, bytes) => match kind {
            Array::Byte => byte::write(group, kind, bytes),
            Array::Short => short::write(group, kind, bytes),
            Array::Int => int::write(group, kind, bytes),
            Array::Long => long::write(group, kind, bytes),
        },
    }
}

/// The skip target: it declares `kept` alone, so the big `skipped` entry is
/// passed over, the serde side through `IgnoredAny` and the derive through
/// `Read::skip`.
#[derive(Deserialize, FromNBT)]
pub struct Sparse {
    /// Only the parse result matters; no entry reads this back.
    #[allow(dead_code)]
    pub kept: i32,
}

/// The skip entries of one shape.
pub fn skip(group: &mut BenchmarkGroup<'_, WallTime>, kind: Skip, bytes: &[u8]) {
    bench_parse(group, "nanonbt-serde", kind.name(), bytes, |b: &[u8]| {
        serde_compat::from_bytes::<Sparse>(b).expect("the document parses")
    });
    bench_parse(group, "nanonbt-derive", kind.name(), bytes, |b: &[u8]| {
        nanonbt::from_bytes::<Sparse>(b).expect("the document parses")
    });
}
