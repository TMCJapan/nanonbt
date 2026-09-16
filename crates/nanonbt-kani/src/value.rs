//! `nanonbt::to_value` and `from_value` against fastnbt's.
//!
//! The two `Value` types are different types, so a harness matches their
//! variants against each other. Compounds are left out: fastnbt's are
//! `HashMap`s, which Kani does not finish on; so are lists, whose `Vec` of
//! either `Value` does not finish in 3 minutes either.

use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::same::Same;

/// The same value in both crates' `Value` types.
fn same_value(nano: &nanonbt::Value, fast: &fastnbt::Value) -> bool {
    use fastnbt::Value as F;
    use nanonbt::Value as N;
    match (nano, fast) {
        (N::Byte(a), F::Byte(b)) => a.same(b),
        (N::Short(a), F::Short(b)) => a.same(b),
        (N::Int(a), F::Int(b)) => a.same(b),
        (N::Long(a), F::Long(b)) => a.same(b),
        (N::Float(a), F::Float(b)) => a.same(b),
        (N::Double(a), F::Double(b)) => a.same(b),
        (N::String(a), F::String(b)) => a.as_bytes().same(&b.as_bytes()),
        (N::ByteArray(a), F::ByteArray(b)) => a[..].same(&b[..]),
        (N::IntArray(a), F::IntArray(b)) => a[..].same(&b[..]),
        (N::LongArray(a), F::LongArray(b)) => a[..].same(&b[..]),
        (N::List(a), F::List(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(a, b)| same_value(a, b))
        }
        _ => false,
    }
}

/// Asserts both crates make the same `Value` of `value`, and that they
/// succeed exactly when `expect_ok` says.
fn check_to<T: Serialize>(value: &T, expect_ok: bool) {
    let nano = nanonbt::to_value(value).ok();
    let fast = fastnbt::to_value(value).ok();
    let same = match (&nano, &fast) {
        (Some(nano), Some(fast)) => same_value(nano, fast),
        (None, None) => true,
        _ => false,
    };
    assert!(same, "nanonbt and fastnbt disagree");
    assert!(nano.is_some() == expect_ok, "unexpected outcome");
}

/// Asserts both crates read their own `Value` as the same `T`, and that
/// they succeed exactly when `expect_ok` says.
fn check_from<T: DeserializeOwned + Same>(
    nano: &nanonbt::Value,
    fast: &fastnbt::Value,
    expect_ok: bool,
) {
    let nano = nanonbt::from_value::<T>(nano).ok();
    let fast = fastnbt::from_value::<T>(fast).ok();
    assert!(nano.same(&fast), "nanonbt and fastnbt disagree");
    assert!(nano.is_some() == expect_ok, "unexpected outcome");
}

#[derive(Serialize)]
enum Unit {
    Bb,
}

#[derive(Serialize)]
struct UnitStruct;

#[derive(Serialize)]
enum Newtype {
    V(i32),
}

#[derive(Deserialize)]
#[serde(transparent)]
struct F32(f32);

impl Same for F32 {
    fn same(&self, other: &Self) -> bool {
        self.0.same(&other.0)
    }
}

#[derive(Deserialize)]
#[serde(transparent)]
struct F64(f64);

impl Same for F64 {
    fn same(&self, other: &Self) -> bool {
        self.0.same(&other.0)
    }
}

/// Harnesses turning a symbolic `$from` into both `Value`s.
macro_rules! to_values {
    ($($name:ident: $from:ty;)*) => {
        proofs! {
            $(
                fn $name() unwind 4 {
                    check_to(&kani::any::<$from>(), true);
                }
            )*
        }
    };
}

to_values! {
    to_i8: i8;
    to_i16: i16;
    to_i32: i32;
    to_i64: i64;
    to_u8: u8;
    to_u32: u32;
    to_f64: f64;
    to_bool: bool;
    to_char: char;
}

proofs! {
    /// A UUID-style int array of four ints.
    fn to_i128() unwind 22 { check_to(&kani::any::<i128>(), true) }
    fn to_unit_variant() unwind 4 { check_to(&Unit::Bb, true) }

    /// fastnbt panics on these, so only nanonbt is checked: it refuses them.
    fn to_value_refused() unwind 4 {
        assert!(nanonbt::to_value(&None::<i32>).is_err(), "None");
        assert!(nanonbt::to_value(&()).is_err(), "unit");
        assert!(nanonbt::to_value(&UnitStruct).is_err(), "unit struct");
        assert!(nanonbt::to_value(&Newtype::V(kani::any())).is_err(), "newtype variant");
    }
}

/// Harnesses reading a symbolic `$variant` payload as a `$to`, which should
/// succeed exactly when `$ok` holds of the value `v`.
macro_rules! from_values {
    ($($name:ident: $variant:ident($from:ty) => $to:ty, ok if |$v:ident| $ok:expr;)*) => {
        proofs! {
            $(
                fn $name() unwind 4 {
                    let $v: $from = kani::any();
                    check_from::<$to>(
                        &nanonbt::Value::$variant($v),
                        &fastnbt::Value::$variant($v),
                        $ok,
                    );
                }
            )*
        }
    };
}

from_values! {
    from_byte_i8: Byte(i8) => i8, ok if |_v| true;
    from_byte_u8: Byte(i8) => u8, ok if |_v| true;
    from_byte_bool: Byte(i8) => bool, ok if |_v| true;
    from_byte_i32: Byte(i8) => i32, ok if |_v| false;
    from_short_i16: Short(i16) => i16, ok if |_v| true;
    from_int_i32: Int(i32) => i32, ok if |_v| true;
    from_int_bool: Int(i32) => bool, ok if |_v| true;
    from_int_i64: Int(i32) => i64, ok if |_v| false;
    from_long_i64: Long(i64) => i64, ok if |_v| true;
    from_float_f32: Float(f32) => F32, ok if |_v| true;
    from_double_f64: Double(f64) => F64, ok if |_v| true;
    from_float_f64: Float(f32) => F64, ok if |_v| false;
    from_int_char: Int(i32) => char, ok if |v| char::from_u32(v as u32).is_some();
}

proofs! {
    fn from_int_array_i128() unwind 132 {
        let ints: [i32; 4] = kani::any();
        check_from::<i128>(
            &nanonbt::Value::IntArray(nanonbt::IntArray::new(Vec::from(ints))),
            &fastnbt::Value::IntArray(fastnbt::IntArray::new(Vec::from(ints))),
            true,
        );
    }

    fn from_int_array_u128() unwind 132 {
        let ints: [i32; 4] = kani::any();
        check_from::<u128>(
            &nanonbt::Value::IntArray(nanonbt::IntArray::new(Vec::from(ints))),
            &fastnbt::Value::IntArray(fastnbt::IntArray::new(Vec::from(ints))),
            true,
        );
    }
}
