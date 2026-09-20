//! The `fastnbt` entry: one serde struct per document.

use criterion::{BenchmarkGroup, measurement::WallTime};
use fastnbt::{ByteArray, IntArray, LongArray};
use serde::{Deserialize, Serialize};

use crate::documents::{Array, Doc};
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
pub fn parse_array(group: &mut BenchmarkGroup<'_, WallTime>, kind: Array, bytes: &[u8]) {
    match kind {
        Array::Byte => {
            bench_parse(group, "fastnbt", kind, bytes, |b| {
                fastnbt::from_bytes::<ByteOwned>(b).expect("the document parses")
            });
            bench_parse(group, "fastnbt-borrow", kind, bytes, |b| {
                fastnbt::from_bytes::<ByteBorrowed<'_>>(b).expect("the document parses")
            });
        }
        Array::Short => {
            bench_parse(group, "fastnbt", kind, bytes, |b| {
                fastnbt::from_bytes::<ShortOwned>(b).expect("the document parses")
            });
        }
        Array::Int => {
            bench_parse(group, "fastnbt", kind, bytes, |b| {
                fastnbt::from_bytes::<IntOwned>(b).expect("the document parses")
            });
            bench_parse(group, "fastnbt-borrow", kind, bytes, |b| {
                fastnbt::from_bytes::<IntBorrowed<'_>>(b).expect("the document parses")
            });
        }
        Array::Long => {
            bench_parse(group, "fastnbt", kind, bytes, |b| {
                fastnbt::from_bytes::<LongOwned>(b).expect("the document parses")
            });
            bench_parse(group, "fastnbt-borrow", kind, bytes, |b| {
                fastnbt::from_bytes::<LongBorrowed<'_>>(b).expect("the document parses")
            });
        }
    }
}

/// Runs the write entries of one array kind.
pub fn write_array(group: &mut BenchmarkGroup<'_, WallTime>, kind: Array, bytes: &[u8]) {
    match kind {
        Array::Byte => {
            bench_write(
                group,
                "fastnbt",
                kind,
                bytes,
                |b| fastnbt::from_bytes::<ByteOwned>(b).expect("the document parses"),
                |v: &ByteOwned| fastnbt::to_bytes(v).expect("the document writes"),
            );
            bench_write(
                group,
                "fastnbt-borrow",
                kind,
                bytes,
                |b| fastnbt::from_bytes::<ByteBorrowed<'_>>(b).expect("the document parses"),
                |v: &ByteBorrowed<'_>| fastnbt::to_bytes(v).expect("the document writes"),
            );
        }
        Array::Short => {
            bench_write(
                group,
                "fastnbt",
                kind,
                bytes,
                |b| fastnbt::from_bytes::<ShortOwned>(b).expect("the document parses"),
                |v: &ShortOwned| fastnbt::to_bytes(v).expect("the document writes"),
            );
        }
        Array::Int => {
            bench_write(
                group,
                "fastnbt",
                kind,
                bytes,
                |b| fastnbt::from_bytes::<IntOwned>(b).expect("the document parses"),
                |v: &IntOwned| fastnbt::to_bytes(v).expect("the document writes"),
            );
            bench_write(
                group,
                "fastnbt-borrow",
                kind,
                bytes,
                |b| fastnbt::from_bytes::<IntBorrowed<'_>>(b).expect("the document parses"),
                |v: &IntBorrowed<'_>| fastnbt::to_bytes(v).expect("the document writes"),
            );
        }
        Array::Long => {
            bench_write(
                group,
                "fastnbt",
                kind,
                bytes,
                |b| fastnbt::from_bytes::<LongOwned>(b).expect("the document parses"),
                |v: &LongOwned| fastnbt::to_bytes(v).expect("the document writes"),
            );
            bench_write(
                group,
                "fastnbt-borrow",
                kind,
                bytes,
                |b| fastnbt::from_bytes::<LongBorrowed<'_>>(b).expect("the document parses"),
                |v: &LongBorrowed<'_>| fastnbt::to_bytes(v).expect("the document writes"),
            );
        }
    }
}

pub fn parse(group: &mut BenchmarkGroup<'_, WallTime>, doc: Doc, bytes: &[u8]) {
    match doc {
        Doc::Small => bench_parse(group, "fastnbt", doc, bytes, |b: &[u8]| {
            fastnbt::from_bytes::<Small>(b).expect("document parses")
        }),
        Doc::Player => bench_parse(group, "fastnbt", doc, bytes, |b: &[u8]| {
            fastnbt::from_bytes::<Player>(b).expect("document parses")
        }),
        Doc::Chunk => bench_parse(group, "fastnbt", doc, bytes, |b: &[u8]| {
            fastnbt::from_bytes::<Chunk>(b).expect("document parses")
        }),
    }
}

pub fn write(group: &mut BenchmarkGroup<'_, WallTime>, doc: Doc, bytes: &[u8]) {
    match doc {
        Doc::Small => bench_write(
            group,
            "fastnbt",
            doc,
            bytes,
            |b: &[u8]| fastnbt::from_bytes::<Small>(b).expect("document parses"),
            |v: &Small| fastnbt::to_bytes(v).expect("struct writes"),
        ),
        Doc::Player => bench_write(
            group,
            "fastnbt",
            doc,
            bytes,
            |b: &[u8]| fastnbt::from_bytes::<Player>(b).expect("document parses"),
            |v: &Player| fastnbt::to_bytes(v).expect("struct writes"),
        ),
        Doc::Chunk => bench_write(
            group,
            "fastnbt",
            doc,
            bytes,
            |b: &[u8]| fastnbt::from_bytes::<Chunk>(b).expect("document parses"),
            |v: &Chunk| fastnbt::to_bytes(v).expect("struct writes"),
        ),
    }
}
