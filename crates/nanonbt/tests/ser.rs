//! `to_bytes` produces the same bytes as fastnbt, or fails where it fails.

use serde::Serialize;

#[track_caller]
fn assert_same_bytes<T: Serialize>(value: &T) -> Vec<u8> {
    let expected = fastnbt::to_bytes(value).ok();
    let actual = nanonbt::to_bytes(value).ok();
    assert_eq!(actual, expected);
    actual.unwrap_or_default()
}

#[test]
fn empty_struct_is_an_unnamed_root_compound() {
    #[derive(Serialize)]
    struct Empty {}

    let bytes = assert_same_bytes(&Empty {});
    assert_eq!(bytes, [0x0a, 0x00, 0x00, 0x00]);
}

#[test]
fn byte_field_is_a_named_byte_tag() {
    #[derive(Serialize)]
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
    #[derive(Serialize)]
    struct Text {
        ascii: String,
        #[serde(rename = "名前\0\u{1f600}")]
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

    #[derive(Serialize)]
    struct Empty {}

    #[derive(Serialize)]
    struct Inner {
        n: i32,
        empty: Empty,
    }

    #[derive(Serialize)]
    struct Outer {
        inner: Inner,
        map: BTreeMap<String, i16>,
        empty_map: BTreeMap<String, i16>,
        char_keys: BTreeMap<char, i8>,
    }

    let root = Outer {
        inner: Inner {
            n: 7,
            empty: Empty {},
        },
        map: [("b".into(), 2), ("a".into(), 1)].into(),
        empty_map: BTreeMap::new(),
        char_keys: [('\0', 1), ('日', 2)].into(),
    };
    assert_same_bytes(&root);
    assert_same_bytes(&BTreeMap::from([("root map", 1u8)]));
}

#[test]
fn sequences_are_lists() {
    #[derive(Serialize)]
    struct Point {
        x: i32,
    }

    #[derive(Serialize)]
    struct Lists {
        ints: Vec<i32>,
        empty: Vec<i64>,
        points: Vec<Point>,
        nested: Vec<Vec<i8>>,
        tuple: (i16, i16),
        strings: [&'static str; 2],
    }

    assert_same_bytes(&Lists {
        ints: vec![1, -1],
        empty: vec![],
        points: vec![Point { x: 1 }, Point { x: 2 }],
        nested: vec![vec![1], vec![], vec![2, 3]],
        tuple: (5, 6),
        strings: ["a", "b"],
    });
}

#[test]
fn options_skip_absent_fields_but_not_list_elements() {
    #[allow(clippy::option_option)] // both layers are serialized
    #[derive(Serialize)]
    struct Fields {
        absent: Option<i32>,
        present: Option<i32>,
        nested: Option<Option<&'static str>>,
    }

    #[derive(Serialize)]
    struct Elements {
        list: Vec<Option<i8>>,
    }

    assert_same_bytes(&Fields {
        absent: None,
        present: Some(3),
        nested: Some(Some("x")),
    });
    assert_same_bytes(&Elements {
        list: vec![Some(1), Some(2)],
    });
    assert_same_bytes(&Elements {
        list: vec![Some(1), None],
    });
    assert_same_bytes(&Elements { list: vec![None] });
}

#[test]
fn enums_follow_fastnbt() {
    #[derive(Serialize)]
    enum Kind {
        Unit,
        Newtype(i32),
        Tuple(i8, i8),
        Struct { a: i8 },
    }

    #[derive(Serialize)]
    struct Holder {
        kind: Kind,
    }

    #[derive(Serialize)]
    #[serde(tag = "id")]
    enum Tagged {
        Creeper { ignited: i8 },
    }

    #[derive(Serialize)]
    #[serde(untagged)]
    enum Untagged {
        Int(i32),
    }

    assert_same_bytes(&Holder { kind: Kind::Unit });
    assert_same_bytes(&Holder {
        kind: Kind::Newtype(1),
    });
    assert_same_bytes(&Holder {
        kind: Kind::Tuple(1, 2),
    });
    assert_same_bytes(&Holder {
        kind: Kind::Struct { a: 1 },
    });
    assert_same_bytes(&Tagged::Creeper { ignited: 1 });
    assert_same_bytes(&std::collections::BTreeMap::from([("u", Untagged::Int(4))]));
    assert_same_bytes(&Kind::Struct { a: 1 });
}

#[test]
fn units_and_bytes_follow_fastnbt() {
    #[derive(Serialize)]
    struct UnitStruct;

    #[derive(Serialize)]
    struct Holder<T> {
        value: T,
    }

    assert_same_bytes(&Holder { value: () });
    assert_same_bytes(&Holder { value: UnitStruct });
    assert_same_bytes(&Holder {
        value: serde_bytes::Bytes::new(&[1, 2, 255]),
    });
    assert_same_bytes(&Holder {
        value: vec![serde_bytes::Bytes::new(&[1]), serde_bytes::Bytes::new(&[])],
    });
    assert_same_bytes(&());
    assert_same_bytes(&1i32);
    assert_same_bytes(&vec![1i32]);
}

#[test]
fn array_types_are_interchangeable_with_fastnbt() {
    #[derive(Serialize)]
    struct Arrays<B, I, L> {
        bytes: B,
        ints: I,
        longs: L,
        many: Vec<L>,
    }

    let fast = Arrays {
        bytes: fastnbt::ByteArray::new(vec![1, -1]),
        ints: fastnbt::IntArray::new(vec![]),
        longs: fastnbt::LongArray::new(vec![i64::MIN]),
        many: vec![fastnbt::LongArray::new(vec![1, 2])],
    };
    let nano = Arrays {
        bytes: nanonbt::ByteArray::new(vec![1, -1]),
        ints: nanonbt::IntArray::new(vec![]),
        longs: nanonbt::LongArray::new(vec![i64::MIN]),
        many: vec![nanonbt::LongArray::new(vec![1, 2])],
    };

    let expected = assert_same_bytes(&fast);
    assert_eq!(nanonbt::to_bytes(&nano).unwrap(), expected);
}

#[test]
fn array_tokens_outside_a_wrapper_follow_fastnbt() {
    use std::collections::BTreeMap;

    #[derive(Serialize)]
    struct Late<'a> {
        a: i8,
        __fastnbt_byte_array: &'a serde_bytes::Bytes,
    }

    #[derive(Serialize)]
    struct Holder<T> {
        value: T,
    }

    let bytes = serde_bytes::Bytes::new(&[1, 2, 3, 4]);
    // At the root, an array cannot stand in for the compound.
    assert_same_bytes(&BTreeMap::from([("__fastnbt_int_array", bytes)]));
    // After another entry, the array has no header of its own.
    assert_same_bytes(&Holder {
        value: Late {
            a: 1,
            __fastnbt_byte_array: bytes,
        },
    });
    // Anything but bytes under a token is refused.
    assert_same_bytes(&Holder {
        value: BTreeMap::from([("__fastnbt_long_array", 1i64)]),
    });
}

/// fastnbt truncates the `u16` length and writes corrupt NBT instead.
#[test]
fn strings_longer_than_a_u16_length_are_refused() {
    #[derive(Serialize)]
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
    assert!(nanonbt::to_bytes(&too_long).is_err());
}

#[test]
fn primitive_fields_map_to_fastnbt_tags() {
    #[derive(Serialize)]
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
