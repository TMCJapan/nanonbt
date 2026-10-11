//! `nanonbt` models for every scenario.
//!
//! Every read benchmark returns a checksum of the decoded value so that the
//! payload is actually touched; see [`crate::checksum`].

use nanonbt::{I32Be, I64Be, from_bytes, from_slice, to_bytes};

use crate::fixtures;
use crate::{checksum, checksum_f32, checksum_f64, checksum_str};

/// Encodes a value with `nanonbt`, allocating a fresh buffer.
///
/// # Panics
///
/// Panics when encoding fails; the fixtures are always valid.
pub fn write<T: nanonbt::SerializeNbt + ?Sized>(value: &T) -> Vec<u8> {
    to_bytes(value).expect("nanonbt encode")
}

/// Encodes a value with `nanonbt` into a reused buffer.
///
/// # Panics
///
/// Panics when encoding fails; the fixtures are always valid.
pub fn write_into<T: nanonbt::SerializeNbt + ?Sized>(value: &T, buffer: &mut Vec<u8>) {
    buffer.clear();
    nanonbt::to_writer(buffer, value).expect("nanonbt encode");
}

// ---------------------------------------------------------------- small

/// The `small` scenario struct.
#[derive(nanonbt::SerializeNbt, nanonbt::DeserializeNbt, Clone, Debug, PartialEq)]
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

/// Builds the `nanonbt` `small` model from the fixture.
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
    write(data)
}

/// Encodes the `small` model into a reused buffer.
pub fn small_write_into(data: &Small, buffer: &mut Vec<u8>) {
    write_into(data, buffer);
}

/// Decodes the `small` model and returns its checksum.
///
/// # Panics
///
/// Panics when decoding fails; the input comes from a valid encoding.
pub fn small_read(bytes: &[u8]) -> u64 {
    let value: Small = from_bytes(bytes).expect("nanonbt decode");
    small_checksum(&value)
}

fn small_checksum(value: &Small) -> u64 {
    let mut acc = 0u64;
    acc = checksum(acc, value.id as u64);
    acc = checksum_f32(acc, value.health);
    acc = checksum_f64(acc, value.mana);
    acc = checksum(acc, value.level as u64);
    acc = checksum(acc, value.xp as u64);
    acc = checksum_str(acc, &value.name);
    acc = checksum(acc, u64::from(value.active));
    checksum(acc, value.hunger as u64)
}

// ----------------------------------------------------------- byte_array

/// Owned `TAG_Byte_Array`.
#[derive(nanonbt::SerializeNbt, nanonbt::DeserializeNbt, Clone, Debug, PartialEq)]
pub struct ByteArrayOwned {
    /// The array payload.
    #[nbt(array)]
    pub data: Vec<u8>,
}

/// Borrowed `TAG_Byte_Array`.
#[derive(nanonbt::SerializeNbt, nanonbt::DeserializeNbt, Clone, Debug, PartialEq)]
pub struct ByteArrayBorrowed<'de> {
    /// The array payload, borrowed from the input.
    #[nbt(array)]
    pub data: &'de [u8],
}

/// Builds the owned `byte_array` model from the fixture.
pub fn byte_array_from_fixture(data: &[u8]) -> ByteArrayOwned {
    ByteArrayOwned {
        data: data.to_vec(),
    }
}

/// Encodes the owned `byte_array` model.
pub fn byte_array_owned(data: &ByteArrayOwned) -> Vec<u8> {
    write(data)
}

/// Encodes the owned `byte_array` model into a reused buffer.
pub fn byte_array_write_into(data: &ByteArrayOwned, buffer: &mut Vec<u8>) {
    write_into(data, buffer);
}

/// Decodes the owned `byte_array` model and returns its checksum.
///
/// # Panics
///
/// Panics when decoding fails.
pub fn byte_array_read(bytes: &[u8]) -> u64 {
    let value: ByteArrayOwned = from_bytes(bytes).expect("nanonbt decode");
    checksum_bytes_slice(0, &value.data)
}

/// Decodes the borrowed `byte_array` model and returns its checksum.
///
/// # Panics
///
/// Panics when decoding fails.
pub fn byte_array_read_borrowed(bytes: &[u8]) -> u64 {
    let value: ByteArrayBorrowed<'_> = from_slice(bytes).expect("nanonbt decode");
    checksum_bytes_slice(0, value.data)
}

fn checksum_bytes_slice(mut acc: u64, data: &[u8]) -> u64 {
    acc = checksum(acc, data.len() as u64);
    for &byte in data {
        acc = checksum(acc, u64::from(byte));
    }
    acc
}

// ------------------------------------------------------------ int_array

/// Owned `TAG_Int_Array` of native integers.
#[derive(nanonbt::SerializeNbt, nanonbt::DeserializeNbt, Clone, Debug, PartialEq)]
pub struct IntArrayOwned {
    /// The array payload.
    #[nbt(array)]
    pub data: Vec<i32>,
}

/// Borrowed `TAG_Int_Array` as big-endian wire values.
#[derive(nanonbt::SerializeNbt, nanonbt::DeserializeNbt, Clone, Debug, PartialEq)]
pub struct IntArrayBorrowed<'de> {
    /// The array payload, borrowed from the input.
    #[nbt(array)]
    pub data: &'de [I32Be],
}

/// Builds the owned `int_array` model from the fixture.
pub fn int_array_from_fixture(data: &[i32]) -> IntArrayOwned {
    IntArrayOwned {
        data: data.to_vec(),
    }
}

/// Encodes the owned `int_array` model.
pub fn int_array_owned(data: &IntArrayOwned) -> Vec<u8> {
    write(data)
}

/// Encodes the owned `int_array` model into a reused buffer.
pub fn int_array_write_into(data: &IntArrayOwned, buffer: &mut Vec<u8>) {
    write_into(data, buffer);
}

/// Decodes the owned `int_array` model and returns its checksum.
///
/// # Panics
///
/// Panics when decoding fails.
pub fn int_array_read(bytes: &[u8]) -> u64 {
    let value: IntArrayOwned = from_bytes(bytes).expect("nanonbt decode");
    let mut acc = checksum(0, value.data.len() as u64);
    for &item in &value.data {
        acc = checksum(acc, item as u64);
    }
    acc
}

/// Decodes the borrowed `int_array` model and returns its checksum.
///
/// # Panics
///
/// Panics when decoding fails.
pub fn int_array_read_borrowed(bytes: &[u8]) -> u64 {
    let value: IntArrayBorrowed<'_> = from_slice(bytes).expect("nanonbt decode");
    let mut acc = checksum(0, value.data.len() as u64);
    for item in value.data {
        acc = checksum(acc, item.get() as u64);
    }
    acc
}

// ----------------------------------------------------------- long_array

/// Owned `TAG_Long_Array` of native integers.
#[derive(nanonbt::SerializeNbt, nanonbt::DeserializeNbt, Clone, Debug, PartialEq)]
pub struct LongArrayOwned {
    /// The array payload.
    #[nbt(array)]
    pub data: Vec<i64>,
}

/// Borrowed `TAG_Long_Array` as big-endian wire values.
#[derive(nanonbt::SerializeNbt, nanonbt::DeserializeNbt, Clone, Debug, PartialEq)]
pub struct LongArrayBorrowed<'de> {
    /// The array payload, borrowed from the input.
    #[nbt(array)]
    pub data: &'de [I64Be],
}

/// Builds the owned `long_array` model from the fixture.
pub fn long_array_from_fixture(data: &[i64]) -> LongArrayOwned {
    LongArrayOwned {
        data: data.to_vec(),
    }
}

/// Encodes the owned `long_array` model.
pub fn long_array_owned(data: &LongArrayOwned) -> Vec<u8> {
    write(data)
}

/// Encodes the owned `long_array` model into a reused buffer.
pub fn long_array_write_into(data: &LongArrayOwned, buffer: &mut Vec<u8>) {
    write_into(data, buffer);
}

/// Decodes the owned `long_array` model and returns its checksum.
///
/// # Panics
///
/// Panics when decoding fails.
pub fn long_array_read(bytes: &[u8]) -> u64 {
    let value: LongArrayOwned = from_bytes(bytes).expect("nanonbt decode");
    let mut acc = checksum(0, value.data.len() as u64);
    for &item in &value.data {
        acc = checksum(acc, item as u64);
    }
    acc
}

/// Decodes the borrowed `long_array` model and returns its checksum.
///
/// # Panics
///
/// Panics when decoding fails.
pub fn long_array_read_borrowed(bytes: &[u8]) -> u64 {
    let value: LongArrayBorrowed<'_> = from_slice(bytes).expect("nanonbt decode");
    let mut acc = checksum(0, value.data.len() as u64);
    for item in value.data {
        acc = checksum(acc, item.get() as u64);
    }
    acc
}

// -------------------------------------------------------------- strings

/// The `strings` scenario struct.
#[derive(nanonbt::SerializeNbt, nanonbt::DeserializeNbt, Clone, Debug, PartialEq)]
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
    write(data)
}

/// Encodes the `strings` model into a reused buffer.
pub fn strings_write_into(data: &Strings, buffer: &mut Vec<u8>) {
    write_into(data, buffer);
}

/// Decodes the `strings` model and returns its checksum.
///
/// # Panics
///
/// Panics when decoding fails.
pub fn strings_read(bytes: &[u8]) -> u64 {
    let value: Strings = from_bytes(bytes).expect("nanonbt decode");
    strings_checksum(&value)
}

fn strings_checksum(value: &Strings) -> u64 {
    let mut acc = checksum_str(0, &value.big);
    acc = checksum(acc, value.words.len() as u64);
    for word in &value.words {
        acc = checksum_str(acc, word);
    }
    acc
}

// ------------------------------------------------------------ compounds

/// One compound item.
#[derive(nanonbt::SerializeNbt, nanonbt::DeserializeNbt, Clone, Debug, PartialEq)]
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
#[derive(nanonbt::SerializeNbt, nanonbt::DeserializeNbt, Clone, Debug, PartialEq)]
pub struct CompoundList {
    /// The list of compounds.
    pub items: Vec<CompoundItem>,
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
    write(data)
}

/// Encodes the `compounds` model into a reused buffer.
pub fn compounds_write_into(data: &CompoundList, buffer: &mut Vec<u8>) {
    write_into(data, buffer);
}

/// Decodes the `compounds` model and returns its checksum.
///
/// # Panics
///
/// Panics when decoding fails.
pub fn compounds_read(bytes: &[u8]) -> u64 {
    let value: CompoundList = from_bytes(bytes).expect("nanonbt decode");
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
        #[derive(nanonbt::SerializeNbt, nanonbt::DeserializeNbt, Clone, Debug, PartialEq)]
        pub struct ManyFields {
            $(
                /// A scalar field.
                pub $field: $ty,
            )+
            /// A string field.
            pub name: String,
        }

        /// Builds the `many_fields` model from the fixture.
        pub fn many_fields_from_fixture(data: &fixtures::ManyFields) -> ManyFields {
            ManyFields {
                $( $field: data.numbers[$index] as $ty, )+
                name: data.name.clone(),
            }
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

/// Encodes the `many_fields` model.
pub fn many_fields_write(data: &ManyFields) -> Vec<u8> {
    write(data)
}

/// Encodes the `many_fields` model into a reused buffer.
pub fn many_fields_write_into(data: &ManyFields, buffer: &mut Vec<u8>) {
    write_into(data, buffer);
}

/// Decodes the `many_fields` model and returns its checksum.
///
/// # Panics
///
/// Panics when decoding fails.
pub fn many_fields_read(bytes: &[u8]) -> u64 {
    let value: ManyFields = from_bytes(bytes).expect("nanonbt decode");
    many_fields_checksum(&value)
}

// ----------------------------------------------------------------- deep

/// Innermost level of the `deep` model.
#[derive(nanonbt::SerializeNbt, nanonbt::DeserializeNbt, Clone, Debug, PartialEq)]
pub struct Deep4 {
    /// `TAG_Long`.
    pub value: i64,
    /// `TAG_String`.
    pub name: String,
}

/// Third level of the `deep` model.
#[derive(nanonbt::SerializeNbt, nanonbt::DeserializeNbt, Clone, Debug, PartialEq)]
pub struct Deep3 {
    /// `TAG_Long`.
    pub value: i64,
    /// `TAG_String`.
    pub name: String,
    /// Nested compound.
    pub inner: Deep4,
}

/// Second level of the `deep` model.
#[derive(nanonbt::SerializeNbt, nanonbt::DeserializeNbt, Clone, Debug, PartialEq)]
pub struct Deep2 {
    /// `TAG_Long`.
    pub value: i64,
    /// `TAG_String`.
    pub name: String,
    /// Nested compound.
    pub inner: Deep3,
}

/// Outermost level of the `deep` model.
#[derive(nanonbt::SerializeNbt, nanonbt::DeserializeNbt, Clone, Debug, PartialEq)]
pub struct Deep1 {
    /// `TAG_Long`.
    pub value: i64,
    /// `TAG_String`.
    pub name: String,
    /// Nested compound.
    pub inner: Deep2,
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
    write(data)
}

/// Encodes the `deep` model into a reused buffer.
pub fn deep_write_into(data: &Deep1, buffer: &mut Vec<u8>) {
    write_into(data, buffer);
}

/// Decodes the `deep` model and returns its checksum.
///
/// # Panics
///
/// Panics when decoding fails.
pub fn deep_read(bytes: &[u8]) -> u64 {
    let value: Deep1 = from_bytes(bytes).expect("nanonbt decode");
    deep_checksum(&value)
}

fn deep_checksum(value: &Deep1) -> u64 {
    let mut acc = 0u64;
    acc = checksum(acc, value.value as u64);
    acc = checksum_str(acc, &value.name);
    acc = checksum(acc, value.inner.value as u64);
    acc = checksum_str(acc, &value.inner.name);
    acc = checksum(acc, value.inner.inner.value as u64);
    acc = checksum_str(acc, &value.inner.inner.name);
    acc = checksum(acc, value.inner.inner.inner.value as u64);
    checksum_str(acc, &value.inner.inner.inner.name)
}

// ---------------------------------------------------------------- chunk

/// One palette entry.
#[derive(nanonbt::SerializeNbt, nanonbt::DeserializeNbt, Clone, Debug, PartialEq)]
pub struct PaletteEntry {
    /// Block name.
    pub name: String,
    /// Block state properties.
    pub properties: Vec<Property>,
}

/// A single property.
#[derive(nanonbt::SerializeNbt, nanonbt::DeserializeNbt, Clone, Debug, PartialEq)]
pub struct Property {
    /// Property name.
    pub name: String,
    /// Property value.
    pub value: String,
}

/// The owned `chunk` model.
#[derive(nanonbt::SerializeNbt, nanonbt::DeserializeNbt, Clone, Debug, PartialEq)]
pub struct Chunk {
    /// Section Y.
    pub y: i8,
    /// `TAG_Long_Array` block states.
    #[nbt(array = long)]
    pub block_states: Vec<i64>,
    /// `TAG_Byte_Array` biomes.
    #[nbt(array = byte)]
    pub biomes: Vec<u8>,
    /// `TAG_Byte_Array` block light.
    #[nbt(array = byte)]
    pub block_light: Vec<u8>,
    /// `TAG_Byte_Array` sky light.
    #[nbt(array = byte)]
    pub sky_light: Vec<u8>,
    /// Compound palette.
    pub palette: Vec<PaletteEntry>,
}

/// The borrowed `chunk` model: arrays borrow, the palette is owned because
/// `nanonbt` cannot derive borrowed list elements.
#[derive(nanonbt::SerializeNbt, nanonbt::DeserializeNbt, Clone, Debug, PartialEq)]
pub struct ChunkBorrowed<'de> {
    /// Section Y.
    pub y: i8,
    /// Borrowed block states.
    #[nbt(array = long)]
    pub block_states: &'de [I64Be],
    /// Borrowed biomes.
    #[nbt(array = byte)]
    pub biomes: &'de [u8],
    /// Borrowed block light.
    #[nbt(array = byte)]
    pub block_light: &'de [u8],
    /// Borrowed sky light.
    #[nbt(array = byte)]
    pub sky_light: &'de [u8],
    /// Owned compound palette.
    pub palette: Vec<PaletteEntry>,
}

/// Builds the owned `chunk` model from the fixture.
pub fn chunk_from_fixture(data: &fixtures::Chunk) -> Chunk {
    Chunk {
        y: data.y,
        block_states: data.block_states.clone(),
        biomes: data.biomes.clone(),
        block_light: data.block_light.clone(),
        sky_light: data.sky_light.clone(),
        palette: palette_from_fixture(&data.palette),
    }
}

fn palette_from_fixture(data: &[fixtures::PaletteEntry]) -> Vec<PaletteEntry> {
    data.iter()
        .map(|entry| PaletteEntry {
            name: entry.name.clone(),
            properties: entry
                .properties
                .iter()
                .map(|property| Property {
                    name: property.name.clone(),
                    value: property.value.clone(),
                })
                .collect(),
        })
        .collect()
}

/// Encodes the owned `chunk` model.
pub fn chunk_write(data: &Chunk) -> Vec<u8> {
    write(data)
}

/// Encodes the owned `chunk` model into a reused buffer.
pub fn chunk_write_into(data: &Chunk, buffer: &mut Vec<u8>) {
    write_into(data, buffer);
}

/// Decodes the owned `chunk` model and returns its checksum.
///
/// # Panics
///
/// Panics when decoding fails.
pub fn chunk_read(bytes: &[u8]) -> u64 {
    let value: Chunk = from_bytes(bytes).expect("nanonbt decode");
    chunk_checksum(&value)
}

/// Decodes the borrowed `chunk` model and returns its checksum.
///
/// # Panics
///
/// Panics when decoding fails.
pub fn chunk_read_borrowed(bytes: &[u8]) -> u64 {
    let value: ChunkBorrowed<'_> = from_slice(bytes).expect("nanonbt decode");
    let mut acc = checksum_bytes_slice(0, value.biomes);
    acc = checksum_bytes_slice(acc, value.block_light);
    acc = checksum_bytes_slice(acc, value.sky_light);
    acc = checksum(acc, value.block_states.len() as u64);
    for state in value.block_states {
        acc = checksum(acc, state.get() as u64);
    }
    acc = checksum(acc, value.y as u64);
    palette_checksum(acc, &value.palette)
}

fn chunk_checksum(value: &Chunk) -> u64 {
    let mut acc = checksum_bytes_slice(0, &value.biomes);
    acc = checksum_bytes_slice(acc, &value.block_light);
    acc = checksum_bytes_slice(acc, &value.sky_light);
    acc = checksum(acc, value.block_states.len() as u64);
    for state in &value.block_states {
        acc = checksum(acc, *state as u64);
    }
    acc = checksum(acc, value.y as u64);
    palette_checksum(acc, &value.palette)
}

fn palette_checksum(mut acc: u64, palette: &[PaletteEntry]) -> u64 {
    acc = checksum(acc, palette.len() as u64);
    for entry in palette {
        acc = checksum_str(acc, &entry.name);
        acc = checksum(acc, entry.properties.len() as u64);
        for property in &entry.properties {
            acc = checksum_str(acc, &property.name);
            acc = checksum_str(acc, &property.value);
        }
    }
    acc
}
