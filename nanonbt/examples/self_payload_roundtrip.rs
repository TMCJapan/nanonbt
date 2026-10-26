//! Rehydrates a captured compound payload as a complete NBT document.
//!
//! `#[nbt(self)]` records every byte of the enclosing compound's payload, so
//! those bytes can be stored or forwarded as-is. They do *not* include the
//! document's root tag/name framing, and they cover the *whole* compound rather
//! than one field; this example shows how to turn them back into a typed value
//! by re-adding that framing.
//!
//! Run with `cargo run --example self_payload_roundtrip --features alloc`.

extern crate alloc;

use alloc::vec::Vec;

use nanonbt::{from_bytes, from_slice, to_bytes};

#[derive(nanonbt::DeserializeNbt, Debug)]
struct RawCapture<'de> {
    x: i64,
    #[nbt(self)]
    payload: &'de [u8],
}

#[derive(nanonbt::SerializeNbt, nanonbt::DeserializeNbt, Debug, PartialEq)]
struct Source {
    x: i64,
    y: Nested,
}

#[derive(nanonbt::SerializeNbt, nanonbt::DeserializeNbt, Debug, PartialEq)]
struct Nested {
    value: alloc::string::String,
}

fn main() {
    let original = Source {
        x: 7,
        y: Nested {
            value: "hello".to_owned(),
        },
    };
    let bytes = to_bytes(&original).expect("encode original");

    // Capture the full raw payload (including the trailing `TAG_End`).
    let captured: RawCapture<'_> = from_slice(&bytes).expect("capture raw payload");
    assert_eq!(captured.x, original.x);
    assert_eq!(captured.payload, &bytes[3..]);

    // The captured bytes are only the payload, so prepend the empty-named root
    // compound framing before decoding them as a complete document again.
    let mut document = Vec::with_capacity(3 + captured.payload.len());
    document.extend_from_slice(&[nanonbt::Tag::Compound.as_u8(), 0, 0]);
    document.extend_from_slice(captured.payload);

    let recovered: Source = from_bytes(&document).expect("decode captured payload");
    assert_eq!(recovered, original);
    assert_eq!(recovered.y, original.y);
    println!("round-tripped {} raw payload bytes", captured.payload.len());
}
