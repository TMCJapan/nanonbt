//! Deterministic fixture data for the comparison benchmarks.
//!
//! Everything is generated from fixed seeds, so every run (and every library)
//! sees exactly the same values. Strings are restricted to plain ASCII to keep
//! the three libraries' modified-UTF-8 encoders byte-identical, and no list is
//! left empty, because the encoders disagree on the element tag of empty
//! lists.

/// Number of bytes in the `byte_array` fixture.
pub const BYTE_ARRAY_LEN: usize = 1 << 20;
/// Number of elements in the `int_array` fixture.
pub const INT_ARRAY_LEN: usize = 1 << 16;
/// Number of elements in the `long_array` fixture.
pub const LONG_ARRAY_LEN: usize = 1 << 16;
/// Number of strings in the `strings` fixture list.
pub const STRING_COUNT: usize = 2048;
/// Length of each string in the `strings` fixture list.
pub const STRING_LEN: usize = 64;
/// Length of the single large string in the `strings` fixture.
pub const BIG_STRING_LEN: usize = 32 * 1024;
/// Number of compound items in the `compounds` fixture.
pub const COMPOUND_COUNT: usize = 1024;
/// Number of strings in each compound item's tag list.
pub const TAGS_PER_ITEM: usize = 4;
/// Length of tag strings.
pub const TAG_LEN: usize = 16;
/// Name length of compound items.
pub const ITEM_NAME_LEN: usize = 24;
/// Number of palette entries in the `chunk` fixture.
pub const PALETTE_COUNT: usize = 64;
/// Number of properties per palette entry.
pub const PROPERTIES_PER_ENTRY: usize = 4;
/// Number of long values in the `chunk` fixture's block state array.
pub const BLOCK_STATES_LEN: usize = 4096;
/// Number of bytes in the `chunk` fixture's biome array.
pub const BIOMES_LEN: usize = 256;
/// Number of bytes in each of the `chunk` fixture's light arrays.
pub const LIGHT_LEN: usize = 2048;
/// Number of scalar fields in the `many_fields` fixture.
pub const MANY_FIELDS: usize = 48;

const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789_";

/// A tiny deterministic xorshift generator.
///
/// Fixtures must not depend on `rand` so that the input is stable across
/// dependency upgrades and machines.
pub struct Rng(u64);

impl Rng {
    /// Creates a generator from a seed. The seed must not be zero.
    pub const fn new(seed: u64) -> Self {
        Self(seed)
    }

    /// Returns the next raw value.
    pub const fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    /// Returns an ASCII string of `len` characters.
    pub fn ascii(&mut self, len: usize) -> String {
        let mut out = String::with_capacity(len);
        for _ in 0..len {
            out.push(char::from(
                ALPHABET[(self.next_u64() as usize) % ALPHABET.len()],
            ));
        }
        out
    }

    /// Returns an `i8` in the full range.
    pub const fn i8(&mut self) -> i8 {
        self.next_u64() as i8
    }

    /// Returns an `i16` in the full range.
    pub const fn i16(&mut self) -> i16 {
        self.next_u64() as i16
    }

    /// Returns an `i32` in the full range.
    pub const fn i32(&mut self) -> i32 {
        self.next_u64() as i32
    }

    /// Returns an `i64` in the full range.
    pub const fn i64(&mut self) -> i64 {
        self.next_u64() as i64
    }

    /// Returns a finite `f32` in `[-1000, 1000)`.
    pub fn f32(&mut self) -> f32 {
        (self.next_u64() % 2_000_000) as f32 / 1000.0 - 1000.0
    }

    /// Returns a finite `f64` in `[-1000, 1000)`.
    pub fn f64(&mut self) -> f64 {
        (self.next_u64() % 2_000_000) as f64 / 1000.0 - 1000.0
    }

    /// Returns a byte.
    pub const fn u8(&mut self) -> u8 {
        self.next_u64() as u8
    }

    /// Returns a boolean.
    pub const fn bool(&mut self) -> bool {
        self.next_u64() & 1 == 0
    }
}

/// A small struct of scalar fields.
#[derive(Clone, Debug, PartialEq)]
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
    /// `TAG_Byte` (0 or 1).
    pub active: bool,
    /// `TAG_Byte`.
    pub hunger: i8,
}

/// Builds the `small` fixture.
pub fn small() -> Small {
    let mut rng = Rng::new(0x5EED_0001);
    Small {
        id: rng.i32(),
        health: rng.f32(),
        mana: rng.f64(),
        level: rng.i16(),
        xp: rng.i64(),
        name: "Steve".to_owned(),
        active: rng.bool(),
        hunger: rng.i8(),
    }
}

/// Builds the `byte_array` fixture: a single 1 MiB `TAG_Byte_Array`.
pub fn byte_array() -> Vec<u8> {
    let mut rng = Rng::new(0x5EED_0002);
    (0..BYTE_ARRAY_LEN).map(|_| rng.u8()).collect()
}

/// Builds the `int_array` fixture: a single `TAG_Int_Array`.
pub fn int_array() -> Vec<i32> {
    let mut rng = Rng::new(0x5EED_0003);
    (0..INT_ARRAY_LEN).map(|_| rng.i32()).collect()
}

/// Builds the `long_array` fixture: a single `TAG_Long_Array`.
pub fn long_array() -> Vec<i64> {
    let mut rng = Rng::new(0x5EED_0004);
    (0..LONG_ARRAY_LEN).map(|_| rng.i64()).collect()
}

/// A string-heavy struct: one large string plus a list of short strings.
#[derive(Clone, Debug, PartialEq)]
pub struct Strings {
    /// A single large `TAG_String`.
    pub big: String,
    /// A `TAG_List<TAG_String>`.
    pub words: Vec<String>,
}

/// Builds the `strings` fixture.
pub fn strings() -> Strings {
    let mut rng = Rng::new(0x5EED_0005);
    Strings {
        big: rng.ascii(BIG_STRING_LEN),
        words: (0..STRING_COUNT).map(|_| rng.ascii(STRING_LEN)).collect(),
    }
}

/// One item of the `compounds` fixture.
#[derive(Clone, Debug, PartialEq)]
pub struct CompoundItem {
    /// `TAG_Long`.
    pub id: i64,
    /// `TAG_String`.
    pub name: String,
    /// `TAG_List<TAG_String>`.
    pub tags: Vec<String>,
    /// `TAG_Float`.
    pub value: f32,
    /// `TAG_Byte` (0 or 1).
    pub active: bool,
}

/// Builds the `compounds` fixture: a list of compound tags.
pub fn compounds() -> Vec<CompoundItem> {
    let mut rng = Rng::new(0x5EED_0006);
    (0..COMPOUND_COUNT)
        .map(|_| CompoundItem {
            id: rng.i64(),
            name: rng.ascii(ITEM_NAME_LEN),
            tags: (0..TAGS_PER_ITEM).map(|_| rng.ascii(TAG_LEN)).collect(),
            value: rng.f32(),
            active: rng.bool(),
        })
        .collect()
}

/// A struct with many scalar fields, stressing field-name lookups.
#[derive(Clone, Debug, PartialEq)]
pub struct ManyFields {
    /// Scalar values, typed in the models by index.
    pub numbers: [i64; MANY_FIELDS],
    /// `TAG_String`.
    pub name: String,
}

/// Builds the `many_fields` fixture.
pub fn many_fields() -> ManyFields {
    let mut rng = Rng::new(0x5EED_0007);
    let mut numbers = [0i64; MANY_FIELDS];
    for value in &mut numbers {
        *value = rng.i64();
    }
    ManyFields {
        numbers,
        name: rng.ascii(32),
    }
}

/// One palette entry of the `chunk` fixture.
#[derive(Clone, Debug, PartialEq)]
pub struct PaletteEntry {
    /// Block name such as `minecraft:stone`.
    pub name: String,
    /// Block state properties.
    pub properties: Vec<Property>,
}

/// A single block state property.
#[derive(Clone, Debug, PartialEq)]
pub struct Property {
    /// Property name.
    pub name: String,
    /// Property value.
    pub value: String,
}

/// A Minecraft-flavored chunk section.
#[derive(Clone, Debug, PartialEq)]
pub struct Chunk {
    /// Section Y coordinate.
    pub y: i8,
    /// `TAG_Long_Array` block states.
    pub block_states: Vec<i64>,
    /// `TAG_Byte_Array` biomes.
    pub biomes: Vec<u8>,
    /// `TAG_Byte_Array` block light.
    pub block_light: Vec<u8>,
    /// `TAG_Byte_Array` sky light.
    pub sky_light: Vec<u8>,
    /// `TAG_List<TAG_Compound>` palette.
    pub palette: Vec<PaletteEntry>,
}

/// Builds the `chunk` fixture.
pub fn chunk() -> Chunk {
    let mut rng = Rng::new(0x5EED_0008);
    Chunk {
        y: 4,
        block_states: (0..BLOCK_STATES_LEN).map(|_| rng.i64()).collect(),
        biomes: (0..BIOMES_LEN).map(|_| rng.u8()).collect(),
        block_light: (0..LIGHT_LEN).map(|_| rng.u8()).collect(),
        sky_light: (0..LIGHT_LEN).map(|_| rng.u8()).collect(),
        palette: (0..PALETTE_COUNT)
            .map(|_| PaletteEntry {
                name: rng.ascii(24),
                properties: (0..PROPERTIES_PER_ENTRY)
                    .map(|_| Property {
                        name: rng.ascii(12),
                        value: rng.ascii(12),
                    })
                    .collect(),
            })
            .collect(),
    }
}

/// The innermost level of the `deep` fixture.
#[derive(Clone, Debug, PartialEq)]
pub struct Deep4 {
    /// `TAG_Long`.
    pub value: i64,
    /// `TAG_String`.
    pub name: String,
}

/// Third level of the `deep` fixture.
#[derive(Clone, Debug, PartialEq)]
pub struct Deep3 {
    /// `TAG_Long`.
    pub value: i64,
    /// `TAG_String`.
    pub name: String,
    /// Nested compound.
    pub inner: Deep4,
}

/// Second level of the `deep` fixture.
#[derive(Clone, Debug, PartialEq)]
pub struct Deep2 {
    /// `TAG_Long`.
    pub value: i64,
    /// `TAG_String`.
    pub name: String,
    /// Nested compound.
    pub inner: Deep3,
}

/// The outermost level of the `deep` fixture.
#[derive(Clone, Debug, PartialEq)]
pub struct Deep1 {
    /// `TAG_Long`.
    pub value: i64,
    /// `TAG_String`.
    pub name: String,
    /// Nested compound.
    pub inner: Deep2,
}

/// Builds the `deep` fixture.
pub fn deep() -> Deep1 {
    let mut rng = Rng::new(0x5EED_0009);
    Deep1 {
        value: rng.i64(),
        name: rng.ascii(16),
        inner: Deep2 {
            value: rng.i64(),
            name: rng.ascii(16),
            inner: Deep3 {
                value: rng.i64(),
                name: rng.ascii(16),
                inner: Deep4 {
                    value: rng.i64(),
                    name: rng.ascii(16),
                },
            },
        },
    }
}
