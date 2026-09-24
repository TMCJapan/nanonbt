#![allow(clippy::items_after_statements)]
//! Zero-copy big-endian numbers and byte arrays.

use nanonbt::{
    F32Be, F64Be, FromNBT, I16Be, I32Be, I64Be, ToNBT, U16Be, U32Be, U64Be, from_bytes, to_bytes,
};
use serde::{Deserialize, Serialize};

/// A root compound with one entry named `name`.
fn root(tag: u8, name: &str, payload: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0x0a, 0x00, 0x00, tag];
    bytes.extend_from_slice(&u16::try_from(name.len()).unwrap().to_be_bytes());
    bytes.extend_from_slice(name.as_bytes());
    bytes.extend_from_slice(payload);
    bytes.push(0x00);
    bytes
}

/// Where the payload of the entry `root` wrote starts.
const fn payload_at(name: &str) -> usize {
    6 + name.len()
}

#[test]
#[allow(clippy::float_cmp)] // exact values, as written
fn big_endian_scalars_borrow_from_the_input() {
    macro_rules! case {
        ($ty:ty, $tag:expr, $payload:expr, $expected:expr) => {{
            #[derive(FromNBT)]
            struct Holder<'a> {
                value: &'a $ty,
            }

            let bytes = root($tag, "value", &$payload);
            let holder = from_bytes::<Holder<'_>>(&bytes).unwrap();
            assert!(
                core::ptr::eq(
                    core::ptr::from_ref(holder.value).cast::<u8>(),
                    bytes[payload_at("value")..].as_ptr(),
                ),
                "{} was copied",
                stringify!($ty),
            );
            assert_eq!(holder.value.get(), $expected);
        }};
    }

    case!(U16Be, 0x02, [0x01, 0x02], 0x0102_u16);
    case!(I16Be, 0x02, [0xff, 0xfe], -2_i16);
    case!(U32Be, 0x03, [0x01, 0x02, 0x03, 0x04], 0x0102_0304_u32);
    case!(I32Be, 0x03, [0xff, 0xff, 0xff, 0xfe], -2_i32);
    case!(
        U64Be,
        0x04,
        [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08],
        0x0102_0304_0506_0708_u64
    );
    case!(
        I64Be,
        0x04,
        [0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xfe],
        -2_i64
    );
    case!(F32Be, 0x05, [0x3f, 0xc0, 0x00, 0x00], 1.5_f32);
    case!(
        F64Be,
        0x06,
        [0xbf, 0xe0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
        -0.5_f64
    );
}

#[test]
fn bytes_borrow_from_the_input() {
    #[derive(FromNBT)]
    struct Holder<'a> {
        u: &'a u8,
        i: &'a i8,
    }

    let mut bytes = vec![0x0a, 0x00, 0x00];
    bytes.extend_from_slice(&[0x01, 0x00, 0x01, b'u', 0xfe]);
    bytes.extend_from_slice(&[0x01, 0x00, 0x01, b'i', 0x02]);
    bytes.push(0x00);

    let holder = from_bytes::<Holder<'_>>(&bytes).unwrap();
    assert_eq!(*holder.u, 0xfe);
    assert_eq!(*holder.i, 2);
    assert!(core::ptr::eq(
        core::ptr::from_ref(holder.u),
        &raw const bytes[7],
    ));
    assert!(core::ptr::eq(
        core::ptr::from_ref(holder.i).cast::<u8>(),
        &raw const bytes[12],
    ));
}

#[test]
fn arrays_borrow_from_the_input() {
    #[derive(FromNBT)]
    struct Holder<'a> {
        bytes: &'a [u8],
        signed: &'a [i8],
        ints: &'a [U32Be],
        longs: &'a [U64Be],
    }

    #[derive(ToNBT)]
    struct Source {
        #[nbt(array = "byte")]
        bytes: Vec<u8>,
        #[nbt(array = "byte")]
        signed: Vec<i8>,
        #[nbt(array = "int")]
        ints: Vec<u32>,
        #[nbt(array = "long")]
        longs: Vec<u64>,
    }

    let bytes = to_bytes(&Source {
        bytes: vec![1, 255],
        signed: vec![1, -1],
        ints: vec![1, u32::MAX - 1],
        longs: vec![1, 1 << 63],
    })
    .unwrap();
    let holder = from_bytes::<Holder<'_>>(&bytes).unwrap();

    assert_eq!(holder.bytes, [1, 255].as_slice());
    assert_eq!(holder.signed, [1, -1].as_slice());
    assert_eq!(
        holder
            .ints
            .iter()
            .copied()
            .map(U32Be::get)
            .collect::<Vec<_>>(),
        [1, 0xffff_fffe]
    );
    assert_eq!(
        holder
            .longs
            .iter()
            .copied()
            .map(U64Be::get)
            .collect::<Vec<_>>(),
        [1, 1 << 63]
    );

    // Every slice points into the document.
    let range = bytes.as_ptr_range();
    for at in [
        holder.bytes.as_ptr(),
        holder.signed.as_ptr().cast::<u8>(),
        holder.ints.as_ptr().cast::<u8>(),
        holder.longs.as_ptr().cast::<u8>(),
    ] {
        assert!(range.contains(&at), "slice was copied");
    }
}

#[test]
fn slices_read_lists_too() {
    #[derive(FromNBT)]
    struct Holder<'a> {
        longs: &'a [U64Be],
    }

    let mut payload = vec![0x04]; // element tag: TAG_Long
    payload.extend_from_slice(&2_i32.to_be_bytes());
    payload.extend_from_slice(&1_i64.to_be_bytes());
    payload.extend_from_slice(&2_i64.to_be_bytes());

    let bytes = root(0x09, "longs", &payload);
    let holder = from_bytes::<Holder<'_>>(&bytes).unwrap();
    assert_eq!(holder.longs.len(), 2);
    assert_eq!(holder.longs[0].get(), 1);
    assert_eq!(holder.longs[1].get(), 2);

    // The first element is the first payload byte.
    let at = payload_at("longs") + 1 + 4;
    assert!(core::ptr::eq(
        holder.longs.as_ptr().cast::<u8>(),
        bytes[at..].as_ptr(),
    ));
}

#[test]
fn lists_of_narrower_elements_borrow() {
    #[derive(FromNBT)]
    struct Holder<'a> {
        shorts: &'a [I16Be],
        floats: &'a [F32Be],
        doubles: &'a [F64Be],
    }

    #[derive(ToNBT)]
    struct Source {
        shorts: Vec<i16>,
        floats: Vec<f32>,
        doubles: Vec<f64>,
    }

    let bytes = to_bytes(&Source {
        shorts: vec![1, -2],
        floats: vec![1.5],
        doubles: vec![-0.5],
    })
    .unwrap();
    let holder = from_bytes::<Holder<'_>>(&bytes).unwrap();
    assert_eq!(
        holder
            .shorts
            .iter()
            .copied()
            .map(I16Be::get)
            .collect::<Vec<_>>(),
        [1, -2]
    );
    assert_eq!(holder.floats[0].get().to_bits(), 1.5_f32.to_bits());
    assert_eq!(holder.doubles[0].get().to_bits(), (-0.5_f64).to_bits());
}

#[test]
fn empty_arrays_and_lists_borrow() {
    #[derive(FromNBT)]
    struct Holder<'a> {
        value: &'a [U64Be],
    }

    // An empty long array.
    let bytes = root(0x0c, "value", &0_i32.to_be_bytes());
    assert!(from_bytes::<Holder<'_>>(&bytes).unwrap().value.is_empty());

    // An empty list is a list of End.
    let mut payload = vec![0x00];
    payload.extend_from_slice(&0_i32.to_be_bytes());
    let bytes = root(0x09, "value", &payload);
    assert!(from_bytes::<Holder<'_>>(&bytes).unwrap().value.is_empty());
}

#[test]
fn malformed_arrays_are_refused() {
    #[derive(FromNBT, Debug)]
    struct Holder<'a> {
        #[allow(dead_code)]
        value: &'a [U64Be],
    }

    // A negative length.
    let bytes = root(0x0c, "value", &(-1_i32).to_be_bytes());
    assert_eq!(
        from_bytes::<Holder<'_>>(&bytes).unwrap_err().to_string(),
        "negative array length"
    );

    // Longer than `DeOpts::max_seq_len`.
    let bytes = root(0x0c, "value", &100_000_001_i32.to_be_bytes());
    assert_eq!(
        from_bytes::<Holder<'_>>(&bytes).unwrap_err().to_string(),
        "size greater than max sequence length"
    );

    // Truncated: two elements claimed, one present.
    let mut payload = 2_i32.to_be_bytes().to_vec();
    payload.extend_from_slice(&[0; 8]);
    let bytes = root(0x0c, "value", &payload);
    assert_eq!(
        from_bytes::<Holder<'_>>(&bytes).unwrap_err().to_string(),
        "eof: unexpectedly ran out of input"
    );

    // A list of the wrong element type.
    let mut payload = vec![0x03]; // TAG_Int, not TAG_Long
    payload.extend_from_slice(&1_i32.to_be_bytes());
    payload.extend_from_slice(&[0; 4]);
    let bytes = root(0x09, "value", &payload);
    assert_eq!(
        from_bytes::<Holder<'_>>(&bytes).unwrap_err().to_string(),
        "invalid nbt tag value: 3"
    );

    // A list of End with elements.
    let mut payload = vec![0x00];
    payload.extend_from_slice(&1_i32.to_be_bytes());
    let bytes = root(0x09, "value", &payload);
    assert_eq!(
        from_bytes::<Holder<'_>>(&bytes).unwrap_err().to_string(),
        "unexpected list of type 'end', which is not supported"
    );
}

#[test]
fn borrowed_numbers_round_trip() {
    #[derive(FromNBT, ToNBT, PartialEq, Debug)]
    struct Holder<'a> {
        byte: &'a u8,
        short: &'a U16Be,
        int: &'a I32Be,
        long: &'a U64Be,
        float: &'a F32Be,
        double: &'a F64Be,
        bytes: &'a [u8],
        ints: &'a [I32Be],
        longs: &'a [U64Be],
    }

    let byte = 0xfe_u8;
    let short = U16Be::new(0x0102);
    let int = I32Be::new(-2);
    let long = U64Be::new(u64::MAX);
    let float = F32Be::new(1.5);
    let double = F64Be::new(-0.5);
    let ints = [I32Be::new(1), I32Be::new(-2)];
    let longs = [U64Be::new(3)];

    let value = Holder {
        byte: &byte,
        short: &short,
        int: &int,
        long: &long,
        float: &float,
        double: &double,
        bytes: &[1, 2],
        ints: &ints,
        longs: &longs,
    };
    let bytes = to_bytes(&value).unwrap();
    let back = from_bytes::<Holder<'_>>(&bytes).unwrap();
    assert_eq!(back, value);
    assert_eq!(back.long.get(), u64::MAX);
}

#[test]
fn writing_a_borrowed_slice_writes_a_list() {
    #[derive(ToNBT)]
    struct Holder<'a> {
        longs: &'a [U64Be],
    }

    let longs = [U64Be::new(1)];
    let bytes = to_bytes(&Holder { longs: &longs }).unwrap();

    let mut expected = vec![0x0a, 0x00, 0x00, 0x09, 0x00, 0x05];
    expected.extend_from_slice(b"longs");
    expected.push(0x04); // element tag: TAG_Long
    expected.extend_from_slice(&1_i32.to_be_bytes());
    expected.extend_from_slice(&1_i64.to_be_bytes());
    expected.push(0x00);
    assert_eq!(bytes, expected);
}

#[test]
fn owned_wrappers_read_and_write_lists() {
    #[derive(FromNBT, ToNBT, PartialEq, Debug)]
    struct Holder {
        longs: Vec<U64Be>,
        ints: [I32Be; 2],
    }

    let value = Holder {
        longs: vec![U64Be::new(1), U64Be::new(u64::MAX)],
        ints: [I32Be::new(-1), I32Be::new(2)],
    };
    let bytes = to_bytes(&value).unwrap();
    let back = from_bytes::<Holder>(&bytes).unwrap();
    assert_eq!(back, value);
}

#[test]
fn slice_and_scalar_constructors() {
    let bytes = [0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 2];
    let longs = U64Be::slice_from_bytes(&bytes).unwrap();
    assert_eq!(longs.len(), 2);
    assert_eq!(longs[0].get(), 1);
    assert_eq!(longs[1].get(), 2);
    assert!(U64Be::slice_from_bytes(&bytes[..15]).is_none());
    assert!(U64Be::slice_from_bytes(&[]).unwrap().is_empty());

    assert_eq!(U64Be::from_bytes([0; 8]).get(), 0);
    assert_eq!(U64Be::from(u64::MAX).get(), u64::MAX);
    assert_eq!(u64::from(U64Be::new(7)), 7);
    assert_eq!(u64::from_be_bytes(*U64Be::new(7).as_bytes()), 7);
}

#[test]
fn any_input_offset_works() {
    #[derive(FromNBT)]
    struct Holder<'a> {
        value: &'a U64Be,
    }

    // The wrapper's alignment is 1, so every root name length puts the value
    // at a different, possibly odd, offset.
    for len in 0..=8 {
        let root_name = "r".repeat(len);
        let mut bytes = vec![0x0a];
        bytes.extend_from_slice(&u16::try_from(root_name.len()).unwrap().to_be_bytes());
        bytes.extend_from_slice(root_name.as_bytes());
        bytes.extend_from_slice(&[0x04, 0x00, 0x05]);
        bytes.extend_from_slice(b"value");
        bytes.extend_from_slice(&42_i64.to_be_bytes());
        bytes.push(0x00);

        let holder = from_bytes::<Holder<'_>>(&bytes).unwrap();
        assert_eq!(holder.value.get(), 42);
    }
}

#[test]
fn decodes_like_fastnbt_borrow_types() {
    #[derive(Serialize)]
    struct Source {
        bytes: fastnbt::ByteArray,
        ints: fastnbt::IntArray,
        longs: fastnbt::LongArray,
    }

    #[derive(Deserialize)]
    struct Fast<'a> {
        #[serde(borrow)]
        bytes: fastnbt::borrow::ByteArray<'a>,
        #[serde(borrow)]
        ints: fastnbt::borrow::IntArray<'a>,
        #[serde(borrow)]
        longs: fastnbt::borrow::LongArray<'a>,
    }

    #[derive(FromNBT)]
    struct Nano<'a> {
        bytes: &'a [u8],
        ints: &'a [I32Be],
        longs: &'a [I64Be],
    }

    let bytes = fastnbt::to_bytes(&Source {
        bytes: fastnbt::ByteArray::new(vec![1, -1, i8::MIN]),
        ints: fastnbt::IntArray::new(vec![i32::MIN, 0, i32::MAX]),
        longs: fastnbt::LongArray::new(vec![i64::MIN, -1, i64::MAX]),
    })
    .unwrap();

    let fast = fastnbt::from_bytes::<Fast>(&bytes).unwrap();
    let nano = nanonbt::from_bytes::<Nano<'_>>(&bytes).unwrap();

    assert_eq!(
        fast.bytes.iter().collect::<Vec<_>>(),
        nano.bytes
            .iter()
            .map(|b| b.cast_signed())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        fast.ints.iter().collect::<Vec<_>>(),
        nano.ints
            .iter()
            .copied()
            .map(I32Be::get)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        fast.longs.iter().collect::<Vec<_>>(),
        nano.longs
            .iter()
            .copied()
            .map(I64Be::get)
            .collect::<Vec<_>>()
    );
}
