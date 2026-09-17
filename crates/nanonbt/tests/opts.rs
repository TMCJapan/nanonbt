#![allow(clippy::items_after_statements)]
//! Serialization and deserialization options behave like fastnbt's.

use nanonbt::{DeOpts, Error, FromNBT, Read, Reader, ToNBT, Value};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, ToNBT, FromNBT, PartialEq, Debug)]
struct Schematic {
    width: i16,
    blocks: Vec<i8>,
}

const SCHEMATIC: Schematic = Schematic {
    width: 3,
    blocks: Vec::new(),
};

#[test]
fn root_name_and_network_nbt_serialize_like_fastnbt() {
    let named =
        fastnbt::to_bytes_with_opts(&SCHEMATIC, fastnbt::SerOpts::new().root_name("Schematic"));
    assert_eq!(
        nanonbt::to_bytes_with_opts(&SCHEMATIC, nanonbt::SerOpts::new().root_name("Schematic"))
            .ok(),
        named.ok()
    );

    let network = fastnbt::to_bytes_with_opts(&SCHEMATIC, fastnbt::SerOpts::network_nbt());
    assert_eq!(
        nanonbt::to_bytes_with_opts(&SCHEMATIC, nanonbt::SerOpts::network_nbt()).ok(),
        network.ok()
    );

    // Setting a root name turns the name back on.
    let renamed = fastnbt::SerOpts::network_nbt().root_name("日\0");
    assert_eq!(
        nanonbt::to_bytes_with_opts(
            &SCHEMATIC,
            nanonbt::SerOpts::network_nbt().root_name("日\0")
        )
        .ok(),
        fastnbt::to_bytes_with_opts(&SCHEMATIC, renamed).ok()
    );
}

#[test]
fn network_nbt_and_sequence_limits_deserialize_like_fastnbt() {
    let network = fastnbt::to_bytes_with_opts(
        &Schematic {
            width: 1,
            blocks: vec![1, 2, 3],
        },
        fastnbt::SerOpts::network_nbt(),
    )
    .unwrap();
    let named = fastnbt::to_bytes(&Schematic {
        width: 1,
        blocks: vec![1, 2, 3],
    })
    .unwrap();

    for bytes in [&network, &named] {
        for (fast, nano) in [
            (
                fastnbt::DeOpts::network_nbt(),
                nanonbt::DeOpts::network_nbt(),
            ),
            (fastnbt::DeOpts::new(), nanonbt::DeOpts::new()),
            (
                fastnbt::DeOpts::new().max_seq_len(2),
                nanonbt::DeOpts::new().max_seq_len(2),
            ),
            (
                fastnbt::DeOpts::new().max_seq_len(3),
                nanonbt::DeOpts::new().max_seq_len(3),
            ),
            (
                fastnbt::DeOpts::network_nbt().expect_coumpound_names(true),
                nanonbt::DeOpts::network_nbt().expect_compound_names(true),
            ),
        ] {
            assert_eq!(
                nanonbt::from_bytes_with_opts::<Schematic>(bytes, nano).ok(),
                fastnbt::from_bytes_with_opts::<Schematic>(bytes, fast).ok(),
            );
        }
    }
}

/// The root compound, then `depth - 1` nested compounds, as a document.
fn nested_compounds(depth: usize) -> Vec<u8> {
    let mut bytes = vec![10, 0, 0];
    for _ in 1..depth {
        bytes.extend_from_slice(&[10, 0, 1, b'x']);
    }
    bytes.resize(bytes.len() + depth, 0);
    bytes
}

/// The root compound, then `depth - 1` nested lists of one element.
fn nested_lists(depth: usize) -> Vec<u8> {
    let mut bytes = vec![10, 0, 0];
    if depth > 1 {
        bytes.extend_from_slice(&[9, 0, 1, b'x']);
        for _ in 2..depth {
            bytes.push(9);
            bytes.extend_from_slice(&1i32.to_be_bytes());
        }
        bytes.push(1);
        bytes.extend_from_slice(&0i32.to_be_bytes());
    }
    bytes.push(0);
    bytes
}

/// A depth bound, because reading recurses and an overflow cannot be caught.
///
/// fastnbt has no such option, so it is not compared here; past its own stack
/// it aborts the process rather than returning an error.
#[test]
fn nesting_deeper_than_max_depth_is_refused() {
    #[derive(FromNBT, Debug)]
    struct Empty {}

    for document in [nested_compounds, nested_lists] {
        for depth in [1, 2, 8, 512] {
            let bytes = document(depth);
            let opts = nanonbt::DeOpts::new().max_depth(depth);
            assert!(
                nanonbt::from_bytes_with_opts::<Value>(&bytes, opts.clone()).is_ok(),
                "depth {depth} refused at its own limit"
            );
            // Skipping the whole document counts the same levels.
            assert!(nanonbt::from_bytes_with_opts::<Empty>(&bytes, opts).is_ok());

            let opts = nanonbt::DeOpts::new().max_depth(depth - 1);
            assert!(
                nanonbt::from_bytes_with_opts::<Value>(&bytes, opts.clone()).is_err(),
                "depth {depth} accepted one level past the limit"
            );
            assert!(nanonbt::from_bytes_with_opts::<Empty>(&bytes, opts).is_err());
        }
    }

    // The default keeps a document that would overflow a small stack out.
    assert!(nanonbt::from_bytes::<Value>(&nested_lists(100_000)).is_err());
    assert!(nanonbt::from_bytes::<Empty>(&nested_compounds(100_000)).is_err());
}

/// A failed read gives its levels back, so the next one has the full bound.
#[test]
fn a_failed_read_does_not_spend_depth() {
    let mut reader = Reader::new(&[], DeOpts::new().max_depth(1));
    let failed = reader.nest(|reader| {
        let inner: nanonbt::Result<()> = reader.nest(|_| Err(Error::invalid_tag(0)));
        inner
    });
    assert!(failed.is_err());
    assert!(
        reader.nest(|_| Ok(())).is_ok(),
        "the failed read kept a level"
    );
}

/// A name that is not valid modified UTF-8 is skipped, not decoded.
#[test]
fn root_names_are_skipped_without_decoding() {
    #[derive(FromNBT, PartialEq, Debug)]
    struct Empty {}

    // `C0 81` is neither UTF-8 nor valid modified UTF-8.
    let bytes = [0x0a, 0x00, 0x02, 0xc0, 0x81, 0x00];
    assert_eq!(
        nanonbt::from_bytes::<Empty>(&bytes),
        Ok(Empty {}),
        "the root name is skipped"
    );
    assert!(fastnbt::from_bytes::<fastnbt::Value>(&bytes).is_ok());
}
