#![allow(clippy::items_after_statements)]
//! `from_bytes` accepts what fastnbt accepts, and yields the same values.

use std::{borrow::Cow, collections::BTreeMap, fmt::Debug};

use nanonbt::{FromNBT, ToNBT};
use serde::{Deserialize, Serialize};

#[track_caller]
fn assert_same_value<'de, T>(bytes: &'de [u8]) -> Option<T>
where
    T: Deserialize<'de> + FromNBT<'de> + PartialEq + Debug,
{
    let expected = fastnbt::from_bytes::<T>(bytes).ok();
    let actual = nanonbt::from_bytes::<T>(bytes).ok();
    assert_eq!(actual, expected, "input {bytes:02x?}");
    actual
}

/// Compares only whether the two crates accept the input, for targets each
/// crate has its own type for.
#[track_caller]
fn assert_same_outcome<F, N>(bytes: &[u8]) -> bool
where
    F: for<'de> Deserialize<'de>,
    N: for<'de> FromNBT<'de>,
{
    let expected = fastnbt::from_bytes::<F>(bytes).is_ok();
    let actual = nanonbt::from_bytes::<N>(bytes).is_ok();
    assert_eq!(actual, expected, "input {bytes:02x?}");
    actual
}

#[test]
fn byte_field_is_read_from_a_root_compound() {
    #[derive(Deserialize, FromNBT, PartialEq, Debug)]
    struct One {
        a: i8,
    }

    let bytes = [0x0a, 0x00, 0x00, 0x01, 0x00, 0x01, b'a', 0xfe, 0x00];
    assert_eq!(assert_same_value::<One>(&bytes), Some(One { a: -2 }));
}

fn encode<T: Serialize>(value: &T) -> Vec<u8> {
    fastnbt::to_bytes(value).unwrap()
}

#[derive(Serialize, Deserialize, ToNBT, FromNBT, PartialEq, Debug)]
struct Scalars {
    byte: i8,
    short: i16,
    int: i32,
    long: i64,
    float: f32,
    double: f64,
    text: String,
}

#[derive(Deserialize, FromNBT, PartialEq, Debug)]
struct Borrowed<'a> {
    text: &'a str,
    #[serde(borrow)]
    cow: Cow<'a, str>,
}

#[test]
fn scalars_and_strings_round_trip() {
    let value = Scalars {
        byte: -1,
        short: -300,
        int: 70_000,
        long: i64::MIN,
        float: 1.5,
        double: -0.25,
        text: "a\0\u{10401}日".into(),
    };
    let bytes = encode(&value);
    assert_eq!(assert_same_value::<Scalars>(&bytes), Some(value));
}

#[test]
fn plain_utf8_strings_are_borrowed_and_modified_ones_are_not() {
    #[derive(Serialize)]
    struct Source {
        text: &'static str,
        cow: &'static str,
    }

    let plain = encode(&Source {
        text: "plain",
        cow: "日本",
    });
    assert!(assert_same_value::<Borrowed>(&plain).is_some());

    // The modified string still reads into owned strings.
    let modified = encode(&Source {
        text: "x\0",
        cow: "\u{1f600}",
    });
    assert_same_value::<Borrowed>(&modified);
    assert_same_value::<Scalars>(&modified);
}

#[test]
fn compounds_and_lists_nest() {
    #[derive(Serialize, Deserialize, ToNBT, FromNBT, PartialEq, Debug)]
    struct Point {
        x: i32,
    }

    #[derive(Serialize, Deserialize, ToNBT, FromNBT, PartialEq, Debug)]
    struct Nested {
        point: Point,
        map: BTreeMap<String, i16>,
        points: Vec<Point>,
        lists: Vec<Vec<i8>>,
        empty: Vec<i64>,
        strings: Vec<String>,
        array: [i16; 2],
    }

    let value = Nested {
        point: Point { x: 1 },
        map: [("a".into(), 1), ("b".into(), 2)].into(),
        points: vec![Point { x: 2 }, Point { x: 3 }],
        lists: vec![vec![1], vec![], vec![2, 3]],
        empty: vec![],
        strings: vec!["x".into(), "日".into()],
        array: [4, 5],
    };
    let bytes = encode(&value);
    assert_eq!(assert_same_value::<Nested>(&bytes), Some(value));
    assert_same_value::<BTreeMap<String, Point>>(&encode(&BTreeMap::from([("p", Point { x: 9 })])));
}

#[test]
fn list_headers_are_checked_like_fastnbt() {
    #[derive(Deserialize, FromNBT, PartialEq, Debug)]
    struct Holder<T> {
        l: T,
    }

    // A list of End with length zero is an empty list; with elements it is not.
    let list = |element: u8, len: i32, payload: &[u8]| {
        let mut bytes = vec![0x0a, 0x00, 0x00, 0x09, 0x00, 0x01, b'l', element];
        bytes.extend_from_slice(&len.to_be_bytes());
        bytes.extend_from_slice(payload);
        bytes.push(0x00);
        bytes
    };
    assert_same_value::<Holder<Vec<i8>>>(&list(0, 0, &[]));
    assert_same_value::<Holder<Vec<i8>>>(&list(0, 1, &[]));
    assert_same_value::<Holder<Vec<i8>>>(&list(1, -1, &[]));
    assert_same_value::<Holder<Vec<i8>>>(&list(1, 2, &[7]));
    assert_same_value::<Holder<Vec<i8>>>(&list(1, 100_000_001, &[]));
    assert_same_value::<Holder<Vec<i8>>>(&list(13, 0, &[]));
    assert_same_value::<Holder<[i8; 2]>>(&list(1, 3, &[1, 2, 3]));
    assert_same_value::<Holder<[i8; 2]>>(&list(1, 1, &[1]));
}

#[test]
fn arrays_are_interchangeable_with_fastnbt() {
    #[derive(Serialize)]
    struct Source {
        bytes: fastnbt::ByteArray,
        ints: fastnbt::IntArray,
        longs: fastnbt::LongArray,
    }

    #[derive(Deserialize, PartialEq, Debug)]
    struct Fast {
        bytes: fastnbt::ByteArray,
        ints: fastnbt::IntArray,
        longs: fastnbt::LongArray,
    }

    #[derive(FromNBT, PartialEq, Debug)]
    struct Nano {
        #[nbt(array = "byte")]
        bytes: Vec<i8>,
        #[nbt(array = "int")]
        ints: Vec<i32>,
        #[nbt(array = "long")]
        longs: Vec<i64>,
    }

    #[derive(Deserialize, FromNBT, PartialEq, Debug)]
    struct AsSeq {
        bytes: Vec<i8>,
    }

    let bytes = encode(&Source {
        bytes: fastnbt::ByteArray::new(vec![1, -1]),
        ints: fastnbt::IntArray::new(vec![i32::MIN, 2]),
        longs: fastnbt::LongArray::new(vec![]),
    });
    assert!(fastnbt::from_bytes::<Fast>(&bytes).is_ok());
    assert_same_value::<AsSeq>(&bytes);
    assert_eq!(
        nanonbt::from_bytes::<Nano>(&bytes).unwrap(),
        Nano {
            bytes: vec![1, -1],
            ints: vec![i32::MIN, 2],
            longs: vec![],
        }
    );
}

#[test]
fn array_lengths_are_checked_like_fastnbt() {
    #[derive(Deserialize)]
    struct Fast<T> {
        #[allow(dead_code)]
        a: T,
    }

    #[derive(FromNBT)]
    struct NanoBytes {
        #[allow(dead_code)]
        #[nbt(array = "byte")]
        a: Vec<i8>,
    }

    #[derive(FromNBT)]
    struct NanoInts {
        #[allow(dead_code)]
        #[nbt(array = "int")]
        a: Vec<i32>,
    }

    #[derive(FromNBT)]
    struct NanoLongs {
        #[allow(dead_code)]
        #[nbt(array = "long")]
        a: Vec<i64>,
    }

    let array = |tag: u8, len: i32, payload: &[u8]| {
        let mut bytes = vec![0x0a, 0x00, 0x00, tag, 0x00, 0x01, b'a'];
        bytes.extend_from_slice(&len.to_be_bytes());
        bytes.extend_from_slice(payload);
        bytes.push(0x00);
        bytes
    };
    for tag in [7, 11, 12] {
        assert_same_outcome::<Fast<fastnbt::ByteArray>, NanoBytes>(&array(tag, -1, &[]));
        assert_same_outcome::<Fast<fastnbt::IntArray>, NanoInts>(&array(tag, 1, &[0; 3]));
        assert_same_outcome::<Fast<fastnbt::LongArray>, NanoLongs>(&array(tag, 1, &[0; 8]));
        assert_same_outcome::<Fast<fastnbt::IntArray>, NanoInts>(&array(tag, 100_000_001, &[]));
    }
}

/// NBT arrays have no serde spelling of their own, so their payload reads as
/// raw bytes, the length already stripped.
#[cfg(feature = "serde")]
#[test]
fn serde_reads_arrays_as_raw_bytes() {
    #[derive(Serialize)]
    struct Source {
        bytes: fastnbt::ByteArray,
        ints: fastnbt::IntArray,
        longs: fastnbt::LongArray,
    }

    #[derive(Deserialize, PartialEq, Debug)]
    struct Raw {
        bytes: serde_bytes::ByteBuf,
        ints: serde_bytes::ByteBuf,
        longs: serde_bytes::ByteBuf,
    }

    let bytes = encode(&Source {
        bytes: fastnbt::ByteArray::new(vec![1, -1, i8::MIN]),
        ints: fastnbt::IntArray::new(vec![i32::MIN, 0, i32::MAX]),
        longs: fastnbt::LongArray::new(vec![i64::MIN, -1, i64::MAX]),
    });
    let raw: Raw = nanonbt::serde_compat::from_bytes(&bytes).unwrap();

    assert_eq!(&raw.bytes[..], &[1, 255, 128]);
    let mut expected = Vec::new();
    for value in [i32::MIN, 0, i32::MAX] {
        expected.extend_from_slice(&value.to_be_bytes());
    }
    assert_eq!(&raw.ints[..], &expected[..]);
    expected.clear();
    for value in [i64::MIN, -1, i64::MAX] {
        expected.extend_from_slice(&value.to_be_bytes());
    }
    assert_eq!(&raw.longs[..], &expected[..]);
}

/// The `#[serde(with = ...)]` modules read an array into its elements, where
/// a plain `Vec` field reads a list.
#[cfg(feature = "serde")]
#[test]
fn serde_with_reads_array_fields() {
    #[derive(Serialize)]
    struct Source {
        bytes: fastnbt::ByteArray,
        ints: fastnbt::IntArray,
        longs: fastnbt::LongArray,
        empty: fastnbt::LongArray,
    }

    #[derive(Deserialize, PartialEq, Debug)]
    struct Nano {
        #[serde(with = "nanonbt::serde_compat::byte_array")]
        bytes: Vec<i8>,
        #[serde(with = "nanonbt::serde_compat::int_array")]
        ints: Vec<i32>,
        #[serde(with = "nanonbt::serde_compat::long_array")]
        longs: Vec<i64>,
        #[serde(with = "nanonbt::serde_compat::long_array")]
        empty: Vec<i64>,
    }

    /// The unsigned spelling reads the same bits.
    #[derive(Deserialize, PartialEq, Debug)]
    struct NanoUnsigned {
        #[serde(with = "nanonbt::serde_compat::byte_array")]
        bytes: Vec<u8>,
        #[serde(with = "nanonbt::serde_compat::int_array")]
        ints: Vec<u32>,
        #[serde(with = "nanonbt::serde_compat::long_array")]
        longs: Vec<u64>,
        #[serde(with = "nanonbt::serde_compat::long_array")]
        empty: Vec<u64>,
    }

    let bytes = encode(&Source {
        bytes: fastnbt::ByteArray::new(vec![1, -1, i8::MIN]),
        ints: fastnbt::IntArray::new(vec![i32::MIN, 0, i32::MAX]),
        longs: fastnbt::LongArray::new(vec![i64::MIN, -1, i64::MAX]),
        empty: fastnbt::LongArray::new(vec![]),
    });
    assert_eq!(
        nanonbt::serde_compat::from_bytes::<Nano>(&bytes).unwrap(),
        Nano {
            bytes: vec![1, -1, i8::MIN],
            ints: vec![i32::MIN, 0, i32::MAX],
            longs: vec![i64::MIN, -1, i64::MAX],
            empty: vec![],
        }
    );
    assert_eq!(
        nanonbt::serde_compat::from_bytes::<NanoUnsigned>(&bytes).unwrap(),
        NanoUnsigned {
            bytes: vec![1, 0xff, 0x80],
            ints: vec![0x8000_0000, 0, 0x7fff_ffff],
            longs: vec![0x8000_0000_0000_0000, u64::MAX, 0x7fff_ffff_ffff_ffff],
            empty: vec![],
        }
    );
}

/// A `#[serde(with = ...)]` field takes the array tag only, refusing another
/// kind of array or a list, as a derived `#[nbt(array = ...)]` field does.
#[cfg(feature = "serde")]
#[test]
fn serde_with_refuses_other_tags() {
    #[derive(Serialize)]
    struct Longs {
        data: fastnbt::LongArray,
    }

    #[derive(Serialize)]
    struct List {
        data: Vec<i32>,
    }

    #[derive(Deserialize)]
    struct AsInts {
        #[allow(dead_code)]
        #[serde(with = "nanonbt::serde_compat::int_array")]
        data: Vec<i32>,
    }

    let longs = encode(&Longs {
        data: fastnbt::LongArray::new(vec![1]),
    });
    assert!(nanonbt::serde_compat::from_bytes::<AsInts>(&longs).is_err());

    let list = encode(&List { data: vec![1] });
    assert!(nanonbt::serde_compat::from_bytes::<AsInts>(&list).is_err());
}

/// Arrays inside a list of compounds — the chunk benchmark's shape — read
/// and write the same bytes through the `#[serde(with = ...)]` modules.
#[cfg(feature = "serde")]
#[test]
fn serde_with_round_trips_nested_array_fields() {
    #[derive(Serialize)]
    struct Inner {
        light: fastnbt::ByteArray,
        data: fastnbt::LongArray,
    }

    #[derive(Serialize)]
    struct Outer {
        sections: Vec<Inner>,
    }

    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct InnerNano {
        #[serde(with = "nanonbt::serde_compat::byte_array")]
        light: Vec<i8>,
        #[serde(with = "nanonbt::serde_compat::long_array")]
        data: Vec<i64>,
    }

    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct OuterNano {
        sections: Vec<InnerNano>,
    }

    let bytes = encode(&Outer {
        sections: vec![
            Inner {
                light: fastnbt::ByteArray::new(vec![0, -1]),
                data: fastnbt::LongArray::new(vec![1, -2]),
            },
            Inner {
                light: fastnbt::ByteArray::new(vec![]),
                data: fastnbt::LongArray::new(vec![i64::MIN]),
            },
        ],
    });
    let parsed: OuterNano = nanonbt::serde_compat::from_bytes(&bytes).unwrap();
    assert_eq!(
        parsed,
        OuterNano {
            sections: vec![
                InnerNano {
                    light: vec![0, -1],
                    data: vec![1, -2],
                },
                InnerNano {
                    light: vec![],
                    data: vec![i64::MIN],
                },
            ],
        }
    );
    assert_eq!(nanonbt::serde_compat::to_bytes(&parsed).unwrap(), bytes);
}

#[test]
fn wide_integers_follow_fastnbt() {
    #[derive(Serialize)]
    struct Source {
        uuid: u128,
        ints: fastnbt::IntArray,
    }

    #[derive(Deserialize, FromNBT, PartialEq, Debug)]
    struct Wide {
        uuid: i128,
        ints: u128,
    }

    let bytes = encode(&Source {
        uuid: 0x0123_4567_89ab_cdef_fedc_ba98_7654_3210,
        ints: fastnbt::IntArray::new(vec![5, 6, 7, 8]),
    });
    assert_eq!(
        assert_same_value::<Wide>(&bytes),
        Some(Wide {
            uuid: 0x0123_4567_89ab_cdef_fedc_ba98_7654_3210,
            ints: (u128::from(5u32) << 96)
                | (u128::from(6u32) << 64)
                | (u128::from(7u32) << 32)
                | u128::from(8u32),
        })
    );

    // An int array of the wrong length is refused by both.
    let short = encode(&Source {
        uuid: 1,
        ints: fastnbt::IntArray::new(vec![1, 2, 3]),
    });
    assert_same_value::<Wide>(&short);
}

#[test]
fn unknown_fields_are_skipped_like_fastnbt() {
    #[derive(Serialize)]
    struct Source {
        keep: i8,
        list: Vec<Vec<String>>,
        compound: Scalars,
        array: fastnbt::LongArray,
        text: &'static str,
        last: i8,
    }

    #[derive(Deserialize, FromNBT, PartialEq, Debug)]
    struct Sparse {
        keep: i8,
        last: i8,
    }

    let bytes = encode(&Source {
        keep: 1,
        list: vec![vec!["a".into()], vec![]],
        compound: Scalars {
            byte: 0,
            short: 0,
            int: 0,
            long: 0,
            float: 0.0,
            double: 0.0,
            text: String::new(),
        },
        array: fastnbt::LongArray::new(vec![1, 2]),
        text: "t",
        last: 2,
    });
    assert_eq!(
        assert_same_value::<Sparse>(&bytes),
        Some(Sparse { keep: 1, last: 2 })
    );
}

/// fastnbt panics skipping a list of End with elements; this refuses it.
#[test]
fn skipping_a_list_of_end_with_elements_is_refused() {
    #[derive(Deserialize, FromNBT, Debug)]
    struct Empty {}

    let skipped = |len: i32| {
        let mut bytes = vec![0x0a, 0x00, 0x00, 0x09, 0x00, 0x01, b'l', 0x09];
        bytes.extend_from_slice(&1i32.to_be_bytes());
        bytes.push(0x00);
        bytes.extend_from_slice(&len.to_be_bytes());
        bytes.push(0x00);
        bytes
    };
    // Pinned, not just `is_err`: a miscounted skip, a changed depth count or
    // a rejected tag would satisfy `is_err` without this being what refused.
    assert_eq!(
        nanonbt::from_bytes::<Empty>(&skipped(1))
            .unwrap_err()
            .to_string(),
        "unexpected list of type 'end', which is not supported"
    );
    // The divergence only exists while fastnbt still panics.
    let fast = std::panic::catch_unwind(|| fastnbt::from_bytes::<Empty>(&skipped(1)));
    assert!(fast.is_err(), "fastnbt no longer panics: {:?}", fast.ok());
    assert!(fastnbt::from_bytes::<Empty>(&skipped(0)).is_ok());
    assert!(nanonbt::from_bytes::<Empty>(&skipped(0)).is_ok());
    // A negative length skips nothing in fastnbt.
    assert!(fastnbt::from_bytes::<Empty>(&skipped(-1)).is_ok());
    assert!(nanonbt::from_bytes::<Empty>(&skipped(-1)).is_ok());
}

#[test]
fn unit_enums_and_newtypes_follow_fastnbt() {
    #[derive(Serialize)]
    struct Source<T> {
        kind: T,
    }

    #[derive(Serialize, Deserialize, ToNBT, FromNBT, PartialEq, Debug)]
    #[serde(rename_all = "snake_case")]
    enum Status {
        #[serde(rename = "empty")]
        #[nbt(rename = "empty")]
        Empty,
        #[serde(rename = "full")]
        #[nbt(rename = "full")]
        Full,
    }

    #[derive(Deserialize, FromNBT, PartialEq, Debug)]
    struct Target {
        kind: Status,
    }

    #[derive(Deserialize, FromNBT, PartialEq, Debug)]
    struct Newtype(i32);

    #[derive(Deserialize, FromNBT, PartialEq, Debug)]
    struct NewtypeTarget {
        kind: Newtype,
    }

    for bytes in [
        encode(&Source { kind: "full" }),
        encode(&Source { kind: "unknown" }),
        encode(&Source { kind: 3 }),
    ] {
        assert_same_value::<Target>(&bytes);
        assert_same_value::<NewtypeTarget>(&bytes);
    }
    assert_eq!(
        assert_same_value::<Target>(&encode(&Source { kind: "empty" })),
        Some(Target {
            kind: Status::Empty
        })
    );
    assert_eq!(
        assert_same_value::<NewtypeTarget>(&encode(&Source { kind: 3i32 })),
        Some(NewtypeTarget { kind: Newtype(3) })
    );
}

#[test]
fn only_a_root_compound_is_accepted() {
    #[derive(Deserialize, FromNBT, PartialEq, Debug)]
    struct Empty {}

    for bytes in [
        &[][..],
        &[0x0a],
        &[0x0a, 0x00],
        &[0x0a, 0x00, 0x00],
        &[0x0a, 0x00, 0x01, b'x', 0x00],
        &[0x08, 0x00, 0x00, 0x00, 0x00],
        &[0x00],
        &[0x1f, 0x8b, 0x08, 0x00],
        &[0x0a, 0x00, 0x00, 0x00, 0xff],
        &[0x0a, 0x00, 0x02, 0xc0, 0x81, 0x00],
    ] {
        assert_same_value::<Empty>(bytes);
        assert_same_value::<BTreeMap<String, i8>>(bytes);
    }
}

/// fastnbt's serde visitors convert between integer tags; this reads its own
/// tag only, so a value is read back as exactly what it was written as.
#[test]
fn integers_require_their_own_tag() {
    #[derive(Serialize)]
    struct Source {
        a: i8,
        b: i16,
        c: i32,
        d: i64,
    }

    #[derive(Deserialize, FromNBT, PartialEq, Debug)]
    struct Same {
        a: i8,
        b: i16,
        c: i32,
        d: i64,
    }

    #[derive(Deserialize, FromNBT, PartialEq, Debug)]
    struct Unsigned {
        a: u8,
        b: u16,
        c: u32,
        d: u64,
    }

    #[derive(Deserialize, FromNBT, PartialEq, Debug)]
    struct Widened {
        a: i64,
        b: i32,
        c: i16,
        d: i8,
    }

    let source = Source {
        a: -1,
        b: -2,
        c: -3,
        d: -4,
    };
    let bytes = encode(&source);
    assert_eq!(
        assert_same_value::<Same>(&bytes),
        Some(Same {
            a: -1,
            b: -2,
            c: -3,
            d: -4
        })
    );
    // The same bytes read as unsigned values of the same width, by bits.
    assert_eq!(
        nanonbt::from_bytes::<Unsigned>(&bytes).unwrap(),
        Unsigned {
            a: u8::MAX,
            b: u16::MAX - 1,
            c: u32::MAX - 2,
            d: u64::MAX - 3,
        }
    );
    // Every field is the wrong width for its tag.
    assert!(nanonbt::from_bytes::<Widened>(&bytes).is_err());
    assert!(fastnbt::from_bytes::<Widened>(&bytes).is_ok());
}

/// `Option` fields are absent, not null, and an absent field is `None`.
#[test]
fn options_read_when_present_and_absent() {
    #[derive(Serialize)]
    struct Source {
        present: i32,
    }

    #[derive(Deserialize, FromNBT, PartialEq, Debug)]
    struct Target {
        present: Option<i32>,
        absent: Option<i32>,
    }

    let bytes = encode(&Source { present: 3 });
    assert_eq!(
        assert_same_value::<Target>(&bytes),
        Some(Target {
            present: Some(3),
            absent: None
        })
    );
}
