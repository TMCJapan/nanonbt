//! `nanonbt::to_bytes` against `fastnbt::to_bytes`.
//!
//! Each harness fixes a shape and leaves its values symbolic, then asserts
//! that both crates succeed or both fail, with the same bytes on success.
//! Each also pins which of the two it is, so that a shape both crates
//! refuse cannot pass vacuously.
//!
//! Field and variant names are one to three bytes long: every name goes
//! through a few loops, which must unwind as far as the longest name. Only
//! the array harnesses unwind further.
//!
//! The shapes derive both `Serialize` (for fastnbt) and `ToNBT` (for
//! nanonbt). Shapes with no `ToNBT` implementation at all — units, tuple
//! and struct variants, `serialize_bytes` — are not here: the derive refuses
//! them at compile time, so no runtime harness can.

use nanonbt::ToNBT;
use serde::Serialize;

/// Asserts the two serializers agree on `value`, and that they succeed
/// exactly when `expect_ok` says they should.
fn check<T: Serialize + ToNBT>(value: &T, expect_ok: impl FnOnce(&T) -> bool) {
    let nano = nanonbt::to_bytes(value).ok();
    let fast = fastnbt::to_bytes(value).ok();
    assert!(
        same(nano.as_deref(), fast.as_deref()),
        "nanonbt and fastnbt disagree"
    );
    assert!(nano.is_some() == expect_ok(value), "unexpected outcome");
}

const fn ok<T>(_: &T) -> bool {
    true
}

const fn err<T>(_: &T) -> bool {
    false
}

/// Asserts both crates write the same bytes for their own array type holding
/// the same data.
fn check_arrays<N: ToNBT, F: Serialize>(nano: &N, fast: &F, expect_ok: bool) {
    let nano = nanonbt::to_bytes(nano).ok();
    let fast = fastnbt::to_bytes(fast).ok();
    assert!(fast.is_some() == expect_ok, "unexpected outcome");
    assert!(
        same(nano.as_deref(), fast.as_deref()),
        "nanonbt's array type disagrees"
    );
}

/// The longest output any harness produces, rounded up to whole words.
const MAX_LEN: usize = 64;

/// `a == b`, without the `memcmp` loop: its unwinding bound would have to be
/// the output length, and would then apply to every other loop too.
fn same(a: Option<&[u8]>, b: Option<&[u8]>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => {
            if a.len() != b.len() {
                return false;
            }
            let [a0, a1, a2, a3] = words(a);
            let [b0, b1, b2, b3] = words(b);
            (a0 ^ b0) | (a1 ^ b1) | (a2 ^ b2) | (a3 ^ b3) == 0
        }
        (None, None) => true,
        _ => false,
    }
}

/// The bytes, zero-padded to [`MAX_LEN`], as four words.
fn words(bytes: &[u8]) -> [u128; 4] {
    assert!(bytes.len() <= MAX_LEN, "output longer than MAX_LEN");
    let mut padded = [0; MAX_LEN];
    padded[..bytes.len()].copy_from_slice(bytes);
    let word = |at: usize| {
        let mut word = [0; 16];
        word.copy_from_slice(&padded[at..at + 16]);
        u128::from_ne_bytes(word)
    };
    [word(0), word(16), word(32), word(48)]
}

fn any_vec<T: kani::Arbitrary, const N: usize>() -> Vec<T> {
    Vec::from(kani::any::<[T; N]>())
}

fn any_option<T: kani::Arbitrary>() -> Option<T> {
    if kani::any() { Some(kani::any()) } else { None }
}

#[derive(Serialize, ToNBT)]
struct One<T> {
    v: T,
}

/// An optional value between two others, to catch a left out entry
/// disturbing its neighbours.
#[derive(Serialize, ToNBT)]
struct Around {
    a: i8,
    v: Option<i32>,
    z: i8,
}

/// A lone optional entry. `Option` is only special on a struct's own field,
/// so a generic `One<Option<i32>>` cannot express this.
#[derive(Serialize, ToNBT)]
struct Maybe {
    v: Option<i32>,
}

#[derive(Serialize, ToNBT, kani::Arbitrary)]
struct Inner {
    x: i16,
    y: f32,
}

#[derive(Serialize, ToNBT)]
struct Nested {
    a: i8,
    n: Inner,
    b: i64,
}

#[derive(Serialize, ToNBT)]
struct Wrap(Inner);

#[derive(Serialize, ToNBT)]
struct Empty {}

#[derive(Serialize, ToNBT, kani::Arbitrary)]
enum Unit {
    A,
    Bb,
    Ccc,
}

proofs! {
    fn prim_i8() unwind 4 { check(&One { v: kani::any::<i8>() }, ok) }
    fn prim_i16() unwind 4 { check(&One { v: kani::any::<i16>() }, ok) }
    fn prim_i32() unwind 4 { check(&One { v: kani::any::<i32>() }, ok) }
    fn prim_i64() unwind 4 { check(&One { v: kani::any::<i64>() }, ok) }
    fn prim_i128() unwind 4 { check(&One { v: kani::any::<i128>() }, ok) }
    fn prim_u8() unwind 4 { check(&One { v: kani::any::<u8>() }, ok) }
    fn prim_u16() unwind 4 { check(&One { v: kani::any::<u16>() }, ok) }
    fn prim_u32() unwind 4 { check(&One { v: kani::any::<u32>() }, ok) }
    fn prim_u64() unwind 4 { check(&One { v: kani::any::<u64>() }, ok) }
    fn prim_u128() unwind 4 { check(&One { v: kani::any::<u128>() }, ok) }
    fn prim_f32() unwind 4 { check(&One { v: kani::any::<f32>() }, ok) }
    fn prim_f64() unwind 4 { check(&One { v: kani::any::<f64>() }, ok) }
    fn prim_bool() unwind 4 { check(&One { v: kani::any::<bool>() }, ok) }
    fn prim_char() unwind 4 { check(&One { v: kani::any::<char>() }, ok) }

    /// `None` leaves the entry out.
    fn option_i32() unwind 4 { check(&Maybe { v: any_option::<i32>() }, ok) }
    fn option_some_between() unwind 4 {
        check(&Around { a: kani::any(), v: Some(kani::any::<i32>()), z: kani::any() }, ok);
    }
    fn option_none_between() unwind 4 {
        check(&Around { a: kani::any(), v: None::<i32>, z: kani::any() }, ok);
    }

    fn vec_i8_0() unwind 4 { check(&One { v: any_vec::<i8, 0>() }, ok) }
    fn vec_i8_1() unwind 4 { check(&One { v: any_vec::<i8, 1>() }, ok) }
    fn vec_i8_2() unwind 4 { check(&One { v: any_vec::<i8, 2>() }, ok) }
    fn vec_i32_0() unwind 4 { check(&One { v: any_vec::<i32, 0>() }, ok) }
    fn vec_i32_1() unwind 4 { check(&One { v: any_vec::<i32, 1>() }, ok) }
    fn vec_i32_2() unwind 4 { check(&One { v: any_vec::<i32, 2>() }, ok) }
    fn vec_f64_0() unwind 4 { check(&One { v: any_vec::<f64, 0>() }, ok) }
    fn vec_f64_1() unwind 4 { check(&One { v: any_vec::<f64, 1>() }, ok) }
    fn vec_f64_2() unwind 4 { check(&One { v: any_vec::<f64, 2>() }, ok) }

    fn nested_struct() unwind 4 {
        check(&Nested { a: kani::any(), n: kani::any(), b: kani::any() }, ok);
    }
    fn list_of_lists() unwind 4 {
        check(&One { v: vec![any_vec::<i8, 1>(), any_vec::<i8, 2>()] }, ok);
    }
    /// The empty inner list is a list of End; the next one is not.
    fn list_of_lists_empty_first() unwind 4 {
        check(&One { v: vec![any_vec::<i16, 0>(), any_vec::<i16, 1>()] }, ok);
    }
    fn array_i16_2() unwind 4 { check(&One { v: kani::any::<[i16; 2]>() }, ok) }

    /// The variant's name, as a string.
    fn unit_variant() unwind 4 { check(&One { v: kani::any::<Unit>() }, ok) }
    fn root_newtype_struct() unwind 4 { check(&Wrap(kani::any()), ok) }
    fn root_empty_struct() unwind 4 { check(&Empty {}, ok) }

    fn root_i32() unwind 4 { check(&kani::any::<i32>(), err) }
    fn root_vec() unwind 4 { check(&any_vec::<i32, 1>(), err) }
    fn root_unit_variant() unwind 4 { check(&kani::any::<Unit>(), err) }
}

/// Array harnesses: nanonbt writes the elements from a derived
/// `#[nbt(array = "...")]` field and fastnbt from its own array type, and
/// the two documents must agree.
macro_rules! arrays {
    ($($name:ident: $attribute:literal, $array:ident<$element:ty>[$len:literal];)*) => {
        proofs! {
            $(
                fn $name() unwind 22 {
                    #[derive(ToNBT)]
                    struct Nano {
                        #[nbt(array = $attribute)]
                        v: Vec<$element>,
                    }

                    let data = any_vec::<$element, $len>();
                    check_arrays(
                        &Nano { v: data.clone() },
                        &One { v: fastnbt::$array::new(data) },
                        true,
                    );
                }
            )*
        }
    };
}

arrays! {
    byte_array_0: "byte", ByteArray<i8>[0];
    byte_array_1: "byte", ByteArray<i8>[1];
    byte_array_2: "byte", ByteArray<i8>[2];
    int_array_0: "int", IntArray<i32>[0];
    int_array_1: "int", IntArray<i32>[1];
    int_array_2: "int", IntArray<i32>[2];
    long_array_0: "long", LongArray<i64>[0];
    long_array_1: "long", LongArray<i64>[1];
    long_array_2: "long", LongArray<i64>[2];
}
