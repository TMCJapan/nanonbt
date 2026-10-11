//! The read dispatch the `hashify` feature generates.
//!
//! The feature changes what a derived `DeserializeNbt` is made of, so these
//! cover key sets big enough to reach each lookup hashify builds: a flat
//! table through 64 keys, and a minimal perfect hash beyond that. Smaller
//! key sets go through the decision tree, which the other derive tests
//! carry. Names are matched as the modified UTF-8 bytes NBT stores, so a
//! name with a NUL or a non-BMP character is a key like any other; names
//! longer than sixteen bytes go to the lookup's long-key table.

#![cfg(feature = "hashify")]

use nanonbt::{DeserializeNbt, SerializeNbt, from_bytes, to_bytes};

/// 24 keys: past the 16-key tree, so the flat table dispatches them. The
/// renamed fields put a long key and two outside plain UTF-8 in it as well.
#[derive(SerializeNbt, DeserializeNbt, PartialEq, Debug)]
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
    bytes.extend_from_slice(&[
        nanonbt::Tag::Int.as_u8(),
        0,
        5,
        b'o',
        b't',
        b'h',
        b'e',
        b'r',
    ]);
    bytes.extend_from_slice(&7i32.to_be_bytes());
    bytes.push(nanonbt::Tag::End.as_u8());
    assert_eq!(from_bytes::<Flat>(&bytes).unwrap(), value);
}

/// 72 keys: past the 64-key flat table, so the minimal perfect hash
/// dispatches them. The long names go to the lookup's long-key table.
macro_rules! mphf {
    ($($field:ident = $value:expr),* $(,)?) => {
        #[derive(SerializeNbt, DeserializeNbt, PartialEq, Debug)]
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

#[derive(SerializeNbt)]
struct Unknown {
    many: u8,
}

/// A struct whose every field is skipped dispatches on no key at all.
#[derive(SerializeNbt, DeserializeNbt, PartialEq, Debug)]
struct Nothing {
    #[nbt(skip)]
    cached: u8,
}

#[test]
fn unknown_entries_and_skipped_fields_take_the_default_arm() {
    let bytes = to_bytes(&Unknown { many: 1 }).unwrap();
    assert_eq!(
        from_bytes::<Nothing>(&bytes).unwrap(),
        Nothing { cached: 0 }
    );
}
