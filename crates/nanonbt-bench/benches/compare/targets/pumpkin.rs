//! The `pumpkin-nbt` entry: one plain struct per document, filled through
//! `NbtCompound` accessors and written back with `put_*`.
//!
//! It has no skip either, so the skip entry at the end reads a whole skip
//! document and only then looks up `kept`.
//!
//! pumpkin-nbt has no serde support, so the mapping is hand-written; that is
//! also how its users read typed data.

use std::io::Cursor;

use iai_callgrind::library_benchmark_group;
use pumpkin_nbt::{Nbt, NbtCompound, deserializer::NbtReadHelperJava, tag::NbtTag};
use random_names::random_names;

use crate::documents::{self, Array, Doc, Skip};
use crate::macros::{parse_bench, write_bench};

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

/// 64 `i32` fields whose keys are a fixed 60-byte prefix plus four random
/// bytes, drawn by the `random_names` attribute.
#[random_names(
    64,
    len = 64,
    prefix = "benchmark_field_with_a_deliberately_long_name_shared_prefix_"
)]
#[derive(Debug)]
pub struct LongNames;

/// The same 64 `i32` fields as [`LongNames`], named by three random bytes.
#[random_names(64, len = 3)]
#[derive(Debug)]
pub struct ShortNames;

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

impl LongNames {
    fn from_compound(c: &NbtCompound) -> Self {
        Self::from_lookup(|name| c.get_int(name).expect(name))
    }

    fn to_compound(&self) -> NbtCompound {
        let mut c = NbtCompound::new();
        self.for_each(|name, value| c.put_int(name, value));
        c
    }
}

impl ShortNames {
    fn from_compound(c: &NbtCompound) -> Self {
        Self::from_lookup(|name| c.get_int(name).expect(name))
    }

    fn to_compound(&self) -> NbtCompound {
        let mut c = NbtCompound::new();
        self.for_each(|name, value| c.put_int(name, value));
        c
    }
}

// ---------------------------------------------------------------------------
// The entries, one `#[bench]` function per document or array.
// ---------------------------------------------------------------------------
//
// A function is named `<kind>_<target>_<id>` — `parse_pumpkin_short_list` is
// `parse/pumpkin/short-list` — and `examples/bench-summary.rs` recovers the id
// from the name plus the `#[bench]` id; see `crate::macros`.

/// The parse and write entries of one document.
macro_rules! document_entry {
    ($ty:ty, $input:expr, $id:ident, $parse:ident, $setup:ident, $write:ident) => {
        parse_bench!($parse, $id, $input, |b: &[u8]| {
            let mut reader = NbtReadHelperJava::new(Cursor::new(b));
            <$ty>::from_compound(
                &Nbt::read(&mut reader)
                    .expect("the document parses")
                    .root_tag,
            )
        });
        write_bench!(
            $write,
            $setup,
            $id,
            $input,
            $ty,
            |b: &[u8]| {
                let mut reader = NbtReadHelperJava::new(Cursor::new(b));
                <$ty>::from_compound(
                    &Nbt::read(&mut reader)
                        .expect("the document parses")
                        .root_tag,
                )
            },
            |v: &$ty| Nbt::new(String::new(), v.to_compound()).write()
        );
    };
}

document_entry!(
    Small,
    documents::doc(Doc::Small),
    small,
    parse_pumpkin_small,
    setup_pumpkin_small,
    write_pumpkin_small
);
document_entry!(
    Player,
    documents::doc(Doc::Player),
    player,
    parse_pumpkin_player,
    setup_pumpkin_player,
    write_pumpkin_player
);
document_entry!(
    Chunk,
    documents::doc(Doc::Chunk),
    chunk,
    parse_pumpkin_chunk,
    setup_pumpkin_chunk,
    write_pumpkin_chunk
);
document_entry!(
    ShortNames,
    documents::doc(Doc::ShortNames),
    short_names,
    parse_pumpkin_short_names,
    setup_pumpkin_short_names,
    write_pumpkin_short_names
);
document_entry!(
    LongNames,
    documents::doc(Doc::LongNames),
    long_names,
    parse_pumpkin_long_names,
    setup_pumpkin_long_names,
    write_pumpkin_long_names
);

// The four array documents. `pumpkin-nbt` reads and writes through its
// accessors, so a write rebuilds the `data` entry from the copied vector.
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

parse_bench!(
    parse_pumpkin_byte_array,
    byte_array,
    documents::array_doc(Array::Byte),
    byte_data
);
parse_bench!(
    parse_pumpkin_short_list,
    short_list,
    documents::array_doc(Array::Short),
    short_data
);
parse_bench!(
    parse_pumpkin_int_array,
    int_array,
    documents::array_doc(Array::Int),
    int_data
);
parse_bench!(
    parse_pumpkin_long_array,
    long_array,
    documents::array_doc(Array::Long),
    long_data
);

write_bench!(
    write_pumpkin_byte_array,
    setup_write_pumpkin_byte_array,
    byte_array,
    documents::array_doc(Array::Byte),
    Vec<i8>,
    byte_data,
    |v: &Vec<i8>| {
        let mut c = NbtCompound::new();
        c.put("data", NbtTag::ByteArray(v.clone().into()));
        Nbt::new(String::new(), c).write()
    }
);
write_bench!(
    write_pumpkin_short_list,
    setup_write_pumpkin_short_list,
    short_list,
    documents::array_doc(Array::Short),
    Vec<i16>,
    short_data,
    |v: &Vec<i16>| {
        let mut c = NbtCompound::new();
        c.put(
            "data",
            NbtTag::List(v.iter().map(|value| NbtTag::Short(*value)).collect()),
        );
        Nbt::new(String::new(), c).write()
    }
);
write_bench!(
    write_pumpkin_int_array,
    setup_write_pumpkin_int_array,
    int_array,
    documents::array_doc(Array::Int),
    Vec<i32>,
    int_data,
    |v: &Vec<i32>| {
        let mut c = NbtCompound::new();
        c.put("data", NbtTag::IntArray(v.clone()));
        Nbt::new(String::new(), c).write()
    }
);
write_bench!(
    write_pumpkin_long_array,
    setup_write_pumpkin_long_array,
    long_array,
    documents::array_doc(Array::Long),
    Vec<i64>,
    long_data,
    |v: &Vec<i64>| {
        let mut c = NbtCompound::new();
        c.put("data", NbtTag::LongArray(v.clone()));
        Nbt::new(String::new(), c).write()
    }
);

// The eleven skip shapes. `pumpkin-nbt` reads a whole tree before any
// accessor, so each entry reads the document and only then looks `kept` up.
parse_bench!(
    skip_pumpkin_byte_list,
    byte_list,
    documents::skip_doc(Skip::ByteList),
    |b: &[u8]| {
        let mut reader = NbtReadHelperJava::new(Cursor::new(b));
        let root = Nbt::read(&mut reader)
            .expect("the document parses")
            .root_tag;
        ::std::hint::black_box(root.get_int("kept").expect("kept"));
    }
);
parse_bench!(
    skip_pumpkin_short_list,
    short_list,
    documents::skip_doc(Skip::ShortList),
    |b: &[u8]| {
        let mut reader = NbtReadHelperJava::new(Cursor::new(b));
        let root = Nbt::read(&mut reader)
            .expect("the document parses")
            .root_tag;
        ::std::hint::black_box(root.get_int("kept").expect("kept"));
    }
);
parse_bench!(
    skip_pumpkin_int_list,
    int_list,
    documents::skip_doc(Skip::IntList),
    |b: &[u8]| {
        let mut reader = NbtReadHelperJava::new(Cursor::new(b));
        let root = Nbt::read(&mut reader)
            .expect("the document parses")
            .root_tag;
        ::std::hint::black_box(root.get_int("kept").expect("kept"));
    }
);
parse_bench!(
    skip_pumpkin_long_list,
    long_list,
    documents::skip_doc(Skip::LongList),
    |b: &[u8]| {
        let mut reader = NbtReadHelperJava::new(Cursor::new(b));
        let root = Nbt::read(&mut reader)
            .expect("the document parses")
            .root_tag;
        ::std::hint::black_box(root.get_int("kept").expect("kept"));
    }
);
parse_bench!(
    skip_pumpkin_float_list,
    float_list,
    documents::skip_doc(Skip::FloatList),
    |b: &[u8]| {
        let mut reader = NbtReadHelperJava::new(Cursor::new(b));
        let root = Nbt::read(&mut reader)
            .expect("the document parses")
            .root_tag;
        ::std::hint::black_box(root.get_int("kept").expect("kept"));
    }
);
parse_bench!(
    skip_pumpkin_double_list,
    double_list,
    documents::skip_doc(Skip::DoubleList),
    |b: &[u8]| {
        let mut reader = NbtReadHelperJava::new(Cursor::new(b));
        let root = Nbt::read(&mut reader)
            .expect("the document parses")
            .root_tag;
        ::std::hint::black_box(root.get_int("kept").expect("kept"));
    }
);
parse_bench!(
    skip_pumpkin_byte_array,
    byte_array,
    documents::skip_doc(Skip::ByteArray),
    |b: &[u8]| {
        let mut reader = NbtReadHelperJava::new(Cursor::new(b));
        let root = Nbt::read(&mut reader)
            .expect("the document parses")
            .root_tag;
        ::std::hint::black_box(root.get_int("kept").expect("kept"));
    }
);
parse_bench!(
    skip_pumpkin_int_array,
    int_array,
    documents::skip_doc(Skip::IntArray),
    |b: &[u8]| {
        let mut reader = NbtReadHelperJava::new(Cursor::new(b));
        let root = Nbt::read(&mut reader)
            .expect("the document parses")
            .root_tag;
        ::std::hint::black_box(root.get_int("kept").expect("kept"));
    }
);
parse_bench!(
    skip_pumpkin_long_array,
    long_array,
    documents::skip_doc(Skip::LongArray),
    |b: &[u8]| {
        let mut reader = NbtReadHelperJava::new(Cursor::new(b));
        let root = Nbt::read(&mut reader)
            .expect("the document parses")
            .root_tag;
        ::std::hint::black_box(root.get_int("kept").expect("kept"));
    }
);
parse_bench!(
    skip_pumpkin_string_list,
    string_list,
    documents::skip_doc(Skip::StringList),
    |b: &[u8]| {
        let mut reader = NbtReadHelperJava::new(Cursor::new(b));
        let root = Nbt::read(&mut reader)
            .expect("the document parses")
            .root_tag;
        ::std::hint::black_box(root.get_int("kept").expect("kept"));
    }
);
parse_bench!(
    skip_pumpkin_compound_list,
    compound_list,
    documents::skip_doc(Skip::CompoundList),
    |b: &[u8]| {
        let mut reader = NbtReadHelperJava::new(Cursor::new(b));
        let root = Nbt::read(&mut reader)
            .expect("the document parses")
            .root_tag;
        ::std::hint::black_box(root.get_int("kept").expect("kept"));
    }
);

library_benchmark_group!(
    name = pumpkin_entries;
    benchmarks =
        parse_pumpkin_small,
        parse_pumpkin_player,
        parse_pumpkin_chunk,
        parse_pumpkin_short_names,
        parse_pumpkin_long_names,
        write_pumpkin_small,
        write_pumpkin_player,
        write_pumpkin_chunk,
        write_pumpkin_short_names,
        write_pumpkin_long_names,
        parse_pumpkin_byte_array,
        parse_pumpkin_short_list,
        parse_pumpkin_int_array,
        parse_pumpkin_long_array,
        write_pumpkin_byte_array,
        write_pumpkin_short_list,
        write_pumpkin_int_array,
        write_pumpkin_long_array,
        skip_pumpkin_byte_list,
        skip_pumpkin_short_list,
        skip_pumpkin_int_list,
        skip_pumpkin_long_list,
        skip_pumpkin_float_list,
        skip_pumpkin_double_list,
        skip_pumpkin_byte_array,
        skip_pumpkin_int_array,
        skip_pumpkin_long_array,
        skip_pumpkin_string_list,
        skip_pumpkin_compound_list
);
