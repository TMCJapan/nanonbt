//! The read dispatch the `hashify` feature generates.
//!
//! The feature changes what a derived `FromNBT` is made of, so these cover
//! key sets big enough to reach each lookup hashify builds: a decision tree
//! through 16 keys, a flat table through 64, and a minimal perfect hash
//! beyond that. Names are matched as the modified UTF-8 bytes NBT stores,
//! so a name with a NUL or a non-BMP character is a key like any other.

#![cfg(feature = "hashify")]

use nanonbt::{FromNBT, ToNBT, from_bytes, to_bytes};

/// 24 keys: past the 16-key tree, so the flat table dispatches them. The
/// renamed fields put a long key and two outside plain UTF-8 in it as well.
#[derive(FromNBT, ToNBT, PartialEq, Debug)]
struct Flat {
    f00: u8,
    f01: u8,
    f02: u8,
    f03: u8,
    f04: u8,
    f05: u8,
    f06: u8,
    f07: u8,
    f08: u8,
    f09: u8,
    f10: u8,
    f11: u8,
    f12: u8,
    f13: u8,
    f14: u8,
    f15: u8,
    f16: u8,
    f17: u8,
    f18: u8,
    f19: u8,
    f20: u8,
    #[nbt(rename = "a name longer than sixteen bytes")]
    long: u8,
    #[nbt(rename = "nul\0name")]
    nul: u8,
    #[nbt(rename = "🦀")]
    crab: u8,
}

#[test]
fn flat_table_dispatches_fields() {
    let value = Flat {
        f00: 0,
        f01: 1,
        f02: 2,
        f03: 3,
        f04: 4,
        f05: 5,
        f06: 6,
        f07: 7,
        f08: 8,
        f09: 9,
        f10: 10,
        f11: 11,
        f12: 12,
        f13: 13,
        f14: 14,
        f15: 15,
        f16: 16,
        f17: 17,
        f18: 18,
        f19: 19,
        f20: 20,
        long: 21,
        nul: 22,
        crab: 23,
    };
    let mut bytes = to_bytes(&value).unwrap();
    assert_eq!(from_bytes::<Flat>(&bytes).unwrap(), value);

    // An entry the type does not name is skipped, hashified or not.
    bytes.pop();
    bytes.extend_from_slice(&[nanonbt::TAG_INT, 0, 5, b'o', b't', b'h', b'e', b'r']);
    bytes.extend_from_slice(&7i32.to_be_bytes());
    bytes.push(nanonbt::TAG_END);
    assert_eq!(from_bytes::<Flat>(&bytes).unwrap(), value);
}

/// 72 keys: past the 64-key flat table, so the minimal perfect hash
/// dispatches them. The long names go to the lookup's long-key table.
macro_rules! mphf {
    ($($field:ident = $value:expr),* $(,)?) => {
        #[derive(FromNBT, ToNBT, PartialEq, Debug)]
        struct Mphf {
            #[nbt(rename = "a name longer than sixteen bytes")]
            first: u8,
            $($field: u8,)*
            #[nbt(rename = "another name longer than sixteen bytes")]
            last: u8,
        }

        #[test]
        fn minimal_perfect_hash_dispatches_fields() {
            let value = Mphf {
                first: 70,
                $($field: $value,)*
                last: 71,
            };
            let bytes = to_bytes(&value).unwrap();
            assert_eq!(from_bytes::<Mphf>(&bytes).unwrap(), value);
        }
    };
}

mphf!(
    f00 = 0,
    f01 = 1,
    f02 = 2,
    f03 = 3,
    f04 = 4,
    f05 = 5,
    f06 = 6,
    f07 = 7,
    f08 = 8,
    f09 = 9,
    f10 = 10,
    f11 = 11,
    f12 = 12,
    f13 = 13,
    f14 = 14,
    f15 = 15,
    f16 = 16,
    f17 = 17,
    f18 = 18,
    f19 = 19,
    f20 = 20,
    f21 = 21,
    f22 = 22,
    f23 = 23,
    f24 = 24,
    f25 = 25,
    f26 = 26,
    f27 = 27,
    f28 = 28,
    f29 = 29,
    f30 = 30,
    f31 = 31,
    f32 = 32,
    f33 = 33,
    f34 = 34,
    f35 = 35,
    f36 = 36,
    f37 = 37,
    f38 = 38,
    f39 = 39,
    f40 = 40,
    f41 = 41,
    f42 = 42,
    f43 = 43,
    f44 = 44,
    f45 = 45,
    f46 = 46,
    f47 = 47,
    f48 = 48,
    f49 = 49,
    f50 = 50,
    f51 = 51,
    f52 = 52,
    f53 = 53,
    f54 = 54,
    f55 = 55,
    f56 = 56,
    f57 = 57,
    f58 = 58,
    f59 = 59,
    f60 = 60,
    f61 = 61,
    f62 = 62,
    f63 = 63,
    f64 = 64,
    f65 = 65,
    f66 = 66,
    f67 = 67,
    f68 = 68,
    f69 = 69,
);

/// 24 variants, the same keys the flat table holds.
#[derive(FromNBT, ToNBT, PartialEq, Debug)]
enum Many {
    V00,
    V01,
    V02,
    V03,
    V04,
    V05,
    V06,
    V07,
    V08,
    V09,
    V10,
    V11,
    V12,
    V13,
    V14,
    V15,
    V16,
    V17,
    V18,
    V19,
    V20,
    #[nbt(rename = "a variant name longer than sixteen bytes")]
    Long,
    #[nbt(rename = "nul\0variant")]
    Nul,
    #[nbt(rename = "🦀")]
    Crab,
}

#[derive(FromNBT, ToNBT, PartialEq, Debug)]
struct Holder {
    many: Many,
}

#[test]
fn flat_table_dispatches_variants() {
    let variants = [
        Many::V00,
        Many::V01,
        Many::V02,
        Many::V03,
        Many::V04,
        Many::V05,
        Many::V06,
        Many::V07,
        Many::V08,
        Many::V09,
        Many::V10,
        Many::V11,
        Many::V12,
        Many::V13,
        Many::V14,
        Many::V15,
        Many::V16,
        Many::V17,
        Many::V18,
        Many::V19,
        Many::V20,
        Many::Long,
        Many::Nul,
        Many::Crab,
    ];
    for variant in variants {
        let expected = Holder { many: variant };
        let bytes = to_bytes(&expected).unwrap();
        assert_eq!(from_bytes::<Holder>(&bytes).unwrap(), expected);
    }
}

#[derive(ToNBT)]
struct Unknown {
    many: String,
}

/// A struct whose every field is ignored dispatches on no key at all.
#[derive(FromNBT, ToNBT, PartialEq, Debug)]
struct Nothing {
    #[nbt(ignore)]
    cached: u8,
}

#[test]
fn unknown_variants_and_skipped_entries_take_the_default_arm() {
    let bytes = to_bytes(&Unknown {
        many: "missing".into(),
    })
    .unwrap();
    assert!(from_bytes::<Holder>(&bytes).is_err());
    assert_eq!(
        from_bytes::<Nothing>(&bytes).unwrap(),
        Nothing { cached: 0 }
    );
}
