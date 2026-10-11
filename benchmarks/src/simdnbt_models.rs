//! `simdnbt` models for every scenario.
//!
//! `simdnbt` splits decoding into two phases: a borrowed tape parse
//! (`simdnbt::borrow::read`) and, optionally, a conversion into owned typed
//! data. The derived [`simdnbt::Deserialize`] implementation always converts
//! into owned fields, so `*_read` functions below measure parse + convert,
//! `borrow_tree_read` measures parse only, and `owned_tree_read` measures the
//! owned tree parse.
//!
//! Writes build an owned tree (the natural `simdnbt` write path) and then
//! serialize it.
//!
//! Every read benchmark walks the decoded value and returns a checksum; see
//! [`crate::checksum`].

use std::io::Cursor;

use simdnbt::owned::{BaseNbt, NbtCompound, NbtList, NbtTag};

use crate::fixtures;
use crate::{checksum, checksum_f32, checksum_f64, checksum_str};

const BYTE_ID: u8 = 1;
const SHORT_ID: u8 = 2;
const INT_ID: u8 = 3;
const LONG_ID: u8 = 4;
const FLOAT_ID: u8 = 5;
const DOUBLE_ID: u8 = 6;
const BYTE_ARRAY_ID: u8 = 7;
const STRING_ID: u8 = 8;
const LIST_ID: u8 = 9;
const COMPOUND_ID: u8 = 10;
const INT_ARRAY_ID: u8 = 11;
const LONG_ARRAY_ID: u8 = 12;

fn parse_borrowed(bytes: &[u8]) -> simdnbt::borrow::BaseNbt<'_> {
    simdnbt::borrow::read(&mut Cursor::new(bytes))
        .expect("simdnbt parse")
        .unwrap()
}

fn parse_owned(bytes: &[u8]) -> BaseNbt {
    simdnbt::owned::read(&mut Cursor::new(bytes))
        .expect("simdnbt parse")
        .unwrap()
}

fn from_bytes_typed<T: simdnbt::Deserialize>(bytes: &[u8]) -> T {
    typed_read(bytes)
}

/// Parses `bytes` with the borrowed tape and converts into the typed model.
///
/// This is the public entry point the interop tests use to compare decoded
/// values against the fixtures.
///
/// # Panics
///
/// Panics when decoding fails.
pub fn typed_read<T: simdnbt::Deserialize>(bytes: &[u8]) -> T {
    let nbt = parse_borrowed(bytes);
    T::from_nbt(&nbt).expect("simdnbt deserialize")
}

/// Parses `bytes` into the borrowed tape and checksums the whole tree.
///
/// # Panics
///
/// Panics when decoding fails.
pub fn borrow_tree_read(bytes: &[u8]) -> u64 {
    let nbt = parse_borrowed(bytes);
    borrow_compound_checksum(nbt.as_compound(), 0)
}

/// Parses `bytes` into an owned tree and checksums the whole tree.
///
/// # Panics
///
/// Panics when decoding fails.
pub fn owned_tree_read(bytes: &[u8]) -> u64 {
    let nbt = parse_owned(bytes);
    owned_compound_checksum(&nbt, 0)
}

// ---------------------------------------------------------------- small

/// The `small` scenario struct.
#[derive(simdnbt::Deserialize, Clone, Debug, PartialEq)]
pub struct Small {
    /// `TAG_Int`.
    pub id: i32,
    /// `TAG_Float`.
    pub health: f32,
    /// `TAG_Double`.
    pub mana: f64,
    /// `TAG_Short`.
    pub level: i16,
    /// `TAG_Long`.
    pub xp: i64,
    /// `TAG_String`.
    pub name: String,
    /// `TAG_Byte`.
    pub active: bool,
    /// `TAG_Byte`.
    pub hunger: i8,
}

/// Builds the `small` model from the fixture.
pub fn small_from_fixture(data: &fixtures::Small) -> Small {
    Small {
        id: data.id,
        health: data.health,
        mana: data.mana,
        level: data.level,
        xp: data.xp,
        name: data.name.clone(),
        active: data.active,
        hunger: data.hunger,
    }
}

/// Encodes the `small` model.
pub fn small_write(data: &Small) -> Vec<u8> {
    let mut out = Vec::new();
    small_nbt(data).write(&mut out);
    out
}

/// Encodes the `small` model into a reused buffer.
pub fn small_write_into(data: &Small, buffer: &mut Vec<u8>) {
    buffer.clear();
    small_nbt(data).write(buffer);
}

fn small_nbt(data: &Small) -> BaseNbt {
    let mut compound = NbtCompound::new();
    compound.insert("id", NbtTag::Int(data.id));
    compound.insert("health", NbtTag::Float(data.health));
    compound.insert("mana", NbtTag::Double(data.mana));
    compound.insert("level", NbtTag::Short(data.level));
    compound.insert("xp", NbtTag::Long(data.xp));
    compound.insert("name", NbtTag::String(data.name.clone().into()));
    compound.insert("active", NbtTag::Byte(i8::from(data.active)));
    compound.insert("hunger", NbtTag::Byte(data.hunger));
    BaseNbt::new("", compound)
}

/// Decodes the `small` model and returns its checksum.
///
/// # Panics
///
/// Panics when decoding fails.
pub fn small_read(bytes: &[u8]) -> u64 {
    let value: Small = from_bytes_typed(bytes);
    let mut acc = checksum(0, value.id as u64);
    acc = checksum_f32(acc, value.health);
    acc = checksum_f64(acc, value.mana);
    acc = checksum(acc, value.level as u64);
    acc = checksum(acc, value.xp as u64);
    acc = checksum_str(acc, &value.name);
    acc = checksum(acc, u64::from(value.active));
    checksum(acc, value.hunger as u64)
}

// ----------------------------------------------------------- byte_array

/// Encodes the `byte_array` fixture.
pub fn byte_array_write(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    byte_array_nbt(data).write(&mut out);
    out
}

/// Encodes the `byte_array` fixture into a reused buffer.
pub fn byte_array_write_into(data: &[u8], buffer: &mut Vec<u8>) {
    buffer.clear();
    byte_array_nbt(data).write(buffer);
}

fn byte_array_nbt(data: &[u8]) -> BaseNbt {
    let mut compound = NbtCompound::new();
    compound.insert("data", NbtTag::ByteArray(data.to_vec()));
    BaseNbt::new("", compound)
}

/// Decodes the byte array with the borrow API and returns its checksum.
///
/// # Panics
///
/// Panics when decoding fails.
pub fn byte_array_read_manual(bytes: &[u8]) -> u64 {
    let nbt = parse_borrowed(bytes);
    checksum_bytes(0, nbt.byte_array("data").expect("byte array"))
}

// ------------------------------------------------------------ int_array

/// Encodes the `int_array` fixture.
pub fn int_array_write(data: &[i32]) -> Vec<u8> {
    let mut out = Vec::new();
    int_array_nbt(data).write(&mut out);
    out
}

/// Encodes the `int_array` fixture into a reused buffer.
pub fn int_array_write_into(data: &[i32], buffer: &mut Vec<u8>) {
    buffer.clear();
    int_array_nbt(data).write(buffer);
}

fn int_array_nbt(data: &[i32]) -> BaseNbt {
    let mut compound = NbtCompound::new();
    compound.insert("data", NbtTag::IntArray(data.to_vec()));
    BaseNbt::new("", compound)
}

/// Decodes the int array with the borrow API and returns its checksum.
///
/// # Panics
///
/// Panics when decoding fails.
pub fn int_array_read_manual(bytes: &[u8]) -> u64 {
    let nbt = parse_borrowed(bytes);
    let data = nbt.int_array("data").expect("int array");
    let mut acc = checksum(0, data.len() as u64);
    for item in data {
        acc = checksum(acc, item as u64);
    }
    acc
}

// ----------------------------------------------------------- long_array

/// Encodes the `long_array` fixture.
pub fn long_array_write(data: &[i64]) -> Vec<u8> {
    let mut out = Vec::new();
    long_array_nbt(data).write(&mut out);
    out
}

/// Encodes the `long_array` fixture into a reused buffer.
pub fn long_array_write_into(data: &[i64], buffer: &mut Vec<u8>) {
    buffer.clear();
    long_array_nbt(data).write(buffer);
}

fn long_array_nbt(data: &[i64]) -> BaseNbt {
    let mut compound = NbtCompound::new();
    compound.insert("data", NbtTag::LongArray(data.to_vec()));
    BaseNbt::new("", compound)
}

/// Decodes the long array with the borrow API and returns its checksum.
///
/// # Panics
///
/// Panics when decoding fails.
pub fn long_array_read_manual(bytes: &[u8]) -> u64 {
    let nbt = parse_borrowed(bytes);
    let data = nbt.long_array("data").expect("long array");
    let mut acc = checksum(0, data.len() as u64);
    for item in data {
        acc = checksum(acc, item as u64);
    }
    acc
}

// -------------------------------------------------------------- strings

/// The `strings` scenario struct.
#[derive(simdnbt::Deserialize, Clone, Debug, PartialEq)]
pub struct Strings {
    /// A single large string.
    pub big: String,
    /// A list of short strings.
    pub words: Vec<String>,
}

/// Builds the `strings` model from the fixture.
pub fn strings_from_fixture(data: &fixtures::Strings) -> Strings {
    Strings {
        big: data.big.clone(),
        words: data.words.clone(),
    }
}

/// Encodes the `strings` model.
pub fn strings_write(data: &Strings) -> Vec<u8> {
    let mut out = Vec::new();
    strings_nbt(data).write(&mut out);
    out
}

/// Encodes the `strings` model into a reused buffer.
pub fn strings_write_into(data: &Strings, buffer: &mut Vec<u8>) {
    buffer.clear();
    strings_nbt(data).write(buffer);
}

fn strings_nbt(data: &Strings) -> BaseNbt {
    let mut compound = NbtCompound::new();
    compound.insert("big", NbtTag::String(data.big.clone().into()));
    compound.insert(
        "words",
        NbtTag::List(NbtList::String(
            data.words.iter().map(|word| word.clone().into()).collect(),
        )),
    );
    BaseNbt::new("", compound)
}

/// Decodes the `strings` model and returns its checksum.
///
/// # Panics
///
/// Panics when decoding fails.
pub fn strings_read(bytes: &[u8]) -> u64 {
    let value: Strings = from_bytes_typed(bytes);
    let mut acc = checksum_str(0, &value.big);
    acc = checksum(acc, value.words.len() as u64);
    for word in &value.words {
        acc = checksum_str(acc, word);
    }
    acc
}

// ------------------------------------------------------------ compounds

/// One compound item.
#[derive(simdnbt::Deserialize, Clone, Debug, PartialEq)]
pub struct CompoundItem {
    /// `TAG_Long`.
    pub id: i64,
    /// `TAG_String`.
    pub name: String,
    /// `TAG_List<TAG_String>`.
    pub tags: Vec<String>,
    /// `TAG_Float`.
    pub value: f32,
    /// `TAG_Byte`.
    pub active: bool,
}

/// The `compounds` scenario struct.
#[derive(simdnbt::Deserialize, Clone, Debug, PartialEq)]
pub struct CompoundList {
    /// The list of compounds.
    pub items: Vec<CompoundItem>,
}

fn compound_item_nbt(item: &CompoundItem) -> NbtCompound {
    let mut compound = NbtCompound::new();
    compound.insert("id", NbtTag::Long(item.id));
    compound.insert("name", NbtTag::String(item.name.clone().into()));
    compound.insert(
        "tags",
        NbtTag::List(NbtList::String(
            item.tags.iter().map(|tag| tag.clone().into()).collect(),
        )),
    );
    compound.insert("value", NbtTag::Float(item.value));
    compound.insert("active", NbtTag::Byte(i8::from(item.active)));
    compound
}

/// Builds the `compounds` model from the fixture.
pub fn compounds_from_fixture(data: &[fixtures::CompoundItem]) -> CompoundList {
    CompoundList {
        items: data
            .iter()
            .map(|item| CompoundItem {
                id: item.id,
                name: item.name.clone(),
                tags: item.tags.clone(),
                value: item.value,
                active: item.active,
            })
            .collect(),
    }
}

/// Encodes the `compounds` model.
pub fn compounds_write(data: &CompoundList) -> Vec<u8> {
    let mut out = Vec::new();
    compounds_nbt(data).write(&mut out);
    out
}

/// Encodes the `compounds` model into a reused buffer.
pub fn compounds_write_into(data: &CompoundList, buffer: &mut Vec<u8>) {
    buffer.clear();
    compounds_nbt(data).write(buffer);
}

fn compounds_nbt(data: &CompoundList) -> BaseNbt {
    let mut compound = NbtCompound::new();
    compound.insert(
        "items",
        NbtTag::List(NbtList::Compound(
            data.items.iter().map(compound_item_nbt).collect(),
        )),
    );
    BaseNbt::new("", compound)
}

/// Decodes the `compounds` model and returns its checksum.
///
/// # Panics
///
/// Panics when decoding fails.
pub fn compounds_read(bytes: &[u8]) -> u64 {
    let value: CompoundList = from_bytes_typed(bytes);
    let mut acc = checksum(0, value.items.len() as u64);
    for item in &value.items {
        acc = checksum(acc, item.id as u64);
        acc = checksum_str(acc, &item.name);
        acc = checksum(acc, item.tags.len() as u64);
        for tag in &item.tags {
            acc = checksum_str(acc, tag);
        }
        acc = checksum_f32(acc, item.value);
        acc = checksum(acc, u64::from(item.active));
    }
    acc
}

// ---------------------------------------------------------- many_fields

macro_rules! define_many_fields {
    ($($field:ident => $ty:ty => $index:tt),+ $(,)?) => {
        /// The `many_fields` scenario struct.
        #[derive(simdnbt::Deserialize, Clone, Debug, PartialEq)]
        pub struct ManyFields {
            $(
                /// A scalar field.
                pub $field: $ty,
            )+
            /// A string field.
            pub name: String,
        }

        fn many_fields_nbt(value: &ManyFields) -> BaseNbt {
            let mut compound = NbtCompound::new();
            $(
                compound.insert(
                    stringify!($field),
                    simdnbt::ToNbtTag::to_nbt_tag(value.$field),
                );
            )+
            compound.insert("name", NbtTag::String(value.name.clone().into()));
            BaseNbt::new("", compound)
        }

        fn many_fields_checksum(value: &ManyFields) -> u64 {
            let mut acc = checksum_str(0, &value.name);
            $( acc = checksum(acc, value.$field as u64); )+
            acc
        }
    };
}

define_many_fields! {
    f00 => i32 => 0, f01 => i64 => 1, f02 => i16 => 2, f03 => i8 => 3,
    f04 => i32 => 4, f05 => i64 => 5, f06 => i16 => 6, f07 => i8 => 7,
    f08 => i32 => 8, f09 => i64 => 9, f10 => i16 => 10, f11 => i8 => 11,
    f12 => i32 => 12, f13 => i64 => 13, f14 => i16 => 14, f15 => i8 => 15,
    f16 => i32 => 16, f17 => i64 => 17, f18 => i16 => 18, f19 => i8 => 19,
    f20 => i32 => 20, f21 => i64 => 21, f22 => i16 => 22, f23 => i8 => 23,
    f24 => i32 => 24, f25 => i64 => 25, f26 => i16 => 26, f27 => i8 => 27,
    f28 => i32 => 28, f29 => i64 => 29, f30 => i16 => 30, f31 => i8 => 31,
    f32 => i32 => 32, f33 => i64 => 33, f34 => i16 => 34, f35 => i8 => 35,
    f36 => i32 => 36, f37 => i64 => 37, f38 => i16 => 38, f39 => i8 => 39,
    f40 => i32 => 40, f41 => i64 => 41, f42 => i16 => 42, f43 => i8 => 43,
    f44 => i32 => 44, f45 => i64 => 45, f46 => i16 => 46, f47 => i8 => 47,
}

/// Builds the `many_fields` model from the fixture.
pub fn many_fields_from_fixture(data: &fixtures::ManyFields) -> ManyFields {
    ManyFields {
        f00: data.numbers[0] as i32,
        f01: data.numbers[1],
        f02: data.numbers[2] as i16,
        f03: data.numbers[3] as i8,
        f04: data.numbers[4] as i32,
        f05: data.numbers[5],
        f06: data.numbers[6] as i16,
        f07: data.numbers[7] as i8,
        f08: data.numbers[8] as i32,
        f09: data.numbers[9],
        f10: data.numbers[10] as i16,
        f11: data.numbers[11] as i8,
        f12: data.numbers[12] as i32,
        f13: data.numbers[13],
        f14: data.numbers[14] as i16,
        f15: data.numbers[15] as i8,
        f16: data.numbers[16] as i32,
        f17: data.numbers[17],
        f18: data.numbers[18] as i16,
        f19: data.numbers[19] as i8,
        f20: data.numbers[20] as i32,
        f21: data.numbers[21],
        f22: data.numbers[22] as i16,
        f23: data.numbers[23] as i8,
        f24: data.numbers[24] as i32,
        f25: data.numbers[25],
        f26: data.numbers[26] as i16,
        f27: data.numbers[27] as i8,
        f28: data.numbers[28] as i32,
        f29: data.numbers[29],
        f30: data.numbers[30] as i16,
        f31: data.numbers[31] as i8,
        f32: data.numbers[32] as i32,
        f33: data.numbers[33],
        f34: data.numbers[34] as i16,
        f35: data.numbers[35] as i8,
        f36: data.numbers[36] as i32,
        f37: data.numbers[37],
        f38: data.numbers[38] as i16,
        f39: data.numbers[39] as i8,
        f40: data.numbers[40] as i32,
        f41: data.numbers[41],
        f42: data.numbers[42] as i16,
        f43: data.numbers[43] as i8,
        f44: data.numbers[44] as i32,
        f45: data.numbers[45],
        f46: data.numbers[46] as i16,
        f47: data.numbers[47] as i8,
        name: data.name.clone(),
    }
}

/// Encodes the `many_fields` model.
pub fn many_fields_write(data: &ManyFields) -> Vec<u8> {
    let mut out = Vec::new();
    many_fields_nbt(data).write(&mut out);
    out
}

/// Encodes the `many_fields` model into a reused buffer.
pub fn many_fields_write_into(data: &ManyFields, buffer: &mut Vec<u8>) {
    buffer.clear();
    many_fields_nbt(data).write(buffer);
}

/// Decodes the `many_fields` model and returns its checksum.
///
/// # Panics
///
/// Panics when decoding fails.
pub fn many_fields_read(bytes: &[u8]) -> u64 {
    let value: ManyFields = from_bytes_typed(bytes);
    many_fields_checksum(&value)
}

// ----------------------------------------------------------------- deep

/// Innermost level of the `deep` model.
#[derive(simdnbt::Deserialize, Clone, Debug, PartialEq)]
pub struct Deep4 {
    /// `TAG_Long`.
    pub value: i64,
    /// `TAG_String`.
    pub name: String,
}

/// Third level of the `deep` model.
#[derive(simdnbt::Deserialize, Clone, Debug, PartialEq)]
pub struct Deep3 {
    /// `TAG_Long`.
    pub value: i64,
    /// `TAG_String`.
    pub name: String,
    /// Nested compound.
    pub inner: Deep4,
}

/// Second level of the `deep` model.
#[derive(simdnbt::Deserialize, Clone, Debug, PartialEq)]
pub struct Deep2 {
    /// `TAG_Long`.
    pub value: i64,
    /// `TAG_String`.
    pub name: String,
    /// Nested compound.
    pub inner: Deep3,
}

/// Outermost level of the `deep` model.
#[derive(simdnbt::Deserialize, Clone, Debug, PartialEq)]
pub struct Deep1 {
    /// `TAG_Long`.
    pub value: i64,
    /// `TAG_String`.
    pub name: String,
    /// Nested compound.
    pub inner: Deep2,
}

fn deep_nbt(value: &Deep1) -> BaseNbt {
    let mut inner4 = NbtCompound::new();
    inner4.insert("value", NbtTag::Long(value.inner.inner.inner.value));
    inner4.insert(
        "name",
        NbtTag::String(value.inner.inner.inner.name.clone().into()),
    );

    let mut inner3 = NbtCompound::new();
    inner3.insert("value", NbtTag::Long(value.inner.inner.value));
    inner3.insert(
        "name",
        NbtTag::String(value.inner.inner.name.clone().into()),
    );
    inner3.insert("inner", NbtTag::Compound(inner4));

    let mut inner2 = NbtCompound::new();
    inner2.insert("value", NbtTag::Long(value.inner.value));
    inner2.insert("name", NbtTag::String(value.inner.name.clone().into()));
    inner2.insert("inner", NbtTag::Compound(inner3));

    let mut compound = NbtCompound::new();
    compound.insert("value", NbtTag::Long(value.value));
    compound.insert("name", NbtTag::String(value.name.clone().into()));
    compound.insert("inner", NbtTag::Compound(inner2));
    BaseNbt::new("", compound)
}

/// Builds the `deep` model from the fixture.
pub fn deep_from_fixture(data: &fixtures::Deep1) -> Deep1 {
    Deep1 {
        value: data.value,
        name: data.name.clone(),
        inner: Deep2 {
            value: data.inner.value,
            name: data.inner.name.clone(),
            inner: Deep3 {
                value: data.inner.inner.value,
                name: data.inner.inner.name.clone(),
                inner: Deep4 {
                    value: data.inner.inner.inner.value,
                    name: data.inner.inner.inner.name.clone(),
                },
            },
        },
    }
}

/// Encodes the `deep` model.
pub fn deep_write(data: &Deep1) -> Vec<u8> {
    let mut out = Vec::new();
    deep_nbt(data).write(&mut out);
    out
}

/// Encodes the `deep` model into a reused buffer.
pub fn deep_write_into(data: &Deep1, buffer: &mut Vec<u8>) {
    buffer.clear();
    deep_nbt(data).write(buffer);
}

/// Decodes the `deep` model and returns its checksum.
///
/// # Panics
///
/// Panics when decoding fails.
pub fn deep_read(bytes: &[u8]) -> u64 {
    let value: Deep1 = from_bytes_typed(bytes);
    let mut acc = checksum(0, value.value as u64);
    acc = checksum_str(acc, &value.name);
    acc = checksum(acc, value.inner.value as u64);
    acc = checksum_str(acc, &value.inner.name);
    acc = checksum(acc, value.inner.inner.value as u64);
    acc = checksum_str(acc, &value.inner.inner.name);
    acc = checksum(acc, value.inner.inner.inner.value as u64);
    checksum_str(acc, &value.inner.inner.inner.name)
}

// ---------------------------------------------------------------- chunk

fn property_nbt(property: &fixtures::Property) -> NbtCompound {
    let mut compound = NbtCompound::new();
    compound.insert("name", NbtTag::String(property.name.clone().into()));
    compound.insert("value", NbtTag::String(property.value.clone().into()));
    compound
}

fn palette_entry_nbt(entry: &fixtures::PaletteEntry) -> NbtCompound {
    let mut compound = NbtCompound::new();
    compound.insert("name", NbtTag::String(entry.name.clone().into()));
    compound.insert(
        "properties",
        NbtTag::List(NbtList::Compound(
            entry.properties.iter().map(property_nbt).collect(),
        )),
    );
    compound
}

/// Encodes the `chunk` fixture.
pub fn chunk_write(data: &fixtures::Chunk) -> Vec<u8> {
    let mut out = Vec::new();
    chunk_nbt(data).write(&mut out);
    out
}

/// Encodes the `chunk` fixture into a reused buffer.
pub fn chunk_write_into(data: &fixtures::Chunk, buffer: &mut Vec<u8>) {
    buffer.clear();
    chunk_nbt(data).write(buffer);
}

fn chunk_nbt(data: &fixtures::Chunk) -> BaseNbt {
    let mut compound = NbtCompound::new();
    compound.insert("y", NbtTag::Byte(data.y));
    compound.insert("block_states", NbtTag::LongArray(data.block_states.clone()));
    compound.insert("biomes", NbtTag::ByteArray(data.biomes.clone()));
    compound.insert("block_light", NbtTag::ByteArray(data.block_light.clone()));
    compound.insert("sky_light", NbtTag::ByteArray(data.sky_light.clone()));
    compound.insert(
        "palette",
        NbtTag::List(NbtList::Compound(
            data.palette.iter().map(palette_entry_nbt).collect(),
        )),
    );
    BaseNbt::new("", compound)
}

/// Decodes the `chunk` document with the borrow API and returns a checksum of
/// the extracted data.
///
/// # Panics
///
/// Panics when decoding fails.
pub fn chunk_read_manual(bytes: &[u8]) -> u64 {
    let nbt = parse_borrowed(bytes);
    let mut acc = checksum(0, u64::from(nbt.byte("y").expect("y") as u8));
    acc = checksum_bytes(acc, nbt.byte_array("biomes").expect("biomes"));
    acc = checksum_bytes(acc, nbt.byte_array("block_light").expect("block_light"));
    acc = checksum_bytes(acc, nbt.byte_array("sky_light").expect("sky_light"));
    let block_states = nbt.long_array("block_states").expect("block_states");
    acc = checksum(acc, block_states.len() as u64);
    for state in block_states {
        acc = checksum(acc, state as u64);
    }
    let palette = nbt
        .list("palette")
        .and_then(|list| list.compounds())
        .expect("palette");
    acc = checksum(acc, palette.len() as u64);
    for entry in palette {
        acc = checksum_str(acc, &entry.string("name").expect("name").to_str());
        let properties = entry
            .list("properties")
            .and_then(|list| list.compounds())
            .expect("properties");
        acc = checksum(acc, properties.len() as u64);
        for property in properties {
            acc = checksum_str(acc, &property.string("name").expect("name").to_str());
            acc = checksum_str(acc, &property.string("value").expect("value").to_str());
        }
    }
    acc
}

// ---------------------------------------------------------------- trees

fn checksum_bytes(mut acc: u64, data: &[u8]) -> u64 {
    acc = checksum(acc, data.len() as u64);
    for &byte in data {
        acc = checksum(acc, u64::from(byte));
    }
    acc
}

fn borrow_compound_checksum(compound: simdnbt::borrow::NbtCompound<'_, '_>, mut acc: u64) -> u64 {
    acc = checksum(acc, compound.len() as u64);
    for (name, tag) in compound.iter() {
        acc = checksum_str(acc, &name.to_str());
        acc = borrow_tag_checksum(tag, acc);
    }
    acc
}

fn borrow_tag_checksum(tag: simdnbt::borrow::NbtTag<'_, '_>, mut acc: u64) -> u64 {
    match tag.id() {
        BYTE_ID => checksum(acc, tag.byte().unwrap_or_default() as u64),
        SHORT_ID => checksum(acc, tag.short().unwrap_or_default() as u64),
        INT_ID => checksum(acc, tag.int().unwrap_or_default() as u64),
        LONG_ID => checksum(acc, tag.long().unwrap_or_default() as u64),
        FLOAT_ID => checksum_f32(acc, tag.float().unwrap_or_default()),
        DOUBLE_ID => checksum_f64(acc, tag.double().unwrap_or_default()),
        BYTE_ARRAY_ID => checksum_bytes(acc, tag.byte_array().unwrap_or_default()),
        STRING_ID => {
            let string = tag.string().map(|value| value.to_str()).unwrap_or_default();
            acc = checksum_str(acc, &string);
            acc
        }
        LIST_ID => tag
            .list()
            .map_or(acc, |list| borrow_list_checksum(list, acc)),
        COMPOUND_ID => tag
            .compound()
            .map_or(acc, |compound| borrow_compound_checksum(compound, acc)),
        INT_ARRAY_ID => {
            let data = tag.int_array().unwrap_or_default();
            acc = checksum(acc, data.len() as u64);
            for item in data {
                acc = checksum(acc, item as u64);
            }
            acc
        }
        LONG_ARRAY_ID => {
            let data = tag.long_array().unwrap_or_default();
            acc = checksum(acc, data.len() as u64);
            for item in data {
                acc = checksum(acc, item as u64);
            }
            acc
        }
        _ => acc,
    }
}

fn borrow_list_checksum(list: simdnbt::borrow::NbtList<'_, '_>, mut acc: u64) -> u64 {
    match list.id() {
        BYTE_ID => {
            for value in list.bytes().unwrap_or_default() {
                acc = checksum(acc, *value as u64);
            }
        }
        SHORT_ID => {
            for value in list.shorts().unwrap_or_default() {
                acc = checksum(acc, value as u64);
            }
        }
        INT_ID => {
            for value in list.ints().unwrap_or_default() {
                acc = checksum(acc, value as u64);
            }
        }
        LONG_ID => {
            for value in list.longs().unwrap_or_default() {
                acc = checksum(acc, value as u64);
            }
        }
        FLOAT_ID => {
            for value in list.floats().unwrap_or_default() {
                acc = checksum_f32(acc, value);
            }
        }
        DOUBLE_ID => {
            for value in list.doubles().unwrap_or_default() {
                acc = checksum_f64(acc, value);
            }
        }
        BYTE_ARRAY_ID => {
            for value in list.byte_arrays().unwrap_or_default() {
                acc = checksum_bytes(acc, value);
            }
        }
        STRING_ID => {
            for value in list.strings().unwrap_or_default() {
                acc = checksum_str(acc, &value.to_str());
            }
        }
        LIST_ID => {
            if let Some(lists) = list.lists() {
                for sub in lists {
                    acc = borrow_list_checksum(sub, acc);
                }
            }
        }
        COMPOUND_ID => {
            if let Some(compounds) = list.compounds() {
                for compound in compounds {
                    acc = borrow_compound_checksum(compound, acc);
                }
            }
        }
        INT_ARRAY_ID => {
            for raw in list.int_arrays().unwrap_or_default() {
                let values = raw.to_vec();
                acc = checksum(acc, values.len() as u64);
                for value in values {
                    acc = checksum(acc, value as u64);
                }
            }
        }
        LONG_ARRAY_ID => {
            for raw in list.long_arrays().unwrap_or_default() {
                let values = raw.to_vec();
                acc = checksum(acc, values.len() as u64);
                for value in values {
                    acc = checksum(acc, value as u64);
                }
            }
        }
        _ => {}
    }
    acc
}

fn owned_compound_checksum(compound: &NbtCompound, mut acc: u64) -> u64 {
    acc = checksum(acc, compound.len() as u64);
    for (name, tag) in compound.iter() {
        acc = checksum_str(acc, &name.to_str());
        acc = owned_tag_checksum(tag, acc);
    }
    acc
}

fn owned_tag_checksum(tag: &NbtTag, mut acc: u64) -> u64 {
    match tag {
        NbtTag::Byte(value) => checksum(acc, *value as u64),
        NbtTag::Short(value) => checksum(acc, *value as u64),
        NbtTag::Int(value) => checksum(acc, *value as u64),
        NbtTag::Long(value) => checksum(acc, *value as u64),
        NbtTag::Float(value) => checksum_f32(acc, *value),
        NbtTag::Double(value) => checksum_f64(acc, *value),
        NbtTag::ByteArray(values) => checksum_bytes(acc, values),
        NbtTag::String(value) => checksum_str(acc, &value.to_str()),
        NbtTag::List(list) => owned_list_checksum(list, acc),
        NbtTag::Compound(compound) => owned_compound_checksum(compound, acc),
        NbtTag::IntArray(values) => {
            acc = checksum(acc, values.len() as u64);
            for value in values {
                acc = checksum(acc, *value as u64);
            }
            acc
        }
        NbtTag::LongArray(values) => {
            acc = checksum(acc, values.len() as u64);
            for value in values {
                acc = checksum(acc, *value as u64);
            }
            acc
        }
    }
}

fn owned_list_checksum(list: &NbtList, mut acc: u64) -> u64 {
    match list {
        NbtList::Empty => checksum(acc, 0),
        NbtList::Byte(values) => {
            acc = checksum(acc, values.len() as u64);
            for value in values {
                acc = checksum(acc, *value as u64);
            }
            acc
        }
        NbtList::Short(values) => {
            acc = checksum(acc, values.len() as u64);
            for value in values {
                acc = checksum(acc, *value as u64);
            }
            acc
        }
        NbtList::Int(values) => {
            acc = checksum(acc, values.len() as u64);
            for value in values {
                acc = checksum(acc, *value as u64);
            }
            acc
        }
        NbtList::Long(values) => {
            acc = checksum(acc, values.len() as u64);
            for value in values {
                acc = checksum(acc, *value as u64);
            }
            acc
        }
        NbtList::Float(values) => {
            acc = checksum(acc, values.len() as u64);
            for value in values {
                acc = checksum_f32(acc, *value);
            }
            acc
        }
        NbtList::Double(values) => {
            acc = checksum(acc, values.len() as u64);
            for value in values {
                acc = checksum_f64(acc, *value);
            }
            acc
        }
        NbtList::ByteArray(values) => {
            acc = checksum(acc, values.len() as u64);
            for value in values {
                acc = checksum_bytes(acc, value);
            }
            acc
        }
        NbtList::String(values) => {
            acc = checksum(acc, values.len() as u64);
            for value in values {
                acc = checksum_str(acc, &value.to_str());
            }
            acc
        }
        NbtList::List(values) => {
            acc = checksum(acc, values.len() as u64);
            for value in values {
                acc = owned_list_checksum(value, acc);
            }
            acc
        }
        NbtList::Compound(values) => {
            acc = checksum(acc, values.len() as u64);
            for value in values {
                acc = owned_compound_checksum(value, acc);
            }
            acc
        }
        NbtList::IntArray(values) => {
            acc = checksum(acc, values.len() as u64);
            for value in values {
                acc = checksum(acc, value.len() as u64);
                for item in value {
                    acc = checksum(acc, *item as u64);
                }
            }
            acc
        }
        NbtList::LongArray(values) => {
            acc = checksum(acc, values.len() as u64);
            for value in values {
                acc = checksum(acc, value.len() as u64);
                for item in value {
                    acc = checksum(acc, *item as u64);
                }
            }
            acc
        }
    }
}
