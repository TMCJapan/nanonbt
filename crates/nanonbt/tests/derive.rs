#![allow(clippy::items_after_statements)]
//! The derive macros and the trait implementations behind them.

use std::{borrow::Cow, collections::BTreeMap, vec, vec::Vec};

use nanonbt::{Cesu8, FromNBT, I32Be, I64Be, ToNBT, U64Be, Write, Writer, from_bytes, to_bytes};

#[derive(FromNBT, ToNBT, PartialEq, Debug)]
struct Basic {
    byte: i8,
    short: i16,
    int: i32,
    long: i64,
    float: f32,
    double: f64,
    flag: bool,
    letter: char,
    text: String,
}

fn basic() -> Basic {
    Basic {
        byte: -2,
        short: -3,
        int: -4,
        long: -5,
        float: 1.5,
        double: -2.5,
        flag: true,
        letter: 'é',
        text: "héllo".into(),
    }
}

#[test]
fn structs_round_trip() {
    let bytes = to_bytes(&basic()).unwrap();
    assert_eq!(from_bytes::<Basic>(&bytes).unwrap(), basic());
}

#[test]
fn unsigned_and_wide_round_trip() {
    #[derive(FromNBT, ToNBT, PartialEq, Debug)]
    struct Wide {
        a: u8,
        b: u16,
        c: u32,
        d: u64,
        e: i128,
        f: u128,
    }

    let wide = Wide {
        a: u8::MAX,
        b: u16::MAX,
        c: u32::MAX,
        d: u64::MAX,
        e: i128::MIN,
        f: u128::MAX,
    };
    let bytes = to_bytes(&wide).unwrap();
    assert_eq!(from_bytes::<Wide>(&bytes).unwrap(), wide);
}

#[test]
fn collections_round_trip() {
    #[derive(FromNBT, ToNBT, PartialEq, Debug)]
    struct Collections {
        list: Vec<i32>,
        empty: Vec<i64>,
        nested: Vec<Vec<i8>>,
        array: [u16; 3],
        map: BTreeMap<String, i16>,
        empty_map: BTreeMap<String, i16>,
        #[nbt(array = "byte")]
        bytes: Vec<i8>,
        #[nbt(array = "int")]
        ints: Vec<i32>,
        #[nbt(array = "long")]
        longs: Vec<i64>,
    }

    let value = Collections {
        list: vec![1, -2, 3],
        empty: Vec::new(),
        nested: vec![vec![1], vec![]],
        array: [1, 2, 3],
        map: BTreeMap::from([("a".into(), 1), ("b".into(), -2)]),
        empty_map: BTreeMap::new(),
        bytes: vec![-1, 0, 1],
        ints: vec![i32::MIN, 0, i32::MAX],
        longs: vec![i64::MIN, i64::MAX],
    };
    let bytes = to_bytes(&value).unwrap();
    assert_eq!(from_bytes::<Collections>(&bytes).unwrap(), value);
}

#[test]
fn options_are_skipped_and_default_to_none() {
    #[derive(FromNBT, ToNBT, PartialEq, Debug)]
    struct Options {
        absent: Option<i32>,
        present: Option<String>,
    }

    let value = Options {
        absent: None,
        present: Some("x".into()),
    };
    let bytes = to_bytes(&value).unwrap();
    assert_eq!(from_bytes::<Options>(&bytes).unwrap(), value);

    let with_present = Options {
        absent: Some(7),
        present: None,
    };
    let bytes = to_bytes(&with_present).unwrap();
    assert_eq!(from_bytes::<Options>(&bytes).unwrap(), with_present);
}

#[test]
fn ignored_fields_are_not_written_and_read_as_default() {
    #[derive(FromNBT, ToNBT, PartialEq, Debug)]
    struct Ignored {
        kept: i32,
        #[nbt(ignore)]
        cached: u64,
    }

    #[derive(FromNBT, ToNBT, PartialEq, Debug)]
    struct Both {
        kept: i32,
        cached: u64,
    }

    let value = Ignored {
        kept: 1,
        cached: 99,
    };
    let bytes = to_bytes(&value).unwrap();
    assert_eq!(
        from_bytes::<Ignored>(&bytes).unwrap(),
        Ignored { kept: 1, cached: 0 }
    );

    // A cached entry present in the document is skipped.
    let bytes = to_bytes(&Both { kept: 1, cached: 7 }).unwrap();
    assert_eq!(
        from_bytes::<Ignored>(&bytes).unwrap(),
        Ignored { kept: 1, cached: 0 }
    );
}

#[test]
fn renames_are_used_for_fields_and_variants() {
    #[derive(FromNBT, ToNBT, PartialEq, Debug)]
    struct Renamed {
        #[nbt(rename = "Name")]
        name: String,
    }

    #[derive(FromNBT, ToNBT, PartialEq, Debug)]
    enum Kind {
        Unit,
        #[nbt(rename = "other")]
        Renamed,
    }

    #[derive(FromNBT, ToNBT, PartialEq, Debug)]
    struct Holder {
        kind: Kind,
    }

    let bytes = to_bytes(&Renamed { name: "x".into() }).unwrap();
    assert!(bytes.windows(4).any(|w| w == b"Name"));
    assert_eq!(
        from_bytes::<Renamed>(&bytes).unwrap(),
        Renamed { name: "x".into() }
    );

    let bytes = to_bytes(&Holder {
        kind: Kind::Renamed,
    })
    .unwrap();
    assert!(bytes.windows(5).any(|w| w == b"other"));
    assert_eq!(
        from_bytes::<Holder>(&bytes).unwrap(),
        Holder {
            kind: Kind::Renamed
        }
    );
}

#[test]
fn newtypes_are_transparent() {
    #[derive(FromNBT, ToNBT, PartialEq, Debug)]
    struct Meters(i32);

    #[derive(FromNBT, ToNBT, PartialEq, Debug)]
    struct Holder {
        distance: Meters,
    }

    let bytes = to_bytes(&Holder {
        distance: Meters(3),
    })
    .unwrap();
    assert_eq!(
        from_bytes::<Holder>(&bytes).unwrap(),
        Holder {
            distance: Meters(3)
        }
    );
}

#[test]
fn generic_structs_round_trip() {
    #[derive(FromNBT, ToNBT, PartialEq, Debug)]
    struct Holder<T> {
        value: T,
    }

    let value = Holder {
        value: vec![1i32, 2],
    };
    let bytes = to_bytes(&value).unwrap();
    assert_eq!(from_bytes::<Holder<Vec<i32>>>(&bytes).unwrap(), value);
}

#[test]
fn strings_borrow_when_they_can() {
    #[derive(FromNBT, ToNBT, PartialEq, Debug)]
    struct Borrowed<'a> {
        plain: &'a str,
        cow: Cow<'a, str>,
        owned: String,
    }

    let bytes = to_bytes(&Borrowed {
        plain: "plain",
        cow: Cow::Borrowed("cow"),
        owned: "owned".into(),
    })
    .unwrap();
    let back = from_bytes::<Borrowed<'_>>(&bytes).unwrap();
    assert_eq!(back.plain, "plain");
    assert!(matches!(back.cow, Cow::Borrowed("cow")));
    assert_eq!(back.owned, "owned");
}

#[test]
fn modified_utf8_owns_and_cannot_borrow() {
    #[derive(FromNBT, ToNBT, PartialEq, Debug)]
    struct CowOnly<'a> {
        text: Cow<'a, str>,
    }

    let bytes = to_bytes(&CowOnly {
        text: "nul\0and 🦀".into(),
    })
    .unwrap();
    let back = from_bytes::<CowOnly<'_>>(&bytes).unwrap();
    assert!(matches!(back.text, Cow::Owned(_)));
    assert_eq!(back.text, "nul\0and 🦀");

    #[derive(FromNBT, ToNBT, PartialEq, Debug)]
    struct Plain<'a> {
        text: &'a str,
    }
    assert!(from_bytes::<Plain<'_>>(&bytes).is_err());
}

#[test]
fn array_fields_are_written_and_read_as_arrays() {
    #[derive(FromNBT, ToNBT, PartialEq, Debug)]
    struct Arrays {
        #[nbt(array = "byte")]
        bytes: Vec<u8>,
        #[nbt(array = "int")]
        ints: Vec<u32>,
        #[nbt(array = "long")]
        longs: Vec<i64>,
    }

    let value = Arrays {
        bytes: vec![0, 1, u8::MAX],
        ints: vec![0, 1, u32::MAX],
        longs: vec![i64::MIN, i64::MAX],
    };
    let bytes = to_bytes(&value).unwrap();

    // The document the Write array methods write for the same values.
    let mut expected = Vec::new();
    let mut writer = Writer::new(&mut expected);
    let root = Cesu8::new(b"").unwrap();
    let name = |name: &'static [u8]| Cesu8::new(name).unwrap();
    writer.write_tag(nanonbt::TAG_COMPOUND).unwrap();
    writer.write_name(root).unwrap();
    writer.write_tag(nanonbt::TAG_BYTE_ARRAY).unwrap();
    writer.write_name(name(b"bytes")).unwrap();
    writer.write_byte_array([0_i8, 1, -1]).unwrap();
    writer.write_tag(nanonbt::TAG_INT_ARRAY).unwrap();
    writer.write_name(name(b"ints")).unwrap();
    writer
        .write_int_array([I32Be::new(0), I32Be::new(1), I32Be::new(-1)])
        .unwrap();
    writer.write_tag(nanonbt::TAG_LONG_ARRAY).unwrap();
    writer.write_name(name(b"longs")).unwrap();
    writer
        .write_long_array([I64Be::new(i64::MIN), I64Be::new(i64::MAX)])
        .unwrap();
    writer.write_end().unwrap();
    assert_eq!(bytes, expected);

    assert_eq!(from_bytes::<Arrays>(&bytes).unwrap(), value);
}

#[test]
fn array_fields_take_fixed_arrays_options_and_borrows() {
    #[derive(FromNBT, ToNBT, PartialEq, Debug)]
    struct Arrays<'a> {
        #[nbt(array = "long")]
        fixed: [u64; 2],
        #[nbt(array = "int")]
        signed: Vec<i32>,
        #[nbt(array = "long")]
        absent: Option<Vec<i64>>,
        #[nbt(array = "byte")]
        present: Option<Vec<u8>>,
        #[nbt(array = "byte")]
        borrowed: &'a [i8],
        #[nbt(array = "int")]
        big: Vec<I32Be>,
        #[nbt(array = "long")]
        unsigned_big: [U64Be; 2],
    }

    let value = Arrays {
        fixed: [1, u64::MAX],
        signed: vec![i32::MIN, i32::MAX],
        absent: None,
        present: Some(vec![5, 6]),
        borrowed: &[-1, 0],
        big: vec![I32Be::new(-1), I32Be::new(2)],
        unsigned_big: [U64Be::new(1), U64Be::new(u64::MAX)],
    };
    let bytes = to_bytes(&value).unwrap();
    assert_eq!(from_bytes::<Arrays<'_>>(&bytes).unwrap(), value);

    // The absent entry is left out, not written empty.
    assert!(!bytes.windows(6).any(|w| w == b"absent"));
}

#[test]
fn array_fields_are_strict() {
    #[derive(FromNBT, ToNBT, PartialEq, Debug)]
    struct Longs {
        #[nbt(array = "long")]
        data: Vec<i64>,
    }

    #[allow(dead_code)] // compared through the parse outcome
    #[derive(FromNBT, Debug)]
    struct Ints {
        #[nbt(array = "int")]
        data: Vec<i32>,
    }

    // A long array does not read as an int array, nor the other way.
    let bytes = to_bytes(&Longs { data: vec![1, 2] }).unwrap();
    assert_eq!(
        from_bytes::<Ints>(&bytes).unwrap_err().to_string(),
        "invalid nbt tag value: 12"
    );

    // A fixed array checks the length.
    #[allow(dead_code)] // compared through the parse outcome
    #[derive(FromNBT, Debug)]
    struct Three {
        #[nbt(array = "long")]
        data: [i64; 3],
    }
    assert_eq!(
        from_bytes::<Three>(&bytes).unwrap_err().to_string(),
        "sequence has a different length than the target array"
    );
}

#[test]
fn missing_fields_error_and_unknown_fields_skip() {
    #[derive(FromNBT, ToNBT, PartialEq, Debug)]
    struct Sparse {
        keep: i8,
        last: i8,
    }

    let mut bytes = to_bytes(&Sparse { keep: 1, last: 2 }).unwrap();
    // Drop the End tag and append an unknown entry, then close again.
    bytes.pop();
    bytes.extend_from_slice(&[nanonbt::TAG_STRING, 0, 5, b'o', b't', b'h', b'e', b'r']);
    bytes.extend_from_slice(&3u16.to_be_bytes());
    bytes.extend_from_slice(b"abc");
    bytes.push(nanonbt::TAG_END);
    assert_eq!(
        from_bytes::<Sparse>(&bytes).unwrap(),
        Sparse { keep: 1, last: 2 }
    );

    // A name that is not valid modified UTF-8 names no field, so it is
    // passed over like any other unknown entry instead of being refused:
    // the dispatch compares the raw bytes and never decodes them.
    let mut bytes = to_bytes(&Sparse { keep: 1, last: 2 }).unwrap();
    bytes.pop();
    bytes.extend_from_slice(&[nanonbt::TAG_INT, 0, 2, 0xc0, 0x81]);
    bytes.extend_from_slice(&7i32.to_be_bytes());
    bytes.push(nanonbt::TAG_END);
    assert_eq!(
        from_bytes::<Sparse>(&bytes).unwrap(),
        Sparse { keep: 1, last: 2 }
    );

    #[allow(dead_code)] // compared through the parse outcome
    #[derive(FromNBT, Debug)]
    struct Needs {
        missing: i8,
    }
    let bytes = to_bytes(&Sparse { keep: 1, last: 2 }).unwrap();
    let error = from_bytes::<Needs>(&bytes).unwrap_err();
    assert_eq!(error.to_string(), "missing field `missing`");
}

#[test]
fn tags_are_strict() {
    // An Int where a Long is expected.
    #[allow(dead_code)] // compared through the parse outcome
    #[derive(FromNBT, Debug)]
    struct Longs {
        value: i64,
    }

    let mut bytes = vec![nanonbt::TAG_COMPOUND, 0, 0];
    bytes.extend_from_slice(&[nanonbt::TAG_INT, 0, 5, b'v', b'a', b'l', b'u', b'e']);
    bytes.extend_from_slice(&1i32.to_be_bytes());
    bytes.push(nanonbt::TAG_END);
    let error = from_bytes::<Longs>(&bytes).unwrap_err();
    assert_eq!(error.to_string(), "invalid nbt tag value: 3");

    // An Int where a bool is expected.
    #[allow(dead_code)] // compared through the parse outcome
    #[derive(FromNBT, Debug)]
    struct Flags {
        value: bool,
    }
    assert!(from_bytes::<Flags>(&bytes).is_err());
}

#[test]
fn fixed_arrays_require_their_length() {
    #[derive(FromNBT, Debug)]
    struct Three {
        #[allow(dead_code)]
        values: [i8; 3],
    }

    #[derive(FromNBT, ToNBT)]
    struct Two {
        values: [i8; 2],
    }

    let bytes = to_bytes(&Two { values: [1, 2] }).unwrap();
    assert!(from_bytes::<Three>(&bytes).is_err());
    assert_eq!(from_bytes::<Two>(&bytes).unwrap().values, [1, 2]);
}

#[test]
fn unit_enums_round_trip_as_strings() {
    #[derive(FromNBT, ToNBT, PartialEq, Debug)]
    enum Status {
        Empty,
        Full,
    }

    #[derive(FromNBT, ToNBT, PartialEq, Debug)]
    struct Holder {
        status: Status,
    }

    let bytes = to_bytes(&Holder {
        status: Status::Full,
    })
    .unwrap();
    assert_eq!(
        from_bytes::<Holder>(&bytes).unwrap(),
        Holder {
            status: Status::Full
        }
    );
    assert!(bytes.windows(4).any(|w| w == b"Full"));
}
