#![allow(clippy::items_after_statements)]
//! `to_bytes` produces the same bytes as fastnbt, or fails where it fails.

use nanonbt::ToNBT;
use serde::Serialize;

#[track_caller]
fn assert_same_bytes<T: Serialize + ToNBT>(value: &T) -> Vec<u8> {
    let expected = fastnbt::to_bytes(value).ok();
    let actual = nanonbt::to_bytes(value).ok();
    assert_eq!(actual, expected);
    actual.unwrap_or_default()
}

#[test]
fn empty_struct_is_an_unnamed_root_compound() {
    #[derive(Serialize, ToNBT)]
    struct Empty {}

    let bytes = assert_same_bytes(&Empty {});
    assert_eq!(bytes, [0x0a, 0x00, 0x00, 0x00]);
}

#[test]
fn byte_field_is_a_named_byte_tag() {
    #[derive(Serialize, ToNBT)]
    struct One {
        a: i8,
    }

    let bytes = assert_same_bytes(&One { a: -2 });
    assert_eq!(
        bytes,
        [0x0a, 0x00, 0x00, 0x01, 0x00, 0x01, b'a', 0xfe, 0x00]
    );
}

#[test]
fn strings_and_names_are_modified_utf8() {
    #[derive(Serialize, ToNBT)]
    struct Text {
        ascii: String,
        #[serde(rename = "名前\0\u{1f600}")]
        #[nbt(rename = "名前\0\u{1f600}")]
        unicode: &'static str,
    }

    assert_same_bytes(&Text {
        ascii: "hello".into(),
        unicode: "a\0\u{10401}日",
    });
}

#[test]
fn nested_structs_and_maps_are_compounds() {
    use std::collections::BTreeMap;

    #[derive(Serialize, ToNBT)]
    struct Empty {}

    #[derive(Serialize, ToNBT)]
    struct Inner {
        n: i32,
        empty: Empty,
    }

    #[derive(Serialize, ToNBT)]
    struct Outer {
        inner: Inner,
        map: BTreeMap<String, i16>,
        empty_map: BTreeMap<String, i16>,
    }

    let root = Outer {
        inner: Inner {
            n: 7,
            empty: Empty {},
        },
        map: [("b".into(), 2), ("a".into(), 1)].into(),
        empty_map: BTreeMap::new(),
    };
    assert_same_bytes(&root);
    assert_same_bytes(&BTreeMap::from([("root map", 1u8)]));
}

#[test]
fn sequences_are_lists() {
    #[derive(Serialize, ToNBT)]
    struct Point {
        x: i32,
    }

    #[derive(Serialize, ToNBT)]
    struct Lists {
        ints: Vec<i32>,
        empty: Vec<i64>,
        points: Vec<Point>,
        nested: Vec<Vec<i8>>,
        strings: [&'static str; 2],
    }

    assert_same_bytes(&Lists {
        ints: vec![1, -1],
        empty: vec![],
        points: vec![Point { x: 1 }, Point { x: 2 }],
        nested: vec![vec![1], vec![], vec![2, 3]],
        strings: ["a", "b"],
    });
}

#[test]
fn slices_and_arrays_are_lists() {
    #[derive(ToNBT)]
    struct VecHolder {
        values: Vec<i32>,
    }

    #[derive(ToNBT)]
    struct SliceHolder<'a> {
        values: &'a [i32],
    }

    #[derive(ToNBT)]
    struct ArrayHolder {
        values: [i32; 3],
    }

    let values = [1i32, -1, 2];
    let expected = nanonbt::to_bytes(&VecHolder {
        values: values.to_vec(),
    })
    .unwrap();
    assert_eq!(
        nanonbt::to_bytes(&SliceHolder { values: &values }).unwrap(),
        expected
    );
    assert_eq!(
        nanonbt::to_bytes(&ArrayHolder { values }).unwrap(),
        expected
    );
}

#[test]
fn options_skip_absent_fields() {
    #[derive(Serialize, ToNBT)]
    struct Fields {
        absent: Option<i32>,
        present: Option<i32>,
    }

    assert_same_bytes(&Fields {
        absent: None,
        present: Some(3),
    });
}

#[test]
fn unit_enums_are_strings() {
    #[derive(Serialize, ToNBT)]
    enum Kind {
        Unit,
        #[serde(rename = "other")]
        #[nbt(rename = "other")]
        Renamed,
    }

    #[derive(Serialize, ToNBT)]
    struct Holder {
        kind: Kind,
    }

    assert_same_bytes(&Holder { kind: Kind::Unit });
    assert_same_bytes(&Holder {
        kind: Kind::Renamed,
    });
}

#[test]
fn non_compound_roots_are_refused() {
    assert!(nanonbt::to_bytes(&1i32).is_err());
    assert!(nanonbt::to_bytes(&vec![1i32]).is_err());
    assert!(fastnbt::to_bytes(&1i32).is_err());
}

#[test]
fn array_fields_are_interchangeable_with_fastnbt() {
    #[derive(Serialize)]
    struct Fast {
        bytes: fastnbt::ByteArray,
        ints: fastnbt::IntArray,
        longs: fastnbt::LongArray,
    }

    #[derive(ToNBT)]
    struct Nano {
        #[nbt(array = "byte")]
        bytes: Vec<i8>,
        #[nbt(array = "int")]
        ints: Vec<i32>,
        #[nbt(array = "long")]
        longs: Vec<i64>,
    }

    let fast = Fast {
        bytes: fastnbt::ByteArray::new(vec![1, -1]),
        ints: fastnbt::IntArray::new(vec![]),
        longs: fastnbt::LongArray::new(vec![i64::MIN]),
    };
    let nano = Nano {
        bytes: vec![1, -1],
        ints: vec![],
        longs: vec![i64::MIN],
    };

    let expected = fastnbt::to_bytes(&fast).unwrap();
    assert_eq!(nanonbt::to_bytes(&nano).unwrap(), expected);
}

/// fastnbt truncates the `u16` length and writes corrupt NBT instead.
#[test]
fn strings_longer_than_a_u16_length_are_refused() {
    #[derive(Serialize, ToNBT)]
    struct Text {
        long: String,
    }

    let fits = Text {
        long: "x".repeat(usize::from(u16::MAX)),
    };
    assert_same_bytes(&fits);

    let too_long = Text {
        long: "x".repeat(usize::from(u16::MAX) + 1),
    };
    // Pinned, not just `is_err`: the refusal reason is the whole divergence,
    // and an incidental failure would satisfy `is_err` just as well.
    assert_eq!(
        nanonbt::to_bytes(&too_long).unwrap_err().to_string(),
        "string longer than 65535 bytes"
    );
    // The divergence only exists while fastnbt still writes the corrupt NBT.
    let corrupt = fastnbt::to_bytes(&too_long).expect("fastnbt no longer truncates");
    assert!(fastnbt::from_bytes::<fastnbt::Value>(&corrupt).is_err());
}

#[test]
fn primitive_fields_map_to_fastnbt_tags() {
    #[derive(Serialize, ToNBT)]
    struct Primitives {
        short: i16,
        int: i32,
        long: i64,
        float: f32,
        double: f64,
        unsigned_byte: u8,
        unsigned_short: u16,
        unsigned_int: u32,
        unsigned_long: u64,
        flag: bool,
        letter: char,
        uuid: u128,
        signed_uuid: i128,
    }

    assert_same_bytes(&Primitives {
        short: -300,
        int: 70_000,
        long: -5_000_000_000,
        float: 1.5,
        double: -0.25,
        unsigned_byte: 200,
        unsigned_short: 60_000,
        unsigned_int: 4_000_000_000,
        unsigned_long: u64::MAX,
        flag: true,
        letter: '日',
        uuid: 0x0123_4567_89ab_cdef_fedc_ba98_7654_3210,
        signed_uuid: -2,
    });
}

/// The new derives write the same document the old serde path did, which
/// `serde_compat` still offers.
#[cfg(feature = "serde")]
#[test]
fn serde_compat_writes_the_same_document() {
    use nanonbt::serde_compat;

    #[derive(Serialize, serde::Deserialize, nanonbt::ToNBT, nanonbt::FromNBT)]
    struct Fields {
        byte: i8,
        text: String,
        list: Vec<i32>,
        absent: Option<i32>,
    }

    let value = Fields {
        byte: -1,
        text: "hi".into(),
        list: vec![1, 2],
        absent: None,
    };
    assert_eq!(
        nanonbt::to_bytes(&value).unwrap(),
        serde_compat::to_bytes(&value).unwrap()
    );
    let bytes = nanonbt::to_bytes(&value).unwrap();
    let through_serde: Fields = serde_compat::from_bytes(&bytes).unwrap();
    assert_eq!(through_serde.byte, value.byte);
    assert_eq!(through_serde.text, value.text);
    assert_eq!(through_serde.list, value.list);
}
