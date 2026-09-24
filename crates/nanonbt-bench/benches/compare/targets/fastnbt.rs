//! The `fastnbt` entry: one serde struct per document.
//!
//! The skip target at the end declares `kept` alone, so the rest of a skip
//! document is passed over through serde's `IgnoredAny`.

use criterion::{BenchmarkGroup, measurement::WallTime};
use fastnbt::{ByteArray, IntArray, LongArray};
use random_names::random_names;
use serde::{Deserialize, Serialize};

use crate::documents::{Array, BenchInput, Doc, Skip};
use crate::{bench_parse, bench_write};

#[derive(Serialize, Deserialize)]
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
#[derive(Serialize, Deserialize)]
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
#[derive(Serialize, Deserialize)]
pub struct InventoryItem {
    pub Slot: i8,
    pub id: String,
    pub Count: i8,
    pub tag: ItemTag,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize)]
pub struct ItemTag {
    pub Damage: i32,
    pub display: Display,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize)]
pub struct Display {
    pub Name: String,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize)]
pub struct Attribute {
    pub Name: String,
    pub Base: f64,
    pub Modifiers: Vec<Modifier>,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize)]
pub struct Modifier {
    pub Amount: f64,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize)]
pub struct Abilities {
    pub flying: i8,
    pub mayfly: i8,
    pub invulnerable: i8,
    pub walkSpeed: f32,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize)]
pub struct RecipeBook {
    pub isFilteringCraftable: i8,
    pub recipes: Vec<String>,
    pub toBeDisplayed: Vec<String>,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize)]
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
#[derive(Serialize, Deserialize)]
pub struct Section {
    pub Y: i8,
    pub block_states: BlockStates,
    pub biomes: Biomes,
    pub BlockLight: ByteArray,
    pub SkyLight: ByteArray,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize)]
pub struct BlockStates {
    pub palette: Vec<PaletteEntry>,
    pub data: LongArray,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize)]
pub struct PaletteEntry {
    pub Name: String,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize)]
pub struct Biomes {
    pub palette: Vec<String>,
    pub data: LongArray,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize)]
pub struct Heightmaps {
    pub MOTION_BLOCKING: LongArray,
    pub WORLD_SURFACE: LongArray,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize)]
pub struct BlockEntity {
    pub id: String,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub KeepPacked: i8,
}

/// 64 `i32` fields whose keys are a fixed 60-byte prefix plus four random
/// bytes, drawn by the `random_names` attribute.
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
#[random_names(
    64,
    len = 64,
    prefix = "benchmark_field_with_a_deliberately_long_name_shared_prefix_"
)]
#[derive(Serialize, Deserialize)]
pub struct LongNames;

/// The same 64 `i32` fields as [`LongNames`], named by three random bytes.
#[random_names(64, len = 3)]
#[derive(Serialize, Deserialize)]
pub struct ShortNames;

// ---------------------------------------------------------------------------
// The array model, one compound per array kind.
// ---------------------------------------------------------------------------

/// The byte array document: `data` is a byte array.
#[derive(Serialize, Deserialize)]
pub struct ByteOwned {
    pub data: ByteArray,
}

/// The borrowed byte array, lent by the input.
#[derive(Serialize, Deserialize)]
pub struct ByteBorrowed<'a> {
    #[serde(borrow)]
    pub data: fastnbt::borrow::ByteArray<'a>,
}

/// The short list document: `data` is a list of shorts.
#[derive(Serialize, Deserialize)]
pub struct ShortOwned {
    pub data: Vec<i16>,
}

/// The int array document: `data` is an int array.
#[derive(Serialize, Deserialize)]
pub struct IntOwned {
    pub data: IntArray,
}

/// The borrowed int array, lent by the input.
#[derive(Serialize, Deserialize)]
pub struct IntBorrowed<'a> {
    #[serde(borrow)]
    pub data: fastnbt::borrow::IntArray<'a>,
}

/// The long array document: `data` is a long array.
#[derive(Serialize, Deserialize)]
pub struct LongOwned {
    pub data: LongArray,
}

/// The borrowed long array, lent by the input.
#[derive(Serialize, Deserialize)]
pub struct LongBorrowed<'a> {
    #[serde(borrow)]
    pub data: fastnbt::borrow::LongArray<'a>,
}

/// Runs the parse entries of one array kind.
pub fn parse(group: &mut BenchmarkGroup<'_, WallTime>, input: BenchInput) {
    match input {
        BenchInput::Doc(doc, bytes) => match doc {
            Doc::Small => bench_parse(group, "fastnbt", doc.name(), bytes, |b: &[u8]| {
                fastnbt::from_bytes::<Small>(b).expect("the document parses")
            }),
            Doc::Player => bench_parse(group, "fastnbt", doc.name(), bytes, |b: &[u8]| {
                fastnbt::from_bytes::<Player>(b).expect("the document parses")
            }),
            Doc::Chunk => bench_parse(group, "fastnbt", doc.name(), bytes, |b: &[u8]| {
                fastnbt::from_bytes::<Chunk>(b).expect("the document parses")
            }),
            Doc::ShortNames => bench_parse(group, "fastnbt", doc.name(), bytes, |b: &[u8]| {
                fastnbt::from_bytes::<ShortNames>(b).expect("the document parses")
            }),
            Doc::LongNames => bench_parse(group, "fastnbt", doc.name(), bytes, |b: &[u8]| {
                fastnbt::from_bytes::<LongNames>(b).expect("the document parses")
            }),
        },
        BenchInput::Array(kind, bytes) => match kind {
            Array::Byte => {
                bench_parse(group, "fastnbt", kind.name(), bytes, |b| {
                    fastnbt::from_bytes::<ByteOwned>(b).expect("the document parses")
                });
                bench_parse(group, "fastnbt-borrow", kind.name(), bytes, |b| {
                    fastnbt::from_bytes::<ByteBorrowed<'_>>(b).expect("the document parses")
                });
            }
            Array::Short => {
                bench_parse(group, "fastnbt", kind.name(), bytes, |b| {
                    fastnbt::from_bytes::<ShortOwned>(b).expect("the document parses")
                });
            }
            Array::Int => {
                bench_parse(group, "fastnbt", kind.name(), bytes, |b| {
                    fastnbt::from_bytes::<IntOwned>(b).expect("the document parses")
                });
                bench_parse(group, "fastnbt-borrow", kind.name(), bytes, |b| {
                    fastnbt::from_bytes::<IntBorrowed<'_>>(b).expect("the document parses")
                });
            }
            Array::Long => {
                bench_parse(group, "fastnbt", kind.name(), bytes, |b| {
                    fastnbt::from_bytes::<LongOwned>(b).expect("the document parses")
                });
                bench_parse(group, "fastnbt-borrow", kind.name(), bytes, |b| {
                    fastnbt::from_bytes::<LongBorrowed<'_>>(b).expect("the document parses")
                });
            }
        },
    }
}

/// Runs the write entries of one array kind.
pub fn write(group: &mut BenchmarkGroup<'_, WallTime>, input: BenchInput) {
    match input {
        BenchInput::Doc(doc, bytes) => match doc {
            Doc::Small => bench_write(
                group,
                "fastnbt",
                doc.name(),
                bytes,
                |b: &[u8]| fastnbt::from_bytes::<Small>(b).expect("the document parses"),
                |v: &Small| fastnbt::to_bytes(v).expect("the document writes"),
            ),
            Doc::Player => bench_write(
                group,
                "fastnbt",
                doc.name(),
                bytes,
                |b: &[u8]| fastnbt::from_bytes::<Player>(b).expect("the document parses"),
                |v: &Player| fastnbt::to_bytes(v).expect("the document writes"),
            ),
            Doc::Chunk => bench_write(
                group,
                "fastnbt",
                doc.name(),
                bytes,
                |b: &[u8]| fastnbt::from_bytes::<Chunk>(b).expect("the document parses"),
                |v: &Chunk| fastnbt::to_bytes(v).expect("the document writes"),
            ),
            Doc::ShortNames => bench_write(
                group,
                "fastnbt",
                doc.name(),
                bytes,
                |b: &[u8]| fastnbt::from_bytes::<ShortNames>(b).expect("the document parses"),
                |v: &ShortNames| fastnbt::to_bytes(v).expect("the document writes"),
            ),
            Doc::LongNames => bench_write(
                group,
                "fastnbt",
                doc.name(),
                bytes,
                |b: &[u8]| fastnbt::from_bytes::<LongNames>(b).expect("the document parses"),
                |v: &LongNames| fastnbt::to_bytes(v).expect("the document writes"),
            ),
        },
        BenchInput::Array(kind, bytes) => match kind {
            Array::Byte => {
                bench_write(
                    group,
                    "fastnbt",
                    kind.name(),
                    bytes,
                    |b| fastnbt::from_bytes::<ByteOwned>(b).expect("the document parses"),
                    |v: &ByteOwned| fastnbt::to_bytes(v).expect("the document writes"),
                );
                bench_write(
                    group,
                    "fastnbt-borrow",
                    kind.name(),
                    bytes,
                    |b| fastnbt::from_bytes::<ByteBorrowed<'_>>(b).expect("the document parses"),
                    |v: &ByteBorrowed<'_>| fastnbt::to_bytes(v).expect("the document writes"),
                );
            }
            Array::Short => {
                bench_write(
                    group,
                    "fastnbt",
                    kind.name(),
                    bytes,
                    |b| fastnbt::from_bytes::<ShortOwned>(b).expect("the document parses"),
                    |v: &ShortOwned| fastnbt::to_bytes(v).expect("the document writes"),
                );
            }
            Array::Int => {
                bench_write(
                    group,
                    "fastnbt",
                    kind.name(),
                    bytes,
                    |b| fastnbt::from_bytes::<IntOwned>(b).expect("the document parses"),
                    |v: &IntOwned| fastnbt::to_bytes(v).expect("the document writes"),
                );
                bench_write(
                    group,
                    "fastnbt-borrow",
                    kind.name(),
                    bytes,
                    |b| fastnbt::from_bytes::<IntBorrowed<'_>>(b).expect("the document parses"),
                    |v: &IntBorrowed<'_>| fastnbt::to_bytes(v).expect("the document writes"),
                );
            }
            Array::Long => {
                bench_write(
                    group,
                    "fastnbt",
                    kind.name(),
                    bytes,
                    |b| fastnbt::from_bytes::<LongOwned>(b).expect("the document parses"),
                    |v: &LongOwned| fastnbt::to_bytes(v).expect("the document writes"),
                );
                bench_write(
                    group,
                    "fastnbt-borrow",
                    kind.name(),
                    bytes,
                    |b| fastnbt::from_bytes::<LongBorrowed<'_>>(b).expect("the document parses"),
                    |v: &LongBorrowed<'_>| fastnbt::to_bytes(v).expect("the document writes"),
                );
            }
        },
    }
}

/// The skip target: it declares `kept` alone, so the big `skipped` entry is
/// passed over through fastnbt's `IgnoredAny`.
#[derive(Deserialize)]
pub struct Sparse {
    /// Only the parse result matters; no entry reads this back.
    #[allow(dead_code)]
    pub kept: i32,
}

/// The skip entry of one shape.
pub fn skip(group: &mut BenchmarkGroup<'_, WallTime>, kind: Skip, bytes: &[u8]) {
    bench_parse(group, "fastnbt", kind.name(), bytes, |b: &[u8]| {
        fastnbt::from_bytes::<Sparse>(b).expect("the document parses")
    });
}
