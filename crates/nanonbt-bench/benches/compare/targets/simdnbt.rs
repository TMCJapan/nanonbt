//! The two `simdnbt` entries: `simdnbt-borrow` reads a tape over the input
//! and `simdnbt-owned` reads an owning tree first.
//!
//! One struct family serves both. `simdnbt` lends byte arrays and strings from
//! either source, but its `int_array` and `long_array` accessors copy on the
//! borrowed side and lend on the owned side; the struct owns those two either
//! way, so the borrowed entry pays a copy the owned one already paid.
//!
//! The two `write` entries share one implementation: once the struct exists,
//! where it was parsed from no longer matters, and the numbers should match.

use std::{borrow::Cow, io::Cursor};

use criterion::{measurement::WallTime, BenchmarkGroup, BenchmarkId, Throughput};
use simdnbt::{borrow, owned, Mutf8Str, Mutf8String};

use crate::{
    bench_parse, bench_write,
    documents::{Array, BenchInput, Doc},
};

// ---------------------------------------------------------------------------
// Conversion helpers.
// ---------------------------------------------------------------------------

fn text(text: Option<&Mutf8Str>) -> Cow<'_, str> {
    text.map(Mutf8Str::to_string_lossy).unwrap_or_default()
}

fn borrow_compounds<'a, T>(
    nbt: &borrow::NbtCompound<'a, '_>,
    name: &str,
    from: impl Fn(borrow::NbtCompound<'a, '_>) -> T,
) -> Vec<T> {
    nbt.list(name)
        .and_then(|list| list.compounds())
        .map(|compounds| compounds.into_iter().map(from).collect())
        .unwrap_or_default()
}

fn borrow_strings<'a>(nbt: &borrow::NbtCompound<'a, '_>, name: &str) -> Vec<Cow<'a, str>> {
    nbt.list(name)
        .and_then(|list| list.strings())
        .map(|strings| strings.iter().map(|s| s.to_string_lossy()).collect())
        .unwrap_or_default()
}

fn borrow_doubles(nbt: &borrow::NbtCompound<'_, '_>, name: &str) -> Vec<f64> {
    nbt.list(name)
        .and_then(|list| list.doubles())
        .unwrap_or_default()
}

fn borrow_floats(nbt: &borrow::NbtCompound<'_, '_>, name: &str) -> Vec<f32> {
    nbt.list(name)
        .and_then(|list| list.floats())
        .unwrap_or_default()
}

fn owned_compounds<'a, T>(
    nbt: &'a owned::NbtCompound,
    name: &str,
    from: impl Fn(&'a owned::NbtCompound) -> T,
) -> Vec<T> {
    nbt.list(name)
        .and_then(|list| list.compounds())
        .map(|compounds| compounds.iter().map(from).collect())
        .unwrap_or_default()
}

fn owned_strings<'a>(nbt: &'a owned::NbtCompound, name: &str) -> Vec<Cow<'a, str>> {
    nbt.list(name)
        .and_then(|list| list.strings())
        .map(|strings| strings.iter().map(|s| s.to_string_lossy()).collect())
        .unwrap_or_default()
}

fn owned_doubles(nbt: &owned::NbtCompound, name: &str) -> Vec<f64> {
    nbt.list(name)
        .and_then(|list| list.doubles())
        .unwrap_or_default()
}

fn owned_floats(nbt: &owned::NbtCompound, name: &str) -> Vec<f32> {
    nbt.list(name)
        .and_then(|list| list.floats())
        .unwrap_or_default()
}

fn short_list(values: &[i16]) -> owned::NbtList {
    if values.is_empty() {
        owned::NbtList::Empty
    } else {
        owned::NbtList::Short(values.to_vec())
    }
}

fn float_list(values: &[f32]) -> owned::NbtList {
    if values.is_empty() {
        owned::NbtList::Empty
    } else {
        owned::NbtList::Float(values.to_vec())
    }
}

fn double_list(values: &[f64]) -> owned::NbtList {
    if values.is_empty() {
        owned::NbtList::Empty
    } else {
        owned::NbtList::Double(values.to_vec())
    }
}

fn string_list(values: &[Cow<'_, str>]) -> owned::NbtList {
    if values.is_empty() {
        owned::NbtList::Empty
    } else {
        owned::NbtList::String(
            values
                .iter()
                .map(|v| Mutf8String::from(v.as_ref()))
                .collect(),
        )
    }
}

fn compound_list(compounds: impl Iterator<Item = owned::NbtCompound>) -> owned::NbtList {
    let compounds: Vec<_> = compounds.collect();
    if compounds.is_empty() {
        owned::NbtList::Empty
    } else {
        owned::NbtList::Compound(compounds)
    }
}

fn list_list(lists: impl Iterator<Item = owned::NbtList>) -> owned::NbtList {
    let lists: Vec<_> = lists.collect();
    if lists.is_empty() {
        owned::NbtList::Empty
    } else {
        owned::NbtList::List(lists)
    }
}

// ---------------------------------------------------------------------------
// The structs, and how each side fills them.
// ---------------------------------------------------------------------------

pub struct Small<'a> {
    dimension: Cow<'a, str>,
    x: i32,
    y: i32,
    z: i32,
    yaw: f32,
    pitch: f32,
    on_ground: i8,
    health: i16,
}

impl<'a> Small<'a> {
    fn from_borrow(nbt: &'a borrow::BaseNbt<'a>) -> Self {
        let nbt = nbt.as_compound();
        Self {
            dimension: text(nbt.string("dimension")),
            x: nbt.int("x").expect("x"),
            y: nbt.int("y").expect("y"),
            z: nbt.int("z").expect("z"),
            yaw: nbt.float("yaw").expect("yaw"),
            pitch: nbt.float("pitch").expect("pitch"),
            on_ground: nbt.byte("on_ground").expect("on_ground"),
            health: nbt.short("health").expect("health"),
        }
    }

    fn from_owned(nbt: &'a owned::BaseNbt) -> Self {
        Self {
            dimension: text(nbt.string("dimension")),
            x: nbt.int("x").expect("x"),
            y: nbt.int("y").expect("y"),
            z: nbt.int("z").expect("z"),
            yaw: nbt.float("yaw").expect("yaw"),
            pitch: nbt.float("pitch").expect("pitch"),
            on_ground: nbt.byte("on_ground").expect("on_ground"),
            health: nbt.short("health").expect("health"),
        }
    }

    fn to_compound(&self) -> owned::NbtCompound {
        let mut c = owned::NbtCompound::new();
        c.insert("dimension", self.dimension.as_ref());
        c.insert("x", self.x);
        c.insert("y", self.y);
        c.insert("z", self.z);
        c.insert("yaw", self.yaw);
        c.insert("pitch", self.pitch);
        c.insert("on_ground", self.on_ground);
        c.insert("health", self.health);
        c
    }
}

#[allow(non_snake_case)]
pub struct Player<'a> {
    DataVersion: i32,
    Health: f32,
    foodLevel: i32,
    XpLevel: i32,
    playerGameType: i32,
    UUID: Vec<i32>,
    Pos: Vec<f64>,
    Motion: Vec<f64>,
    Rotation: Vec<f32>,
    Inventory: Vec<InventoryItem<'a>>,
    Attributes: Vec<Attribute<'a>>,
    EnderItems: Vec<InventoryItem<'a>>,
    abilities: Abilities,
    recipeBook: RecipeBook<'a>,
    Tags: Vec<Cow<'a, str>>,
}

#[allow(non_snake_case)]
pub struct InventoryItem<'a> {
    Slot: i8,
    id: Cow<'a, str>,
    Count: i8,
    tag: ItemTag<'a>,
}

#[allow(non_snake_case)]
pub struct ItemTag<'a> {
    Damage: i32,
    display: Display<'a>,
}

#[allow(non_snake_case)]
pub struct Display<'a> {
    Name: Cow<'a, str>,
}

#[allow(non_snake_case)]
pub struct Attribute<'a> {
    Name: Cow<'a, str>,
    Base: f64,
    Modifiers: Vec<Modifier>,
}

#[allow(non_snake_case)]
pub struct Modifier {
    Amount: f64,
}

#[allow(non_snake_case)]
pub struct Abilities {
    flying: i8,
    mayfly: i8,
    invulnerable: i8,
    walkSpeed: f32,
}

#[allow(non_snake_case)]
pub struct RecipeBook<'a> {
    isFilteringCraftable: i8,
    recipes: Vec<Cow<'a, str>>,
    toBeDisplayed: Vec<Cow<'a, str>>,
}

impl<'a> Player<'a> {
    fn from_borrow(nbt: &'a borrow::BaseNbt<'a>) -> Self {
        let nbt = nbt.as_compound();
        Self {
            DataVersion: nbt.int("DataVersion").expect("DataVersion"),
            Health: nbt.float("Health").expect("Health"),
            foodLevel: nbt.int("foodLevel").expect("foodLevel"),
            XpLevel: nbt.int("XpLevel").expect("XpLevel"),
            playerGameType: nbt.int("playerGameType").expect("playerGameType"),
            UUID: nbt.int_array("UUID").expect("UUID"),
            Pos: borrow_doubles(&nbt, "Pos"),
            Motion: borrow_doubles(&nbt, "Motion"),
            Rotation: borrow_floats(&nbt, "Rotation"),
            Inventory: borrow_compounds(&nbt, "Inventory", InventoryItem::from_borrow),
            Attributes: borrow_compounds(&nbt, "Attributes", Attribute::from_borrow),
            EnderItems: borrow_compounds(&nbt, "EnderItems", InventoryItem::from_borrow),
            abilities: Abilities::from_borrow(nbt.compound("abilities").expect("abilities")),
            recipeBook: RecipeBook::from_borrow(nbt.compound("recipeBook").expect("recipeBook")),
            Tags: borrow_strings(&nbt, "Tags"),
        }
    }

    fn from_owned(nbt: &'a owned::BaseNbt) -> Self {
        Self {
            DataVersion: nbt.int("DataVersion").expect("DataVersion"),
            Health: nbt.float("Health").expect("Health"),
            foodLevel: nbt.int("foodLevel").expect("foodLevel"),
            XpLevel: nbt.int("XpLevel").expect("XpLevel"),
            playerGameType: nbt.int("playerGameType").expect("playerGameType"),
            UUID: nbt.int_array("UUID").expect("UUID").to_vec(),
            Pos: owned_doubles(nbt, "Pos"),
            Motion: owned_doubles(nbt, "Motion"),
            Rotation: owned_floats(nbt, "Rotation"),
            Inventory: owned_compounds(nbt, "Inventory", InventoryItem::from_owned),
            Attributes: owned_compounds(nbt, "Attributes", Attribute::from_owned),
            EnderItems: owned_compounds(nbt, "EnderItems", InventoryItem::from_owned),
            abilities: Abilities::from_owned(nbt.compound("abilities").expect("abilities")),
            recipeBook: RecipeBook::from_owned(nbt.compound("recipeBook").expect("recipeBook")),
            Tags: owned_strings(nbt, "Tags"),
        }
    }

    fn to_compound(&self) -> owned::NbtCompound {
        let mut c = owned::NbtCompound::new();
        c.insert("DataVersion", self.DataVersion);
        c.insert("Health", self.Health);
        c.insert("foodLevel", self.foodLevel);
        c.insert("XpLevel", self.XpLevel);
        c.insert("playerGameType", self.playerGameType);
        c.insert("UUID", owned::NbtTag::IntArray(self.UUID.clone()));
        c.insert("Pos", double_list(&self.Pos));
        c.insert("Motion", double_list(&self.Motion));
        c.insert("Rotation", float_list(&self.Rotation));
        c.insert(
            "Inventory",
            compound_list(self.Inventory.iter().map(InventoryItem::to_compound)),
        );
        c.insert(
            "Attributes",
            compound_list(self.Attributes.iter().map(Attribute::to_compound)),
        );
        c.insert(
            "EnderItems",
            compound_list(self.EnderItems.iter().map(InventoryItem::to_compound)),
        );
        c.insert("abilities", self.abilities.to_compound());
        c.insert("recipeBook", self.recipeBook.to_compound());
        c.insert("Tags", string_list(&self.Tags));
        c
    }
}

impl<'a> InventoryItem<'a> {
    fn from_borrow(nbt: borrow::NbtCompound<'a, '_>) -> Self {
        Self {
            Slot: nbt.byte("Slot").expect("Slot"),
            id: text(nbt.string("id")),
            Count: nbt.byte("Count").expect("Count"),
            tag: ItemTag::from_borrow(nbt.compound("tag").expect("tag")),
        }
    }

    fn from_owned(nbt: &'a owned::NbtCompound) -> Self {
        Self {
            Slot: nbt.byte("Slot").expect("Slot"),
            id: text(nbt.string("id")),
            Count: nbt.byte("Count").expect("Count"),
            tag: ItemTag::from_owned(nbt.compound("tag").expect("tag")),
        }
    }

    fn to_compound(&self) -> owned::NbtCompound {
        let mut c = owned::NbtCompound::new();
        c.insert("Slot", self.Slot);
        c.insert("id", self.id.as_ref());
        c.insert("Count", self.Count);
        c.insert("tag", self.tag.to_compound());
        c
    }
}

impl<'a> ItemTag<'a> {
    fn from_borrow(nbt: borrow::NbtCompound<'a, '_>) -> Self {
        Self {
            Damage: nbt.int("Damage").expect("Damage"),
            display: Display::from_borrow(nbt.compound("display").expect("display")),
        }
    }

    fn from_owned(nbt: &'a owned::NbtCompound) -> Self {
        Self {
            Damage: nbt.int("Damage").expect("Damage"),
            display: Display::from_owned(nbt.compound("display").expect("display")),
        }
    }

    fn to_compound(&self) -> owned::NbtCompound {
        let mut c = owned::NbtCompound::new();
        c.insert("Damage", self.Damage);
        c.insert("display", self.display.to_compound());
        c
    }
}

impl<'a> Display<'a> {
    fn from_borrow(nbt: borrow::NbtCompound<'a, '_>) -> Self {
        Self {
            Name: text(nbt.string("Name")),
        }
    }

    fn from_owned(nbt: &'a owned::NbtCompound) -> Self {
        Self {
            Name: text(nbt.string("Name")),
        }
    }

    fn to_compound(&self) -> owned::NbtCompound {
        let mut c = owned::NbtCompound::new();
        c.insert("Name", self.Name.as_ref());
        c
    }
}

impl<'a> Attribute<'a> {
    fn from_borrow(nbt: borrow::NbtCompound<'a, '_>) -> Self {
        Self {
            Name: text(nbt.string("Name")),
            Base: nbt.double("Base").expect("Base"),
            Modifiers: borrow_compounds(&nbt, "Modifiers", Modifier::from_borrow),
        }
    }

    fn from_owned(nbt: &'a owned::NbtCompound) -> Self {
        Self {
            Name: text(nbt.string("Name")),
            Base: nbt.double("Base").expect("Base"),
            Modifiers: owned_compounds(nbt, "Modifiers", Modifier::from_owned),
        }
    }

    fn to_compound(&self) -> owned::NbtCompound {
        let mut c = owned::NbtCompound::new();
        c.insert("Name", self.Name.as_ref());
        c.insert("Base", self.Base);
        c.insert(
            "Modifiers",
            compound_list(self.Modifiers.iter().map(Modifier::to_compound)),
        );
        c
    }
}

impl Modifier {
    fn from_borrow(nbt: borrow::NbtCompound<'_, '_>) -> Self {
        Self {
            Amount: nbt.double("Amount").expect("Amount"),
        }
    }

    fn from_owned(nbt: &owned::NbtCompound) -> Self {
        Self {
            Amount: nbt.double("Amount").expect("Amount"),
        }
    }

    fn to_compound(&self) -> owned::NbtCompound {
        let mut c = owned::NbtCompound::new();
        c.insert("Amount", self.Amount);
        c
    }
}

impl Abilities {
    fn from_borrow(nbt: borrow::NbtCompound<'_, '_>) -> Self {
        Self {
            flying: nbt.byte("flying").expect("flying"),
            mayfly: nbt.byte("mayfly").expect("mayfly"),
            invulnerable: nbt.byte("invulnerable").expect("invulnerable"),
            walkSpeed: nbt.float("walkSpeed").expect("walkSpeed"),
        }
    }

    fn from_owned(nbt: &owned::NbtCompound) -> Self {
        Self {
            flying: nbt.byte("flying").expect("flying"),
            mayfly: nbt.byte("mayfly").expect("mayfly"),
            invulnerable: nbt.byte("invulnerable").expect("invulnerable"),
            walkSpeed: nbt.float("walkSpeed").expect("walkSpeed"),
        }
    }

    fn to_compound(&self) -> owned::NbtCompound {
        let mut c = owned::NbtCompound::new();
        c.insert("flying", self.flying);
        c.insert("mayfly", self.mayfly);
        c.insert("invulnerable", self.invulnerable);
        c.insert("walkSpeed", self.walkSpeed);
        c
    }
}

impl<'a> RecipeBook<'a> {
    fn from_borrow(nbt: borrow::NbtCompound<'a, '_>) -> Self {
        Self {
            isFilteringCraftable: nbt
                .byte("isFilteringCraftable")
                .expect("isFilteringCraftable"),
            recipes: borrow_strings(&nbt, "recipes"),
            toBeDisplayed: borrow_strings(&nbt, "toBeDisplayed"),
        }
    }

    fn from_owned(nbt: &'a owned::NbtCompound) -> Self {
        Self {
            isFilteringCraftable: nbt
                .byte("isFilteringCraftable")
                .expect("isFilteringCraftable"),
            recipes: owned_strings(nbt, "recipes"),
            toBeDisplayed: owned_strings(nbt, "toBeDisplayed"),
        }
    }

    fn to_compound(&self) -> owned::NbtCompound {
        let mut c = owned::NbtCompound::new();
        c.insert("isFilteringCraftable", self.isFilteringCraftable);
        c.insert("recipes", string_list(&self.recipes));
        c.insert("toBeDisplayed", string_list(&self.toBeDisplayed));
        c
    }
}

#[allow(non_snake_case)]
pub struct Chunk<'a> {
    DataVersion: i32,
    xPos: i32,
    zPos: i32,
    Status: Cow<'a, str>,
    LastUpdate: i64,
    InhabitedTime: i64,
    sections: Vec<Section<'a>>,
    block_entities: Vec<BlockEntity<'a>>,
    Heightmaps: Heightmaps,
    PostProcessing: Vec<Vec<i16>>,
}

#[allow(non_snake_case)]
pub struct Section<'a> {
    Y: i8,
    block_states: BlockStates<'a>,
    biomes: Biomes<'a>,
    BlockLight: &'a [u8],
    SkyLight: &'a [u8],
}

#[allow(non_snake_case)]
pub struct BlockStates<'a> {
    palette: Vec<PaletteEntry<'a>>,
    data: Vec<i64>,
}

#[allow(non_snake_case)]
pub struct PaletteEntry<'a> {
    Name: Cow<'a, str>,
}

#[allow(non_snake_case)]
pub struct Biomes<'a> {
    palette: Vec<Cow<'a, str>>,
    data: Vec<i64>,
}

#[allow(non_snake_case)]
pub struct Heightmaps {
    MOTION_BLOCKING: Vec<i64>,
    WORLD_SURFACE: Vec<i64>,
}

#[allow(non_snake_case)]
pub struct BlockEntity<'a> {
    id: Cow<'a, str>,
    x: i32,
    y: i32,
    z: i32,
    KeepPacked: i8,
}

impl<'a> Chunk<'a> {
    fn from_borrow(nbt: &'a borrow::BaseNbt<'a>) -> Self {
        let nbt = nbt.as_compound();
        Self {
            DataVersion: nbt.int("DataVersion").expect("DataVersion"),
            xPos: nbt.int("xPos").expect("xPos"),
            zPos: nbt.int("zPos").expect("zPos"),
            Status: text(nbt.string("Status")),
            LastUpdate: nbt.long("LastUpdate").expect("LastUpdate"),
            InhabitedTime: nbt.long("InhabitedTime").expect("InhabitedTime"),
            sections: borrow_compounds(&nbt, "sections", Section::from_borrow),
            block_entities: borrow_compounds(&nbt, "block_entities", BlockEntity::from_borrow),
            Heightmaps: Heightmaps::from_borrow(nbt.compound("Heightmaps").expect("Heightmaps")),
            PostProcessing: borrow_lists_of_shorts(&nbt, "PostProcessing"),
        }
    }

    fn from_owned(nbt: &'a owned::BaseNbt) -> Self {
        Self {
            DataVersion: nbt.int("DataVersion").expect("DataVersion"),
            xPos: nbt.int("xPos").expect("xPos"),
            zPos: nbt.int("zPos").expect("zPos"),
            Status: text(nbt.string("Status")),
            LastUpdate: nbt.long("LastUpdate").expect("LastUpdate"),
            InhabitedTime: nbt.long("InhabitedTime").expect("InhabitedTime"),
            sections: owned_compounds(nbt, "sections", Section::from_owned),
            block_entities: owned_compounds(nbt, "block_entities", BlockEntity::from_owned),
            Heightmaps: Heightmaps::from_owned(nbt.compound("Heightmaps").expect("Heightmaps")),
            PostProcessing: owned_lists_of_shorts(nbt, "PostProcessing"),
        }
    }

    fn to_compound(&self) -> owned::NbtCompound {
        let mut c = owned::NbtCompound::new();
        c.insert("DataVersion", self.DataVersion);
        c.insert("xPos", self.xPos);
        c.insert("zPos", self.zPos);
        c.insert("Status", self.Status.as_ref());
        c.insert("LastUpdate", self.LastUpdate);
        c.insert("InhabitedTime", self.InhabitedTime);
        c.insert(
            "sections",
            compound_list(self.sections.iter().map(Section::to_compound)),
        );
        c.insert(
            "block_entities",
            compound_list(self.block_entities.iter().map(BlockEntity::to_compound)),
        );
        c.insert("Heightmaps", self.Heightmaps.to_compound());
        c.insert(
            "PostProcessing",
            list_list(self.PostProcessing.iter().map(|row| short_list(row))),
        );
        c
    }
}

fn borrow_lists_of_shorts<'a>(nbt: &borrow::NbtCompound<'a, '_>, name: &str) -> Vec<Vec<i16>> {
    nbt.list(name)
        .and_then(|list| list.lists())
        .map(|lists| {
            lists
                .into_iter()
                .map(|list| list.shorts().unwrap_or_default())
                .collect()
        })
        .unwrap_or_default()
}

fn owned_lists_of_shorts(nbt: &owned::NbtCompound, name: &str) -> Vec<Vec<i16>> {
    nbt.list(name)
        .and_then(|list| list.lists())
        .map(|lists| {
            lists
                .iter()
                .map(|list| list.shorts().unwrap_or_default())
                .collect()
        })
        .unwrap_or_default()
}

impl<'a> Section<'a> {
    fn from_borrow(nbt: borrow::NbtCompound<'a, '_>) -> Self {
        Self {
            Y: nbt.byte("Y").expect("Y"),
            block_states: BlockStates::from_borrow(
                nbt.compound("block_states").expect("block_states"),
            ),
            biomes: Biomes::from_borrow(nbt.compound("biomes").expect("biomes")),
            BlockLight: nbt.byte_array("BlockLight").expect("BlockLight"),
            SkyLight: nbt.byte_array("SkyLight").expect("SkyLight"),
        }
    }

    fn from_owned(nbt: &'a owned::NbtCompound) -> Self {
        Self {
            Y: nbt.byte("Y").expect("Y"),
            block_states: BlockStates::from_owned(
                nbt.compound("block_states").expect("block_states"),
            ),
            biomes: Biomes::from_owned(nbt.compound("biomes").expect("biomes")),
            BlockLight: nbt.byte_array("BlockLight").expect("BlockLight"),
            SkyLight: nbt.byte_array("SkyLight").expect("SkyLight"),
        }
    }

    fn to_compound(&self) -> owned::NbtCompound {
        let mut c = owned::NbtCompound::new();
        c.insert("Y", self.Y);
        c.insert("block_states", self.block_states.to_compound());
        c.insert("biomes", self.biomes.to_compound());
        c.insert(
            "BlockLight",
            owned::NbtTag::ByteArray(self.BlockLight.to_vec()),
        );
        c.insert("SkyLight", owned::NbtTag::ByteArray(self.SkyLight.to_vec()));
        c
    }
}

impl<'a> BlockStates<'a> {
    fn from_borrow(nbt: borrow::NbtCompound<'a, '_>) -> Self {
        Self {
            palette: borrow_compounds(&nbt, "palette", PaletteEntry::from_borrow),
            data: nbt.long_array("data").expect("data"),
        }
    }

    fn from_owned(nbt: &'a owned::NbtCompound) -> Self {
        Self {
            palette: owned_compounds(nbt, "palette", PaletteEntry::from_owned),
            data: nbt.long_array("data").expect("data").to_vec(),
        }
    }

    fn to_compound(&self) -> owned::NbtCompound {
        let mut c = owned::NbtCompound::new();
        c.insert(
            "palette",
            compound_list(self.palette.iter().map(PaletteEntry::to_compound)),
        );
        c.insert("data", owned::NbtTag::LongArray(self.data.clone()));
        c
    }
}

impl<'a> PaletteEntry<'a> {
    fn from_borrow(nbt: borrow::NbtCompound<'a, '_>) -> Self {
        Self {
            Name: text(nbt.string("Name")),
        }
    }

    fn from_owned(nbt: &'a owned::NbtCompound) -> Self {
        Self {
            Name: text(nbt.string("Name")),
        }
    }

    fn to_compound(&self) -> owned::NbtCompound {
        let mut c = owned::NbtCompound::new();
        c.insert("Name", self.Name.as_ref());
        c
    }
}

impl<'a> Biomes<'a> {
    fn from_borrow(nbt: borrow::NbtCompound<'a, '_>) -> Self {
        Self {
            palette: borrow_strings(&nbt, "palette"),
            data: nbt.long_array("data").expect("data"),
        }
    }

    fn from_owned(nbt: &'a owned::NbtCompound) -> Self {
        Self {
            palette: owned_strings(nbt, "palette"),
            data: nbt.long_array("data").expect("data").to_vec(),
        }
    }

    fn to_compound(&self) -> owned::NbtCompound {
        let mut c = owned::NbtCompound::new();
        c.insert("palette", string_list(&self.palette));
        c.insert("data", owned::NbtTag::LongArray(self.data.clone()));
        c
    }
}

impl Heightmaps {
    fn from_borrow(nbt: borrow::NbtCompound<'_, '_>) -> Self {
        Self {
            MOTION_BLOCKING: nbt.long_array("MOTION_BLOCKING").expect("MOTION_BLOCKING"),
            WORLD_SURFACE: nbt.long_array("WORLD_SURFACE").expect("WORLD_SURFACE"),
        }
    }

    fn from_owned(nbt: &owned::NbtCompound) -> Self {
        Self {
            MOTION_BLOCKING: nbt
                .long_array("MOTION_BLOCKING")
                .expect("MOTION_BLOCKING")
                .to_vec(),
            WORLD_SURFACE: nbt
                .long_array("WORLD_SURFACE")
                .expect("WORLD_SURFACE")
                .to_vec(),
        }
    }

    fn to_compound(&self) -> owned::NbtCompound {
        let mut c = owned::NbtCompound::new();
        c.insert(
            "MOTION_BLOCKING",
            owned::NbtTag::LongArray(self.MOTION_BLOCKING.clone()),
        );
        c.insert(
            "WORLD_SURFACE",
            owned::NbtTag::LongArray(self.WORLD_SURFACE.clone()),
        );
        c
    }
}

impl<'a> BlockEntity<'a> {
    fn from_borrow(nbt: borrow::NbtCompound<'a, '_>) -> Self {
        Self {
            id: text(nbt.string("id")),
            x: nbt.int("x").expect("x"),
            y: nbt.int("y").expect("y"),
            z: nbt.int("z").expect("z"),
            KeepPacked: nbt.byte("KeepPacked").expect("KeepPacked"),
        }
    }

    fn from_owned(nbt: &'a owned::NbtCompound) -> Self {
        Self {
            id: text(nbt.string("id")),
            x: nbt.int("x").expect("x"),
            y: nbt.int("y").expect("y"),
            z: nbt.int("z").expect("z"),
            KeepPacked: nbt.byte("KeepPacked").expect("KeepPacked"),
        }
    }

    fn to_compound(&self) -> owned::NbtCompound {
        let mut c = owned::NbtCompound::new();
        c.insert("id", self.id.as_ref());
        c.insert("x", self.x);
        c.insert("y", self.y);
        c.insert("z", self.z);
        c.insert("KeepPacked", self.KeepPacked);
        c
    }
}

// ---------------------------------------------------------------------------
// The entries.
// ---------------------------------------------------------------------------

/// Writes a struct that is already parsed, through a fresh tree per iteration.
fn bench_write_value<T, S: std::fmt::Display>(
    group: &mut BenchmarkGroup<'_, WallTime>,
    name: &str,
    label: S,
    value: &T,
    to_compound: impl Fn(&T) -> owned::NbtCompound,
) {
    let mut measure = Vec::new();
    owned::BaseNbt::new("", to_compound(value)).write(&mut measure);
    let len = measure.len();
    group.throughput(Throughput::Bytes(len as u64));
    group.bench_function(BenchmarkId::new(name, label), |b| {
        b.iter(|| {
            let mut out = Vec::with_capacity(len);
            owned::BaseNbt::new("", to_compound(std::hint::black_box(value))).write(&mut out);
            out
        });
    });
}

pub fn parse(group: &mut BenchmarkGroup<'_, WallTime>, input: BenchInput) {
    match input {
        BenchInput::Doc(doc, bytes) => match doc {
            Doc::Small => {
                bench_parse(
                    group,
                    "simdnbt-borrow",
                    doc.name(),
                    bytes,
                    |b: &'_ [u8]| {
                        let base = borrow::read(&mut Cursor::new(std::hint::black_box(b)))
                            .expect("the document parses")
                            .unwrap();
                        let _ = std::hint::black_box(Small::from_borrow(&base));
                    },
                );
                bench_parse(
                    group,
                    "simdnbt-owned",
                    doc.name(),
                    bytes,
                    |b: &'_ [u8]| {
                        let base = owned::read(&mut Cursor::new(std::hint::black_box(b)))
                            .expect("the document parses")
                            .unwrap();
                        let _ = std::hint::black_box(Small::from_owned(&base));
                    },
                );
            }
            Doc::Player => {
                bench_parse(
                    group,
                    "simdnbt-borrow",
                    doc.name(),
                    bytes,
                    |b: &'_ [u8]| {
                        let base = borrow::read(&mut Cursor::new(std::hint::black_box(b)))
                            .expect("the document parses")
                            .unwrap();
                        let _ = std::hint::black_box(Player::from_borrow(&base));
                    },
                );
                bench_parse(
                    group,
                    "simdnbt-owned",
                    doc.name(),
                    bytes,
                    |b: &'_ [u8]| {
                        let base = owned::read(&mut Cursor::new(std::hint::black_box(b)))
                            .expect("the document parses")
                            .unwrap();
                        let _ = std::hint::black_box(Player::from_owned(&base));
                    },
                );
            }
            Doc::Chunk => {
                bench_parse(
                    group,
                    "simdnbt-borrow",
                    doc.name(),
                    bytes,
                    |b: &'_ [u8]| {
                        let base = borrow::read(&mut Cursor::new(std::hint::black_box(b)))
                            .expect("the document parses")
                            .unwrap();
                        let _ = std::hint::black_box(Chunk::from_borrow(&base));
                    },
                );
                bench_parse(
                    group,
                    "simdnbt-owned",
                    doc.name(),
                    bytes,
                    |b: &'_ [u8]| {
                        let base = owned::read(&mut Cursor::new(std::hint::black_box(b)))
                            .expect("the document parses")
                            .unwrap();
                        let _ = std::hint::black_box(Chunk::from_owned(&base));
                    },
                );
            }
        },
        BenchInput::Array(kind, bytes) => match kind {
            Array::Byte => {
                bench_parse(
                    group,
                    "simdnbt-borrow",
                    kind.name(),
                    bytes,
                    |b: &'_ [u8]| {
                        let base = borrow::read(&mut Cursor::new(std::hint::black_box(b)))
                            .expect("the document parses")
                            .unwrap();
                        let _ = std::hint::black_box(
                            base.as_compound().byte_array("data").expect("data"),
                        );
                    },
                );
                bench_parse(
                    group,
                    "simdnbt-owned",
                    kind.name(),
                    bytes,
                    |b: &'_ [u8]| {
                        let base = owned::read(&mut Cursor::new(std::hint::black_box(b)))
                            .expect("the document parses")
                            .unwrap();
                        let _ = std::hint::black_box(
                            base.as_compound().byte_array("data").expect("data"),
                        );
                    },
                );
            }
            Array::Short => {
                bench_parse(
                    group,
                    "simdnbt-borrow",
                    kind.name(),
                    bytes,
                    |b: &'_ [u8]| {
                        let base = borrow::read(&mut Cursor::new(std::hint::black_box(b)))
                            .expect("the document parses")
                            .unwrap();
                        let _ = std::hint::black_box(
                            base.as_compound()
                                .list("data")
                                .and_then(|list| list.shorts())
                                .expect("data"),
                        );
                    },
                );
                bench_parse(
                    group,
                    "simdnbt-owned",
                    kind.name(),
                    bytes,
                    |b: &'_ [u8]| {
                        let base = owned::read(&mut Cursor::new(std::hint::black_box(b)))
                            .expect("the document parses")
                            .unwrap();
                        let _ = std::hint::black_box(
                            base.as_compound()
                                .list("data")
                                .and_then(|list| list.shorts())
                                .expect("data"),
                        );
                    },
                );
            }
            Array::Int => {
                bench_parse(
                    group,
                    "simdnbt-borrow",
                    kind.name(),
                    bytes,
                    |b: &'_ [u8]| {
                        let base = borrow::read(&mut Cursor::new(std::hint::black_box(b)))
                            .expect("the document parses")
                            .unwrap();
                        let _ = std::hint::black_box(
                            base.as_compound().int_array("data").expect("data"),
                        );
                    },
                );
                bench_parse(
                    group,
                    "simdnbt-owned",
                    kind.name(),
                    bytes,
                    |b: &'_ [u8]| {
                        let base = owned::read(&mut Cursor::new(std::hint::black_box(b)))
                            .expect("the document parses")
                            .unwrap();
                        let _ = std::hint::black_box(
                            base.as_compound().int_array("data").expect("data"),
                        );
                    },
                );
            }
            Array::Long => {
                bench_parse(
                    group,
                    "simdnbt-borrow",
                    kind.name(),
                    bytes,
                    |b: &'_ [u8]| {
                        let base = borrow::read(&mut Cursor::new(std::hint::black_box(b)))
                            .expect("the document parses")
                            .unwrap();
                        let _ = std::hint::black_box(
                            base.as_compound().long_array("data").expect("data"),
                        );
                    },
                );
                bench_parse(
                    group,
                    "simdnbt-owned",
                    kind.name(),
                    bytes,
                    |b: &'_ [u8]| {
                        let base = owned::read(&mut Cursor::new(std::hint::black_box(b)))
                            .expect("the document parses")
                            .unwrap();
                        let _ = std::hint::black_box(
                            base.as_compound().long_array("data").expect("data"),
                        );
                    },
                );
            }
        },
    }
}

/// The owned `data` entry of each kind, for the write entries.
fn byte_data(bytes: &[u8]) -> Vec<u8> {
    let base = owned::read(&mut Cursor::new(bytes))
        .expect("the document parses")
        .unwrap();
    base.as_compound()
        .byte_array("data")
        .expect("data")
        .to_vec()
}

fn short_data(bytes: &[u8]) -> Vec<i16> {
    let base = owned::read(&mut Cursor::new(bytes))
        .expect("the document parses")
        .unwrap();
    base.as_compound()
        .list("data")
        .and_then(|list| list.shorts())
        .expect("data")
}

fn int_data(bytes: &[u8]) -> Vec<i32> {
    let base = owned::read(&mut Cursor::new(bytes))
        .expect("the document parses")
        .unwrap();
    base.as_compound().int_array("data").expect("data").to_vec()
}

fn long_data(bytes: &[u8]) -> Vec<i64> {
    let base = owned::read(&mut Cursor::new(bytes))
        .expect("the document parses")
        .unwrap();
    base.as_compound()
        .long_array("data")
        .expect("data")
        .to_vec()
}

pub fn write(group: &mut BenchmarkGroup<'_, WallTime>, input: BenchInput) {
    match input {
        BenchInput::Doc(doc, bytes) => match doc {
            Doc::Small => {
                let base = borrow::read(&mut Cursor::new(bytes))
                    .expect("the document parses")
                    .unwrap();
                let value = Small::from_borrow(&base);
                bench_write_value(
                    group,
                    "simdnbt-borrow",
                    doc.name(),
                    &value,
                    Small::to_compound,
                );
                let base = owned::read(&mut Cursor::new(bytes))
                    .expect("the document parses")
                    .unwrap();
                let value = Small::from_owned(&base);
                bench_write_value(
                    group,
                    "simdnbt-owned",
                    doc.name(),
                    &value,
                    Small::to_compound,
                );
            }
            Doc::Player => {
                let base = borrow::read(&mut Cursor::new(bytes))
                    .expect("the document parses")
                    .unwrap();
                let value = Player::from_borrow(&base);
                bench_write_value(
                    group,
                    "simdnbt-borrow",
                    doc.name(),
                    &value,
                    Player::to_compound,
                );
                let base = owned::read(&mut Cursor::new(bytes))
                    .expect("the document parses")
                    .unwrap();
                let value = Player::from_owned(&base);
                bench_write_value(
                    group,
                    "simdnbt-owned",
                    doc.name(),
                    &value,
                    Player::to_compound,
                );
            }
            Doc::Chunk => {
                let base = borrow::read(&mut Cursor::new(bytes))
                    .expect("the document parses")
                    .unwrap();
                let value = Chunk::from_borrow(&base);
                bench_write_value(
                    group,
                    "simdnbt-borrow",
                    doc.name(),
                    &value,
                    Chunk::to_compound,
                );
                let base = owned::read(&mut Cursor::new(bytes))
                    .expect("the document parses")
                    .unwrap();
                let value = Chunk::from_owned(&base);
                bench_write_value(
                    group,
                    "simdnbt-owned",
                    doc.name(),
                    &value,
                    Chunk::to_compound,
                );
            }
        },
        BenchInput::Array(kind, bytes) => match kind {
            Array::Byte => {
                let compound = |v: &Vec<u8>| {
                    let mut c = owned::NbtCompound::new();
                    c.insert("data", owned::NbtTag::ByteArray(v.clone()));
                    c
                };
                bench_write(
                    group,
                    "simdnbt-borrow",
                    kind.name(),
                    bytes,
                    byte_data,
                    |v| {
                        let mut out = Vec::new();
                        owned::BaseNbt::new("", compound(v)).write(&mut out);
                        out
                    },
                );
                bench_write(group, "simdnbt-owned", kind.name(), bytes, byte_data, |v| {
                    let mut out = Vec::new();
                    owned::BaseNbt::new("", compound(v)).write(&mut out);
                    out
                });
            }
            Array::Short => {
                let compound = |v: &Vec<i16>| {
                    let mut c = owned::NbtCompound::new();
                    c.insert("data", short_list(v));
                    c
                };
                bench_write(
                    group,
                    "simdnbt-borrow",
                    kind.name(),
                    bytes,
                    short_data,
                    |v| {
                        let mut out = Vec::new();
                        owned::BaseNbt::new("", compound(v)).write(&mut out);
                        out
                    },
                );
                bench_write(
                    group,
                    "simdnbt-owned",
                    kind.name(),
                    bytes,
                    short_data,
                    |v| {
                        let mut out = Vec::new();
                        owned::BaseNbt::new("", compound(v)).write(&mut out);
                        out
                    },
                );
            }
            Array::Int => {
                let compound = |v: &Vec<i32>| {
                    let mut c = owned::NbtCompound::new();
                    c.insert("data", owned::NbtTag::IntArray(v.clone()));
                    c
                };
                bench_write(group, "simdnbt-borrow", kind.name(), bytes, int_data, |v| {
                    let mut out = Vec::new();
                    owned::BaseNbt::new("", compound(v)).write(&mut out);
                    out
                });
                bench_write(group, "simdnbt-owned", kind.name(), bytes, int_data, |v| {
                    let mut out = Vec::new();
                    owned::BaseNbt::new("", compound(v)).write(&mut out);
                    out
                });
            }
            Array::Long => {
                let compound = |v: &Vec<i64>| {
                    let mut c = owned::NbtCompound::new();
                    c.insert("data", owned::NbtTag::LongArray(v.clone()));
                    c
                };
                bench_write(
                    group,
                    "simdnbt-borrow",
                    kind.name(),
                    bytes,
                    long_data,
                    |v| {
                        let mut out = Vec::new();
                        owned::BaseNbt::new("", compound(v)).write(&mut out);
                        out
                    },
                );
                bench_write(group, "simdnbt-owned", kind.name(), bytes, long_data, |v| {
                    let mut out = Vec::new();
                    owned::BaseNbt::new("", compound(v)).write(&mut out);
                    out
                });
            }
        },
    }
}
