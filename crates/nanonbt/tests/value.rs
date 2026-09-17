//! `nanonbt::Value` holds what `fastnbt::Value` holds, and converts alike.

mod common;

use std::collections::BTreeMap;

use common::{from_fast, show, to_fast};
use nanonbt::Value;
use serde::Serialize;

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

/// A map in any key order, duplicates included.
struct Pairs<K, V>(Vec<(K, V)>);

impl<K: Serialize, V: Serialize> Serialize for Pairs<K, V> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_map(self.0.iter().map(|(k, v)| (k, v)))
    }
}

/// fastnbt panics on some input; nanonbt must refuse exactly those.
#[track_caller]
fn assert_same_to_value<T: Serialize>(value: &T) -> Option<Value> {
    let actual = nanonbt::to_value(value);
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| fastnbt::to_value(value))) {
        Ok(expected) => assert_same_tree(actual.as_ref().ok(), expected.ok()),
        Err(_) => assert!(
            actual.is_err(),
            "fastnbt panicked but nanonbt gave {actual:?}"
        ),
    }
    actual.ok()
}

/// fastnbt panics on `value`, and nanonbt refuses it instead.
///
/// [`assert_same_to_value`] alone cannot pin this: if fastnbt ever stopped
/// panicking, the case would slide into the comparing branch and the
/// refusal this crate documents would stop being tested at all.
#[track_caller]
fn assert_refused_where_fastnbt_panics<T: Serialize>(value: &T) {
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| fastnbt::to_value(value)))
            .is_err(),
        "fastnbt no longer panics"
    );
    assert!(
        nanonbt::to_value(value).is_err(),
        "nanonbt no longer refuses"
    );
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
fn serializable_values_convert_to_the_same_tree() {
    #[derive(Serialize)]
    struct UnitStruct;

    #[derive(Serialize)]
    struct Newtype(i8);

    #[derive(Serialize)]
    enum Kind {
        Unit,
        Newtype(i32),
        Tuple(i8, i8),
        Struct { a: i8 },
    }

    #[derive(Serialize)]
    struct Everything {
        flag: bool,
        byte: u8,
        short: u16,
        int: u32,
        long: u64,
        wide: i128,
        wider: u128,
        letter: char,
        bytes: serde_bytes::ByteBuf,
        newtype: Newtype,
        kind: Kind,
        tuple: Kind,
        structure: Kind,
        array: nanonbt::LongArray,
        fast_array: fastnbt::IntArray,
        some: Option<i8>,
        map: BTreeMap<i32, (i8, String)>,
    }

    let everything = Everything {
        flag: true,
        byte: 200,
        short: 60_000,
        int: 4_000_000_000,
        long: u64::MAX,
        wide: -2,
        wider: 0x0123_4567_89ab_cdef_fedc_ba98_7654_3210,
        letter: '日',
        bytes: serde_bytes::ByteBuf::from(vec![1, 255]),
        newtype: Newtype(3),
        kind: Kind::Unit,
        tuple: Kind::Tuple(1, 2),
        structure: Kind::Struct { a: 1 },
        array: nanonbt::LongArray::new(vec![1, -1]),
        fast_array: fastnbt::IntArray::new(vec![7]),
        some: Some(4),
        map: BTreeMap::from([(-1, (1, "x".into()))]),
    };
    assert!(assert_same_to_value(&everything).is_some());

    // fastnbt panics on these; nanonbt refuses them instead.
    assert_refused_where_fastnbt_panics(&None::<i8>);
    assert_refused_where_fastnbt_panics(&());
    assert_refused_where_fastnbt_panics(&UnitStruct);
    assert_refused_where_fastnbt_panics(&Kind::Newtype(1));
    assert_refused_where_fastnbt_panics(&BTreeMap::from([("__fastnbt_byte_array", 5)]));
    assert_refused_where_fastnbt_panics(&BTreeMap::from([("__fastnbt_int_array", vec!["x"])]));

    // A non-string key: both refuse it, rather than either one panicking.
    assert!(assert_same_to_value(&Pairs(vec![(1.5f32, 1)])).is_none());
    assert!(assert_same_to_value(&Pairs(vec![(true, 1)])).is_none());

    // And these both accept, so the trees themselves are what is compared:
    // a duplicate key, a `char` key, an array token whose payload is a list
    // of the wrong element type, one whose payload is a partial element, and
    // one whose elements need the `as` truncation fastnbt does.
    assert!(assert_same_to_value(&Pairs(vec![("k", 1), ("k", 2)])).is_some());
    assert!(assert_same_to_value(&Pairs(vec![('c', 1), ('d', 2)])).is_some());
    assert!(
        assert_same_to_value(&BTreeMap::from([("__fastnbt_long_array", vec![1.9f32; 8])]))
            .is_some()
    );
    assert!(
        assert_same_to_value(&BTreeMap::from([(
            "__fastnbt_int_array",
            serde_bytes::Bytes::new(&[1, 2, 3, 4, 5]),
        )]))
        .is_some()
    );
    assert!(assert_same_to_value(&BTreeMap::from([("__fastnbt_byte_array", vec![300])])).is_some());
}

/// fastnbt panics on some input; nanonbt must refuse exactly those.
#[track_caller]
fn assert_same_from_value<T>(value: &Value) -> Option<T>
where
    T: for<'de> serde::Deserialize<'de> + PartialEq + std::fmt::Debug,
{
    let actual = nanonbt::from_value::<T>(value);
    let fast = to_fast(value.clone());
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        fastnbt::from_value::<T>(&fast)
    })) {
        Ok(expected) => assert_eq!(actual.as_ref().ok(), expected.as_ref().ok(), "{value:?}"),
        Err(_) => assert!(
            actual.is_err(),
            "fastnbt panicked but nanonbt gave {actual:?}"
        ),
    }
    actual.ok()
}

#[test]
fn trees_convert_to_the_same_values() {
    use serde::Deserialize;
    use serde_bytes::ByteBuf;

    #[derive(Deserialize, PartialEq, Debug)]
    struct Newtype(i32);

    #[derive(Deserialize, PartialEq, Debug)]
    enum Kind {
        Unit,
        Newtype(i32),
        Tuple(i8, i8),
        Struct { a: i8 },
    }

    #[derive(Deserialize, PartialEq, Debug)]
    struct Typed {
        byte: u8,
        flag: bool,
        letter: char,
        text: String,
        list: Vec<i16>,
        pair: (i16, i16),
        newtype: Newtype,
        uuid: u128,
        bytes: ByteBuf,
        array: nanonbt::IntArray,
        fast_array: fastnbt::LongArray,
        maybe: Option<i8>,
        nothing: (),
    }

    let compound = |entries: Vec<(&str, Value)>| {
        Value::Compound(
            entries
                .into_iter()
                .map(|(k, v)| (k.to_owned(), v))
                .collect(),
        )
    };
    let typed = compound(vec![
        ("byte", Value::Byte(-1)),
        ("flag", Value::Long(2)),
        ("letter", Value::Int(0x65e5)),
        ("text", Value::String("日".into())),
        ("list", Value::List(vec![Value::Short(1)])),
        ("pair", Value::List(vec![Value::Short(1), Value::Short(2)])),
        ("newtype", Value::Int(3)),
        (
            "uuid",
            Value::IntArray(nanonbt::IntArray::new(vec![1, 2, 3, -4])),
        ),
        ("bytes", Value::List(vec![Value::Byte(1)])),
        ("array", Value::IntArray(nanonbt::IntArray::new(vec![5]))),
        (
            "fast_array",
            Value::LongArray(nanonbt::LongArray::new(vec![6])),
        ),
        ("maybe", Value::Byte(7)),
        ("nothing", Value::String("anything".into())),
    ]);
    assert!(assert_same_from_value::<Typed>(&typed).is_some());

    for kind in [
        Value::String("Unit".into()),
        Value::String("Newtype".into()),
        compound(vec![("Unit", Value::Int(1))]),
        compound(vec![("Newtype", Value::Int(1))]),
        compound(vec![(
            "Tuple",
            Value::List(vec![Value::Byte(1), Value::Byte(2)]),
        )]),
        compound(vec![("Tuple", Value::List(vec![]))]),
        compound(vec![("Tuple", Value::Int(1))]),
        compound(vec![("Struct", compound(vec![("a", Value::Byte(1))]))]),
        compound(vec![("Struct", Value::Int(1))]),
        compound(vec![]),
        compound(vec![("Unit", Value::Int(1)), ("Newtype", Value::Int(2))]),
        Value::Int(1),
    ] {
        assert_same_from_value::<Kind>(&kind);
    }

    let scalars = [
        Value::Byte(-1),
        Value::Short(-1),
        Value::Int(-1),
        Value::Int(0x11_0000),
        Value::Long(1),
        Value::Float(1.5),
        Value::Double(2.5),
        Value::String(String::new()),
        Value::String("ab".into()),
        Value::List(vec![Value::Byte(1), Value::Byte(2)]),
        Value::List(vec![Value::Int(1)]),
        Value::ByteArray(nanonbt::ByteArray::new(vec![1])),
        Value::IntArray(nanonbt::IntArray::new(vec![1, 2])),
        compound(vec![("k", Value::Byte(1))]),
    ];
    for value in &scalars {
        assert_same_from_value::<i8>(value);
        assert_same_from_value::<u8>(value);
        assert_same_from_value::<u16>(value);
        assert_same_from_value::<i64>(value);
        assert_same_from_value::<f32>(value);
        assert_same_from_value::<f64>(value);
        assert_same_from_value::<bool>(value);
        assert_same_from_value::<char>(value);
        assert_same_from_value::<String>(value);
        assert_same_from_value::<ByteBuf>(value);
        assert_same_from_value::<Vec<i8>>(value);
        assert_same_from_value::<(i8, i8)>(value);
        assert_same_from_value::<i128>(value);
        assert_same_from_value::<Newtype>(value);
        assert_same_from_value::<BTreeMap<String, i8>>(value);
        assert_same_from_value::<BTreeMap<i32, i8>>(value);
        assert_same_from_value::<Option<i8>>(value);
        assert_same_from_value::<()>(value);
    }
    assert_same_from_value::<BTreeMap<i32, i8>>(&compound(vec![("12", Value::Byte(1))]));

    // Round trip through the tree itself.
    for value in scalars {
        let back = nanonbt::from_value::<Value>(&value).map(|v| format!("{v:?}"));
        let fast = fastnbt::from_value::<fastnbt::Value>(&to_fast(value))
            .map(|v| format!("{:?}", from_fast(v)));
        assert_eq!(back.ok(), fast.ok());
    }
}

/// fastnbt's array wrapper never runs out of keys, so a map visitor loops
/// until memory runs out; here it has exactly one entry.
#[test]
fn arrays_convert_to_a_single_entry_map() {
    let array = Value::ByteArray(nanonbt::ByteArray::new(vec![1, 2]));
    let map = nanonbt::from_value::<BTreeMap<String, serde_bytes::ByteBuf>>(&array).unwrap();
    assert_eq!(map.len(), 1);
    assert_eq!(map["__fastnbt_byte_array"].as_slice(), [1, 2]);
}
