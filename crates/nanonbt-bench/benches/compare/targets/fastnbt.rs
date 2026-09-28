//! The `fastnbt` entry: one serde struct per document.
//!
//! The skip target at the end declares `kept` alone, so the rest of a skip
//! document is passed over through serde's `IgnoredAny`.

use fastnbt::{ByteArray, IntArray, LongArray};
use random_names::random_names;
use serde::{Deserialize, Serialize};

use crate::documents::{self, Array, Doc, Skip};
use crate::macros::{library_benchmark_group, parse_bench, write_bench, write_bench_leaked};

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

// ---------------------------------------------------------------------------
// The entries, one `#[bench]` function per document or array.
// ---------------------------------------------------------------------------
//
// A function is named `<kind>_<target>_<id>` — `parse_fastnbt_borrow_int_array`
// is `parse/fastnbt-borrow/int-array` — and `examples/bench-summary.rs`
// recovers the id from the name plus the `#[bench]` id; see `crate::macros`.

/// The parse and write entries of one owned document or array.
macro_rules! owned_entry {
    ($ty:ty, $input:expr, $id:ident, $parse:ident, $setup:ident, $write:ident) => {
        parse_bench!($parse, $id, $input, |b: &[u8]| {
            fastnbt::from_bytes::<$ty>(b).expect("the document parses")
        });
        write_bench!(
            $write,
            $setup,
            $id,
            $input,
            $ty,
            |b: &[u8]| fastnbt::from_bytes::<$ty>(b).expect("the document parses"),
            |v: &$ty| fastnbt::to_bytes(v).expect("the document writes")
        );
    };
}

/// The parse and write entries of one borrowed array, whose value holds the
/// document bytes; the write setup leaks them to keep that borrow.
macro_rules! borrowed_entry {
    ($ty:ty, $static:ty, $input:expr, $id:ident, $parse:ident, $setup:ident, $write:ident) => {
        parse_bench!($parse, $id, $input, |b: &[u8]| {
            let _ =
                ::std::hint::black_box(fastnbt::from_bytes::<$ty>(b).expect("the document parses"));
        });
        write_bench_leaked!(
            $write,
            $setup,
            $id,
            $input,
            $static,
            |b: &'static [u8]| fastnbt::from_bytes::<$static>(b).expect("the document parses"),
            |v: &$static| fastnbt::to_bytes(v).expect("the document writes")
        );
    };
}

owned_entry!(
    Small,
    documents::doc(Doc::Small),
    small,
    parse_fastnbt_small,
    setup_fastnbt_small,
    write_fastnbt_small
);
owned_entry!(
    Player,
    documents::doc(Doc::Player),
    player,
    parse_fastnbt_player,
    setup_fastnbt_player,
    write_fastnbt_player
);
owned_entry!(
    Chunk,
    documents::doc(Doc::Chunk),
    chunk,
    parse_fastnbt_chunk,
    setup_fastnbt_chunk,
    write_fastnbt_chunk
);
owned_entry!(
    ShortNames,
    documents::doc(Doc::ShortNames),
    short_names,
    parse_fastnbt_short_names,
    setup_fastnbt_short_names,
    write_fastnbt_short_names
);
owned_entry!(
    LongNames,
    documents::doc(Doc::LongNames),
    long_names,
    parse_fastnbt_long_names,
    setup_fastnbt_long_names,
    write_fastnbt_long_names
);

owned_entry!(
    ByteOwned,
    documents::array_doc(Array::Byte),
    byte_array,
    parse_fastnbt_byte_array,
    setup_fastnbt_byte_array,
    write_fastnbt_byte_array
);
borrowed_entry!(
    ByteBorrowed<'_>,
    ByteBorrowed<'static>,
    documents::array_doc(Array::Byte),
    byte_array,
    parse_fastnbt_borrow_byte_array,
    setup_fastnbt_borrow_byte_array,
    write_fastnbt_borrow_byte_array
);
owned_entry!(
    ShortOwned,
    documents::array_doc(Array::Short),
    short_list,
    parse_fastnbt_short_list,
    setup_fastnbt_short_list,
    write_fastnbt_short_list
);
owned_entry!(
    IntOwned,
    documents::array_doc(Array::Int),
    int_array,
    parse_fastnbt_int_array,
    setup_fastnbt_int_array,
    write_fastnbt_int_array
);
borrowed_entry!(
    IntBorrowed<'_>,
    IntBorrowed<'static>,
    documents::array_doc(Array::Int),
    int_array,
    parse_fastnbt_borrow_int_array,
    setup_fastnbt_borrow_int_array,
    write_fastnbt_borrow_int_array
);
owned_entry!(
    LongOwned,
    documents::array_doc(Array::Long),
    long_array,
    parse_fastnbt_long_array,
    setup_fastnbt_long_array,
    write_fastnbt_long_array
);
borrowed_entry!(
    LongBorrowed<'_>,
    LongBorrowed<'static>,
    documents::array_doc(Array::Long),
    long_array,
    parse_fastnbt_borrow_long_array,
    setup_fastnbt_borrow_long_array,
    write_fastnbt_borrow_long_array
);

/// The skip target: it declares `kept` alone, so the big `skipped` entry is
/// passed over through fastnbt's `IgnoredAny`.
#[derive(Deserialize)]
pub struct Sparse {
    /// Only the parse result matters; no entry reads this back.
    #[allow(dead_code)]
    pub kept: i32,
}

// The eleven skip shapes, through `IgnoredAny`.
parse_bench!(
    skip_fastnbt_byte_list,
    byte_list,
    documents::skip_doc(Skip::ByteList),
    |b: &[u8]| fastnbt::from_bytes::<Sparse>(b).expect("the document parses")
);
parse_bench!(
    skip_fastnbt_short_list,
    short_list,
    documents::skip_doc(Skip::ShortList),
    |b: &[u8]| fastnbt::from_bytes::<Sparse>(b).expect("the document parses")
);
parse_bench!(
    skip_fastnbt_int_list,
    int_list,
    documents::skip_doc(Skip::IntList),
    |b: &[u8]| fastnbt::from_bytes::<Sparse>(b).expect("the document parses")
);
parse_bench!(
    skip_fastnbt_long_list,
    long_list,
    documents::skip_doc(Skip::LongList),
    |b: &[u8]| fastnbt::from_bytes::<Sparse>(b).expect("the document parses")
);
parse_bench!(
    skip_fastnbt_float_list,
    float_list,
    documents::skip_doc(Skip::FloatList),
    |b: &[u8]| fastnbt::from_bytes::<Sparse>(b).expect("the document parses")
);
parse_bench!(
    skip_fastnbt_double_list,
    double_list,
    documents::skip_doc(Skip::DoubleList),
    |b: &[u8]| fastnbt::from_bytes::<Sparse>(b).expect("the document parses")
);
parse_bench!(
    skip_fastnbt_byte_array,
    byte_array,
    documents::skip_doc(Skip::ByteArray),
    |b: &[u8]| fastnbt::from_bytes::<Sparse>(b).expect("the document parses")
);
parse_bench!(
    skip_fastnbt_int_array,
    int_array,
    documents::skip_doc(Skip::IntArray),
    |b: &[u8]| fastnbt::from_bytes::<Sparse>(b).expect("the document parses")
);
parse_bench!(
    skip_fastnbt_long_array,
    long_array,
    documents::skip_doc(Skip::LongArray),
    |b: &[u8]| fastnbt::from_bytes::<Sparse>(b).expect("the document parses")
);
parse_bench!(
    skip_fastnbt_string_list,
    string_list,
    documents::skip_doc(Skip::StringList),
    |b: &[u8]| fastnbt::from_bytes::<Sparse>(b).expect("the document parses")
);
parse_bench!(
    skip_fastnbt_compound_list,
    compound_list,
    documents::skip_doc(Skip::CompoundList),
    |b: &[u8]| fastnbt::from_bytes::<Sparse>(b).expect("the document parses")
);

library_benchmark_group!(
    name = fastnbt_entries;
    benchmarks =
        parse_fastnbt_small,
        parse_fastnbt_player,
        parse_fastnbt_chunk,
        parse_fastnbt_short_names,
        parse_fastnbt_long_names,
        write_fastnbt_small,
        write_fastnbt_player,
        write_fastnbt_chunk,
        write_fastnbt_short_names,
        write_fastnbt_long_names,
        parse_fastnbt_byte_array,
        parse_fastnbt_borrow_byte_array,
        parse_fastnbt_short_list,
        parse_fastnbt_int_array,
        parse_fastnbt_borrow_int_array,
        parse_fastnbt_long_array,
        parse_fastnbt_borrow_long_array,
        write_fastnbt_byte_array,
        write_fastnbt_borrow_byte_array,
        write_fastnbt_short_list,
        write_fastnbt_int_array,
        write_fastnbt_borrow_int_array,
        write_fastnbt_long_array,
        write_fastnbt_borrow_long_array,
        skip_fastnbt_byte_list,
        skip_fastnbt_short_list,
        skip_fastnbt_int_list,
        skip_fastnbt_long_list,
        skip_fastnbt_float_list,
        skip_fastnbt_double_list,
        skip_fastnbt_byte_array,
        skip_fastnbt_int_array,
        skip_fastnbt_long_array,
        skip_fastnbt_string_list,
        skip_fastnbt_compound_list
);
