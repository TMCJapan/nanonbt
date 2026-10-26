//! Captures the complete serialized payload of a compound as raw bytes.
//!
//! Run with `cargo run --example self_raw --features alloc`.

extern crate alloc;

use alloc::vec::Vec;

use nanonbt::{from_bytes, from_slice, to_bytes};

#[derive(nanonbt::SerializeNbt, nanonbt::DeserializeNbt, Debug, PartialEq)]
struct Player {
    name: alloc::string::String,
    health: f32,
}

#[derive(nanonbt::DeserializeNbt, Debug)]
struct RawSnapshot {
    #[nbt(self)]
    payload: Vec<u8>,
}

#[derive(nanonbt::DeserializeNbt, Debug)]
struct RawSnapshotBorrowed<'de> {
    #[nbt(self)]
    payload: &'de [u8],
}

fn main() {
    let player = Player {
        name: "Steve".to_owned(),
        health: 20.0,
    };
    let bytes = to_bytes(&player).expect("encode");

    // `payload` receives every byte of the compound's payload, including the
    // trailing `TAG_End`, so it can be stored or forwarded untouched.
    let snapshot: RawSnapshot = from_bytes(&bytes).expect("decode");
    assert_eq!(snapshot.payload, bytes[3..]);
    println!("captured {} raw bytes", snapshot.payload.len());

    // The same works zero-copy when the reader can hand out slices.
    let borrowed: RawSnapshotBorrowed<'_> = from_slice(&bytes).expect("decode borrowed");
    assert_eq!(borrowed.payload, &bytes[3..]);
    println!("borrowed {} raw bytes in place", borrowed.payload.len());
}
