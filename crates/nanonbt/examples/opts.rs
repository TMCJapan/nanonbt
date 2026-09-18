//! Root names, network NBT, and the limits that guard untrusted input.

use std::collections::BTreeMap;

use nanonbt::{
    DeOpts, FromNBT, SerOpts, ToNBT, Value, from_bytes_with_opts, to_bytes, to_bytes_with_opts,
};

#[derive(FromNBT, ToNBT, PartialEq, Debug)]
struct Level {
    seed: i64,
    spawn: [i32; 3],
}

fn main() -> nanonbt::Result<()> {
    let level = Level {
        seed: -1,
        spawn: [0, 64, 0],
    };

    // A named root compound is the classic Minecraft layout; network NBT
    // leaves the name out, which saves its four bytes.
    let named = to_bytes_with_opts(&level, SerOpts::new().root_name("Data"))?;
    let network = to_bytes_with_opts(&level, SerOpts::network_nbt())?;
    println!("named:   {} bytes", named.len());
    println!("network: {} bytes", network.len());

    let back: Level = from_bytes_with_opts(&named, DeOpts::new())?;
    assert_eq!(back, level);
    let back: Level = from_bytes_with_opts(&network, DeOpts::network_nbt())?;
    assert_eq!(back, level);
    println!("both documents read back");

    // A list longer than `max_seq_len` is refused before it is allocated.
    let long_list = Value::Compound(BTreeMap::from([(
        "entries".into(),
        Value::List((0..100).map(Value::Int).collect()),
    )]));
    let bytes = to_bytes(&long_list)?;
    let error = from_bytes_with_opts::<Value>(&bytes, DeOpts::new().max_seq_len(10)).unwrap_err();
    println!("max_seq_len(10): {error}");

    // Reading recurses once per level, so deep documents are refused too.
    let bytes = nested_compounds(8);
    let error = from_bytes_with_opts::<Value>(&bytes, DeOpts::new().max_depth(4)).unwrap_err();
    println!("max_depth(4): {error}");

    Ok(())
}

/// The root compound, then `depth - 1` nested compounds, as a document.
fn nested_compounds(depth: usize) -> Vec<u8> {
    let mut bytes = vec![nanonbt::TAG_COMPOUND, 0, 0];
    for _ in 1..depth {
        bytes.extend_from_slice(&[nanonbt::TAG_COMPOUND, 0, 1, b'x']);
    }
    bytes.resize(bytes.len() + depth, nanonbt::TAG_END);
    bytes
}
