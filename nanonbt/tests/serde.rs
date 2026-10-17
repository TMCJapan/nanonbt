//! serde's derives over the same wire format as the native derives.
//!
//! The interesting property is equivalence: a serde-derived struct and a
//! natively derived one with the same field names and encodings must produce
//! the same bytes, so the two derive families can decode each other's
//! documents.

use std::collections::BTreeMap;

use nanonbt::serde::{from_bytes, from_reader, to_bytes};
use nanonbt::{DeserializeNbt, F32Be, F64Be, I16Be, I32Be, I64Be, SerializeNbt, Tag, VecReader};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct Pet {
    name: String,
    hungry: bool,
}

#[derive(SerializeNbt, DeserializeNbt, Debug, PartialEq)]
struct NativePet {
    name: String,
    hungry: bool,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct Player {
    name: String,
    health: f32,
    #[serde(rename = "score")]
    score: i32,
    #[serde(skip)]
    hidden: i64,
    #[serde(with = "serde_bytes")]
    skin: Vec<u8>,
    scores: Vec<i32>,
    pet: Pet,
    coordinates: Vec<f64>,
}

#[derive(SerializeNbt, DeserializeNbt, Debug, PartialEq)]
struct NativePlayer {
    name: String,
    health: f32,
    #[nbt(rename = "score")]
    score: i32,
    #[nbt(skip)]
    hidden: i64,
    #[nbt(array = byte)]
    skin: Vec<u8>,
    scores: Vec<i32>,
    pet: NativePet,
    coordinates: Vec<f64>,
}

fn player() -> (Player, NativePlayer) {
    (
        Player {
            name: "Steve".to_owned(),
            health: 20.0,
            score: -12,
            hidden: 0,
            skin: vec![1, 2, 3],
            scores: vec![10, -20],
            pet: Pet {
                name: "Rex".to_owned(),
                hungry: true,
            },
            coordinates: vec![0.5, -1.5],
        },
        NativePlayer {
            name: "Steve".to_owned(),
            health: 20.0,
            score: -12,
            hidden: 0,
            skin: vec![1, 2, 3],
            scores: vec![10, -20],
            pet: NativePet {
                name: "Rex".to_owned(),
                hungry: true,
            },
            coordinates: vec![0.5, -1.5],
        },
    )
}

#[test]
fn serde_and_native_derives_write_the_same_bytes() {
    let (serde_player, native_player) = player();
    assert_eq!(
        to_bytes(&serde_player).unwrap(),
        nanonbt::to_bytes(&native_player).unwrap()
    );

    // The skipped field stays off the wire on both paths.
    let (mut serde_player, mut native_player) = player();
    serde_player.hidden = 99;
    native_player.hidden = 99;
    assert_eq!(
        to_bytes(&serde_player).unwrap(),
        nanonbt::to_bytes(&native_player).unwrap()
    );
    assert_eq!(
        to_bytes(&serde_player).unwrap(),
        to_bytes(&player().0).unwrap()
    );
}

#[test]
fn serde_decodes_what_the_native_derive_wrote() {
    let (serde_player, native_player) = player();
    let bytes = nanonbt::to_bytes(&native_player).unwrap();
    assert_eq!(from_bytes::<Player>(&bytes).unwrap(), serde_player);

    let bytes = to_bytes(&serde_player).unwrap();
    assert_eq!(
        nanonbt::from_bytes::<NativePlayer>(&bytes).unwrap(),
        native_player
    );
}

#[test]
fn names_with_modified_utf8_roundtrip() {
    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct Text {
        #[serde(rename = "😀")]
        face: String,
    }

    #[derive(SerializeNbt, DeserializeNbt, Debug, PartialEq)]
    struct NativeText {
        #[nbt(rename = "😀")]
        face: String,
    }

    let value = Text {
        face: "a\0😀b".to_owned(),
    };
    let bytes = to_bytes(&value).unwrap();
    assert_eq!(
        bytes,
        nanonbt::to_bytes(&NativeText {
            face: "a\0😀b".to_owned(),
        })
        .unwrap()
    );
    assert_eq!(from_bytes::<Text>(&bytes).unwrap(), value);
}

#[test]
fn root_scalars_roundtrip() {
    assert_eq!(to_bytes(&7i64).unwrap(), nanonbt::to_bytes(&7i64).unwrap());
    assert_eq!(from_bytes::<i64>(&to_bytes(&7i64).unwrap()).unwrap(), 7);
    assert_eq!(to_bytes("hi").unwrap(), nanonbt::to_bytes("hi").unwrap());
    assert_eq!(
        from_bytes::<String>(&to_bytes("hi").unwrap()).unwrap(),
        "hi"
    );
}

#[test]
fn newtype_and_unit_structs_match_the_native_shapes() {
    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct Meters(i32);

    #[derive(SerializeNbt, DeserializeNbt, Debug, PartialEq)]
    struct NativeMeters(i32);

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct Marker;

    #[derive(SerializeNbt, DeserializeNbt, Debug, PartialEq)]
    struct NativeMarker;

    assert_eq!(
        to_bytes(&Meters(-5)).unwrap(),
        nanonbt::to_bytes(&NativeMeters(-5)).unwrap()
    );
    assert_eq!(
        to_bytes(&Marker).unwrap(),
        nanonbt::to_bytes(&NativeMarker).unwrap()
    );
    assert_eq!(
        from_bytes::<Meters>(&to_bytes(&Meters(3)).unwrap()).unwrap(),
        Meters(3)
    );
    assert_eq!(
        from_bytes::<Marker>(&to_bytes(&Marker).unwrap()).unwrap(),
        Marker
    );
}

#[test]
fn single_byte_values_share_the_byte_tag() {
    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct Bytes {
        flag: bool,
        signed: i8,
        unsigned: u8,
    }

    #[derive(SerializeNbt, DeserializeNbt, Debug, PartialEq)]
    struct NativeBytes {
        flag: bool,
        signed: i8,
        unsigned: u8,
    }

    let value = Bytes {
        flag: true,
        signed: -5,
        unsigned: 200,
    };
    let bytes = to_bytes(&value).unwrap();
    assert_eq!(
        bytes,
        nanonbt::to_bytes(&NativeBytes {
            flag: true,
            signed: -5,
            unsigned: 200,
        })
        .unwrap()
    );
    assert_eq!(from_bytes::<Bytes>(&bytes).unwrap(), value);
}

#[test]
fn options_are_fields_that_may_be_absent() {
    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct Maybe {
        value: i32,
        #[serde(skip_serializing_if = "Option::is_none")]
        maybe: Option<i32>,
    }

    let present = Maybe {
        value: 1,
        maybe: Some(2),
    };
    assert_eq!(
        from_bytes::<Maybe>(&to_bytes(&present).unwrap()).unwrap(),
        present
    );

    let absent = Maybe {
        value: 1,
        maybe: None,
    };
    assert_eq!(
        from_bytes::<Maybe>(&to_bytes(&absent).unwrap()).unwrap(),
        absent
    );
}

#[test]
fn none_without_skip_is_rejected() {
    #[derive(Serialize)]
    struct StrictNone {
        maybe: Option<i32>,
    }

    let error = to_bytes(&StrictNone { maybe: None }).unwrap_err();
    assert!(matches!(error, nanonbt::Error::InvalidData(_)), "{error:?}");
}

#[test]
fn none_is_rejected_at_the_root() {
    let error = to_bytes(&None::<i32>).unwrap_err();
    assert!(matches!(error, nanonbt::Error::InvalidData(_)), "{error:?}");
}

#[test]
fn maps_are_compounds() {
    #[derive(SerializeNbt)]
    struct Native {
        a: i32,
        b: i32,
    }

    let map = BTreeMap::from([("a".to_owned(), 1i32), ("b".to_owned(), 2i32)]);
    let bytes = to_bytes(&map).unwrap();
    assert_eq!(from_bytes::<BTreeMap<String, i32>>(&bytes).unwrap(), map);
    assert_eq!(bytes, nanonbt::to_bytes(&Native { a: 1, b: 2 }).unwrap());
}

#[test]
fn borrowed_strings_point_into_the_input() {
    #[derive(Deserialize, Debug, PartialEq)]
    struct Borrowed<'a> {
        #[serde(borrow)]
        name: &'a str,
    }

    #[derive(Serialize)]
    struct Owned {
        name: String,
    }

    let bytes = to_bytes(&Owned {
        name: "Steve".to_owned(),
    })
    .unwrap();
    let borrowed: Borrowed<'_> = from_bytes(&bytes).unwrap();
    assert_eq!(borrowed.name, "Steve");
    assert!(bytes.as_ptr_range().contains(&borrowed.name.as_ptr()));
}

#[test]
fn streaming_decode_is_owned() {
    let (serde_player, _) = player();
    let bytes = to_bytes(&serde_player).unwrap();
    let decoded: Player = from_reader(VecReader::new(bytes)).unwrap();
    assert_eq!(decoded, serde_player);
}

#[test]
fn int_arrays_decode_as_sequences() {
    #[derive(SerializeNbt)]
    struct Native {
        #[nbt(array = int)]
        values: Vec<i32>,
    }

    #[derive(Deserialize, Debug, PartialEq)]
    struct Decoded {
        values: Vec<i32>,
    }

    let bytes = nanonbt::to_bytes(&Native {
        values: vec![1, -2],
    })
    .unwrap();
    assert_eq!(
        from_bytes::<Decoded>(&bytes).unwrap(),
        Decoded {
            values: vec![1, -2]
        }
    );
}

#[test]
fn empty_lists_use_the_end_element_tag() {
    assert_eq!(
        to_bytes(&Vec::<i32>::new()).unwrap(),
        nanonbt::to_bytes(&Vec::<i32>::new()).unwrap()
    );
}

#[test]
fn wire_types_are_scalars_in_serde_structs() {
    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct Grid {
        cells: Vec<I32Be>,
        shorts: Vec<I16Be>,
        longs: Vec<I64Be>,
        floats: Vec<F32Be>,
        doubles: Vec<F64Be>,
    }

    let value = Grid {
        cells: vec![I32Be::new(-1), I32Be::new(2)],
        shorts: vec![I16Be::new(-3), I16Be::new(4)],
        longs: vec![I64Be::new(-5), I64Be::new(6)],
        floats: vec![F32Be::new(-1.5), F32Be::new(2.5)],
        doubles: vec![F64Be::new(-3.5), F64Be::new(4.5)],
    };
    assert_eq!(
        from_bytes::<Grid>(&to_bytes(&value).unwrap()).unwrap(),
        value
    );
}

#[test]
fn unknown_fields_are_skipped() {
    #[derive(Deserialize, Debug, PartialEq)]
    struct Small {
        a: i32,
    }

    let bytes = to_bytes(&BTreeMap::from([
        ("a".to_owned(), 1i32),
        ("b".to_owned(), 2i32),
    ]))
    .unwrap();
    assert_eq!(from_bytes::<Small>(&bytes).unwrap(), Small { a: 1 });
}

#[test]
fn chars_are_single_character_strings() {
    let bytes = to_bytes(&'x').unwrap();
    assert_eq!(from_bytes::<char>(&bytes).unwrap(), 'x');

    let bytes = to_bytes("xy").unwrap();
    assert!(from_bytes::<char>(&bytes).is_err());
}

#[test]
fn type_mismatches_report_the_tag() {
    let error = from_bytes::<Player>(&to_bytes(&5i32).unwrap()).unwrap_err();
    assert!(
        matches!(
            error,
            nanonbt::Error::InvalidTag {
                expected: Tag::Compound,
                actual,
            } if actual == Tag::Int.as_u8()
        ),
        "{error:?}"
    );
}

#[test]
fn missing_fields_are_reported() {
    #[derive(Deserialize, Debug, PartialEq)]
    struct Target {
        value: i32,
    }

    let bytes = to_bytes(&BTreeMap::<String, i32>::new()).unwrap();
    let error = from_bytes::<Target>(&bytes).unwrap_err();
    assert!(matches!(error, nanonbt::Error::Message(_)), "{error:?}");
}

#[test]
fn oversized_unsigned_values_are_rejected() {
    assert!(to_bytes(&u32::MAX).is_err());

    let bytes = to_bytes(&(-1i32)).unwrap();
    assert!(from_bytes::<u32>(&bytes).is_err());
}

#[test]
fn enums_are_rejected() {
    #[derive(Serialize, Deserialize)]
    enum Kind {
        A,
    }

    let error = to_bytes(&Kind::A).unwrap_err();
    assert!(matches!(error, nanonbt::Error::InvalidData(_)), "{error:?}");
}
