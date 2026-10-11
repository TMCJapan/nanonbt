//! Round-trips a Minecraft-flavored struct through NBT and decodes packed
//! arrays straight out of the input buffer.
//!
//! Run with `cargo run --example player`.

extern crate alloc;

use alloc::borrow::Cow;

use nanonbt::{DeserializeNbt, I64Be, SerializeNbt, from_bytes, from_slice, to_bytes};

#[derive(SerializeNbt, DeserializeNbt, Debug, PartialEq)]
struct Player {
    /// `TAG_List<TAG_Byte>`: a plain list.
    inventory: Vec<u8>,
    /// `TAG_Byte_Array` via the attribute.
    #[nbt(array = byte)]
    skin: Vec<u8>,
    /// `TAG_Int_Array` via the attribute.
    #[nbt(array = int)]
    scores: Vec<i32>,
    /// `TAG_Long_Array` with the route inferred from the element type.
    #[nbt(array)]
    timestamps: Vec<i64>,
}

#[derive(SerializeNbt, DeserializeNbt, Debug, PartialEq)]
struct ChunkSection<'de> {
    y: i8,
    /// Borrowed straight out of the input buffer instead of copied.
    #[nbt(array = long)]
    states: Cow<'de, [I64Be]>,
}

fn main() {
    let player = Player {
        inventory: vec![1, 2, 3, 4],
        skin: vec![0xaa, 0xbb, 0xcc],
        scores: vec![10, -20, 30],
        timestamps: vec![1_000_000, 2_000_000],
    };

    let bytes = to_bytes(&player).expect("encode");
    println!("Player encoded into {} bytes", bytes.len());

    let decoded: Player = from_bytes(&bytes).expect("decode");
    assert_eq!(player, decoded);
    println!("owned round trip: {decoded:?}");

    let section = ChunkSection {
        y: 4,
        states: Cow::Owned(vec![I64Be::new(1), I64Be::new(2)]),
    };
    let section_bytes = to_bytes(&section).expect("encode");
    let borrowed: ChunkSection<'_> = from_slice(&section_bytes).expect("decode");

    let start = section_bytes.as_ptr() as usize;
    let in_input =
        (start..start + section_bytes.len()).contains(&(borrowed.states.as_ptr() as usize));
    assert!(in_input, "states should point into the input buffer");
    println!(
        "zero-copy borrow verified: {} states read directly from the input buffer",
        borrowed.states.len()
    );

    // A reader that owns its buffer cannot be used for borrowed decoding; the
    // following would fail to compile because `VecReader` does not implement
    // `nanonbt::ReadRef`:
    //
    //     let mut reader = nanonbt::VecReader::new(section_bytes.clone());
    //     let _ = <ChunkSection<'_> as nanonbt::DeserializeNbtRef<'_>>::deserialize_nbt_ref(
    //         &mut reader,
    //     );
}
