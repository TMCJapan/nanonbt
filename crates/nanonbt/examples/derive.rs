//! A Minecraft-like player document, written and read through the derive macros.

use nanonbt::{FromNBT, ToNBT, from_bytes, to_bytes};

#[derive(FromNBT, ToNBT, PartialEq, Debug)]
struct Player {
    #[nbt(rename = "Health")]
    health: f32,
    #[nbt(rename = "Name")]
    name: String,
    food: i32,
    xp: i32,
    dimension: Dimension,
    position: Position,
    inventory: Vec<ItemStack>,
    #[nbt(ignore)]
    cached_hurt_time: u16,
}

#[derive(FromNBT, ToNBT, PartialEq, Debug)]
struct Position {
    x: f64,
    y: f64,
    z: f64,
}

#[derive(FromNBT, ToNBT, PartialEq, Debug)]
enum Dimension {
    Overworld,
    #[nbt(rename = "the_nether")]
    Nether,
    #[nbt(rename = "the_end")]
    End,
}

#[derive(FromNBT, ToNBT, PartialEq, Debug)]
struct ItemStack {
    id: String,
    count: i8,
    slot: Slot,
    #[nbt(rename = "CustomName")]
    custom_name: Option<String>,
}

/// A newtype is transparent: it writes as the inner value, with no compound.
#[derive(FromNBT, ToNBT, PartialEq, Debug)]
struct Slot(u8);

fn main() -> nanonbt::Result<()> {
    let player = Player {
        health: 20.0,
        name: "topi".into(),
        food: 20,
        xp: 7,
        dimension: Dimension::Nether,
        position: Position {
            x: 0.5,
            y: 64.0,
            z: -12.25,
        },
        inventory: vec![
            ItemStack {
                id: "minecraft:diamond_sword".into(),
                count: 1,
                slot: Slot(0),
                custom_name: Some("Excalibur".into()),
            },
            ItemStack {
                id: "minecraft:apple".into(),
                count: 12,
                slot: Slot(1),
                custom_name: None,
            },
        ],
        cached_hurt_time: 99,
    };

    let bytes = to_bytes(&player)?;
    println!("wrote {} bytes", bytes.len());

    let back: Player = from_bytes(&bytes)?;
    println!("{back:#?}");

    // `cached_hurt_time` was not written, so it reads back as its default.
    let expected = Player {
        cached_hurt_time: 0,
        ..player
    };
    assert_eq!(back, expected);
    println!(
        "round trip matches, ignored field is {}",
        back.cached_hurt_time
    );

    Ok(())
}
