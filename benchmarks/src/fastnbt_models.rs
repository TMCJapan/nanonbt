//! `fastnbt` models for every scenario.
//!
//! Owned structs use `fastnbt`'s array newtypes; borrowed structs use the
//! `fastnbt::borrow` array types and `Cow<'a, str>` strings. Every read
//! benchmark walks the decoded value and returns a checksum; see
//! [`crate::checksum`].

use std::borrow::Cow;

use fastnbt::{ByteArray, IntArray, LongArray, Value};
use serde::{Deserialize, Serialize};

use crate::fixtures;
use crate::{checksum, checksum_f32, checksum_f64, checksum_str};

/// Encodes a value with `fastnbt`, allocating a fresh buffer.
///
/// # Panics
///
/// Panics when encoding fails; the fixtures are always valid.
pub fn write<T: Serialize>(value: &T) -> Vec<u8> {
    fastnbt::to_bytes(value).expect("fastnbt encode")
}

/// Encodes a value with `fastnbt` into a reused buffer.
///
/// # Panics
///
/// Panics when encoding fails; the fixtures are always valid.
pub fn write_into<T: Serialize>(value: &T, buffer: &mut Vec<u8>) {
    buffer.clear();
    fastnbt::to_writer(&mut *buffer, value).expect("fastnbt encode");
}

// ---------------------------------------------------------------- small

/// The `small` scenario struct.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
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

/// Borrowed `small` struct.
#[derive(Deserialize, Debug)]
pub struct SmallBorrowed<'a> {
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
    /// Borrowed string.
    #[serde(borrow)]
    pub name: Cow<'a, str>,
    /// `TAG_Byte`.
    pub active: bool,
    /// `TAG_Byte`.
    pub hunger: i8,
}

/// Builds the `fastnbt` `small` model from the fixture.
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

/// Decodes the owned `small` model and returns its checksum.
///
/// # Panics
///
/// Panics when decoding fails; the input comes from a valid encoding.
pub fn small_read(bytes: &[u8]) -> u64 {
    let value: Small = fastnbt::from_bytes(bytes).expect("fastnbt decode");
    small_checksum(&value)
}

/// Decodes the borrowed `small` model and returns its checksum.
///
/// # Panics
///
/// Panics when decoding fails.
pub fn small_read_borrowed(bytes: &[u8]) -> u64 {
    let value: SmallBorrowed<'_> = fastnbt::from_bytes(bytes).expect("fastnbt decode");
    let mut acc = checksum(0, value.id as u64);
    acc = checksum_f32(acc, value.health);
    acc = checksum_f64(acc, value.mana);
    acc = checksum(acc, value.level as u64);
    acc = checksum(acc, value.xp as u64);
    acc = checksum_str(acc, &value.name);
    acc = checksum(acc, u64::from(value.active));
    checksum(acc, value.hunger as u64)
}

fn small_checksum(value: &Small) -> u64 {
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

/// Owned `TAG_Byte_Array`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ByteArrayOwned {
    /// The array payload.
    pub data: ByteArray,
}

/// Borrowed `TAG_Byte_Array`.
#[derive(Deserialize, Debug)]
pub struct ByteArrayBorrowed<'a> {
    /// The array payload, borrowed from the input.
    #[serde(borrow)]
    pub data: fastnbt::borrow::ByteArray<'a>,
}

/// Builds the owned `byte_array` model from the fixture.
pub fn byte_array_from_fixture(data: &[u8]) -> ByteArrayOwned {
    ByteArrayOwned {
        data: ByteArray::new(data.iter().map(|&byte| byte as i8).collect()),
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
    let value: ByteArrayOwned = fastnbt::from_bytes(bytes).expect("fastnbt decode");
    let mut acc = checksum(0, value.data.len() as u64);
    for &byte in value.data.iter() {
        acc = checksum(acc, byte as u64);
    }
    acc
}

/// Decodes the borrowed `byte_array` model and returns its checksum.
///
/// # Panics
///
/// Panics when decoding fails.
pub fn byte_array_read_borrowed(bytes: &[u8]) -> u64 {
    let value: ByteArrayBorrowed<'_> = fastnbt::from_bytes(bytes).expect("fastnbt decode");
    let mut acc = 0u64;
    let mut count = 0u64;
    for byte in value.data.iter() {
        acc = checksum(acc, byte as u64);
        count += 1;
    }
    checksum(acc, count)
}

// ------------------------------------------------------------ int_array

/// Owned `TAG_Int_Array`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct IntArrayOwned {
    /// The array payload.
    pub data: IntArray,
}

/// Borrowed `TAG_Int_Array`.
#[derive(Deserialize, Debug)]
pub struct IntArrayBorrowed<'a> {
    /// The array payload, borrowed from the input.
    #[serde(borrow)]
    pub data: fastnbt::borrow::IntArray<'a>,
}

/// Builds the owned `int_array` model from the fixture.
pub fn int_array_from_fixture(data: &[i32]) -> IntArrayOwned {
    IntArrayOwned {
        data: IntArray::new(data.to_vec()),
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
    let value: IntArrayOwned = fastnbt::from_bytes(bytes).expect("fastnbt decode");
    let mut acc = checksum(0, value.data.len() as u64);
    for &item in value.data.iter() {
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
    let value: IntArrayBorrowed<'_> = fastnbt::from_bytes(bytes).expect("fastnbt decode");
    let mut acc = 0u64;
    let mut count = 0u64;
    for item in value.data.iter() {
        acc = checksum(acc, item as u64);
        count += 1;
    }
    checksum(acc, count)
}

// ----------------------------------------------------------- long_array

/// Owned `TAG_Long_Array`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct LongArrayOwned {
    /// The array payload.
    pub data: LongArray,
}

/// Borrowed `TAG_Long_Array`.
#[derive(Deserialize, Debug)]
pub struct LongArrayBorrowed<'a> {
    /// The array payload, borrowed from the input.
    #[serde(borrow)]
    pub data: fastnbt::borrow::LongArray<'a>,
}

/// Builds the owned `long_array` model from the fixture.
pub fn long_array_from_fixture(data: &[i64]) -> LongArrayOwned {
    LongArrayOwned {
        data: LongArray::new(data.to_vec()),
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
    let value: LongArrayOwned = fastnbt::from_bytes(bytes).expect("fastnbt decode");
    let mut acc = checksum(0, value.data.len() as u64);
    for &item in value.data.iter() {
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
    let value: LongArrayBorrowed<'_> = fastnbt::from_bytes(bytes).expect("fastnbt decode");
    let mut acc = 0u64;
    let mut count = 0u64;
    for item in value.data.iter() {
        acc = checksum(acc, item as u64);
        count += 1;
    }
    checksum(acc, count)
}

// -------------------------------------------------------------- strings

/// The `strings` scenario struct.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Strings {
    /// A single large string.
    pub big: String,
    /// A list of short strings.
    pub words: Vec<String>,
}

/// Borrowed `strings` struct.
#[derive(Deserialize, Debug)]
pub struct StringsBorrowed<'a> {
    /// Borrowed large string.
    #[serde(borrow)]
    pub big: Cow<'a, str>,
    /// Borrowed list of strings.
    #[serde(borrow)]
    pub words: Vec<Cow<'a, str>>,
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

/// Decodes the owned `strings` model and returns its checksum.
///
/// # Panics
///
/// Panics when decoding fails.
pub fn strings_read(bytes: &[u8]) -> u64 {
    let value: Strings = fastnbt::from_bytes(bytes).expect("fastnbt decode");
    strings_checksum(&value)
}

/// Decodes the borrowed `strings` model and returns its checksum.
///
/// # Panics
///
/// Panics when decoding fails.
pub fn strings_read_borrowed(bytes: &[u8]) -> u64 {
    let value: StringsBorrowed<'_> = fastnbt::from_bytes(bytes).expect("fastnbt decode");
    let mut acc = checksum_str(0, &value.big);
    acc = checksum(acc, value.words.len() as u64);
    for word in &value.words {
        acc = checksum_str(acc, word);
    }
    acc
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
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
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
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CompoundList {
    /// The list of compounds.
    pub items: Vec<CompoundItem>,
}

/// Borrowed compound item.
#[derive(Deserialize, Debug)]
pub struct CompoundItemBorrowed<'a> {
    /// `TAG_Long`.
    pub id: i64,
    /// Borrowed name.
    #[serde(borrow)]
    pub name: Cow<'a, str>,
    /// Borrowed tag list.
    #[serde(borrow)]
    pub tags: Vec<Cow<'a, str>>,
    /// `TAG_Float`.
    pub value: f32,
    /// `TAG_Byte`.
    pub active: bool,
}

/// Borrowed `compounds` struct.
#[derive(Deserialize, Debug)]
pub struct CompoundListBorrowed<'a> {
    /// Borrowed list of compounds.
    #[serde(borrow)]
    pub items: Vec<CompoundItemBorrowed<'a>>,
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

/// Decodes the owned `compounds` model and returns its checksum.
///
/// # Panics
///
/// Panics when decoding fails.
pub fn compounds_read(bytes: &[u8]) -> u64 {
    let value: CompoundList = fastnbt::from_bytes(bytes).expect("fastnbt decode");
    let mut acc = checksum(0, value.items.len() as u64);
    for item in &value.items {
        acc = compound_item_checksum(acc, item);
    }
    acc
}

/// Decodes the borrowed `compounds` model and returns its checksum.
///
/// # Panics
///
/// Panics when decoding fails.
pub fn compounds_read_borrowed(bytes: &[u8]) -> u64 {
    let value: CompoundListBorrowed<'_> = fastnbt::from_bytes(bytes).expect("fastnbt decode");
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

fn compound_item_checksum(mut acc: u64, item: &CompoundItem) -> u64 {
    acc = checksum(acc, item.id as u64);
    acc = checksum_str(acc, &item.name);
    acc = checksum(acc, item.tags.len() as u64);
    for tag in &item.tags {
        acc = checksum_str(acc, tag);
    }
    acc = checksum_f32(acc, item.value);
    checksum(acc, u64::from(item.active))
}

/// Decodes any document into a `fastnbt::Value` tree and returns its checksum.
///
/// # Panics
///
/// Panics when decoding fails.
pub fn value_read(bytes: &[u8]) -> u64 {
    let value: Value = fastnbt::from_bytes(bytes).expect("fastnbt decode");
    value_checksum(&value)
}

// ---------------------------------------------------------- many_fields

macro_rules! define_many_fields {
    ($($field:ident => $ty:ty => $index:tt),+ $(,)?) => {
        /// The `many_fields` scenario struct.
        #[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
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
    let value: ManyFields = fastnbt::from_bytes(bytes).expect("fastnbt decode");
    many_fields_checksum(&value)
}

// ----------------------------------------------------------------- deep

/// Innermost level of the `deep` model.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Deep4 {
    /// `TAG_Long`.
    pub value: i64,
    /// `TAG_String`.
    pub name: String,
}

/// Third level of the `deep` model.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Deep3 {
    /// `TAG_Long`.
    pub value: i64,
    /// `TAG_String`.
    pub name: String,
    /// Nested compound.
    pub inner: Deep4,
}

/// Second level of the `deep` model.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Deep2 {
    /// `TAG_Long`.
    pub value: i64,
    /// `TAG_String`.
    pub name: String,
    /// Nested compound.
    pub inner: Deep3,
}

/// Outermost level of the `deep` model.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Deep1 {
    /// `TAG_Long`.
    pub value: i64,
    /// `TAG_String`.
    pub name: String,
    /// Nested compound.
    pub inner: Deep2,
}

/// Borrowed innermost level.
#[derive(Deserialize, Debug)]
pub struct Deep4Borrowed<'a> {
    /// `TAG_Long`.
    pub value: i64,
    /// Borrowed string.
    #[serde(borrow)]
    pub name: Cow<'a, str>,
}

/// Borrowed third level.
#[derive(Deserialize, Debug)]
pub struct Deep3Borrowed<'a> {
    /// `TAG_Long`.
    pub value: i64,
    /// Borrowed string.
    #[serde(borrow)]
    pub name: Cow<'a, str>,
    /// Nested compound.
    #[serde(borrow)]
    pub inner: Deep4Borrowed<'a>,
}

/// Borrowed second level.
#[derive(Deserialize, Debug)]
pub struct Deep2Borrowed<'a> {
    /// `TAG_Long`.
    pub value: i64,
    /// Borrowed string.
    #[serde(borrow)]
    pub name: Cow<'a, str>,
    /// Nested compound.
    #[serde(borrow)]
    pub inner: Deep3Borrowed<'a>,
}

/// Borrowed outermost level.
#[derive(Deserialize, Debug)]
pub struct Deep1Borrowed<'a> {
    /// `TAG_Long`.
    pub value: i64,
    /// Borrowed string.
    #[serde(borrow)]
    pub name: Cow<'a, str>,
    /// Nested compound.
    #[serde(borrow)]
    pub inner: Deep2Borrowed<'a>,
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

/// Decodes the owned `deep` model and returns its checksum.
///
/// # Panics
///
/// Panics when decoding fails.
pub fn deep_read(bytes: &[u8]) -> u64 {
    let value: Deep1 = fastnbt::from_bytes(bytes).expect("fastnbt decode");
    let mut acc = checksum(0, value.value as u64);
    acc = checksum_str(acc, &value.name);
    acc = checksum(acc, value.inner.value as u64);
    acc = checksum_str(acc, &value.inner.name);
    acc = checksum(acc, value.inner.inner.value as u64);
    acc = checksum_str(acc, &value.inner.inner.name);
    acc = checksum(acc, value.inner.inner.inner.value as u64);
    checksum_str(acc, &value.inner.inner.inner.name)
}

/// Decodes the borrowed `deep` model and returns its checksum.
///
/// # Panics
///
/// Panics when decoding fails.
pub fn deep_read_borrowed(bytes: &[u8]) -> u64 {
    let value: Deep1Borrowed<'_> = fastnbt::from_bytes(bytes).expect("fastnbt decode");
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

/// One palette entry.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PaletteEntry {
    /// Block name.
    pub name: String,
    /// Block state properties.
    pub properties: Vec<Property>,
}

/// A single property.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Property {
    /// Property name.
    pub name: String,
    /// Property value.
    pub value: String,
}

/// The owned `chunk` model.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Chunk {
    /// Section Y.
    pub y: i8,
    /// Block states.
    pub block_states: LongArray,
    /// Biomes.
    pub biomes: ByteArray,
    /// Block light.
    pub block_light: ByteArray,
    /// Sky light.
    pub sky_light: ByteArray,
    /// Compound palette.
    pub palette: Vec<PaletteEntry>,
}

/// Borrowed palette entry.
#[derive(Deserialize, Debug)]
pub struct PaletteEntryBorrowed<'a> {
    /// Borrowed block name.
    #[serde(borrow)]
    pub name: Cow<'a, str>,
    /// Borrowed properties.
    #[serde(borrow)]
    pub properties: Vec<PropertyBorrowed<'a>>,
}

/// Borrowed property.
#[derive(Deserialize, Debug)]
pub struct PropertyBorrowed<'a> {
    /// Borrowed property name.
    #[serde(borrow)]
    pub name: Cow<'a, str>,
    /// Borrowed property value.
    #[serde(borrow)]
    pub value: Cow<'a, str>,
}

/// The borrowed `chunk` model.
#[derive(Deserialize, Debug)]
pub struct ChunkBorrowed<'a> {
    /// Section Y.
    pub y: i8,
    /// Borrowed block states.
    #[serde(borrow)]
    pub block_states: fastnbt::borrow::LongArray<'a>,
    /// Borrowed biomes.
    #[serde(borrow)]
    pub biomes: fastnbt::borrow::ByteArray<'a>,
    /// Borrowed block light.
    #[serde(borrow)]
    pub block_light: fastnbt::borrow::ByteArray<'a>,
    /// Borrowed sky light.
    #[serde(borrow)]
    pub sky_light: fastnbt::borrow::ByteArray<'a>,
    /// Borrowed compound palette.
    #[serde(borrow)]
    pub palette: Vec<PaletteEntryBorrowed<'a>>,
}

/// Builds the owned `chunk` model from the fixture.
pub fn chunk_from_fixture(data: &fixtures::Chunk) -> Chunk {
    Chunk {
        y: data.y,
        block_states: LongArray::new(data.block_states.clone()),
        biomes: ByteArray::new(data.biomes.iter().map(|&byte| byte as i8).collect()),
        block_light: ByteArray::new(data.block_light.iter().map(|&byte| byte as i8).collect()),
        sky_light: ByteArray::new(data.sky_light.iter().map(|&byte| byte as i8).collect()),
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
    let value: Chunk = fastnbt::from_bytes(bytes).expect("fastnbt decode");
    let mut acc = checksum(0, value.y as u64);
    acc = byte_array_checksum(acc, &value.biomes);
    acc = byte_array_checksum(acc, &value.block_light);
    acc = byte_array_checksum(acc, &value.sky_light);
    acc = checksum(acc, value.block_states.len() as u64);
    for &state in value.block_states.iter() {
        acc = checksum(acc, state as u64);
    }
    palette_checksum(acc, &value.palette)
}

/// Decodes the borrowed `chunk` model and returns its checksum.
///
/// # Panics
///
/// Panics when decoding fails.
pub fn chunk_read_borrowed(bytes: &[u8]) -> u64 {
    let value: ChunkBorrowed<'_> = fastnbt::from_bytes(bytes).expect("fastnbt decode");
    let mut acc = checksum(0, value.y as u64);
    acc = borrowed_byte_array_checksum(acc, &value.biomes);
    acc = borrowed_byte_array_checksum(acc, &value.block_light);
    acc = borrowed_byte_array_checksum(acc, &value.sky_light);
    let mut count = 0u64;
    for state in value.block_states.iter() {
        acc = checksum(acc, state as u64);
        count += 1;
    }
    acc = checksum(acc, count);
    acc = checksum(acc, value.palette.len() as u64);
    for entry in &value.palette {
        acc = checksum_str(acc, &entry.name);
        acc = checksum(acc, entry.properties.len() as u64);
        for property in &entry.properties {
            acc = checksum_str(acc, &property.name);
            acc = checksum_str(acc, &property.value);
        }
    }
    acc
}

fn byte_array_checksum(mut acc: u64, data: &ByteArray) -> u64 {
    acc = checksum(acc, data.len() as u64);
    for &byte in data.iter() {
        acc = checksum(acc, byte as u64);
    }
    acc
}

fn borrowed_byte_array_checksum(mut acc: u64, data: &fastnbt::borrow::ByteArray<'_>) -> u64 {
    let mut count = 0u64;
    for byte in data.iter() {
        acc = checksum(acc, byte as u64);
        count += 1;
    }
    checksum(acc, count)
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

// --------------------------------------------------------------- tree

/// Recursively folds a `fastnbt::Value` tree into a checksum.
///
/// Compound iteration order is unspecified (the tree stores a `HashMap`), so
/// the checksum is only used to keep the tree walk alive, never compared.
pub fn value_checksum(value: &Value) -> u64 {
    match value {
        Value::Byte(v) => checksum(0, *v as u64),
        Value::Short(v) => checksum(0, *v as u64),
        Value::Int(v) => checksum(0, *v as u64),
        Value::Long(v) => checksum(0, *v as u64),
        Value::Float(v) => checksum_f32(0, *v),
        Value::Double(v) => checksum_f64(0, *v),
        Value::String(v) => checksum_str(0, v),
        Value::ByteArray(v) => byte_array_checksum(0, v),
        Value::IntArray(v) => {
            let mut acc = checksum(0, v.len() as u64);
            for &item in v.iter() {
                acc = checksum(acc, item as u64);
            }
            acc
        }
        Value::LongArray(v) => {
            let mut acc = checksum(0, v.len() as u64);
            for &item in v.iter() {
                acc = checksum(acc, item as u64);
            }
            acc
        }
        Value::List(values) => {
            let mut acc = checksum(0, values.len() as u64);
            for item in values {
                acc = crate::checksum(acc, value_checksum(item));
            }
            acc
        }
        Value::Compound(compound) => {
            let mut acc = checksum(0, compound.len() as u64);
            for (key, item) in compound {
                acc = checksum_str(acc, key);
                acc = crate::checksum(acc, value_checksum(item));
            }
            acc
        }
    }
}
