//! `nanonbt::from_bytes` against `fastnbt::from_bytes`.
//!
//! Each harness writes a document by hand, with fixed tags, names and
//! lengths, and a payload symbolic only where it is read, then asserts that
//! both crates read it into the same value or both refuse it, and which of
//! the two it is.
//!
//! Tags are matched strictly here and converted in fastnbt, so a harness
//! that reads a value under a tag of another width pins nanonbt's refusal
//! on its own, without fastnbt beside it.

use nanonbt::FromNBT;
use serde::{Deserialize, de::DeserializeOwned};

use crate::same::Same;

const END: u8 = 0;
const BYTE: u8 = 1;
const SHORT: u8 = 2;
const INT: u8 = 3;
const LONG: u8 = 4;
const FLOAT: u8 = 5;
const DOUBLE: u8 = 6;
const BYTE_ARRAY: u8 = 7;
const LIST: u8 = 9;
const COMPOUND: u8 = 10;
const INT_ARRAY: u8 = 11;
const LONG_ARRAY: u8 = 12;

/// A document being written, kept on the stack: Kani loses track of which
/// bytes are concrete once they are on the heap.
struct Nbt {
    buf: [u8; 48],
    len: usize,
}

impl Nbt {
    /// A root compound with an empty name.
    const fn root() -> Self {
        Self::raw(&[COMPOUND, 0, 0])
    }

    /// A root compound without a name, as network NBT has it.
    const fn network_root() -> Self {
        Self::raw(&[COMPOUND])
    }

    const fn raw(start: &[u8]) -> Self {
        let mut buf = [0; 48];
        let mut len = 0;
        while len < start.len() {
            buf[len] = start[len];
            len += 1;
        }
        Self { buf, len }
    }

    fn put(mut self, bytes: &[u8]) -> Self {
        self.buf[self.len..self.len + bytes.len()].copy_from_slice(bytes);
        self.len += bytes.len();
        self
    }

    /// An entry's tag and one-byte name, before its payload.
    fn entry(self, tag: u8, name: u8) -> Self {
        self.put(&[tag, 0, 1, name])
    }

    /// A list or array header.
    fn header(self, tag: Option<u8>, len: i32) -> Self {
        let this = match tag {
            Some(tag) => self.put(&[tag]),
            None => self,
        };
        this.put(&len.to_be_bytes())
    }

    fn end(self) -> Self {
        self.put(&[END])
    }

    fn bytes(&self) -> &[u8] {
        &self.buf[..self.len]
    }
}

/// Asserts both crates read `bytes` as the same `T`, or both refuse them,
/// and that they succeed exactly when `expect_ok`.
fn check<T: DeserializeOwned + for<'de> FromNBT<'de> + Same>(bytes: &[u8], expect_ok: bool) {
    let nano = nanonbt::from_bytes::<T>(bytes).ok();
    let fast = fastnbt::from_bytes::<T>(bytes).ok();
    assert!(nano.same(&fast), "nanonbt and fastnbt disagree");
    assert!(nano.is_some() == expect_ok, "unexpected outcome");
}

/// Asserts nanonbt refuses `bytes`; fastnbt accepts some of these, since its
/// visitors convert between tags.
fn refuse<T: for<'de> FromNBT<'de>>(bytes: &[u8]) {
    assert!(
        nanonbt::from_bytes::<T>(bytes).is_err(),
        "unexpectedly accepted"
    );
}

#[derive(Deserialize, FromNBT)]
struct One<T> {
    v: T,
}

impl<T: Same> Same for One<T> {
    fn same(&self, other: &Self) -> bool {
        self.v.same(&other.v)
    }
}

#[derive(Deserialize, FromNBT)]
struct Inner {
    x: i16,
}

/// A lone optional entry. `Option` is only special on a struct's own field,
/// so a generic `One<Option<i32>>` cannot express this.
#[derive(Deserialize, FromNBT)]
struct Maybe {
    v: Option<i32>,
}

impl Same for Maybe {
    fn same(&self, other: &Self) -> bool {
        self.v.same(&other.v)
    }
}

#[derive(Deserialize, FromNBT)]
struct Nested {
    a: i8,
    n: Inner,
    b: i64,
}

impl Same for Nested {
    fn same(&self, other: &Self) -> bool {
        self.a == other.a && self.n.x == other.n.x && self.b == other.b
    }
}

/// A document of one entry `v` holding a scalar.
fn scalar(tag: u8, payload: &[u8]) -> Nbt {
    Nbt::root().entry(tag, b'v').put(payload).end()
}

/// Harnesses reading a symbolic scalar of type `$from` under `$tag` as a
/// `$to` of the same width, which is accepted.
macro_rules! scalars {
    ($($name:ident: $tag:ident $from:ty => $to:ty, unwind $unwind:tt;)*) => {
        proofs! {
            $(
                fn $name() unwind $unwind {
                    let v: $from = kani::any();
                    check::<One<$to>>(scalar($tag, &v.to_be_bytes()).bytes(), true);
                }
            )*
        }
    };
}

scalars! {
    byte: BYTE i8 => i8, unwind 4;
    short: SHORT i16 => i16, unwind 4;
    int: INT i32 => i32, unwind 4;
    long: LONG i64 => i64, unwind 4;
    float: FLOAT f32 => f32, unwind 4;
    double: DOUBLE f64 => f64, unwind 4;
    bool_from_byte: BYTE i8 => bool, unwind 4;
}

/// Harnesses reading a symbolic signed scalar under its own tag as the
/// unsigned type of the same width. nanonbt reinterprets the bits; fastnbt
/// range-checks, so it refuses negatives; where it accepts, they agree.
macro_rules! unsigned {
    ($($name:ident: $tag:ident $from:ty => $to:ty, unwind $unwind:tt;)*) => {
        proofs! {
            $(
                fn $name() unwind $unwind {
                    let v: $from = kani::any();
                    let bytes = scalar($tag, &v.to_be_bytes());
                    let nano = nanonbt::from_bytes::<One<$to>>(bytes.bytes()).ok();
                    assert!(
                        nano.as_ref().map(|one| one.v) == Some(v as $to),
                        "nanonbt reinterprets the bits",
                    );
                    let fast = fastnbt::from_bytes::<One<$to>>(bytes.bytes()).ok();
                    assert!(
                        fast.as_ref().map(|one| one.v) == <$to>::try_from(v).ok(),
                        "fastnbt converts in range only",
                    );
                }
            )*
        }
    };
}

unsigned! {
    u8_from_byte: BYTE i8 => u8, unwind 4;
    u16_from_short: SHORT i16 => u16, unwind 4;
    u32_from_int: INT i32 => u32, unwind 4;
    u64_from_long: LONG i64 => u64, unwind 4;
}

/// The same tags read as a type of another width or kind, which nanonbt
/// refuses: fastnbt's visitors would convert them. The payload is never
/// read, so a fixed one is used; a symbolic one only makes CBMC prove the
/// tag decides the outcome, which it does slowly.
macro_rules! strict {
    ($($name:ident: $tag:ident $from:ty => $to:ty, unwind $unwind:tt;)*) => {
        proofs! {
            $(
                fn $name() unwind $unwind {
                    refuse::<One<$to>>(scalar($tag, &<$from>::default().to_be_bytes()).bytes());
                }
            )*
        }
    };
}

strict! {
    bool_from_short: SHORT i16 => bool, unwind 4;
    bool_from_int: INT i32 => bool, unwind 6;
    bool_from_long: LONG i64 => bool, unwind 10;
    i64_from_int: INT i32 => i64, unwind 4;
    i8_from_int: INT i32 => i8, unwind 4;
    i32_from_float: FLOAT f32 => i32, unwind 4;
    f64_from_float: FLOAT f32 => f64, unwind 4;
    f32_from_double: DOUBLE f64 => f32, unwind 8;
}

proofs! {
    fn network_int() unwind 4 {
        let v: i32 = kani::any();
        let nbt = Nbt::network_root().entry(INT, b'v').put(&v.to_be_bytes()).end();
        let nano = nanonbt::from_bytes_with_opts::<One<i32>>(
            nbt.bytes(),
            nanonbt::DeOpts::network_nbt(),
        );
        let fast = fastnbt::from_bytes_with_opts::<One<i32>>(
            nbt.bytes(),
            fastnbt::DeOpts::network_nbt(),
        );
        let nano = nano.ok();
        assert!(nano.same(&fast.ok()), "nanonbt and fastnbt disagree");
        assert!(nano.is_some(), "unexpected outcome");
    }

    fn list_int_0() unwind 4 {
        check::<One<Vec<i32>>>(Nbt::root().entry(LIST, b'v').header(Some(INT), 0).end().bytes(), true);
    }
    fn list_int_1() unwind 4 {
        let a: i32 = kani::any();
        let nbt = Nbt::root().entry(LIST, b'v').header(Some(INT), 1).put(&a.to_be_bytes()).end();
        check::<One<Vec<i32>>>(nbt.bytes(), true);
    }
    fn list_int_2() unwind 4 {
        let [a, b]: [i32; 2] = kani::any();
        let nbt = Nbt::root()
            .entry(LIST, b'v')
            .header(Some(INT), 2)
            .put(&a.to_be_bytes())
            .put(&b.to_be_bytes())
            .end();
        check::<One<Vec<i32>>>(nbt.bytes(), true);
    }
    /// How old chunks store an empty list.
    fn list_end_0() unwind 4 {
        check::<One<Vec<i32>>>(Nbt::root().entry(LIST, b'v').header(Some(END), 0).end().bytes(), true);
    }
    /// A list of End with elements is refused.
    fn list_end_1() unwind 4 {
        refuse::<One<Vec<i32>>>(Nbt::root().entry(LIST, b'v').header(Some(END), 1).end().bytes());
    }

    fn i128_from_int_array() unwind 4 {
        let v: i128 = kani::any();
        let nbt = Nbt::root().entry(INT_ARRAY, b'v').header(None, 4).put(&v.to_be_bytes()).end();
        check::<One<i128>>(nbt.bytes(), true);
    }
    fn u128_from_int_array() unwind 4 {
        let v: u128 = kani::any();
        let nbt = Nbt::root().entry(INT_ARRAY, b'v').header(None, 4).put(&v.to_be_bytes()).end();
        check::<One<u128>>(nbt.bytes(), true);
    }

    fn nested_compound() unwind 4 {
        let (a, x, b): (i8, i16, i64) = kani::any();
        let nbt = Nbt::root()
            .entry(BYTE, b'a')
            .put(&a.to_be_bytes())
            .entry(COMPOUND, b'n')
            .entry(SHORT, b'x')
            .put(&x.to_be_bytes())
            .end()
            .entry(LONG, b'b')
            .put(&b.to_be_bytes())
            .end();
        check::<Nested>(nbt.bytes(), true);
    }

    /// An entry the struct has no field for is skipped.
    fn skip_scalar() unwind 4 {
        let (a, v): (i16, i32) = kani::any();
        let nbt = Nbt::root()
            .entry(SHORT, b'a')
            .put(&a.to_be_bytes())
            .entry(INT, b'v')
            .put(&v.to_be_bytes())
            .end();
        check::<One<i32>>(nbt.bytes(), true);
    }
    fn skip_list() unwind 4 {
        let (a, b, v): (i32, i32, i32) = kani::any();
        let nbt = Nbt::root()
            .entry(LIST, b'a')
            .header(Some(INT), 2)
            .put(&a.to_be_bytes())
            .put(&b.to_be_bytes())
            .entry(INT, b'v')
            .put(&v.to_be_bytes())
            .end();
        check::<One<i32>>(nbt.bytes(), true);
    }

    fn option_present() unwind 4 {
        let v: i32 = kani::any();
        check::<Maybe>(scalar(INT, &v.to_be_bytes()).bytes(), true);
    }
    fn option_absent() unwind 4 {
        check::<Maybe>(Nbt::root().end().bytes(), true);
    }
    /// A tag of another width is not an `Option`'s payload. The payload is
    /// never read, so a fixed one is used, as in `strict!`.
    fn option_wrong_tag() unwind 4 {
        refuse::<Maybe>(scalar(SHORT, &0i16.to_be_bytes()).bytes());
    }
}

/// Harnesses reading an array of `$len` symbolic elements into nanonbt's and
/// fastnbt's own array types, and into a `Vec`, which both refuse.
macro_rules! arrays {
    ($($name:ident: $tag:ident $array:ident<$element:ty>[$len:literal];)*) => {
        proofs! {
            $(
                fn $name() unwind 22 {
                    let elements: [$element; $len] = kani::any();
                    let mut nbt = Nbt::root().entry($tag, b'v').header(None, $len);
                    for element in elements {
                        nbt = nbt.put(&element.to_be_bytes());
                    }
                    let nbt = nbt.end();
                    let bytes = nbt.bytes();
                    let expected = Some(&elements[..]);
                    let read = nanonbt::from_bytes::<One<nanonbt::$array>>(bytes).ok();
                    assert!(read.as_ref().map(|a| &a.v[..]).same(&expected), "nanonbt into its type");
                    let read = fastnbt::from_bytes::<One<fastnbt::$array>>(bytes).ok();
                    assert!(read.as_ref().map(|a| &a.v[..]).same(&expected), "fastnbt into its type");
                    assert!(nanonbt::from_bytes::<One<Vec<$element>>>(bytes).is_err(), "nanonbt into a Vec");
                    assert!(fastnbt::from_bytes::<One<Vec<$element>>>(bytes).is_err(), "fastnbt into a Vec");
                }
            )*
        }
    };
}

arrays! {
    byte_array_0: BYTE_ARRAY ByteArray<i8>[0];
    byte_array_1: BYTE_ARRAY ByteArray<i8>[1];
    byte_array_2: BYTE_ARRAY ByteArray<i8>[2];
    int_array_0: INT_ARRAY IntArray<i32>[0];
    int_array_1: INT_ARRAY IntArray<i32>[1];
    int_array_2: INT_ARRAY IntArray<i32>[2];
    long_array_0: LONG_ARRAY LongArray<i64>[0];
    long_array_1: LONG_ARRAY LongArray<i64>[1];
    long_array_2: LONG_ARRAY LongArray<i64>[2];
}
