//! The `pumpkin-nbt` entry: one plain struct per document, filled through
//! `NbtCompound` accessors and written back with `put_*`.
//!
//! pumpkin-nbt has no serde support, so the mapping is hand-written; that is
//! also how its users read typed data.

use std::io::Cursor;

use criterion::{BenchmarkGroup, measurement::WallTime};
use pumpkin_nbt::{Nbt, NbtCompound, deserializer::NbtReadHelperJava, tag::NbtTag};

use crate::documents::{Array, Doc};
use crate::{bench_parse, bench_write};

/// Extracts each tag of a list of compounds.
fn compounds<T>(list: &[NbtTag], from: impl Fn(&NbtCompound) -> T) -> Vec<T> {
    list.iter()
        .map(|tag| from(tag.extract_compound().expect("compound")))
        .collect()
}

fn doubles(list: &[NbtTag]) -> Vec<f64> {
    list.iter()
        .map(|tag| tag.extract_double().expect("double"))
        .collect()
}

fn floats(list: &[NbtTag]) -> Vec<f32> {
    list.iter()
        .map(|tag| tag.extract_float().expect("float"))
        .collect()
}

fn strings(list: &[NbtTag]) -> Vec<String> {
    list.iter()
        .map(|tag| tag.extract_string().expect("string").to_owned())
        .collect()
}

fn double_list(values: &[f64]) -> Vec<NbtTag> {
    values.iter().map(|v| NbtTag::Double(*v)).collect()
}

fn float_list(values: &[f32]) -> Vec<NbtTag> {
    values.iter().map(|v| NbtTag::Float(*v)).collect()
}

fn string_list(values: &[String]) -> Vec<NbtTag> {
    values
        .iter()
        .map(|v| NbtTag::String(v.as_str().into()))
        .collect()
}

#[derive(Debug)]
pub struct Small {
    dimension: String,
    x: i32,
    y: i32,
    z: i32,
    yaw: f32,
    pitch: f32,
    on_ground: i8,
    health: i16,
}

impl Small {
    fn from_compound(c: &NbtCompound) -> Self {
        Self {
            dimension: c.get_string("dimension").expect("dimension").to_owned(),
            x: c.get_int("x").expect("x"),
            y: c.get_int("y").expect("y"),
            z: c.get_int("z").expect("z"),
            yaw: c.get_float("yaw").expect("yaw"),
            pitch: c.get_float("pitch").expect("pitch"),
            on_ground: c.get_byte("on_ground").expect("on_ground"),
            health: c.get_short("health").expect("health"),
        }
    }

    fn to_compound(&self) -> NbtCompound {
        let mut c = NbtCompound::new();
        c.put_string("dimension", self.dimension.clone());
        c.put_int("x", self.x);
        c.put_int("y", self.y);
        c.put_int("z", self.z);
        c.put_float("yaw", self.yaw);
        c.put_float("pitch", self.pitch);
        c.put_byte("on_ground", self.on_ground);
        c.put_short("health", self.health);
        c
    }
}

#[allow(non_snake_case)]
#[derive(Debug)]
pub struct Player {
    DataVersion: i32,
    Health: f32,
    foodLevel: i32,
    XpLevel: i32,
    playerGameType: i32,
    UUID: Vec<i32>,
    Pos: Vec<f64>,
    Motion: Vec<f64>,
    Rotation: Vec<f32>,
    Inventory: Vec<InventoryItem>,
    Attributes: Vec<Attribute>,
    EnderItems: Vec<InventoryItem>,
    abilities: Abilities,
    recipeBook: RecipeBook,
    Tags: Vec<String>,
}

#[allow(non_snake_case)]
#[derive(Debug)]
pub struct InventoryItem {
    Slot: i8,
    id: String,
    Count: i8,
    tag: ItemTag,
}

#[allow(non_snake_case)]
#[derive(Debug)]
pub struct ItemTag {
    Damage: i32,
    display: Display,
}

#[allow(non_snake_case)]
#[derive(Debug)]
pub struct Display {
    Name: String,
}

#[allow(non_snake_case)]
#[derive(Debug)]
pub struct Attribute {
    Name: String,
    Base: f64,
    Modifiers: Vec<Modifier>,
}

#[allow(non_snake_case)]
#[derive(Debug)]
pub struct Modifier {
    Amount: f64,
}

#[allow(non_snake_case)]
#[derive(Debug)]
pub struct Abilities {
    flying: i8,
    mayfly: i8,
    invulnerable: i8,
    walkSpeed: f32,
}

#[allow(non_snake_case)]
#[derive(Debug)]
pub struct RecipeBook {
    isFilteringCraftable: i8,
    recipes: Vec<String>,
    toBeDisplayed: Vec<String>,
}

impl Player {
    fn from_compound(c: &NbtCompound) -> Self {
        Self {
            DataVersion: c.get_int("DataVersion").expect("DataVersion"),
            Health: c.get_float("Health").expect("Health"),
            foodLevel: c.get_int("foodLevel").expect("foodLevel"),
            XpLevel: c.get_int("XpLevel").expect("XpLevel"),
            playerGameType: c.get_int("playerGameType").expect("playerGameType"),
            UUID: c.get_int_array("UUID").expect("UUID").to_vec(),
            Pos: doubles(c.get_list("Pos").expect("Pos")),
            Motion: doubles(c.get_list("Motion").expect("Motion")),
            Rotation: floats(c.get_list("Rotation").expect("Rotation")),
            Inventory: compounds(
                c.get_list("Inventory").expect("Inventory"),
                InventoryItem::from_compound,
            ),
            Attributes: compounds(
                c.get_list("Attributes").expect("Attributes"),
                Attribute::from_compound,
            ),
            EnderItems: compounds(
                c.get_list("EnderItems").expect("EnderItems"),
                InventoryItem::from_compound,
            ),
            abilities: Abilities::from_compound(c.get_compound("abilities").expect("abilities")),
            recipeBook: RecipeBook::from_compound(
                c.get_compound("recipeBook").expect("recipeBook"),
            ),
            Tags: strings(c.get_list("Tags").expect("Tags")),
        }
    }

    fn to_compound(&self) -> NbtCompound {
        let mut c = NbtCompound::new();
        c.put_int("DataVersion", self.DataVersion);
        c.put_float("Health", self.Health);
        c.put_int("foodLevel", self.foodLevel);
        c.put_int("XpLevel", self.XpLevel);
        c.put_int("playerGameType", self.playerGameType);
        c.put("UUID", NbtTag::IntArray(self.UUID.clone()));
        c.put("Pos", NbtTag::List(double_list(&self.Pos)));
        c.put("Motion", NbtTag::List(double_list(&self.Motion)));
        c.put("Rotation", NbtTag::List(float_list(&self.Rotation)));
        c.put(
            "Inventory",
            NbtTag::List(
                self.Inventory
                    .iter()
                    .map(InventoryItem::to_compound)
                    .map(NbtTag::Compound)
                    .collect(),
            ),
        );
        c.put(
            "Attributes",
            NbtTag::List(
                self.Attributes
                    .iter()
                    .map(Attribute::to_compound)
                    .map(NbtTag::Compound)
                    .collect(),
            ),
        );
        c.put(
            "EnderItems",
            NbtTag::List(
                self.EnderItems
                    .iter()
                    .map(InventoryItem::to_compound)
                    .map(NbtTag::Compound)
                    .collect(),
            ),
        );
        c.put("abilities", self.abilities.to_compound());
        c.put("recipeBook", self.recipeBook.to_compound());
        c.put("Tags", NbtTag::List(string_list(&self.Tags)));
        c
    }
}

impl InventoryItem {
    fn from_compound(c: &NbtCompound) -> Self {
        Self {
            Slot: c.get_byte("Slot").expect("Slot"),
            id: c.get_string("id").expect("id").to_owned(),
            Count: c.get_byte("Count").expect("Count"),
            tag: ItemTag::from_compound(c.get_compound("tag").expect("tag")),
        }
    }

    fn to_compound(&self) -> NbtCompound {
        let mut c = NbtCompound::new();
        c.put_byte("Slot", self.Slot);
        c.put_string("id", self.id.clone());
        c.put_byte("Count", self.Count);
        c.put("tag", self.tag.to_compound());
        c
    }
}

impl ItemTag {
    fn from_compound(c: &NbtCompound) -> Self {
        Self {
            Damage: c.get_int("Damage").expect("Damage"),
            display: Display::from_compound(c.get_compound("display").expect("display")),
        }
    }

    fn to_compound(&self) -> NbtCompound {
        let mut c = NbtCompound::new();
        c.put_int("Damage", self.Damage);
        c.put("display", self.display.to_compound());
        c
    }
}

impl Display {
    fn from_compound(c: &NbtCompound) -> Self {
        Self {
            Name: c.get_string("Name").expect("Name").to_owned(),
        }
    }

    fn to_compound(&self) -> NbtCompound {
        let mut c = NbtCompound::new();
        c.put_string("Name", self.Name.clone());
        c
    }
}

impl Attribute {
    fn from_compound(c: &NbtCompound) -> Self {
        Self {
            Name: c.get_string("Name").expect("Name").to_owned(),
            Base: c.get_double("Base").expect("Base"),
            Modifiers: compounds(
                c.get_list("Modifiers").expect("Modifiers"),
                Modifier::from_compound,
            ),
        }
    }

    fn to_compound(&self) -> NbtCompound {
        let mut c = NbtCompound::new();
        c.put_string("Name", self.Name.clone());
        c.put_double("Base", self.Base);
        c.put(
            "Modifiers",
            NbtTag::List(
                self.Modifiers
                    .iter()
                    .map(Modifier::to_compound)
                    .map(NbtTag::Compound)
                    .collect(),
            ),
        );
        c
    }
}

impl Modifier {
    fn from_compound(c: &NbtCompound) -> Self {
        Self {
            Amount: c.get_double("Amount").expect("Amount"),
        }
    }

    fn to_compound(&self) -> NbtCompound {
        let mut c = NbtCompound::new();
        c.put_double("Amount", self.Amount);
        c
    }
}

impl Abilities {
    fn from_compound(c: &NbtCompound) -> Self {
        Self {
            flying: c.get_byte("flying").expect("flying"),
            mayfly: c.get_byte("mayfly").expect("mayfly"),
            invulnerable: c.get_byte("invulnerable").expect("invulnerable"),
            walkSpeed: c.get_float("walkSpeed").expect("walkSpeed"),
        }
    }

    fn to_compound(&self) -> NbtCompound {
        let mut c = NbtCompound::new();
        c.put_byte("flying", self.flying);
        c.put_byte("mayfly", self.mayfly);
        c.put_byte("invulnerable", self.invulnerable);
        c.put_float("walkSpeed", self.walkSpeed);
        c
    }
}

impl RecipeBook {
    fn from_compound(c: &NbtCompound) -> Self {
        Self {
            isFilteringCraftable: c
                .get_byte("isFilteringCraftable")
                .expect("isFilteringCraftable"),
            recipes: strings(c.get_list("recipes").expect("recipes")),
            toBeDisplayed: strings(c.get_list("toBeDisplayed").expect("toBeDisplayed")),
        }
    }

    fn to_compound(&self) -> NbtCompound {
        let mut c = NbtCompound::new();
        c.put_byte("isFilteringCraftable", self.isFilteringCraftable);
        c.put("recipes", NbtTag::List(string_list(&self.recipes)));
        c.put(
            "toBeDisplayed",
            NbtTag::List(string_list(&self.toBeDisplayed)),
        );
        c
    }
}

#[allow(non_snake_case)]
#[derive(Debug)]
pub struct Chunk {
    DataVersion: i32,
    xPos: i32,
    zPos: i32,
    Status: String,
    LastUpdate: i64,
    InhabitedTime: i64,
    sections: Vec<Section>,
    block_entities: Vec<BlockEntity>,
    Heightmaps: Heightmaps,
    PostProcessing: Vec<Vec<i16>>,
}

#[allow(non_snake_case)]
#[derive(Debug)]
pub struct Section {
    Y: i8,
    block_states: BlockStates,
    biomes: Biomes,
    BlockLight: Vec<i8>,
    SkyLight: Vec<i8>,
}

#[allow(non_snake_case)]
#[derive(Debug)]
pub struct BlockStates {
    palette: Vec<PaletteEntry>,
    data: Vec<i64>,
}

#[allow(non_snake_case)]
#[derive(Debug)]
pub struct PaletteEntry {
    Name: String,
}

#[allow(non_snake_case)]
#[derive(Debug)]
pub struct Biomes {
    palette: Vec<String>,
    data: Vec<i64>,
}

#[allow(non_snake_case)]
#[derive(Debug)]
pub struct Heightmaps {
    MOTION_BLOCKING: Vec<i64>,
    WORLD_SURFACE: Vec<i64>,
}

#[allow(non_snake_case)]
#[derive(Debug)]
pub struct BlockEntity {
    id: String,
    x: i32,
    y: i32,
    z: i32,
    KeepPacked: i8,
}

impl Chunk {
    fn from_compound(c: &NbtCompound) -> Self {
        Self {
            DataVersion: c.get_int("DataVersion").expect("DataVersion"),
            xPos: c.get_int("xPos").expect("xPos"),
            zPos: c.get_int("zPos").expect("zPos"),
            Status: c.get_string("Status").expect("Status").to_owned(),
            LastUpdate: c.get_long("LastUpdate").expect("LastUpdate"),
            InhabitedTime: c.get_long("InhabitedTime").expect("InhabitedTime"),
            sections: compounds(
                c.get_list("sections").expect("sections"),
                Section::from_compound,
            ),
            block_entities: compounds(
                c.get_list("block_entities").expect("block_entities"),
                BlockEntity::from_compound,
            ),
            Heightmaps: Heightmaps::from_compound(
                c.get_compound("Heightmaps").expect("Heightmaps"),
            ),
            PostProcessing: c
                .get_list("PostProcessing")
                .expect("PostProcessing")
                .iter()
                .map(|tag| {
                    tag.extract_list()
                        .expect("list")
                        .iter()
                        .map(|short| short.extract_short().expect("short"))
                        .collect()
                })
                .collect(),
        }
    }

    fn to_compound(&self) -> NbtCompound {
        let mut c = NbtCompound::new();
        c.put_int("DataVersion", self.DataVersion);
        c.put_int("xPos", self.xPos);
        c.put_int("zPos", self.zPos);
        c.put_string("Status", self.Status.clone());
        c.put_long("LastUpdate", self.LastUpdate);
        c.put_long("InhabitedTime", self.InhabitedTime);
        c.put(
            "sections",
            NbtTag::List(
                self.sections
                    .iter()
                    .map(Section::to_compound)
                    .map(NbtTag::Compound)
                    .collect(),
            ),
        );
        c.put(
            "block_entities",
            NbtTag::List(
                self.block_entities
                    .iter()
                    .map(BlockEntity::to_compound)
                    .map(NbtTag::Compound)
                    .collect(),
            ),
        );
        c.put("Heightmaps", self.Heightmaps.to_compound());
        c.put(
            "PostProcessing",
            NbtTag::List(
                self.PostProcessing
                    .iter()
                    .map(|row| NbtTag::List(row.iter().map(|v| NbtTag::Short(*v)).collect()))
                    .collect(),
            ),
        );
        c
    }
}

impl Section {
    fn from_compound(c: &NbtCompound) -> Self {
        Self {
            Y: c.get_byte("Y").expect("Y"),
            block_states: BlockStates::from_compound(
                c.get_compound("block_states").expect("block_states"),
            ),
            biomes: Biomes::from_compound(c.get_compound("biomes").expect("biomes")),
            BlockLight: c.get_byte_array("BlockLight").expect("BlockLight").to_vec(),
            SkyLight: c.get_byte_array("SkyLight").expect("SkyLight").to_vec(),
        }
    }

    fn to_compound(&self) -> NbtCompound {
        let mut c = NbtCompound::new();
        c.put_byte("Y", self.Y);
        c.put("block_states", self.block_states.to_compound());
        c.put("biomes", self.biomes.to_compound());
        c.put(
            "BlockLight",
            NbtTag::ByteArray(self.BlockLight.clone().into()),
        );
        c.put("SkyLight", NbtTag::ByteArray(self.SkyLight.clone().into()));
        c
    }
}

impl BlockStates {
    fn from_compound(c: &NbtCompound) -> Self {
        Self {
            palette: compounds(
                c.get_list("palette").expect("palette"),
                PaletteEntry::from_compound,
            ),
            data: c.get_long_array("data").expect("data").to_vec(),
        }
    }

    fn to_compound(&self) -> NbtCompound {
        let mut c = NbtCompound::new();
        c.put(
            "palette",
            NbtTag::List(
                self.palette
                    .iter()
                    .map(PaletteEntry::to_compound)
                    .map(NbtTag::Compound)
                    .collect(),
            ),
        );
        c.put("data", NbtTag::LongArray(self.data.clone()));
        c
    }
}

impl PaletteEntry {
    fn from_compound(c: &NbtCompound) -> Self {
        Self {
            Name: c.get_string("Name").expect("Name").to_owned(),
        }
    }

    fn to_compound(&self) -> NbtCompound {
        let mut c = NbtCompound::new();
        c.put_string("Name", self.Name.clone());
        c
    }
}

impl Biomes {
    fn from_compound(c: &NbtCompound) -> Self {
        Self {
            palette: strings(c.get_list("palette").expect("palette")),
            data: c.get_long_array("data").expect("data").to_vec(),
        }
    }

    fn to_compound(&self) -> NbtCompound {
        let mut c = NbtCompound::new();
        c.put("palette", NbtTag::List(string_list(&self.palette)));
        c.put("data", NbtTag::LongArray(self.data.clone()));
        c
    }
}

impl Heightmaps {
    fn from_compound(c: &NbtCompound) -> Self {
        Self {
            MOTION_BLOCKING: c
                .get_long_array("MOTION_BLOCKING")
                .expect("MOTION_BLOCKING")
                .to_vec(),
            WORLD_SURFACE: c
                .get_long_array("WORLD_SURFACE")
                .expect("WORLD_SURFACE")
                .to_vec(),
        }
    }

    fn to_compound(&self) -> NbtCompound {
        let mut c = NbtCompound::new();
        c.put(
            "MOTION_BLOCKING",
            NbtTag::LongArray(self.MOTION_BLOCKING.clone()),
        );
        c.put(
            "WORLD_SURFACE",
            NbtTag::LongArray(self.WORLD_SURFACE.clone()),
        );
        c
    }
}

impl BlockEntity {
    fn from_compound(c: &NbtCompound) -> Self {
        Self {
            id: c.get_string("id").expect("id").to_owned(),
            x: c.get_int("x").expect("x"),
            y: c.get_int("y").expect("y"),
            z: c.get_int("z").expect("z"),
            KeepPacked: c.get_byte("KeepPacked").expect("KeepPacked"),
        }
    }

    fn to_compound(&self) -> NbtCompound {
        let mut c = NbtCompound::new();
        c.put_string("id", self.id.clone());
        c.put_int("x", self.x);
        c.put_int("y", self.y);
        c.put_int("z", self.z);
        c.put_byte("KeepPacked", self.KeepPacked);
        c
    }
}

// ---------------------------------------------------------------------------
// The entries.
// ---------------------------------------------------------------------------

pub fn parse(group: &mut BenchmarkGroup<'_, WallTime>, doc: Doc, bytes: &[u8]) {
    match doc {
        Doc::Small => bench_parse(group, "pumpkin", doc, bytes, |b: &[u8]| {
            let mut reader = NbtReadHelperJava::new(Cursor::new(b));
            Small::from_compound(&Nbt::read(&mut reader).expect("document parses").root_tag)
        }),
        Doc::Player => bench_parse(group, "pumpkin", doc, bytes, |b: &[u8]| {
            let mut reader = NbtReadHelperJava::new(Cursor::new(b));
            Player::from_compound(&Nbt::read(&mut reader).expect("document parses").root_tag)
        }),
        Doc::Chunk => bench_parse(group, "pumpkin", doc, bytes, |b: &[u8]| {
            let mut reader = NbtReadHelperJava::new(Cursor::new(b));
            Chunk::from_compound(&Nbt::read(&mut reader).expect("document parses").root_tag)
        }),
    }
}

pub fn write(group: &mut BenchmarkGroup<'_, WallTime>, doc: Doc, bytes: &[u8]) {
    match doc {
        Doc::Small => bench_write(
            group,
            "pumpkin",
            doc,
            bytes,
            |b: &[u8]| {
                let mut reader = NbtReadHelperJava::new(Cursor::new(b));
                Small::from_compound(&Nbt::read(&mut reader).expect("document parses").root_tag)
            },
            |v: &Small| Nbt::new(String::new(), v.to_compound()).write(),
        ),
        Doc::Player => bench_write(
            group,
            "pumpkin",
            doc,
            bytes,
            |b: &[u8]| {
                let mut reader = NbtReadHelperJava::new(Cursor::new(b));
                Player::from_compound(&Nbt::read(&mut reader).expect("document parses").root_tag)
            },
            |v: &Player| Nbt::new(String::new(), v.to_compound()).write(),
        ),
        Doc::Chunk => bench_write(
            group,
            "pumpkin",
            doc,
            bytes,
            |b: &[u8]| {
                let mut reader = NbtReadHelperJava::new(Cursor::new(b));
                Chunk::from_compound(&Nbt::read(&mut reader).expect("document parses").root_tag)
            },
            |v: &Chunk| Nbt::new(String::new(), v.to_compound()).write(),
        ),
    }
}

// ---------------------------------------------------------------------------
// The array entries.
// ---------------------------------------------------------------------------

/// The owned `data` entry of each kind, for the write entries.
fn byte_data(bytes: &[u8]) -> Vec<i8> {
    let mut reader = NbtReadHelperJava::new(Cursor::new(bytes));
    Nbt::read(&mut reader)
        .expect("the document parses")
        .root_tag
        .get_byte_array("data")
        .expect("data")
        .to_vec()
}

fn short_data(bytes: &[u8]) -> Vec<i16> {
    let mut reader = NbtReadHelperJava::new(Cursor::new(bytes));
    Nbt::read(&mut reader)
        .expect("the document parses")
        .root_tag
        .get_list("data")
        .expect("data")
        .iter()
        .map(|tag| tag.extract_short().expect("short"))
        .collect()
}

fn int_data(bytes: &[u8]) -> Vec<i32> {
    let mut reader = NbtReadHelperJava::new(Cursor::new(bytes));
    Nbt::read(&mut reader)
        .expect("the document parses")
        .root_tag
        .get_int_array("data")
        .expect("data")
        .to_vec()
}

fn long_data(bytes: &[u8]) -> Vec<i64> {
    let mut reader = NbtReadHelperJava::new(Cursor::new(bytes));
    Nbt::read(&mut reader)
        .expect("the document parses")
        .root_tag
        .get_long_array("data")
        .expect("data")
        .to_vec()
}

/// Runs the parse entries of one array kind.
pub fn parse_array(group: &mut BenchmarkGroup<'_, WallTime>, kind: Array, bytes: &[u8]) {
    match kind {
        Array::Byte => bench_parse(group, "pumpkin", kind, bytes, byte_data),
        Array::Short => bench_parse(group, "pumpkin", kind, bytes, short_data),
        Array::Int => bench_parse(group, "pumpkin", kind, bytes, int_data),
        Array::Long => bench_parse(group, "pumpkin", kind, bytes, long_data),
    }
}

/// Runs the write entries of one array kind.
pub fn write_array(group: &mut BenchmarkGroup<'_, WallTime>, kind: Array, bytes: &[u8]) {
    match kind {
        Array::Byte => bench_write(group, "pumpkin", kind, bytes, byte_data, |v: &Vec<i8>| {
            let mut c = NbtCompound::new();
            c.put("data", NbtTag::ByteArray(v.clone().into()));
            Nbt::new(String::new(), c).write()
        }),
        Array::Short => bench_write(group, "pumpkin", kind, bytes, short_data, |v: &Vec<i16>| {
            let mut c = NbtCompound::new();
            c.put(
                "data",
                NbtTag::List(v.iter().map(|value| NbtTag::Short(*value)).collect()),
            );
            Nbt::new(String::new(), c).write()
        }),
        Array::Int => bench_write(group, "pumpkin", kind, bytes, int_data, |v: &Vec<i32>| {
            let mut c = NbtCompound::new();
            c.put("data", NbtTag::IntArray(v.clone()));
            Nbt::new(String::new(), c).write()
        }),
        Array::Long => bench_write(group, "pumpkin", kind, bytes, long_data, |v: &Vec<i64>| {
            let mut c = NbtCompound::new();
            c.put("data", NbtTag::LongArray(v.clone()));
            Nbt::new(String::new(), c).write()
        }),
    }
}
