//! Zero-copy borrowed string fields in a derived struct.
//!
//! `&'de Cesu8`, `&'de str`, `Cow<'de, Cesu8>`, and lists of borrowed
//! strings can all be decoded straight out of the input buffer without
//! copying. This example round-trips such a struct and verifies that the
//! borrowed fields point back into the encoded bytes.
//!
//! Run with `cargo run --example borrowed_strings`.

extern crate alloc;

use alloc::borrow::Cow;

use nanonbt::{Cesu8, DeserializeNbt, SerializeNbt, from_slice, to_bytes};

#[derive(SerializeNbt, DeserializeNbt, Debug, PartialEq)]
struct Sign<'de> {
    /// `TAG_String` borrowed as modified UTF-8 without copying.
    title: &'de Cesu8,
    /// The same tag, decoded straight into a UTF-8 `&str`.
    body: &'de str,
    /// Owned-or-borrowed string, lent out of the input here.
    tag: Cow<'de, Cesu8>,
    /// A `TAG_List<TAG_String>` whose elements keep borrowing.
    labels: Vec<&'de str>,
}

fn main() {
    let sign = Sign {
        title: Cesu8::try_from_str("Welcome").expect("valid modified UTF-8"),
        body: "Hello, world!",
        tag: Cow::Borrowed(Cesu8::try_from_str("notice").expect("valid modified UTF-8")),
        labels: vec!["rust", "nbt", "zero-copy"],
    };

    let bytes = to_bytes(&sign).expect("encode");
    println!("encoded {} bytes", bytes.len());

    let borrowed: Sign<'_> = from_slice(&bytes).expect("decode borrowed");
    println!("decoded: {borrowed:?}");
    assert_eq!(borrowed, sign);

    // The title bytes should alias the input buffer rather than a fresh
    // allocation. (Some spellings borrow, others own their storage.)
    let start = bytes.as_ptr() as usize;
    let end = start + bytes.len();
    let title_in_input =
        (start..end).contains(&(borrowed.title.as_bytes().as_ptr() as usize));
    println!("title borrowed from input: {title_in_input}");

    // Serializing the borrowed value writes the same bytes back.
    let reencoded = to_bytes(&borrowed).expect("re-encode");
    assert_eq!(reencoded, bytes);
    println!("round trip byte-identical");
}
