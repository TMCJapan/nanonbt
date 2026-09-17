#![allow(clippy::items_after_statements)]
//! `nanonbt::Value` holds what `fastnbt::Value` holds, and converts alike.

mod common;

use std::{borrow::Cow, collections::BTreeMap, fmt::Debug};

use common::{from_fast, show, to_fast};
use nanonbt::{FromNBT, ToNBT, Value};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct Document {
    byte: i8,
    short: i16,
    int: i32,
    long: i64,
    float: f32,
    double: f64,
    text: &'static str,
    bytes: fastnbt::ByteArray,
    ints: fastnbt::IntArray,
    longs: fastnbt::LongArray,
    list: Vec<Vec<i16>>,
    empty: Vec<i8>,
    compounds: Vec<BTreeMap<&'static str, &'static str>>,
    empty_compound: BTreeMap<&'static str, i8>,
}

fn document() -> Vec<u8> {
    fastnbt::to_bytes(&Document {
        byte: -1,
        short: 2,
        int: 3,
        long: 4,
        float: 5.5,
        double: f64::NAN,
        text: "日\0\u{1f600}",
        bytes: fastnbt::ByteArray::new(vec![1, -2]),
        ints: fastnbt::IntArray::new(vec![3]),
        longs: fastnbt::LongArray::new(vec![]),
        list: vec![vec![1, 2], vec![]],
        empty: vec![],
        compounds: vec![BTreeMap::from([("a", "b")]), BTreeMap::new()],
        empty_compound: BTreeMap::new(),
    })
    .unwrap()
}

/// Compares floats by their bits, so that a NaN equals only the same NaN.
///
/// `document()` carries a NaN on purpose, and plain `Debug` prints every NaN
/// the same way, so it cannot see a payload or sign changing.
#[track_caller]
fn assert_same_tree(actual: Option<&Value>, expected: Option<fastnbt::Value>) {
    let expected = expected.map(from_fast);
    assert_eq!(actual.map(show), expected.as_ref().map(show));
}

#[test]
fn documents_read_into_the_same_tree() {
    let bytes = document();
    let expected = fastnbt::from_bytes::<fastnbt::Value>(&bytes).ok();
    assert!(expected.is_some());
    assert_same_tree(nanonbt::from_bytes::<Value>(&bytes).ok().as_ref(), expected);

    for broken in [
        &bytes[..bytes.len() - 1],
        &bytes[..10],
        &[0x0a, 0x00, 0x00, 0x00][..],
    ] {
        assert_same_tree(
            nanonbt::from_bytes::<Value>(broken).ok().as_ref(),
            fastnbt::from_bytes::<fastnbt::Value>(broken).ok(),
        );
    }
}

#[test]
fn trees_write_the_same_document() {
    let bytes = document();
    let tree = nanonbt::from_bytes::<Value>(&bytes).unwrap();
    let written = nanonbt::to_bytes(&tree).unwrap();
    let reread = fastnbt::from_bytes::<fastnbt::Value>(&written).unwrap();
    // Comparing lengths alone would pass for any corruption that keeps the
    // total size, so compare the documents entry by entry as well; the two
    // crates order compounds differently, so that is done on the trees.
    assert_eq!(
        fastnbt::to_bytes(&reread).unwrap().len(),
        bytes.len(),
        "same document, up to compound order"
    );
    let original = fastnbt::from_bytes::<fastnbt::Value>(&bytes).unwrap();
    assert_eq!(
        show(&from_fast(reread.clone())),
        show(&from_fast(original)),
        "same document, entry by entry"
    );
    assert_same_tree(Some(&tree), Some(reread));

    // Single-entry compounds have one order, so the bytes match exactly.
    let single = Value::Compound(BTreeMap::from([(
        "k".into(),
        Value::List(vec![Value::Int(1)]),
    )]));
    assert_eq!(
        nanonbt::to_bytes(&single).ok(),
        fastnbt::to_bytes(&to_fast(single)).ok()
    );
    for not_root in [
        Value::Int(1),
        Value::List(vec![]),
        Value::ByteArray(nanonbt::ByteArray::new(vec![])),
    ] {
        assert_eq!(
            nanonbt::to_bytes(&not_root).ok(),
            fastnbt::to_bytes(&to_fast(not_root)).ok()
        );
    }
}

#[test]
fn writable_values_convert_to_the_same_tree() {
    #[derive(Serialize, ToNBT, PartialEq, Debug)]
    struct Everything {
        flag: bool,
        byte: i8,
        short: i16,
        int: i32,
        long: i64,
        wide: i128,
        wider: u128,
        letter: char,
        text: String,
        list: Vec<i16>,
        nested: Vec<Vec<i8>>,
        map: BTreeMap<String, i8>,
        some: Option<i8>,
    }

    let value = Everything {
        flag: true,
        byte: -1,
        short: -300,
        int: 70_000,
        long: i64::MIN,
        wide: -2,
        wider: 0x0123_4567_89ab_cdef_fedc_ba98_7654_3210,
        letter: '日',
        text: "text".into(),
        list: vec![1, -2],
        nested: vec![vec![1], vec![]],
        map: BTreeMap::from([("a".into(), 1), ("b".into(), -2)]),
        some: Some(4),
    };
    let nano = nanonbt::to_value(&value).unwrap();
    let fast = fastnbt::to_value(&value).unwrap();
    assert_eq!(show(&nano), show(&from_fast(fast)));
}

#[test]
fn array_values_convert_to_the_same_tree() {
    #[derive(ToNBT)]
    struct Nano {
        bytes: nanonbt::ByteArray,
        ints: nanonbt::IntArray,
        longs: nanonbt::LongArray,
        many: Vec<nanonbt::LongArray>,
    }

    #[derive(Serialize)]
    struct Fast {
        bytes: fastnbt::ByteArray,
        ints: fastnbt::IntArray,
        longs: fastnbt::LongArray,
        many: Vec<fastnbt::LongArray>,
    }

    let nano = Nano {
        bytes: nanonbt::ByteArray::new(vec![1, -1]),
        ints: nanonbt::IntArray::new(vec![i32::MIN]),
        longs: nanonbt::LongArray::new(vec![]),
        many: vec![nanonbt::LongArray::new(vec![1, 2])],
    };
    let fast = Fast {
        bytes: fastnbt::ByteArray::new(vec![1, -1]),
        ints: fastnbt::IntArray::new(vec![i32::MIN]),
        longs: fastnbt::LongArray::new(vec![]),
        many: vec![fastnbt::LongArray::new(vec![1, 2])],
    };
    assert_eq!(
        show(&nanonbt::to_value(&nano).unwrap()),
        show(&from_fast(fastnbt::to_value(&fast).unwrap()))
    );
}

/// Compares the value both crates read from the same tree.
#[track_caller]
fn assert_same_from_value<T>(value: &Value) -> Option<T>
where
    T: for<'de> Deserialize<'de> + for<'de> FromNBT<'de> + PartialEq + Debug,
{
    let actual = nanonbt::from_value::<T>(value);
    let fast = to_fast(value.clone());
    let expected = fastnbt::from_value::<T>(&fast);
    assert_eq!(actual.as_ref().ok(), expected.as_ref().ok(), "{value:?}");
    actual.ok()
}

fn compound(entries: Vec<(&str, Value)>) -> Value {
    Value::Compound(
        entries
            .into_iter()
            .map(|(k, v)| (k.to_owned(), v))
            .collect(),
    )
}

#[test]
fn trees_convert_to_the_same_values() {
    #[derive(Deserialize, FromNBT, PartialEq, Debug)]
    struct Typed {
        byte: i8,
        flag: bool,
        letter: char,
        text: String,
        list: Vec<i16>,
        pair: [i16; 2],
        uuid: u128,
        maybe: Option<i8>,
    }

    let typed = compound(vec![
        ("byte", Value::Byte(-1)),
        ("flag", Value::Byte(1)),
        ("letter", Value::Int(0x65e5)),
        ("text", Value::String("日".into())),
        ("list", Value::List(vec![Value::Short(1), Value::Short(-2)])),
        ("pair", Value::List(vec![Value::Short(1), Value::Short(2)])),
        (
            "uuid",
            Value::IntArray(nanonbt::IntArray::new(vec![1, 2, 3, -4])),
        ),
        ("maybe", Value::Byte(7)),
    ]);
    assert_eq!(
        assert_same_from_value::<Typed>(&typed),
        Some(Typed {
            byte: -1,
            flag: true,
            letter: '日',
            text: "日".into(),
            list: vec![1, -2],
            pair: [1, 2],
            uuid: 0x0000_0001_0000_0002_0000_0003_ffff_fffc,
            maybe: Some(7),
        })
    );

    // Round trip through the tree itself.
    for value in [
        Value::Byte(-1),
        Value::Short(-1),
        Value::Int(-1),
        Value::Long(1),
        Value::Float(1.5),
        Value::Double(2.5),
        Value::String("ab".into()),
        Value::List(vec![Value::Byte(1), Value::Byte(2)]),
        Value::ByteArray(nanonbt::ByteArray::new(vec![1])),
        Value::IntArray(nanonbt::IntArray::new(vec![1, 2])),
        compound(vec![("k", Value::Byte(1))]),
    ] {
        assert_eq!(
            nanonbt::from_value::<Value>(&value).unwrap(),
            value,
            "{value:?}"
        );
    }
}

/// Strict tags: a value reads its own tag only, where fastnbt's visitors
/// convert. The divergences are pinned here.
#[test]
fn trees_convert_strictly() {
    // A bool is a byte, not any integer.
    assert!(nanonbt::from_value::<bool>(&Value::Int(1)).is_err());
    assert!(nanonbt::from_value::<bool>(&Value::Byte(1)).is_ok());
    // An i64 is a long, not an int.
    assert!(nanonbt::from_value::<i64>(&Value::Int(1)).is_err());
    assert!(nanonbt::from_value::<i64>(&Value::Long(1)).is_ok());
    // An f64 is a double, not a float.
    assert!(nanonbt::from_value::<f64>(&Value::Float(1.5)).is_err());
    // A byte array is not a list of bytes.
    assert!(
        nanonbt::from_value::<Vec<i8>>(&Value::ByteArray(nanonbt::ByteArray::new(vec![1])))
            .is_err()
    );
}

#[test]
fn from_value_borrows_from_the_tree() {
    #[derive(FromNBT, PartialEq, Debug)]
    struct Borrowed<'a> {
        text: &'a str,
        cow: Cow<'a, str>,
        owned: String,
    }

    let value = compound(vec![
        ("text", Value::String("plain".into())),
        ("cow", Value::String("cow".into())),
        ("owned", Value::String("owned".into())),
    ]);
    let back = nanonbt::from_value::<Borrowed<'_>>(&value).unwrap();
    assert_eq!(back.text, "plain");
    assert!(matches!(back.cow, Cow::Borrowed("cow")));
    assert_eq!(back.owned, "owned");
}

#[test]
fn from_value_reads_arrays_directly() {
    assert_eq!(
        nanonbt::from_value::<nanonbt::ByteArray>(&Value::ByteArray(nanonbt::ByteArray::new(
            vec![1, -1]
        )))
        .unwrap(),
        nanonbt::ByteArray::new(vec![1, -1])
    );
    assert_eq!(
        nanonbt::from_value::<nanonbt::IntArray>(&Value::IntArray(nanonbt::IntArray::new(vec![
            i32::MIN
        ])))
        .unwrap(),
        nanonbt::IntArray::new(vec![i32::MIN])
    );
    assert_eq!(
        nanonbt::from_value::<nanonbt::LongArray>(&Value::LongArray(nanonbt::LongArray::new(
            vec![i64::MAX]
        )))
        .unwrap(),
        nanonbt::LongArray::new(vec![i64::MAX])
    );
}

#[test]
fn to_value_writes_unit_enums_as_strings() {
    #[derive(ToNBT)]
    enum Status {
        #[allow(dead_code)]
        Empty,
        #[nbt(rename = "full")]
        Full,
    }

    #[derive(ToNBT)]
    struct Holder {
        status: Status,
    }

    assert_eq!(
        nanonbt::to_value(&Holder {
            status: Status::Full
        })
        .unwrap(),
        compound(vec![("status", Value::String("full".into()))])
    );
}
