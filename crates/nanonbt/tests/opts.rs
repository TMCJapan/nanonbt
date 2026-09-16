//! Serialization and deserialization options behave like fastnbt's.

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, PartialEq, Debug)]
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
