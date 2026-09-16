//! `from_bytes` accepts what fastnbt accepts, and yields the same values.

use std::fmt::Debug;

use serde::{Deserialize, Serialize};

#[track_caller]
fn assert_same_value<'de, T>(bytes: &'de [u8]) -> Option<T>
where
    T: Deserialize<'de> + PartialEq + Debug,
{
    let expected = fastnbt::from_bytes::<T>(bytes).ok();
    let actual = nanonbt::from_bytes::<T>(bytes).ok();
    assert_eq!(actual, expected, "input {bytes:02x?}");
    actual
}

#[test]
fn byte_field_is_read_from_a_root_compound() {
    #[derive(Deserialize, PartialEq, Debug)]
    struct One {
        a: i8,
    }

    let bytes = [0x0a, 0x00, 0x00, 0x01, 0x00, 0x01, b'a', 0xfe, 0x00];
    assert_eq!(assert_same_value::<One>(&bytes), Some(One { a: -2 }));
}

fn encode<T: Serialize>(value: &T) -> Vec<u8> {
    fastnbt::to_bytes(value).unwrap()
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct Scalars {
    byte: i8,
    short: i16,
    int: i32,
    long: i64,
    float: f32,
    double: f64,
    text: String,
}

#[derive(Deserialize, PartialEq, Debug)]
struct Borrowed<'a> {
    text: &'a str,
    #[serde(borrow)]
    cow: std::borrow::Cow<'a, str>,
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

    let modified = encode(&Source {
        text: "x\0",
        cow: "\u{1f600}",
    });
    assert_same_value::<Borrowed>(&modified);
    assert_same_value::<Scalars>(&modified);
}

#[test]
fn compounds_and_lists_nest() {
    use std::collections::BTreeMap;

    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct Point {
        x: i32,
    }

    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct Nested {
        point: Point,
        map: BTreeMap<String, i16>,
        points: Vec<Point>,
        lists: Vec<Vec<i8>>,
        empty: Vec<i64>,
        strings: Vec<String>,
        pair: (i16, i16),
    }

    let value = Nested {
        point: Point { x: 1 },
        map: [("a".into(), 1), ("b".into(), 2)].into(),
        points: vec![Point { x: 2 }, Point { x: 3 }],
        lists: vec![vec![1], vec![], vec![2, 3]],
        empty: vec![],
        strings: vec!["x".into(), "日".into()],
        pair: (4, 5),
    };
    let bytes = encode(&value);
    assert_eq!(assert_same_value::<Nested>(&bytes), Some(value));
    assert_same_value::<BTreeMap<String, Point>>(&encode(&BTreeMap::from([("p", Point { x: 9 })])));
}

#[test]
fn list_headers_are_checked_like_fastnbt() {
    #[derive(Deserialize, PartialEq, Debug)]
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
    assert_same_value::<Holder<(i8, i8)>>(&list(1, 3, &[1, 2, 3]));
    assert_same_value::<Holder<(i8, i8)>>(&list(1, 1, &[1]));
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

    #[derive(Deserialize, PartialEq, Debug)]
    struct Nano {
        bytes: nanonbt::ByteArray,
        ints: nanonbt::IntArray,
        longs: nanonbt::LongArray,
    }

    #[derive(Deserialize, PartialEq, Debug)]
    struct AsSeq {
        bytes: Vec<i8>,
    }

    let bytes = encode(&Source {
        bytes: fastnbt::ByteArray::new(vec![1, -1]),
        ints: fastnbt::IntArray::new(vec![i32::MIN, 2]),
        longs: fastnbt::LongArray::new(vec![]),
    });
    assert_same_value::<Fast>(&bytes).unwrap();
    assert_same_value::<AsSeq>(&bytes);
    assert_eq!(
        nanonbt::from_bytes::<Nano>(&bytes).unwrap(),
        Nano {
            bytes: nanonbt::ByteArray::new(vec![1, -1]),
            ints: nanonbt::IntArray::new(vec![i32::MIN, 2]),
            longs: nanonbt::LongArray::new(vec![]),
        }
    );
}

#[test]
fn array_lengths_are_checked_like_fastnbt() {
    #[derive(Deserialize, PartialEq, Debug)]
    struct Holder<T> {
        a: T,
    }

    let array = |tag: u8, len: i32, payload: &[u8]| {
        let mut bytes = vec![0x0a, 0x00, 0x00, tag, 0x00, 0x01, b'a'];
        bytes.extend_from_slice(&len.to_be_bytes());
        bytes.extend_from_slice(payload);
        bytes.push(0x00);
        bytes
    };
    for tag in [7, 11, 12] {
        assert_same_value::<Holder<fastnbt::ByteArray>>(&array(tag, -1, &[]));
        assert_same_value::<Holder<fastnbt::IntArray>>(&array(tag, 1, &[0; 3]));
        assert_same_value::<Holder<fastnbt::LongArray>>(&array(tag, 1, &[0; 8]));
        assert_same_value::<Holder<fastnbt::IntArray>>(&array(tag, 100_000_001, &[]));
    }
}

#[test]
fn wide_integers_and_bytes_follow_fastnbt() {
    use serde_bytes::ByteBuf;

    #[derive(Serialize)]
    struct Source {
        uuid: u128,
        list: Vec<i16>,
        bytes: fastnbt::ByteArray,
        longs: fastnbt::LongArray,
        text: &'static str,
        ints: fastnbt::IntArray,
    }

    #[derive(Deserialize, PartialEq, Debug)]
    struct Wide {
        uuid: i128,
        list: ByteBuf,
        bytes: ByteBuf,
        longs: ByteBuf,
        text: ByteBuf,
        ints: u128,
    }

    #[derive(Deserialize, PartialEq, Debug)]
    struct Refused {
        uuid: u128,
        list: u128,
        bytes: u128,
        longs: ByteBuf,
        text: ByteBuf,
        ints: ByteBuf,
    }

    #[derive(Deserialize, PartialEq, Debug)]
    struct Borrowed<'a> {
        uuid: &'a [u8],
        list: &'a [u8],
        bytes: &'a [u8],
        longs: &'a [u8],
        text: &'a [u8],
    }

    let bytes = encode(&Source {
        uuid: 0x0123_4567_89ab_cdef_fedc_ba98_7654_3210,
        list: vec![1, -2],
        bytes: fastnbt::ByteArray::new(vec![3]),
        longs: fastnbt::LongArray::new(vec![4]),
        text: "日\0",
        ints: fastnbt::IntArray::new(vec![5, 6]),
    });
    assert!(assert_same_value::<Wide>(&bytes).is_none());
    assert_same_value::<Refused>(&bytes);
    assert_same_value::<Borrowed>(&bytes);

    let ok = encode(&Source {
        uuid: 1,
        list: vec![7],
        bytes: fastnbt::ByteArray::new(vec![]),
        longs: fastnbt::LongArray::new(vec![]),
        text: "",
        ints: fastnbt::IntArray::new(vec![1, 2, 3, 4]),
    });
    assert!(assert_same_value::<Wide>(&ok).is_some());
}

#[test]
fn unknown_and_unit_fields_are_skipped_like_fastnbt() {
    #[derive(Serialize)]
    struct Source {
        keep: i8,
        list: Vec<Vec<String>>,
        compound: Scalars,
        array: fastnbt::LongArray,
        text: &'static str,
        last: i8,
    }

    #[derive(Deserialize, PartialEq, Debug)]
    struct Sparse {
        keep: i8,
        last: i8,
    }

    #[derive(Deserialize, PartialEq, Debug)]
    struct Units {
        list: (),
        compound: (),
        array: (),
        text: (),
        missing: Option<()>,
        keep: Option<i8>,
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
    assert!(assert_same_value::<Units>(&bytes).is_some());
}

/// fastnbt panics skipping a list of End with elements; this refuses it.
#[test]
fn skipping_a_list_of_end_with_elements_is_refused() {
    #[derive(Deserialize, Debug)]
    struct Empty {}

    let skipped = |len: i32| {
        let mut bytes = vec![0x0a, 0x00, 0x00, 0x09, 0x00, 0x01, b'l', 0x09];
        bytes.extend_from_slice(&1i32.to_be_bytes());
        bytes.push(0x00);
        bytes.extend_from_slice(&len.to_be_bytes());
        bytes.push(0x00);
        bytes
    };
    assert!(nanonbt::from_bytes::<Empty>(&skipped(1)).is_err());
    // The divergence only exists while fastnbt still panics.
    let fast = std::panic::catch_unwind(|| fastnbt::from_bytes::<Empty>(&skipped(1)));
    assert!(fast.is_err(), "fastnbt no longer panics: {:?}", fast.ok());
    assert_same_value::<()>(&skipped(0));
    assert!(fastnbt::from_bytes::<Empty>(&skipped(0)).is_ok());
    assert!(nanonbt::from_bytes::<Empty>(&skipped(0)).is_ok());
    // A negative length skips nothing in fastnbt.
    assert!(fastnbt::from_bytes::<Empty>(&skipped(-1)).is_ok());
    assert!(nanonbt::from_bytes::<Empty>(&skipped(-1)).is_ok());
}

#[test]
fn enums_and_newtypes_follow_fastnbt() {
    #[derive(Serialize)]
    struct Source<T> {
        kind: T,
    }

    #[derive(Deserialize, PartialEq, Debug)]
    #[serde(rename_all = "snake_case")]
    enum Status {
        Empty,
        Full,
        Holder(i32),
    }

    #[derive(Deserialize, PartialEq, Debug)]
    struct Target {
        kind: Status,
    }

    #[derive(Deserialize, PartialEq, Debug)]
    struct Newtype(i32);

    #[derive(Deserialize, PartialEq, Debug)]
    struct NewtypeTarget {
        kind: Newtype,
    }

    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    #[serde(tag = "id")]
    enum Entity {
        #[serde(rename = "minecraft:bat")]
        Bat {
            #[serde(rename = "BatFlags")]
            flags: i8,
        },
        #[serde(other)]
        Unknown,
    }

    #[derive(Deserialize, PartialEq, Debug)]
    #[serde(untagged)]
    enum Untagged {
        Int(i32),
        Text(String),
        List(Vec<i8>),
    }

    #[derive(Deserialize, PartialEq, Debug)]
    struct UntaggedTarget {
        kind: Untagged,
    }

    for bytes in [
        encode(&Source { kind: "full" }),
        encode(&Source { kind: "unknown" }),
        encode(&Source { kind: "holder" }),
        encode(&Source { kind: 3 }),
        encode(&Source { kind: vec![1i8] }),
    ] {
        assert_same_value::<Target>(&bytes);
        assert_same_value::<NewtypeTarget>(&bytes);
        assert_same_value::<UntaggedTarget>(&bytes);
    }
    assert!(assert_same_value::<Target>(&encode(&Source { kind: "empty" })).is_some());

    let bat = encode(&Entity::Bat { flags: 3 });
    assert_eq!(
        assert_same_value::<Entity>(&bat),
        Some(Entity::Bat { flags: 3 })
    );
    let other = encode(&Source { kind: 1i8 });
    assert_same_value::<Entity>(&other);
}

#[test]
fn only_a_root_compound_is_accepted() {
    #[derive(Deserialize, PartialEq, Debug)]
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
        assert_same_value::<std::collections::BTreeMap<String, i8>>(bytes);
    }
}

#[test]
fn integers_convert_like_fastnbt() {
    #[derive(Serialize)]
    struct Source {
        a: i8,
        b: i16,
        c: i32,
        d: i64,
    }

    #[derive(Deserialize, PartialEq, Debug)]
    struct Unsigned {
        a: u8,
        b: u16,
        c: u32,
        d: u64,
    }

    #[allow(clippy::struct_excessive_bools)] // integers read as bools
    #[derive(Deserialize, PartialEq, Debug)]
    struct Flags {
        a: bool,
        b: bool,
        c: bool,
        d: bool,
    }

    #[derive(Deserialize, PartialEq, Debug)]
    struct Widened {
        a: i64,
        b: f64,
        c: i16,
        d: i32,
    }

    for source in [
        Source {
            a: 1,
            b: 2,
            c: 3,
            d: 4,
        },
        Source {
            a: -1,
            b: 0,
            c: 0,
            d: 0,
        },
        Source {
            a: 0,
            b: 0,
            c: i32::MAX,
            d: i64::from(i32::MIN) - 1,
        },
    ] {
        let bytes = encode(&source);
        assert_same_value::<Unsigned>(&bytes);
        assert_same_value::<Flags>(&bytes);
        assert_same_value::<Widened>(&bytes);
        assert_same_value::<Scalars>(&bytes);
    }
}
