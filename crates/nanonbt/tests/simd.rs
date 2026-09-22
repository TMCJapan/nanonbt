#![cfg(feature = "simd")]
// The sample values are cast down and wrapped on purpose, to cover every bit
// pattern of each element width, so the numeric lints have nothing to say.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::suboptimal_flops
)]
//! The vectorized byte-order decode, through the public API.
//!
//! The `simd` feature settles array and numeric-list byte order a vector at
//! a time. These tests walk every length around the vector blocks, for every
//! numeric list and array, and hold the bytes and the values against
//! fastnbt, so that the vector path, the scalar tail and the scalar fallback
//! all agree.

use core::fmt::Debug;

use nanonbt::{FromNBT, ToNBT};
use serde::Serialize;

/// Lengths around the 16- and 32-byte vector blocks, and the seams between.
const LENGTHS: [usize; 21] = [
    0, 1, 2, 3, 4, 5, 7, 8, 9, 15, 16, 17, 31, 32, 33, 63, 64, 65, 127, 128, 129,
];

/// Both crates must write the same bytes for the value, and nanonbt must
/// read the value back from either crate's bytes.
///
/// fastnbt's readers refuse a negative value read into an unsigned type,
/// where nanonbt reinterprets the bits, so only nanonbt reads back here.
#[track_caller]
fn assert_same<T>(value: &T)
where
    T: Serialize + ToNBT + for<'de> FromNBT<'de> + PartialEq + Debug,
{
    let nano = nanonbt::to_bytes(value).unwrap();
    let fast = fastnbt::to_bytes(value).unwrap();
    assert_eq!(nano, fast, "the two crates write different bytes");
    assert_eq!(&nanonbt::from_bytes::<T>(&nano).unwrap(), value);
    assert_eq!(&nanonbt::from_bytes::<T>(&fast).unwrap(), value);
}

/// One numeric list kind: its derived `Vec<T>` field is a list.
macro_rules! list_shapes {
    ($($module:ident: $ty:ty = $sample:expr),* $(,)?) => {$(
        mod $module {
            use nanonbt::{FromNBT, ToNBT};
            use serde::Serialize;

            use super::{assert_same, LENGTHS};

            #[derive(Serialize, ToNBT, FromNBT, PartialEq, Debug)]
            struct Holder {
                data: Vec<$ty>,
            }

            #[test]
            fn lists_round_trip_around_the_block_boundaries() {
                for len in LENGTHS {
                    let data: Vec<$ty> = (0..len).map($sample).collect();
                    assert_same(&Holder { data });
                }
            }

            #[test]
            fn lists_of_the_extremes_round_trip() {
                assert_same(&Holder {
                    data: vec![<$ty>::MIN, <$ty>::MAX],
                });
            }
        }
    )*};
}

list_shapes! {
    byte_list: i8 = |i: usize| (i as i8).wrapping_mul(7).wrapping_add(3),
    unsigned_byte_list: u8 = |i: usize| (i as u8).wrapping_mul(7).wrapping_add(3),
    short_list: i16 = |i: usize| (i as i16).wrapping_mul(7919).wrapping_sub(1234),
    unsigned_short_list: u16 = |i: usize| (i as u16).wrapping_mul(7919).wrapping_sub(1234),
    int_list: i32 = |i: usize| (i as i32).wrapping_mul(2_654_435_761u32 as i32).wrapping_sub(7),
    unsigned_int_list: u32 = |i: usize| (i as u32).wrapping_mul(2_654_435_761).wrapping_sub(7),
    long_list: i64 = |i: usize| (i as i64).wrapping_mul(6_364_136_223_846_793_005).wrapping_sub(11),
    unsigned_long_list: u64 = |i: usize| (i as u64).wrapping_mul(6_364_136_223_846_793_005).wrapping_sub(11),
    float_list: f32 = |i: usize| i as f32 * 1.5 - 7.25,
    double_list: f64 = |i: usize| i as f64 * 1.5 - 7.25,
}

/// One array kind: the two crates' types for it, and the element they hold.
macro_rules! array_shapes {
    ($($module:ident: $fast:ty, $nano:ty, $ty:ty = $sample:expr),* $(,)?) => {$(
        mod $module {
            use nanonbt::{FromNBT, ToNBT};
            use serde::{Deserialize, Serialize};

            use super::LENGTHS;

            #[derive(Serialize, Deserialize)]
            struct Fast {
                data: $fast,
            }

            #[derive(ToNBT, FromNBT, PartialEq, Debug)]
            struct Nano {
                data: $nano,
            }

            #[test]
            fn arrays_round_trip_around_the_block_boundaries() {
                for len in LENGTHS {
                    let items: Vec<$ty> = (0..len).map($sample).collect();
                    let fast = fastnbt::to_bytes(&Fast {
                        data: <$fast>::new(items.clone()),
                    })
                    .unwrap();
                    let nano = nanonbt::to_bytes(&Nano {
                        data: <$nano>::new(items.clone()),
                    })
                    .unwrap();
                    assert_eq!(nano, fast, "the two crates write different bytes at {len}");
                    assert_eq!(&nanonbt::from_bytes::<Nano>(&fast).unwrap().data[..], &items[..]);
                    assert_eq!(&fastnbt::from_bytes::<Fast>(&nano).unwrap().data[..], &items[..]);
                }
            }
        }
    )*};
}

array_shapes! {
    byte_array: fastnbt::ByteArray, nanonbt::ByteArray, i8 = |i: usize| (i as i8).wrapping_mul(7),
    int_array: fastnbt::IntArray, nanonbt::IntArray, i32 = |i: usize| (i as i32).wrapping_mul(2_654_435_761u32 as i32),
    long_array: fastnbt::LongArray, nanonbt::LongArray, i64 = |i: usize| (i as i64).wrapping_mul(6_364_136_223_846_793_005),
}

/// One derived `#[nbt(array = ...)]` field: nanonbt writes the array, and
/// fastnbt's array type is what it must be interchangeable with.
macro_rules! derived_array_shapes {
    ($($module:ident: $attribute:literal, $fast:ty, $ty:ty = $sample:expr),* $(,)?) => {$(
        mod $module {
            use nanonbt::{FromNBT, ToNBT};
            use serde::Serialize;

            use super::LENGTHS;

            #[derive(Serialize)]
            struct Fast {
                data: $fast,
            }

            #[derive(ToNBT, FromNBT, PartialEq, Debug)]
            struct Nano {
                #[nbt(array = $attribute)]
                data: Vec<$ty>,
            }

            #[test]
            fn derived_array_fields_round_trip_around_the_block_boundaries() {
                for len in LENGTHS {
                    let items: Vec<$ty> = (0..len).map($sample).collect();
                    let fast = fastnbt::to_bytes(&Fast {
                        data: <$fast>::new(items.clone()),
                    })
                    .unwrap();
                    let nano = nanonbt::to_bytes(&Nano {
                        data: items.clone(),
                    })
                    .unwrap();
                    assert_eq!(nano, fast, "the two crates write different bytes at {len}");
                    assert_eq!(nanonbt::from_bytes::<Nano>(&fast).unwrap().data, items);
                }
            }
        }
    )*};
}

derived_array_shapes! {
    derived_byte_array: "byte", fastnbt::ByteArray, i8 = |i: usize| (i as i8).wrapping_mul(7),
    derived_int_array: "int", fastnbt::IntArray, i32 = |i: usize| (i as i32).wrapping_mul(2_654_435_761u32 as i32),
    derived_long_array: "long", fastnbt::LongArray, i64 = |i: usize| (i as i64).wrapping_mul(6_364_136_223_846_793_005),
}

/// The unsigned spelling of a derived array field reads the same bytes by
/// bits, which is the same bulk decode with the other element type.
#[test]
fn unsigned_derived_array_fields_read_by_bits() {
    #[derive(ToNBT)]
    struct Signed {
        #[nbt(array = "long")]
        data: Vec<i64>,
    }

    #[derive(FromNBT, PartialEq, Debug)]
    struct Unsigned {
        #[nbt(array = "long")]
        data: Vec<u64>,
    }

    for len in LENGTHS {
        let data: Vec<i64> = (0..len).map(|i| (i as i64).wrapping_mul(-3)).collect();
        let bytes = nanonbt::to_bytes(&Signed { data: data.clone() }).unwrap();
        assert_eq!(
            nanonbt::from_bytes::<Unsigned>(&bytes).unwrap().data,
            data.into_iter()
                .map(<i64>::cast_unsigned)
                .collect::<Vec<_>>()
        );
    }
}

/// The array types' `Serialize`/`Deserialize` go through the same bulk
/// decode, through the map wrapper fastnbt uses.
#[cfg(feature = "serde")]
#[test]
fn serde_arrays_round_trip_around_the_block_boundaries() {
    use nanonbt::serde_compat;

    #[derive(Serialize, serde::Deserialize)]
    struct Holder {
        bytes: nanonbt::ByteArray,
        ints: nanonbt::IntArray,
        longs: nanonbt::LongArray,
    }

    for len in LENGTHS {
        let value = Holder {
            bytes: nanonbt::ByteArray::new((0..len).map(|i| (i as i8).wrapping_mul(7)).collect()),
            ints: nanonbt::IntArray::new(
                (0..len)
                    .map(|i| (i as i32).wrapping_mul(2_654_435_761u32 as i32))
                    .collect(),
            ),
            longs: nanonbt::LongArray::new(
                (0..len)
                    .map(|i| (i as i64).wrapping_mul(6_364_136_223_846_793_005))
                    .collect(),
            ),
        };
        let bytes = serde_compat::to_bytes(&value).unwrap();
        let back: Holder = serde_compat::from_bytes(&bytes).unwrap();
        assert_eq!(back.bytes, value.bytes, "bytes at {len}");
        assert_eq!(back.ints, value.ints, "ints at {len}");
        assert_eq!(back.longs, value.longs, "longs at {len}");
    }
}

/// A root compound holding `data`, one list of `element` with `len` and
/// `payload`.
fn list_document(element: u8, len: i32, payload: &[u8]) -> Vec<u8> {
    let mut bytes = vec![
        0x0a, 0x00, 0x00, 0x09, 0x00, 0x04, b'd', b'a', b't', b'a', element,
    ];
    bytes.extend_from_slice(&len.to_be_bytes());
    bytes.extend_from_slice(payload);
    bytes.push(0x00);
    bytes
}

#[derive(FromNBT, Debug)]
struct Shorts {
    data: Vec<i16>,
}

#[test]
fn an_element_of_the_wrong_tag_is_refused() {
    // An Int element read as a `Vec<i16>`.
    let bytes = list_document(3, 1, &[0, 0, 0, 1]);
    assert_eq!(
        nanonbt::from_bytes::<Shorts>(&bytes)
            .unwrap_err()
            .to_string(),
        "invalid nbt tag value: 3"
    );
}

#[test]
fn a_list_of_end_with_elements_is_refused() {
    assert_eq!(
        nanonbt::from_bytes::<Shorts>(&list_document(0, 1, &[]))
            .unwrap_err()
            .to_string(),
        "unexpected list of type 'end', which is not supported"
    );
    assert!(
        nanonbt::from_bytes::<Shorts>(&list_document(0, 0, &[]))
            .unwrap()
            .data
            .is_empty()
    );
}

#[test]
fn a_truncated_payload_is_refused() {
    // Two elements promised, one present.
    assert_eq!(
        nanonbt::from_bytes::<Shorts>(&list_document(2, 2, &[0, 7]))
            .unwrap_err()
            .to_string(),
        "eof: unexpectedly ran out of input"
    );
}

#[test]
fn a_negative_list_length_is_refused() {
    assert_eq!(
        nanonbt::from_bytes::<Shorts>(&list_document(2, -1, &[]))
            .unwrap_err()
            .to_string(),
        "negative array length"
    );
}

#[test]
fn a_list_longer_than_the_limit_is_refused() {
    let bytes = list_document(2, 100_000_001, &[]);
    assert_eq!(
        nanonbt::from_bytes::<Shorts>(&bytes)
            .unwrap_err()
            .to_string(),
        "size greater than max sequence length"
    );
}
